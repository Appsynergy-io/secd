//! Dialog — port of `components/ui/dialog.tsx` (Radix Dialog, modal).
//! Machine (WAI-ARIA modal dialog pattern, Radix semantics):
//!
//! ```text
//! Closed --trigger click--> Open (focus into content)
//! Open --Escape | overlay pointerdown | Close click--> Closed (focus → trigger)
//! Tab / Shift+Tab while open: cycle within content (focus trap)
//! ```
//!
//! Open-state DOM mirrors Radix exactly: focus-guard spans bracket the body;
//! every body child except the content carries `aria-hidden="true"` +
//! `data-aria-hidden` (the overlay too); overlay and content mount as direct
//! body children — the two Portal hosts are claimed as the overlay and the
//! dialog element themselves, no wrapper level; the body gains
//! `data-scroll-locked="1"` with `overflow:hidden`, `position:relative` and
//! `pointer-events:none` — the computed result of react-remove-scroll's
//! injected sheet at zero scrollbar width. The reference's `animate-in/out`
//! classes compile to nothing (tw-animate-css is never imported upstream):
//! no entry/exit animation.

use crate::behavior::portal::Portal;
use crate::components::button::{ButtonSize, ButtonVariant, BTN};
use crate::icons::{Icon, RI_CLOSE_LINE};
use leptos::prelude::*;

pub const DIALOG_OVERLAY: &str = "asy-dialog-overlay";
pub const DIALOG: &str = "asy-dialog";
pub const DIALOG_CLOSE: &str = "asy-dialog__close";
pub const DIALOG_CLOSE_GLYPH: &str = "asy-dialog__close-glyph";
pub const DIALOG_HEADER: &str = "asy-dialog__header";
pub const DIALOG_HEADER_ICON: &str = "asy-dialog__header--icon";
pub const DIALOG_CHIP: &str = "asy-dialog__chip";
pub const DIALOG_HEADER_COL: &str = "asy-dialog__header-col";
pub const DIALOG_FOOTER: &str = "asy-dialog__footer";
pub const DIALOG_TITLE: &str = "asy-dialog__title";
pub const DIALOG_DESCRIPTION: &str = "asy-dialog__description";

#[derive(Clone, Copy)]
pub(crate) struct DialogCtx {
    /// Dismiss channel: the modal machinery and close buttons flip this
    /// false. Uncontrolled roots also use it as the open state.
    pub(crate) open: RwSignal<bool>,
    /// What `DialogContent` renders. Equal to `open` on uncontrolled
    /// roots; the consumer's `open` prop on controlled ones.
    visible: Signal<bool>,
    /// Deterministic Radix-equivalent ids: (content, title, description).
    ids: StoredValue<(String, String, String)>,
    trigger: NodeRef<leptos::html::Button>,
}

/// Render-order id counter — same sequence on the server and in the
/// hydrating client (Radix's `useId` has the identical property).
static DIALOG_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[component]
pub fn Dialog(children: Children) -> impl IntoView {
    let n = DIALOG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let open = RwSignal::new(false);
    let ctx = DialogCtx {
        open,
        visible: open.into(),
        ids: StoredValue::new((
            format!("asy-dialog-{n}"),
            format!("asy-dialog-{n}-title"),
            format!("asy-dialog-{n}-desc"),
        )),
        trigger: NodeRef::new(),
    };
    // Scoped Provider — bare provide_context would let a later sibling
    // instance shadow this ctx for lazily-built children (see select.rs).
    view! { <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider> }
}

/// Controlled Dialog root — the Radix `open`/`onOpenChange` contract, for
/// composite dialogs whose open state the consumer owns (the dashboard
/// dialog suites render `<Dialog open onOpenChange>` with no trigger).
/// Crate-internal: the public `Dialog` stays trigger-driven. Dismissals
/// (Esc, outside click, close X) report outward via `on_open_change(false)`
/// and the dialog stays mounted until the owner drops `open` — the
/// CommandPalette relay pattern.
#[component]
pub(crate) fn DialogControlled(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    children: Children,
) -> impl IntoView {
    let n = DIALOG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let relay = RwSignal::new(true);
    Effect::new(move |_| {
        if !relay.get() {
            relay.set(true);
            on_open_change.run(false);
        }
    });
    let ctx = DialogCtx {
        open: relay,
        visible: open,
        ids: StoredValue::new((
            format!("asy-dialog-{n}"),
            format!("asy-dialog-{n}-title"),
            format!("asy-dialog-{n}-desc"),
        )),
        trigger: NodeRef::new(),
    };
    // Scoped Provider — see `Dialog`.
    view! { <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider> }
}

/// The trigger in its rendered form on the site: Radix `Trigger asChild`
/// merging onto a `Button` — mirrored, per the Button precedent, as the
/// trigger owning a `<button>` with Button's styling props. Radix's Trigger
/// contributes `type="button"`, `aria-haspopup`, `aria-expanded`,
/// `aria-controls` (present closed too), `data-state`.
#[component]
pub fn DialogTrigger(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<DialogCtx>().expect("invariant: DialogTrigger inside Dialog");
    let mut cls = format!("{BTN} {} {}", variant.class(), size.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <button
            class=cls
            node_ref=ctx.trigger
            type="button"
            aria-haspopup="dialog"
            aria-expanded=move || if ctx.open.get() { "true" } else { "false" }
            aria-controls=ctx.ids.with_value(|(c, _, _)| c.clone())
            data-state=move || if ctx.open.get() { "open" } else { "closed" }
            on:click=move |_| ctx.open.set(true)
        >
            {children()}
        </button>
    }
}

#[component]
pub fn DialogContent(
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let ctx = use_context::<DialogCtx>().expect("invariant: DialogContent inside Dialog");
    let open = ctx.open;
    let visible = ctx.visible;
    let trigger = ctx.trigger;
    let ids = ctx.ids;
    let cls = StoredValue::new({
        let mut cls = DIALOG.to_owned();
        if let Some(extra) = class {
            cls.push(' ');
            cls.push_str(&extra);
        }
        cls
    });
    let children = StoredValue::new(children);
    view! {
        <Show when=move || visible.get()>
            // Empty portal: its host is claimed as the overlay element by
            // the open effect (Radix portals add no wrapper level).
            <Portal>{()}</Portal>
            <Portal>
                {
                    // Children mount directly into the host — the host IS
                    // the dialog element; the close button anchors the ref.
                    let close_ref: NodeRef<leptos::html::Button> = NodeRef::new();
                    modal_open_effects(close_ref, trigger, open, ids, cls, "dialog", true, None);
                    view! {
                        {children.with_value(|c| c())}
                        <DialogCloseX node_ref=close_ref />
                    }
                }
            </Portal>
        </Show>
    }
}

/// The built-in top-right close button (`aria-label="Close"`, RiCloseLine).
#[component]
fn DialogCloseX(#[prop(optional)] node_ref: NodeRef<leptos::html::Button>) -> impl IntoView {
    let ctx = use_context::<DialogCtx>().expect("invariant: inside Dialog");
    view! {
        <button
            type="button"
            class=DIALOG_CLOSE
            aria-label="Close"
            node_ref=node_ref
            on:click=move |_| ctx.open.set(false)
        >
            <Icon d=RI_CLOSE_LINE class=DIALOG_CLOSE_GLYPH />
        </button>
    }
}

/// `DialogClose asChild` form — a Button that closes the dialog (footer
/// "Cancel" buttons in consumers).
#[component]
pub fn DialogClose(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<DialogCtx>().expect("invariant: DialogClose inside Dialog");
    let mut cls = format!("{BTN} {} {}", variant.class(), size.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <button class=cls on:click=move |_| ctx.open.set(false)>
            {children()}
        </button>
    }
}

#[component]
pub fn DialogHeader(
    /// Accent icon-chip beside the title (the premium destructive-modal
    /// flourish); chip layout only when given.
    #[prop(optional, into)] icon: Option<ViewFnOnce>,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    use leptos::either::Either;
    let with_extra = |base: &str, extra: &Option<String>| {
        let mut cls = base.to_owned();
        if let Some(extra) = extra {
            cls.push(' ');
            cls.push_str(extra);
        }
        cls
    };
    match icon {
        Some(icon) => Either::Left(view! {
            <div class=with_extra(DIALOG_HEADER_ICON, &class)>
                <span class=DIALOG_CHIP>{icon.run()}</span>
                <div class=DIALOG_HEADER_COL>{children()}</div>
            </div>
        }),
        None => Either::Right(view! {
            <div class=with_extra(DIALOG_HEADER, &class)>{children()}</div>
        }),
    }
}

#[component]
pub fn DialogFooter(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = DIALOG_FOOTER.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <div class=cls>{children()}</div> }
}

#[component]
pub fn DialogTitle(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<DialogCtx>().expect("invariant: DialogTitle inside Dialog");
    let mut cls = DIALOG_TITLE.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <h2 id=ctx.ids.with_value(|(_, t, _)| t.clone()) class=cls>{children()}</h2> }
}

#[component]
pub fn DialogDescription(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<DialogCtx>().expect("invariant: DialogDescription inside Dialog");
    let mut cls = DIALOG_DESCRIPTION.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <p id=ctx.ids.with_value(|(_, _, d)| d.clone()) class=cls>{children()}</p> }
}

/// Everything an open modal does to the page, mirrored from Radix and
/// unwound in reverse on close: claim the two portal hosts as overlay and
/// modal element, bracket the body with focus guards, `aria-hidden` every
/// sibling, apply the scroll-lock body state, install dismiss and the focus
/// trap with restore-to-trigger. Shared by Dialog (`role="dialog"`, overlay
/// pointerdown dismisses), AlertDialog (`role="alertdialog"`,
/// Escape-only — `outside_dismiss: false` protects the whole body) and
/// CommandPalette (`aria_label` stamped like Radix's pass-through prop). The
/// `anchor` is any element rendered inside the content; the modal element
/// is its ancestor sitting directly under `<body>` (the portal host).
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(unused_variables))]
pub(crate) fn modal_open_effects<E>(
    anchor: NodeRef<E>,
    trigger: NodeRef<leptos::html::Button>,
    open: RwSignal<bool>,
    ids: StoredValue<(String, String, String)>,
    cls: StoredValue<String>,
    role: &'static str,
    outside_dismiss: bool,
    aria_label: Option<&'static str>,
) where
    E: leptos::html::ElementType + 'static,
    E::Output: wasm_bindgen::JsCast + Clone + Into<web_sys::Element> + 'static,
{
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        use crate::behavior::dismiss::DismissGuard;
        use crate::behavior::focus_trap::{self, FocusTrapGuard};
        use crate::behavior::scroll_lock::{self, SavedBodyStyle};
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        struct OpenSession {
            guards: Vec<(web_sys::Element, Closure<dyn FnMut(web_sys::Event)>)>,
            hidden: Vec<(web_sys::Element, Option<String>)>,
            saved_body: Option<SavedBodyStyle>,
            /// Manual body props beyond scroll_lock (position, pointer-events).
            body_style: Vec<(&'static str, String)>,
            _dismiss: DismissGuard,
            _trap: FocusTrapGuard,
        }

        impl Drop for OpenSession {
            fn drop(&mut self) {
                for (guard, handler) in self.guards.drain(..) {
                    let _ =
                        guard.remove_event_listener_with_callback("focus", handler.as_ref().unchecked_ref());
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
                if let Some(ref saved) = self.saved_body {
                    scroll_lock::restore(&document, saved);
                }
            }
        }

        let session: StoredValue<Option<send_wrapper::SendWrapper<OpenSession>>> =
            StoredValue::new(None);
        Effect::new(move |_| {
            if session.with_value(Option::is_some) {
                return;
            }
            let Some(anchor_el) = anchor.get() else { return };
            let document = leptos::tachys::dom::document();
            let Some(body) = document.body() else { return };
            // Ascend from the anchor to the element directly under <body> —
            // the content portal host.
            let body_el: &web_sys::Element = body.as_ref();
            let mut host: web_sys::Element = anchor_el.clone().into();
            loop {
                match host.parent_element() {
                    Some(parent) if &parent == body_el => break,
                    Some(parent) => host = parent,
                    None => return,
                }
            }
            // The empty overlay portal mounted just before this one.
            let Some(overlay) = host.previous_element_sibling() else { return };

            overlay.set_class_name(DIALOG_OVERLAY);
            let _ = overlay.set_attribute("data-state", "open");
            if let Some(o) = overlay.dyn_ref::<web_sys::HtmlElement>() {
                let _ = o.style().set_property("pointer-events", "auto");
            }

            host.set_class_name(&cls.get_value());
            let (content_id, title_id, desc_id) = ids.get_value();
            let _ = host.set_attribute("role", role);
            let _ = host.set_attribute("id", &content_id);
            let _ = host.set_attribute("aria-labelledby", &title_id);
            let _ = host.set_attribute("aria-describedby", &desc_id);
            let _ = host.set_attribute("data-state", "open");
            let _ = host.set_attribute("tabindex", "-1");
            if let Some(label) = aria_label {
                let _ = host.set_attribute("aria-label", label);
            }
            if let Some(h) = host.dyn_ref::<web_sys::HtmlElement>() {
                let _ = h.style().set_property("pointer-events", "auto");
            }

            // Focus guards bracketing the body; focus landing on a guard
            // bounces to the trap's edge tabbable (Radix guard behavior).
            let mut guards = Vec::new();
            for lead in [true, false] {
                let span = document.create_element("span").expect("invariant: create guard");
                let _ = span.set_attribute("tabindex", "0");
                let _ = span.set_attribute("aria-hidden", "true");
                let _ = span.set_attribute("data-aria-hidden", "true");
                crate::behavior::focus_trap::style_guard(&span);
                let handler = {
                    let host = host.clone();
                    Closure::wrap(Box::new(move |_: web_sys::Event| {
                        let items = focus_trap::tabbables(&host);
                        let target = if lead { items.first() } else { items.last() };
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

            // aria-hidden every body child except the dialog content
            // (guards already carry it; overlay receives it like the rest).
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

            // Scroll lock: overflow + scrollbar padding via scroll_lock;
            // position/pointer-events still match react-remove-scroll inert.
            let window = leptos::tachys::dom::window();
            let saved_body = Some(scroll_lock::apply(&document, &window));
            let mut body_style = Vec::new();
            for (name, value) in [("position", "relative"), ("pointer-events", "none")] {
                let prev = body.style().get_property_value(name).unwrap_or_default();
                body_style.push((name, prev));
                let _ = body.style().set_property(name, value);
            }
            let _ = body.set_attribute("data-scroll-locked", "1");

            // `outside_dismiss: false` protects the whole body: no
            // pointerdown ever counts as outside, so only Escape fires.
            let protected: Vec<web_sys::Node> = if outside_dismiss {
                vec![host.clone().into()]
            } else {
                vec![body.clone().into()]
            };
            let dismiss = DismissGuard::install(&document, protected, move || open.set(false));
            let trap = FocusTrapGuard::install(&document, &host);
            let _ = trigger;

            session.set_value(Some(send_wrapper::SendWrapper::new(OpenSession {
                guards,
                hidden,
                saved_body,
                body_style,
                _dismiss: dismiss,
                _trap: trap,
            })));
        });
        on_cleanup(move || session.set_value(None));
    }
}

/// Overlay `fixed inset-0 z-50 bg-black/60 backdrop-blur-sm`; content
/// `fixed left-1/2 top-1/2 z-50 grid w-full max-w-md -translate-x-1/2
/// -translate-y-1/2 gap-4 rounded-lg border bg-surface p-5` with the
/// reference's literal shadow; close button `absolute right-3 top-3 size-7`
/// muted with hover fills; header/footer/title/description per their
/// utility strings.
pub fn css() -> String {
    format!(
        ".{DIALOG_OVERLAY}{{position:fixed;inset:0;z-index:50;\
background-color:oklab(0 0 0 / 0.6);backdrop-filter:blur(8px)}}\
.{DIALOG}{{position:fixed;left:50%;top:50%;z-index:50;display:grid;\
width:calc(100% - 2rem);max-width:28rem;max-height:calc(100dvh - 2rem);\
overflow-y:auto;translate:-50% -50%;gap:1rem;\
border-radius:var(--radius-lg);border:1px solid var(--color-border);\
background-color:var(--color-surface);padding:1.25rem;\
box-shadow:0 12px 40px oklch(0% 0 0 / 0.35)}}\
.{DIALOG_CLOSE}{{position:absolute;right:.75rem;top:.75rem;\
display:inline-flex;width:1.75rem;height:1.75rem;align-items:center;\
justify-content:center;border-radius:var(--radius-sm);\
color:var(--color-text-muted);transition-property:color,background-color,\
border-color,outline-color,text-decoration-color,fill,stroke;\
transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}\
@media(hover:hover){{.{DIALOG_CLOSE}:hover{{\
background-color:var(--color-surface-2);color:var(--color-text)}}}}\
.{DIALOG_CLOSE}:focus-visible{{outline-style:none}}\
.{DIALOG_CLOSE_GLYPH}{{width:1rem;height:1rem}}\
.{DIALOG_HEADER}{{display:flex;flex-direction:column;gap:.375rem}}\
.{DIALOG_HEADER_ICON}{{display:flex;align-items:flex-start;gap:.75rem}}\
.{DIALOG_CHIP}{{margin-top:.125rem;display:inline-flex;width:2rem;\
height:2rem;flex-shrink:0;align-items:center;justify-content:center;\
border-radius:var(--radius-md);border:1px solid var(--color-accent-line);\
background-color:var(--color-accent-soft);color:var(--color-accent)}}\
.{DIALOG_HEADER_COL}{{display:flex;flex-direction:column;gap:.375rem}}\
.{DIALOG_FOOTER}{{display:flex;flex-direction:row;justify-content:flex-end;\
gap:.5rem}}\
.{DIALOG_TITLE}{{font-size:1rem;line-height:1.5rem;font-weight:600;\
letter-spacing:-.025em}}\
.{DIALOG_DESCRIPTION}{{font-size:.875rem;line-height:1.25rem;\
color:var(--color-text-muted)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            DIALOG_OVERLAY,
            DIALOG,
            DIALOG_CLOSE,
            DIALOG_CLOSE_GLYPH,
            DIALOG_HEADER,
            DIALOG_HEADER_ICON,
            DIALOG_CHIP,
            DIALOG_HEADER_COL,
            DIALOG_FOOTER,
            DIALOG_TITLE,
            DIALOG_DESCRIPTION,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
