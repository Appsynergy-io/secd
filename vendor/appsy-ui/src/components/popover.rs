//! Popover — port of `components/ui/popover.tsx` (Radix Popover,
//! non-modal). Machine:
//!
//! ```text
//! Closed --trigger click--> Open (focus → content)
//! Open --trigger click--> Closed (toggle)
//! Open --Escape | outside pointerdown--> Closed (focus → trigger)
//! ```
//!
//! Open-state DOM mirrors Radix: focus-guard spans bracket the body
//! (without `aria-hidden` — non-modal: no sibling hiding, no scroll lock,
//! body untouched); the portal host is claimed as the popper wrapper
//! (shared `asy-popper`, translate rounded to device pixels) holding the
//! `role="dialog"` content, default side bottom / align center / offset 6.
//! The `animate-in/out` classes compile to nothing upstream — no entry
//! animation. `PopoverAnchor` has no rendered use on the site and is not
//! ported.

use crate::behavior::portal::Portal;
use crate::components::button::{ButtonSize, ButtonVariant, BTN};
use leptos::prelude::*;

pub const POPOVER: &str = "asy-popover";

#[derive(Clone, Copy)]
struct PopoverCtx {
    open: RwSignal<bool>,
    content_id: StoredValue<String>,
    trigger: NodeRef<leptos::html::Button>,
}

static POPOVER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[component]
pub fn Popover(children: Children) -> impl IntoView {
    let n = POPOVER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ctx = PopoverCtx {
        open: RwSignal::new(false),
        content_id: StoredValue::new(format!("asy-popover-{n}")),
        trigger: NodeRef::new(),
    };
    // Scoped Provider — bare provide_context would let a later sibling
    // instance shadow this ctx for lazily-built children (see select.rs).
    view! { <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider> }
}

/// `Trigger asChild` on a Button, per the Button precedent; toggles.
#[component]
pub fn PopoverTrigger(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<PopoverCtx>().expect("invariant: PopoverTrigger inside Popover");
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
            aria-controls=ctx.content_id.get_value()
            data-state=move || if ctx.open.get() { "open" } else { "closed" }
            on:click=move |_| ctx.open.update(|o| *o = !*o)
        >
            {children()}
        </button>
    }
}

#[component]
pub fn PopoverContent(
    /// Gap to the anchor in px (reference default 6).
    #[prop(optional, default = 6.0)] side_offset: f64,
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let ctx = use_context::<PopoverCtx>().expect("invariant: PopoverContent inside Popover");
    let open = ctx.open;
    let trigger = ctx.trigger;
    let content_id = ctx.content_id;
    let cls = StoredValue::new({
        let mut cls = POPOVER.to_owned();
        if let Some(extra) = class {
            cls.push(' ');
            cls.push_str(&extra);
        }
        cls
    });
    let children = StoredValue::new(children);
    view! {
        <Show when=move || open.get()>
            <Portal>
                {
                    let content_ref: NodeRef<leptos::html::Div> = NodeRef::new();
                    open_effects(content_ref, trigger, open, content_id, side_offset);
                    view! {
                        <div
                            class=cls.get_value()
                            node_ref=content_ref
                            role="dialog"
                            id=content_id.get_value()
                            tabindex="-1"
                            data-align="center"
                            data-state="open"
                        >
                            {children.with_value(|c| c())}
                        </div>
                    }
                }
            </Portal>
        </Show>
    }
}

/// Open-session behavior: claim the portal host as the popper wrapper and
/// position it (bottom/center/offset, DPR-rounded translate), bracket the
/// body with plain focus guards, focus the content, dismiss on Escape /
/// outside pointerdown (trigger protected — its own click toggles) / focus
/// moving outside, restore focus to the trigger on close.
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(unused_variables))]
fn open_effects(
    content_ref: NodeRef<leptos::html::Div>,
    trigger: NodeRef<leptos::html::Button>,
    open: RwSignal<bool>,
    content_id: StoredValue<String>,
    side_offset: f64,
) {
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        use crate::behavior::dismiss::DismissGuard;
        use crate::behavior::floating::{self, Align, Side};
        use crate::components::tooltip::POPPER;
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        struct OpenSession {
            guards: Vec<web_sys::Element>,
            document: web_sys::Document,
            focusin: Closure<dyn FnMut(web_sys::Event)>,
            reposition: Closure<dyn FnMut(web_sys::Event)>,
            visual_viewport: Option<web_sys::EventTarget>,
            previous_focus: Option<web_sys::HtmlElement>,
            /// Escape/pointer closes restore focus to the trigger; a
            /// focus-outside close leaves focus where the user sent it
            /// (Radix parity — the guard the focus landed on vanishes, so
            /// the browser falls back to `<body>`).
            restore: std::rc::Rc<std::cell::Cell<bool>>,
            _dismiss: DismissGuard,
        }

        impl Drop for OpenSession {
            fn drop(&mut self) {
                let _ = self.document.remove_event_listener_with_callback(
                    "focusin",
                    self.focusin.as_ref().unchecked_ref(),
                );
                let window = leptos::tachys::dom::window();
                let cb = self.reposition.as_ref().unchecked_ref();
                let _ = window.remove_event_listener_with_callback_and_bool("scroll", cb, true);
                let _ = window.remove_event_listener_with_callback("resize", cb);
                if let Some(vv) = self.visual_viewport.take() {
                    let _ = vv.remove_event_listener_with_callback("resize", cb);
                    let _ = vv.remove_event_listener_with_callback("scroll", cb);
                }
                for guard in self.guards.drain(..) {
                    guard.remove();
                }
                if self.restore.get() {
                    if let Some(prev) = self.previous_focus.take() {
                        let _ = prev.focus();
                    }
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
            let window = leptos::tachys::dom::window();

            let place = {
                let host = host.clone();
                let content = content.clone();
                let trigger_el = trigger_el.clone();
                std::rc::Rc::new(move || {
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
                    let placement = floating::compute(
                        anchor,
                        size,
                        viewport,
                        Side::Bottom,
                        Align::Center,
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
                    }
                })
            };
            place();
            let reposition = {
                let place = std::rc::Rc::clone(&place);
                Closure::wrap(Box::new(move |_: web_sys::Event| {
                    place();
                }) as Box<dyn FnMut(web_sys::Event)>)
            };
            let cb = reposition.as_ref().unchecked_ref();
            let _ = window.add_event_listener_with_callback_and_bool("scroll", cb, true);
            let _ = window.add_event_listener_with_callback("resize", cb);
            // VisualViewport via Reflect — avoids a Cargo.toml web-sys feature pin.
            let visual_viewport = js_sys::Reflect::get(
                &window,
                &wasm_bindgen::JsValue::from_str("visualViewport"),
            )
            .ok()
            .filter(|v| !v.is_null() && !v.is_undefined())
            .and_then(|v| v.dyn_into::<web_sys::EventTarget>().ok());
            if let Some(ref vv) = visual_viewport {
                let _ = vv.add_event_listener_with_callback("resize", cb);
                let _ = vv.add_event_listener_with_callback("scroll", cb);
            }

            // Plain focus guards (no aria-hidden, no handlers — non-modal).
            let mut guards = Vec::new();
            for lead in [true, false] {
                let span = document.create_element("span").expect("invariant: create guard");
                let _ = span.set_attribute("tabindex", "0");
                crate::behavior::focus_trap::style_guard(&span);
                if lead {
                    let _ = body.insert_before(&span, body.first_child().as_ref());
                } else {
                    let _ = body.append_child(&span);
                }
                guards.push(span);
            }

            let previous_focus = document
                .active_element()
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
            let _ = content.focus();

            // Non-modal dismissal: focus moving to any element outside the
            // content (guards included) closes — Radix's focus-outside.
            // A blur to `<body>` fires no focusin, so resetting focus does
            // not close; only another element gaining focus does.
            let restore = std::rc::Rc::new(std::cell::Cell::new(true));
            let focusin = {
                let host = host.clone();
                let restore = std::rc::Rc::clone(&restore);
                Closure::wrap(Box::new(move |event: web_sys::Event| {
                    let outside = event
                        .target()
                        .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                        .is_some_and(|node| !host.contains(Some(&node)));
                    if outside {
                        restore.set(false);
                        open.set(false);
                    }
                }) as Box<dyn FnMut(web_sys::Event)>)
            };
            let _ = document
                .add_event_listener_with_callback("focusin", focusin.as_ref().unchecked_ref());

            // Trigger protected: its own click toggles closed without the
            // dismiss firing first (Radix protects the anchor the same way).
            let protected: Vec<web_sys::Node> =
                vec![host.clone().into(), (*trigger_el).clone().into()];
            let dismiss = DismissGuard::install(&document, protected, move || open.set(false));
            let _ = content_id;

            session.set_value(Some(send_wrapper::SendWrapper::new(OpenSession {
                guards,
                document: document.clone(),
                focusin,
                reposition,
                visual_viewport,
                previous_focus,
                restore,
                _dismiss: dismiss,
            })));
        });
        on_cleanup(move || session.set_value(None));
    }
}

/// `z-50 w-72 rounded-md border bg-surface p-3 shadow-md outline-none`,
/// shadow-md as v4 compiles it (`#0000001a` layers). Wrapper reuses
/// `asy-popper`.
pub fn css() -> String {
    format!(
        ".{POPOVER}{{z-index:60;width:min(18rem,calc(100vw - 1.25rem));border-radius:var(--radius-md);\
border:1px solid var(--color-border);background-color:var(--color-surface);\
padding:.75rem;outline:none;\
box-shadow:0 4px 6px -1px #0000001a,0 2px 4px -2px #0000001a}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        assert!(css().contains(&format!(".{POPOVER}{{")));
    }
}
