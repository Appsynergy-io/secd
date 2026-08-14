//! Select — port of `components/ui/select.tsx` (Radix Select, popper
//! position — the site wrapper's default). Machine (WAI-ARIA listbox /
//! combobox pattern, Radix semantics):
//!
//! ```text
//! Closed --trigger click | ArrowDown/Up | Enter | Space--> Open
//!         (focus → the selected option)
//! Open --Escape | outside pointerdown--> Closed (focus → trigger)
//! Open --window resize | window blur--> Closed (focus → trigger)
//! Open --item click | Enter | Space--> value selected, Closed
//! ArrowDown/Up: move highlight (no wrap); Tab trapped; typeahead
//! ```
//!
//! Modal page state like DropdownMenu (guards with FocusScope memory,
//! `aria-hidden` siblings, scroll-locked body). Popper: side bottom, align
//! start, offset 4, **collision padding 10** (`floating::compute_padded`);
//! size is clamped to the viewport and `max-height` is set after place so
//! long lists scroll inside the content. The viewport takes the trigger's
//! width as `min-width` (Radix's `--radix-select-trigger-width`).
//! `SelectValue` renders the selected value string (label == value
//! everywhere the site renders a Select); a label-mapping form ports with
//! its first consumer.

use crate::behavior::portal::Portal;
use crate::icons::{Icon, RI_ARROW_DOWN_S_LINE, RI_CHECK_LINE};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

pub const SELECT_TRIGGER: &str = "asy-select__trigger";
pub const SELECT_TRIGGER_ICON: &str = "asy-select__trigger-icon";
pub const SELECT: &str = "asy-select";
pub const SELECT_BOTTOM: &str = "asy-select--bottom";
pub const SELECT_TOP: &str = "asy-select--top";
pub const SELECT_VIEWPORT: &str = "asy-select__viewport";
pub const SELECT_ITEM: &str = "asy-select__item";
pub const SELECT_ITEM_DISABLED: &str = "asy-select__item--disabled";
pub const SELECT_ITEM_CHECK: &str = "asy-select__item-check";
pub const SELECT_ITEM_CHECK_GLYPH: &str = "asy-select__item-check-glyph";
pub const SELECT_LABEL: &str = "asy-select__label";
pub const SELECT_SEP: &str = "asy-select__sep";

#[derive(Clone, Copy)]
struct SelectCtx {
    open: RwSignal<bool>,
    value: RwSignal<Option<String>>,
    /// Radix's controlled mode: the value only moves through the consumer
    /// (`onValueChange` → prop), never by an item click directly.
    controlled: bool,
    on_value_change: Option<Callback<String>>,
    content_id: StoredValue<String>,
    trigger: NodeRef<leptos::html::Button>,
    /// Radix `isFormControl`: true until the mounted trigger proves no
    /// `<form>` ancestor exists (server renders assume a form).
    is_form_control: RwSignal<bool>,
    /// Native options mirrored from mounted items (value, text, disabled)
    /// — Radix collects them per-item on mount, so they exist only while
    /// the content is open.
    native_options: RwSignal<Vec<(String, String, bool)>>,
}

static SELECT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[component]
pub fn Select(
    #[prop(optional, into)] default_value: Option<String>,
    /// Controlled value (Radix's `value` prop). When given, item clicks
    /// only fire `on_value_change`; the shown value follows this signal.
    #[prop(optional, into)]
    value: Option<Signal<String>>,
    #[prop(optional)] on_value_change: Option<Callback<String>>,
    children: Children,
) -> impl IntoView {
    let n = SELECT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ctx = SelectCtx {
        open: RwSignal::new(false),
        value: RwSignal::new(value.map(|v| v.get_untracked()).or(default_value)),
        controlled: value.is_some(),
        on_value_change,
        content_id: StoredValue::new(format!("asy-select-{n}")),
        trigger: NodeRef::new(),
        is_form_control: RwSignal::new(true),
        native_options: RwSignal::new(Vec::new()),
    };
    Effect::new(move |_| {
        if let Some(btn) = ctx.trigger.get() {
            ctx.is_form_control.set(btn.closest("form").ok().flatten().is_some());
        }
    });
    if let Some(value) = value {
        Effect::new(move |_| {
            let v = value.get();
            if ctx.value.with_untracked(|cur| cur.as_deref() != Some(v.as_str())) {
                ctx.value.set(Some(v));
            }
        });
    }
    // Scoped Provider, not bare provide_context: sibling Selects share a
    // reactive owner, so bare provision lets the last sibling's ctx shadow
    // the others for lazily-built children (items render only while open).
    view! {
        <leptos::context::Provider value=ctx>
            {children()}
            // Radix's SelectBubbleInput: a visually-hidden native select
            // rendered only inside forms, carrying the mounted items as
            // native options.
            {move || {
                ctx.is_form_control
                    .get()
                    .then(|| {
                        view! {
                            <select
                                aria-hidden="true"
                                tabindex="-1"
                                prop:value=move || ctx.value.get().unwrap_or_default()
                                // Radix's visually-hidden style, as CSSOM
                                // bindings so style-src stays attribute-free.
                                style:position="absolute"
                                style:border="0px"
                                style:width="1px"
                                style:height="1px"
                                style:padding="0px"
                                style:margin="-1px"
                                style:overflow="hidden"
                                style:clip="rect(0px, 0px, 0px, 0px)"
                                style:white-space="nowrap"
                                style:overflow-wrap="normal"
                            >
                                {move || {
                                    ctx.native_options
                                        .get()
                                        .into_iter()
                                        .map(|(v, text, dis)| {
                                            let sel = {
                                                let v = v.clone();
                                                move || {
                                                    ctx.value
                                                        .with(|cur| {
                                                            cur.as_deref() == Some(v.as_str())
                                                        })
                                                }
                                            };
                                            view! {
                                                <option
                                                    value=v
                                                    disabled=dis
                                                    selected=sel
                                                >
                                                    {text}
                                                </option>
                                            }
                                        })
                                        .collect_view()
                                }}
                            </select>
                        }
                    })
            }}
        </leptos::context::Provider>
    }
}

/// The trigger: `role="combobox"` button with the value slot and chevron
/// icon (Radix stamps `aria-controls` closed too, `aria-autocomplete`,
/// `dir`, `data-state`).
#[component]
pub fn SelectTrigger(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<SelectCtx>().expect("invariant: SelectTrigger inside Select");
    let mut cls = SELECT_TRIGGER.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <button
            class=cls
            node_ref=ctx.trigger
            type="button"
            role="combobox"
            aria-controls=ctx.content_id.get_value()
            aria-expanded=move || if ctx.open.get() { "true" } else { "false" }
            aria-autocomplete="none"
            dir="ltr"
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
            <Icon d=RI_ARROW_DOWN_S_LINE class=SELECT_TRIGGER_ICON attr:aria-hidden="true" />
        </button>
    }
}

/// The value slot: shows the selected value, or the placeholder while
/// nothing is selected. Radix renders it as a bare `pointer-events:none`
/// span.
#[component]
pub fn SelectValue(
    #[prop(optional, into)] placeholder: Option<String>,
    /// Display text for the selected value — the port of Radix's ItemText
    /// projection (upstream, the selected item's text is portaled into the
    /// value node even while closed). Omit when item labels equal their
    /// values; pass the label lookup when they differ.
    #[prop(optional, into)]
    label: Option<Signal<String>>,
) -> impl IntoView {
    let ctx = use_context::<SelectCtx>().expect("invariant: SelectValue inside Select");
    let span_ref: NodeRef<leptos::html::Span> = NodeRef::new();
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    Effect::new(move |_| {
        // Marker comments beside the value text defeat Chromium's pruning
        // of the trigger child that duplicates the trigger's aria-label.
        if let Some(span) = span_ref.get() {
            crate::components::strip_comment_children(span.as_ref());
        }
    });
    view! {
        // Bare span with Radix's inline style — an inline style (vs a class)
        // is what keeps Chromium treating this node as `generic` instead of
        // pruning it as uninteresting, which the AX comparison measures.
        // CSSOM binding: reflects into the inline style without tripping a
        // style-src that blocks attributes.
        <span style:pointer-events="none" node_ref=span_ref>
            {move || {
                match label {
                    Some(label) if ctx.value.with(Option::is_some) => Some(label.get()),
                    _ => ctx.value.get(),
                }
                .or_else(|| placeholder.clone())
                .unwrap_or_default()
            }}
        </span>
    }
}

#[component]
pub fn SelectContent(
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let ctx = use_context::<SelectCtx>().expect("invariant: SelectContent inside Select");
    let open = ctx.open;
    let trigger = ctx.trigger;
    let content_id = ctx.content_id;
    let cls = StoredValue::new({
        let mut cls = SELECT.to_owned();
        if let Some(extra) = class {
            cls.push(' ');
            cls.push_str(&extra);
        }
        cls
    });
    let children = StoredValue::new(children);
    let typeahead: StoredValue<crate::behavior::typeahead::Typeahead> =
        StoredValue::new(Default::default());
    // While closed, Radix portals the children into a detached
    // DocumentFragment so items mount invisibly — the selected ItemText
    // projection and the native-option registry stay live. Mirrored with
    // a detached-element mount, dropped whenever the real content opens.
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        let closed_mount: StoredValue<Option<send_wrapper::SendWrapper<(
            web_sys::Element,
            leptos::prelude::UnmountHandle<leptos::tachys::view::any_view::AnyViewState>,
        )>>> = StoredValue::new(None);
        Effect::new(move |_| {
            if open.get() {
                if let Some(wrapped) = closed_mount.write_value().take() {
                    drop(wrapped.take());
                }
            } else if closed_mount.with_value(|m| m.is_none()) {
                let doc = leptos::tachys::dom::document();
                if let Ok(div) = doc.create_element("div") {
                    let handle = leptos::mount::mount_to(
                        div.clone().unchecked_into::<web_sys::HtmlElement>(),
                        move || children.with_value(|c| c()),
                    );
                    closed_mount
                        .set_value(Some(send_wrapper::SendWrapper::new((div, handle))));
                }
            }
        });
        on_cleanup(move || {
            if let Some(wrapped) = closed_mount.write_value().take() {
                drop(wrapped.take());
            }
        });
    }
    view! {
        <Show when=move || open.get()>
            <Portal>
                {
                    let content_ref: NodeRef<leptos::html::Div> = NodeRef::new();
                    let viewport_ref: NodeRef<leptos::html::Div> = NodeRef::new();
                    open_effects(content_ref, viewport_ref, trigger, open);
                    view! {
                        <div
                            class=move || format!("{} {SELECT_BOTTOM}", cls.get_value())
                            node_ref=content_ref
                            role="listbox"
                            id=content_id.get_value()
                            data-state="open"
                            data-align="start"
                            dir="ltr"
                            tabindex="-1"
                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                listbox_keydown(&ev, typeahead)
                            }
                        >
                            // Radix injects this scrollbar-hiding style tag as
                            // the content's first child; mirrored verbatim for
                            // DOM-shape parity (the equivalent rules live on
                            // the viewport class).
                            <style>
                                "[data-radix-select-viewport]{scrollbar-width:none;-ms-overflow-style:none;-webkit-overflow-scrolling:touch;}[data-radix-select-viewport]::-webkit-scrollbar{display:none}"
                            </style>
                            <div
                                class=SELECT_VIEWPORT
                                node_ref=viewport_ref
                                role="presentation"
                            >
                                {children.with_value(|c| c())}
                            </div>
                        </div>
                    }
                }
            </Portal>
        </Show>
    }
}

#[component]
pub fn SelectItem(
    #[prop(into)] value: String,
    #[prop(optional)] disabled: bool,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<SelectCtx>().expect("invariant: SelectItem inside Select");
    let mut cls = SELECT_ITEM.to_owned();
    if disabled {
        cls.push(' ');
        cls.push_str(SELECT_ITEM_DISABLED);
    }
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let is_selected = {
        let value = value.clone();
        move || ctx.value.with(|v| v.as_deref() == Some(value.as_str()))
    };
    let is_selected_state = is_selected.clone();
    let is_selected_check = is_selected.clone();
    let text_id = format!("{}-item-{value}", ctx.content_id.get_value());
    let label_ref = text_id.clone();
    let select_value = value.clone();
    // Radix registers a native option (value, textContent, disabled) per
    // mounted item; it leaves the set with the item.
    let item_ref = NodeRef::<leptos::html::Div>::new();
    let native_value = value.clone();
    Effect::new(move |_| {
        if let Some(el) = item_ref.get() {
            let text = el.text_content().unwrap_or_default();
            let v = native_value.clone();
            ctx.native_options.update(|opts| {
                if !opts.iter().any(|(ov, _, _)| *ov == v) {
                    opts.push((v, text, disabled));
                }
            });
        }
    });
    {
        let v = value.clone();
        on_cleanup(move || {
            ctx.native_options.update(|opts| opts.retain(|(ov, _, _)| *ov != v));
        });
    }
    view! {
        <div
            node_ref=item_ref
            role="option"
            aria-labelledby=label_ref
            aria-selected=move || if is_selected() { "true" } else { "false" }
            data-state=move || if is_selected_state() { "checked" } else { "unchecked" }
            data-disabled=disabled.then_some("")
            tabindex="-1"
            class=cls
            on:click=move |_| {
                if disabled {
                    return;
                }
                if !ctx.controlled {
                    ctx.value.set(Some(select_value.clone()));
                }
                if let Some(cb) = ctx.on_value_change {
                    cb.run(select_value.clone());
                }
                ctx.open.set(false);
            }
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
        >
            <span class=SELECT_ITEM_CHECK>
                {
                    let is_selected = is_selected_check.clone();
                    move || {
                        is_selected().then(|| {
                            view! {
                                <span aria-hidden="true">
                                    <Icon d=RI_CHECK_LINE class=SELECT_ITEM_CHECK_GLYPH />
                                </span>
                            }
                        })
                    }
                }
            </span>
            <span id=text_id>{children()}</span>
        </div>
    }
}

#[component]
pub fn SelectLabel(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = SELECT_LABEL.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <div class=cls>{children()}</div> }
}

#[component]
pub fn SelectSeparator(#[prop(optional, into)] class: Option<String>) -> impl IntoView {
    let mut cls = SELECT_SEP.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <div aria-hidden="true" class=cls></div> }
}

/// Listbox keyboard core: arrows move real focus through the options
/// (no wrap), Tab trapped, Enter/Space select the focused option,
/// printable characters feed the typeahead machine.
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(unused_variables))]
fn listbox_keydown(
    ev: &web_sys::KeyboardEvent,
    typeahead: StoredValue<crate::behavior::typeahead::Typeahead>,
) {
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        let Some(listbox) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        let items: Vec<web_sys::HtmlElement> = listbox
            .query_selector_all("[role=option]:not([data-disabled])")
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
                        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                        let target = typeahead
                            .try_update_value(|t| t.on_char(ch, now, &label_refs, current));
                        if let Some(Some(i)) = target {
                            if let Some(el) = items.get(i) {
                                let _ = el.focus();
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Open-session behavior: claim the portal host as the popper wrapper
/// (`dir="ltr"`, bottom/start/offset-4, collision padding 10, DPR-rounded
/// translate + max-height clamp), give the viewport the trigger's width as
/// `min-width`, install the modal page state (guards with FocusScope memory,
/// `aria-hidden` siblings, scroll-locked body), focus the selected option,
/// dismiss on Escape / outside pointerdown (trigger protected), restore
/// focus to the trigger on close.
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(unused_variables))]
fn open_effects(
    content_ref: NodeRef<leptos::html::Div>,
    viewport_ref: NodeRef<leptos::html::Div>,
    trigger: NodeRef<leptos::html::Button>,
    open: RwSignal<bool>,
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
            window_close: Closure<dyn FnMut(web_sys::Event)>,
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
                let window = leptos::tachys::dom::window();
                for kind in ["resize", "blur"] {
                    let _ = window.remove_event_listener_with_callback(
                        kind,
                        self.window_close.as_ref().unchecked_ref(),
                    );
                }
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
            let window = leptos::tachys::dom::window();
            let viewport = (
                window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
            );
            let anchor = floating::rect_of(&trigger_el);
            // The trigger's width becomes the viewport's min-width before
            // measuring, exactly as Radix's CSS variable resolves.
            if let Some(vp) = viewport_ref.get() {
                let vp: &web_sys::HtmlElement = vp.as_ref();
                let _ = vp.style().set_property("min-width", &format!("{}px", anchor.width));
            }
            let size_rect = floating::rect_of(&host);
            let size = floating::clamp_size_to_viewport(
                (size_rect.width, size_rect.height),
                viewport,
                10.0,
            );
            let placement = floating::compute_padded(
                anchor,
                size,
                viewport,
                Side::Bottom,
                Align::Start,
                4.0,
                10.0,
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
            if side == "top" {
                let content_el: &web_sys::Element = content.as_ref();
                let cls = content_el.class_name().replace(SELECT_BOTTOM, SELECT_TOP);
                content_el.set_class_name(&cls);
            }
            if let Some(h) = host.dyn_ref::<web_sys::HtmlElement>() {
                let _ = h.style().set_property(
                    "transform",
                    &format!("translate({}px, {}px)", round(placement.x), round(placement.y)),
                );
                let _ = h.style().set_property("pointer-events", "auto");
            }
            if let Some(c) = content.dyn_ref::<web_sys::HtmlElement>() {
                let mh = floating::max_height_for_y(placement.y, viewport.1, 10.0);
                let _ = c.style().set_property("max-height", &format!("{mh}px"));
            }

            // Focus memory: the focused option is remembered so the guards
            // can bounce focus back to it (Radix FocusScope memory). Unlike
            // DropdownMenu, Select does not rove `tabindex` — options stay
            // `-1` and focus moves programmatically.
            let last_focused: std::rc::Rc<std::cell::RefCell<Option<web_sys::HtmlElement>>> =
                std::rc::Rc::new(std::cell::RefCell::new(None));
            let focusin = {
                let last_focused = std::rc::Rc::clone(&last_focused);
                let content = content.clone();
                Closure::wrap(Box::new(move |event: web_sys::Event| {
                    let Some(item) = event
                        .target()
                        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                        .filter(|el| el.get_attribute("role").as_deref() == Some("option"))
                    else {
                        return;
                    };
                    // Radix: `aria-selected` is `isSelected && isFocused`, so
                    // moving focus recomputes it across the listbox.
                    let item_el: &web_sys::Element = item.as_ref();
                    if let Ok(options) = content.query_selector_all("[role=option]") {
                        for i in 0..options.length() {
                            let Some(el) =
                                options.get(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok())
                            else {
                                continue;
                            };
                            let selected = el.get_attribute("data-state").as_deref()
                                == Some("checked")
                                && el == *item_el;
                            let _ = el.set_attribute(
                                "aria-selected",
                                if selected { "true" } else { "false" },
                            );
                        }
                    }
                    last_focused.borrow_mut().replace(item);
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
                let handler = {
                    let content = content.clone();
                    let last_focused = std::rc::Rc::clone(&last_focused);
                    Closure::wrap(Box::new(move |_: web_sys::Event| {
                        let remembered = last_focused.borrow().clone();
                        let target = remembered.or_else(|| {
                            content
                                .query_selector("[role=option]:not([data-disabled])")
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
            // Focus the selected option (Radix focuses it on open).
            let focus_target = content
                .query_selector("[role=option][aria-selected=true]")
                .ok()
                .flatten()
                .or_else(|| {
                    content.query_selector("[role=option]:not([data-disabled])").ok().flatten()
                })
                .and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok());
            match focus_target {
                Some(el) => {
                    let _ = el.focus();
                }
                None => {
                    let _ = content.focus();
                }
            }

            let protected: Vec<web_sys::Node> =
                vec![host.clone().into(), (*trigger_el).clone().into()];
            let dismiss = DismissGuard::install(&document, protected, move || open.set(false));

            // Radix closes an open Select on window resize and window blur.
            let window_close = Closure::wrap(Box::new(move |_: web_sys::Event| {
                open.set(false);
            }) as Box<dyn FnMut(web_sys::Event)>);
            for kind in ["resize", "blur"] {
                let _ = window
                    .add_event_listener_with_callback(kind, window_close.as_ref().unchecked_ref());
            }

            session.set_value(Some(send_wrapper::SendWrapper::new(OpenSession {
                guards,
                content: content_el.clone(),
                focusin,
                window_close,
                hidden,
                body_style,
                previous_focus,
                _dismiss: dismiss,
            })));
        });
        on_cleanup(move || session.set_value(None));
    }
}

/// Trigger `flex h-8 w-full items-center justify-between gap-2 rounded-sm
/// border bg-surface-2 px-3 text-[13px]` with `:focus` accent ring;
/// content `relative z-50 min-w-[8rem] overflow-hidden rounded-md border
/// bg-surface shadow-md` + Radix's inline flex column; viewport
/// `p-1 w-full` + Radix's inline scroll box; item per its utility string
/// with the absolute check slot. Side gap is the floating offset (4px).
pub fn css() -> String {
    format!(
        ".{SELECT_TRIGGER}{{display:flex;height:2rem;width:100%;\
align-items:center;justify-content:space-between;gap:.5rem;\
border-radius:var(--radius-sm);border:1px solid var(--color-border);\
background-color:var(--color-surface-2);color:var(--color-text);\
padding-left:.75rem;padding-right:.75rem;font-size:13px}}\
.{SELECT_TRIGGER}:focus{{outline-style:none;\
border-color:var(--color-accent-line);\
box-shadow:0 0 0 2px var(--color-accent-soft)}}\
.{SELECT_TRIGGER}:disabled{{cursor:not-allowed;opacity:.5}}\
.{SELECT_TRIGGER_ICON}{{width:1rem;height:1rem;color:var(--color-text-dim)}}\
.{SELECT}{{position:relative;z-index:60;min-width:8rem;overflow-x:hidden;overflow-y:auto;\
border-radius:var(--radius-md);border:1px solid var(--color-border);\
background-color:var(--color-surface);display:flex;flex-direction:column;\
outline:none;box-shadow:0 4px 6px -1px #0000001a,0 2px 4px -2px #0000001a}}\
.{SELECT_BOTTOM},.{SELECT_TOP}{{margin:0}}\
.{SELECT_VIEWPORT}{{padding:.25rem;width:100%;position:relative;\
flex:1 1 0%;overflow:hidden auto;scrollbar-width:none}}\
.{SELECT_ITEM}{{position:relative;display:flex;cursor:pointer;\
user-select:none;align-items:center;border-radius:var(--radius-sm);\
padding:.375rem .5rem .375rem 1.75rem;font-size:.875rem;\
line-height:calc(1.25/.875);outline:none}}\
.{SELECT_ITEM}:focus{{background-color:var(--color-surface-2)}}\
.{SELECT_ITEM_DISABLED}{{pointer-events:none;opacity:.5}}\
.{SELECT_ITEM_CHECK}{{position:absolute;left:.5rem;display:flex;\
width:.75rem;height:.75rem;align-items:center;justify-content:center}}\
.{SELECT_ITEM_CHECK_GLYPH}{{width:.75rem;height:.75rem}}\
.{SELECT_LABEL}{{padding:.375rem .5rem;font-size:11.5px;font-weight:500;\
text-transform:uppercase;letter-spacing:.04em;color:var(--color-text-dim)}}\
.{SELECT_SEP}{{margin:.25rem -.25rem;height:1px;\
background-color:var(--color-border-soft)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            SELECT_TRIGGER,
            SELECT_TRIGGER_ICON,
            SELECT,
            SELECT_BOTTOM,
            SELECT_TOP,
            SELECT_VIEWPORT,
            SELECT_ITEM,
            SELECT_ITEM_DISABLED,
            SELECT_ITEM_CHECK,
            SELECT_ITEM_CHECK_GLYPH,
            SELECT_LABEL,
            SELECT_SEP,
        ] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
        assert!(css.contains(&format!(".{SELECT_ITEM}:focus")));
    }
}
