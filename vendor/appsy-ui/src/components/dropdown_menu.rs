//! DropdownMenu — port of `components/ui/dropdown-menu.tsx` (Radix
//! DropdownMenu, modal). Machine (WAI-ARIA menu-button pattern, Radix
//! semantics):
//!
//! ```text
//! Closed --trigger click | ArrowDown/Up | Enter | Space--> Open (focus → menu)
//! Open --trigger click--> Closed (toggle)
//! Open --Escape | outside pointerdown | item select--> Closed (focus → trigger)
//! ArrowDown/Up: move highlight through items (no wrap, Radix default)
//! Tab: prevented (Radix menus trap it; guards bounce into the items)
//! printable chars: typeahead over item labels
//! ```
//!
//! Modal like Dialog: focus guards bracket the body, siblings get
//! `aria-hidden`, the body takes the react-remove-scroll state. The portal
//! host is claimed as the popper wrapper (shared `asy-popper` + `dir="ltr"`,
//! bottom/center/offset-6). Items are `role="menuitem"` divs at
//! `tabindex="-1"`; highlight is real focus. `DropdownMenuCheckboxItem` has
//! no storied rendering — it ports with its first consumer.

use crate::behavior::portal::Portal;
use crate::components::button::{ButtonSize, ButtonVariant, BTN};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

pub const DD: &str = "asy-dd";
pub const DD_ITEM: &str = "asy-dd__item";
pub const DD_ITEM_DISABLED: &str = "asy-dd__item--disabled";
pub const DD_LABEL: &str = "asy-dd__label";
pub const DD_SEP: &str = "asy-dd__sep";

#[derive(Clone, Copy)]
struct DropdownCtx {
    open: RwSignal<bool>,
    ids: StoredValue<(String, String)>,
    trigger: NodeRef<leptos::html::Button>,
}

static DD_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[component]
pub fn DropdownMenu(children: Children) -> impl IntoView {
    let n = DD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ctx = DropdownCtx {
        open: RwSignal::new(false),
        ids: StoredValue::new((format!("asy-dd-{n}-trigger"), format!("asy-dd-{n}-content"))),
        trigger: NodeRef::new(),
    };
    // Scoped Provider — bare provide_context would let a later sibling
    // instance shadow this ctx for lazily-built children (see select.rs).
    view! { <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider> }
}

/// `Trigger asChild` on a Button per the Button precedent. Radix stamps
/// `id`, `aria-haspopup="menu"`, `aria-expanded`, `aria-controls`,
/// `data-state`; ArrowDown/Up/Enter/Space open from the keyboard.
#[component]
pub fn DropdownMenuTrigger(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx =
        use_context::<DropdownCtx>().expect("invariant: DropdownMenuTrigger inside DropdownMenu");
    let mut cls = format!("{BTN} {} {}", variant.class(), size.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let (trigger_id, _) = ctx.ids.get_value();
    view! {
        <button
            class=cls
            node_ref=ctx.trigger
            type="button"
            id=trigger_id
            aria-haspopup="menu"
            aria-expanded=move || if ctx.open.get() { "true" } else { "false" }
            aria-controls=move || ctx.open.get().then(|| ctx.ids.with_value(|(_, c)| c.clone()))
            data-state=move || if ctx.open.get() { "open" } else { "closed" }
            on:click=move |_| ctx.open.update(|o| *o = !*o)
            on:keydown=move |ev: web_sys::KeyboardEvent| {
                if ctx.open.get_untracked() {
                    return;
                }
                match ev.key().as_str() {
                    "ArrowDown" | "ArrowUp" | "Enter" | " " => {
                        ev.prevent_default();
                        ctx.open.set(true);
                    }
                    _ => {}
                }
            }
        >
            {children()}
        </button>
    }
}

#[component]
pub fn DropdownMenuContent(
    /// Gap to the anchor in px (reference default 6).
    #[prop(optional, default = 6.0)] side_offset: f64,
    /// Radix `side` — placement side, default bottom.
    #[prop(optional, into, default = "bottom".into())]
    side: String,
    /// Radix `align` — cross-axis alignment, default center.
    #[prop(optional, into, default = "center".into())]
    align: String,
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let ctx =
        use_context::<DropdownCtx>().expect("invariant: DropdownMenuContent inside DropdownMenu");
    let open = ctx.open;
    let trigger = ctx.trigger;
    let ids = ctx.ids;
    let cls = StoredValue::new({
        let mut cls = DD.to_owned();
        if let Some(extra) = class {
            cls.push(' ');
            cls.push_str(&extra);
        }
        cls
    });
    let children = StoredValue::new(children);
    let placement = StoredValue::new((side, align));
    let typeahead: StoredValue<crate::behavior::typeahead::Typeahead> =
        StoredValue::new(Default::default());
    view! {
        <Show when=move || open.get()>
            <Portal>
                {
                    let content_ref: NodeRef<leptos::html::Div> = NodeRef::new();
                    open_effects(content_ref, trigger, open, side_offset, placement);
                    let (trigger_id, content_id) = ids.get_value();
                    view! {
                        <div
                            class=cls.get_value()
                            node_ref=content_ref
                            role="menu"
                            aria-orientation="vertical"
                            id=content_id
                            aria-labelledby=trigger_id
                            data-state="open"
                            data-orientation="vertical"
                            data-align=placement.with_value(|(_, a)| a.clone())
                            dir="ltr"
                            tabindex="-1"
                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                menu_keydown(&ev, open, typeahead)
                            }
                        >
                            {children.with_value(|c| c())}
                        </div>
                    }
                }
            </Portal>
        </Show>
    }
}

/// `Trigger asChild` merging onto arbitrary markup (the sidebar's account
/// row): the same Radix wiring as `DropdownMenuTrigger` with the caller's
/// classes only — no Button chrome. Crate-internal; the public trigger
/// stays Button-shaped.
#[component]
pub(crate) fn DropdownMenuTriggerBare(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<DropdownCtx>()
        .expect("invariant: DropdownMenuTriggerBare inside DropdownMenu");
    let (trigger_id, _) = ctx.ids.get_value();
    view! {
        <button
            class=class
            node_ref=ctx.trigger
            type="button"
            id=trigger_id
            aria-haspopup="menu"
            aria-expanded=move || if ctx.open.get() { "true" } else { "false" }
            aria-controls=move || ctx.open.get().then(|| ctx.ids.with_value(|(_, c)| c.clone()))
            data-state=move || if ctx.open.get() { "open" } else { "closed" }
            on:click=move |_| ctx.open.update(|o| *o = !*o)
            on:keydown=move |ev: web_sys::KeyboardEvent| {
                if ctx.open.get_untracked() {
                    return;
                }
                match ev.key().as_str() {
                    "ArrowDown" | "ArrowUp" | "Enter" | " " => {
                        ev.prevent_default();
                        ctx.open.set(true);
                    }
                    _ => {}
                }
            }
        >
            {children()}
        </button>
    }
}

/// `Item asChild` on a link (the sidebar/topbar account menus): Radix
/// stamps `role="menuitem"` onto the anchor; navigation is the href
/// (consumer-supplied), selection still closes the menu. Crate-internal.
#[component]
pub(crate) fn DropdownMenuLinkItem(
    #[prop(into)] href: String,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx =
        use_context::<DropdownCtx>().expect("invariant: DropdownMenuLinkItem inside DropdownMenu");
    let mut cls = DD_ITEM.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <a
            role="menuitem"
            tabindex="-1"
            data-orientation="vertical"
            class=cls
            href=href
            on:click=move |_| ctx.open.set(false)
            on:pointermove=move |ev: web_sys::PointerEvent| {
                if let Some(el) = ev
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = el.focus();
                }
            }
            on:pointerleave=move |ev: web_sys::PointerEvent| {
                if let Some(menu) = ev
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    .and_then(|el| el.closest("[role=menu]").ok().flatten())
                    .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = menu.focus();
                }
            }
        >
            {children()}
        </a>
    }
}

#[component]
pub fn DropdownMenuItem(
    #[prop(optional)] disabled: bool,
    #[prop(optional)] on_select: Option<Callback<()>>,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx =
        use_context::<DropdownCtx>().expect("invariant: DropdownMenuItem inside DropdownMenu");
    let mut cls = DD_ITEM.to_owned();
    if disabled {
        cls.push(' ');
        cls.push_str(DD_ITEM_DISABLED);
    }
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <div
            role="menuitem"
            tabindex="-1"
            data-orientation="vertical"
            data-disabled=disabled.then_some("")
            class=cls
            on:click=move |_| {
                if disabled {
                    return;
                }
                if let Some(cb) = on_select {
                    cb.run(());
                }
                ctx.open.set(false);
            }
            // Highlight follows the pointer: real focus in, menu focus out.
            on:pointermove=move |ev| {
                if disabled {
                    return;
                }
                if let Some(el) = ev
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = el.focus();
                }
            }
            on:pointerleave=move |ev| {
                if let Some(menu) = ev
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    .and_then(|el| el.closest("[role=menu]").ok().flatten())
                    .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = menu.focus();
                }
            }
        >
            {children()}
        </div>
    }
}

#[component]
pub fn DropdownMenuLabel(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = DD_LABEL.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <div class=cls>{children()}</div> }
}

#[component]
pub fn DropdownMenuSeparator(#[prop(optional, into)] class: Option<String>) -> impl IntoView {
    let mut cls = DD_SEP.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <div role="separator" aria-orientation="horizontal" class=cls></div> }
}

/// Menu keyboard core, on the content element: arrows move real focus
/// through the `[role=menuitem]` collection (no wrap), Tab is trapped,
/// Enter/Space activate the focused item, printable characters feed the
/// shared typeahead machine.
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(unused_variables))]
fn menu_keydown(
    ev: &web_sys::KeyboardEvent,
    open: RwSignal<bool>,
    typeahead: StoredValue<crate::behavior::typeahead::Typeahead>,
) {
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        let Some(menu) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        let items: Vec<web_sys::HtmlElement> = menu
            .query_selector_all("[role=menuitem]:not([data-disabled])")
            .ok()
            .map(|list| {
                (0..list.length())
                    .filter_map(|i| list.get(i))
                    .filter_map(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
                    .collect()
            })
            .unwrap_or_default();
        let document = leptos::tachys::dom::document();
        let current = document.active_element().and_then(|a| {
            items.iter().position(|el| {
                let el: &web_sys::Element = el.as_ref();
                *el == a
            })
        });
        let key = ev.key();
        match key.as_str() {
            "Tab" => ev.prevent_default(),
            "ArrowDown" => {
                ev.prevent_default();
                let next = match current {
                    Some(i) => (i + 1).min(items.len().saturating_sub(1)),
                    None => 0,
                };
                if let Some(el) = items.get(next) {
                    let _ = el.focus();
                }
            }
            "ArrowUp" => {
                ev.prevent_default();
                let next = match current {
                    Some(i) => i.saturating_sub(1),
                    None => items.len().saturating_sub(1),
                };
                if let Some(el) = items.get(next) {
                    let _ = el.focus();
                }
            }
            "Enter" | " " => {
                if let Some(i) = current {
                    ev.prevent_default();
                    items[i].click();
                }
            }
            k => {
                let mut chars = k.chars();
                if let (Some(ch), None) = (chars.next(), chars.next()) {
                    if !ev.ctrl_key() && !ev.meta_key() && !ev.alt_key() {
                        let now = leptos::tachys::dom::window()
                            .performance()
                            .map(|p| p.now())
                            .unwrap_or(0.0);
                        let labels: Vec<String> = items
                            .iter()
                            .map(|el| el.text_content().unwrap_or_default())
                            .collect();
                        let label_refs: Vec<&str> =
                            labels.iter().map(String::as_str).collect();
                        let target = typeahead.try_update_value(|t| {
                            t.on_char(ch, now, &label_refs, current)
                        });
                        if let Some(Some(i)) = target {
                            if let Some(el) = items.get(i) {
                                let _ = el.focus();
                            }
                        }
                    }
                }
            }
        }
        let _ = open;
    }
}

/// Open-session behavior: claim the portal host as the popper wrapper
/// (`dir="ltr"`, bottom/center/offset, DPR-rounded translate), install the
/// modal page state (guards redirecting into the item collection,
/// `aria-hidden` siblings, scroll-locked body), focus the menu, dismiss on
/// Escape / outside pointerdown (trigger protected — its click toggles),
/// restore focus to the trigger on close.
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(unused_variables))]
fn open_effects(
    content_ref: NodeRef<leptos::html::Div>,
    trigger: NodeRef<leptos::html::Button>,
    open: RwSignal<bool>,
    side_offset: f64,
    placement_pref: StoredValue<(String, String)>,
) {
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        use crate::behavior::dismiss::DismissGuard;
        use crate::behavior::floating::{self, Align, Side};
        use crate::components::tooltip::POPPER;
        use wasm_bindgen::closure::Closure;

        struct OpenSession {
            guards: Vec<(web_sys::Element, Closure<dyn FnMut(web_sys::Event)>)>,
            content: web_sys::Element,
            focusin: Closure<dyn FnMut(web_sys::Event)>,
            hidden: Vec<(web_sys::Element, Option<String>)>,
            body_style: Vec<(&'static str, String)>,
            previous_focus: Option<web_sys::HtmlElement>,
            _dismiss: DismissGuard,
        }

        impl Drop for OpenSession {
            fn drop(&mut self) {
                let _ = self.content.remove_event_listener_with_callback(
                    "focusin",
                    self.focusin.as_ref().unchecked_ref(),
                );
                for (guard, handler) in self.guards.drain(..) {
                    let _ = guard.remove_event_listener_with_callback(
                        "focus",
                        handler.as_ref().unchecked_ref(),
                    );
                    guard.remove();
                }
                for (el, prev) in self.hidden.drain(..) {
                    match prev {
                        Some(v) => {
                            let _ = el.set_attribute("aria-hidden", &v);
                        }
                        None => {
                            let _ = el.remove_attribute("aria-hidden");
                        }
                    }
                    let _ = el.remove_attribute("data-aria-hidden");
                }
                let document = leptos::tachys::dom::document();
                if let Some(body) = document.body() {
                    let _ = body.remove_attribute("data-scroll-locked");
                    for (name, prev) in self.body_style.drain(..) {
                        if prev.is_empty() {
                            let _ = body.style().remove_property(name);
                        } else {
                            let _ = body.style().set_property(name, &prev);
                        }
                    }
                }
                if let Some(prev) = self.previous_focus.take() {
                    let _ = prev.focus();
                }
            }
        }

        let session: StoredValue<Option<send_wrapper::SendWrapper<OpenSession>>> =
            StoredValue::new(None);
        Effect::new(move |_| {
            if session.with_value(Option::is_some) {
                return;
            }
            let (Some(content), Some(trigger_el)) = (content_ref.get(), trigger.get()) else {
                return;
            };
            let Some(host) = content.parent_element() else { return };
            let document = leptos::tachys::dom::document();
            let Some(body) = document.body() else { return };

            host.set_class_name(POPPER);
            let _ = host.set_attribute("dir", "ltr");
            // The trigger carries the SSR-minted ids; the client-rendered
            // content re-derives them from the live DOM so the pair stays
            // wired even though the id counters diverge across renders.
            let trigger_dom_id = trigger_el.id();
            if !trigger_dom_id.is_empty() {
                let _ = content.set_attribute("aria-labelledby", &trigger_dom_id);
            }
            if let Some(controls) = trigger_el.get_attribute("aria-controls") {
                let _ = content.set_attribute("id", &controls);
            }
            let window = leptos::tachys::dom::window();
            let viewport = (
                window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
            );
            let anchor = floating::rect_of(&trigger_el);
            let size_rect = floating::rect_of(&host);
            let size = floating::clamp_size_to_viewport(
                (size_rect.width, size_rect.height),
                viewport,
                0.0,
            );
            let (pref_side, pref_align) = placement_pref.get_value();
            let placement = floating::compute(
                anchor,
                size,
                viewport,
                match pref_side.as_str() {
                    "top" => Side::Top,
                    "left" => Side::Left,
                    "right" => Side::Right,
                    _ => Side::Bottom,
                },
                match pref_align.as_str() {
                    "start" => Align::Start,
                    "end" => Align::End,
                    _ => Align::Center,
                },
                side_offset,
            );
            let dpr = window.device_pixel_ratio();
            let round = |v: f64| (v * dpr).round() / dpr;
            let side = match placement.side {
                Side::Top => "top",
                Side::Bottom => "bottom",
                Side::Left => "left",
                Side::Right => "right",
            };
            let _ = content.set_attribute("data-side", side);
            if let Some(h) = host.dyn_ref::<web_sys::HtmlElement>() {
                let _ = h.style().set_property(
                    "transform",
                    &format!("translate({}px, {}px)", round(placement.x), round(placement.y)),
                );
                let _ = h.style().set_property("pointer-events", "auto");
            }
            if let Some(c) = content.dyn_ref::<web_sys::HtmlElement>() {
                let mh = floating::max_height_for_y(placement.y, viewport.1, 0.0);
                let _ = c.style().set_property("max-height", &format!("{mh}px"));
            }

            // Roving tab stop + scope memory: the focused item takes
            // tabindex 0 (previous back to -1) and is remembered so the
            // guards can bounce focus back to it, per Radix FocusScope.
            let last_focused: std::rc::Rc<std::cell::RefCell<Option<web_sys::HtmlElement>>> =
                std::rc::Rc::new(std::cell::RefCell::new(None));
            let focusin = {
                let last_focused = std::rc::Rc::clone(&last_focused);
                Closure::wrap(Box::new(move |event: web_sys::Event| {
                    let Some(item) = event
                        .target()
                        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                        .filter(|el| el.get_attribute("role").as_deref() == Some("menuitem"))
                    else {
                        return;
                    };
                    if let Some(prev) = last_focused.borrow_mut().replace(item.clone()) {
                        let _ = prev.set_attribute("tabindex", "-1");
                    }
                    let _ = item.set_attribute("tabindex", "0");
                }) as Box<dyn FnMut(web_sys::Event)>)
            };
            let content_el: &web_sys::Element = content.as_ref();
            let _ = content_el
                .add_event_listener_with_callback("focusin", focusin.as_ref().unchecked_ref());

            let mut guards = Vec::new();
            for lead in [true, false] {
                let span = document.create_element("span").expect("invariant: create guard");
                let _ = span.set_attribute("tabindex", "0");
                let _ = span.set_attribute("aria-hidden", "true");
                let _ = span.set_attribute("data-aria-hidden", "true");
                crate::behavior::focus_trap::style_guard(&span);
                // Either guard bounces into the scope: the last-focused
                // item if any, else the first (Radix FocusScope memory).
                // Chrome resumes Tab navigation from the blurred element,
                // so the trailing guard is the one a body-Tab reaches.
                let handler = {
                    let content = content.clone();
                    let last_focused = std::rc::Rc::clone(&last_focused);
                    Closure::wrap(Box::new(move |_: web_sys::Event| {
                        let remembered = last_focused.borrow().clone();
                        let target = remembered.or_else(|| {
                            content
                                .query_selector("[role=menuitem]:not([data-disabled])")
                                .ok()
                                .flatten()
                                .and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
                        });
                        if let Some(el) = target {
                            let _ = el.focus();
                        }
                    }) as Box<dyn FnMut(web_sys::Event)>)
                };
                let _ = span
                    .add_event_listener_with_callback("focus", handler.as_ref().unchecked_ref());
                if lead {
                    let _ = body.insert_before(&span, body.first_child().as_ref());
                } else {
                    let _ = body.append_child(&span);
                }
                guards.push((span, handler));
            }

            // aria-hidden every body child except the popper host.
            let mut hidden = Vec::new();
            let body_children = body.children();
            for i in 0..body_children.length() {
                let Some(el) = body_children.item(i) else { continue };
                if el == host || guards.iter().any(|(g, _)| *g == el) {
                    continue;
                }
                hidden.push((el.clone(), el.get_attribute("aria-hidden")));
                let _ = el.set_attribute("aria-hidden", "true");
                let _ = el.set_attribute("data-aria-hidden", "true");
            }

            // Scroll lock: react-remove-scroll's computed body state.
            let mut body_style = Vec::new();
            for (name, value) in
                [("overflow", "hidden"), ("position", "relative"), ("pointer-events", "none")]
            {
                let prev = body.style().get_property_value(name).unwrap_or_default();
                body_style.push((name, prev));
                let _ = body.style().set_property(name, value);
            }
            let _ = body.set_attribute("data-scroll-locked", "1");

            let previous_focus = document
                .active_element()
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
            let _ = content.focus();

            let protected: Vec<web_sys::Node> =
                vec![host.clone().into(), (*trigger_el).clone().into()];
            let dismiss = DismissGuard::install(&document, protected, move || open.set(false));

            session.set_value(Some(send_wrapper::SendWrapper::new(OpenSession {
                guards,
                content: content_el.clone(),
                focusin,
                hidden,
                body_style,
                previous_focus,
                _dismiss: dismiss,
            })));
        });
        on_cleanup(move || session.set_value(None));
    }
}

/// Content `z-50 min-w-[12rem] overflow-hidden rounded-md border bg-surface
/// p-1 text-sm shadow-md` (+ Radix's inline `outline:none`); item
/// `relative flex cursor-pointer select-none items-center gap-2 rounded-sm
/// px-2 py-1.5 text-sm outline-none` with `:focus` surface highlight and
/// disabled inert; label `px-2 py-1.5` 11.5px uppercase dim; separator
/// `-mx-1 my-1 h-px` soft border.
pub fn css() -> String {
    format!(
        ".{DD}{{z-index:60;min-width:12rem;overflow-x:hidden;overflow-y:auto;\
border-radius:var(--radius-md);border:1px solid var(--color-border);\
background-color:var(--color-surface);padding:.25rem;\
font-size:.875rem;line-height:calc(1.25/.875);outline:none;\
box-shadow:0 4px 6px -1px #0000001a,0 2px 4px -2px #0000001a}}\
.{DD_ITEM}{{position:relative;display:flex;cursor:pointer;\
user-select:none;align-items:center;gap:.5rem;\
border-radius:var(--radius-sm);padding:.375rem .5rem;\
font-size:.875rem;line-height:calc(1.25/.875);outline:none}}\
.{DD_ITEM}:focus{{background-color:var(--color-surface-2);\
color:var(--color-text)}}\
.{DD_ITEM_DISABLED}{{pointer-events:none;opacity:.5}}\
.{DD_LABEL}{{padding:.375rem .5rem;font-size:11.5px;font-weight:500;\
text-transform:uppercase;letter-spacing:.04em;color:var(--color-text-dim)}}\
.{DD_SEP}{{margin:.25rem -.25rem;height:1px;\
background-color:var(--color-border-soft)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [DD, DD_ITEM, DD_ITEM_DISABLED, DD_LABEL, DD_SEP] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
        assert!(css.contains(&format!(".{DD_ITEM}:focus")));
    }
}
