//! Tooltip — port of `components/ui/tooltip.tsx` (Radix Tooltip).
//! Machine (WAI-ARIA tooltip pattern, Radix semantics):
//!
//! ```text
//! Closed --pointerenter--> Waiting(delay) --timer--> Open(delayed)
//! Closed --pointerenter within skip window--> Open(instant)
//! Closed --focus (not from pointer)--> Open(instant)
//! Waiting --pointerleave--> Closed (timer cancelled)
//! Open --pointerleave | blur | pointerdown | Escape | outside-pointerdown--> Closed
//! ```
//!
//! The Provider carries `delay_duration` (Radix default 700ms) and a shared
//! `skip_delay_duration` window (300ms) across sibling tooltips. Open-state
//! DOM mirrors Radix: the trigger gains `aria-describedby`; a portal appends
//! a fixed popper wrapper (`left:0;top:0;transform:translate(x,y)`,
//! `min-width:max-content`) holding the content plus a visually-hidden
//! `role="tooltip"` duplicate. Positioning is `behavior/floating`
//! (offset → flip → shift), side top / align center / offset 6, translate
//! rounded to device pixels as Radix's popper does. The reference's
//! `animate-in fade-in-0 zoom-in-95` classes compile to nothing (upstream
//! never imports tw-animate-css), so the rendered truth has no entry
//! animation and neither does the port.

use crate::behavior::portal::Portal;
use crate::components::button::{ButtonSize, ButtonVariant, BTN};
use leptos::prelude::*;

pub const TOOLTIP: &str = "asy-tooltip";
pub const POPPER: &str = "asy-popper";
pub const VISUALLY_HIDDEN: &str = "asy-visually-hidden";

#[derive(Clone, Copy)]
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(dead_code))]
struct ProviderCtx {
    delay_duration: f64,
    skip_delay_duration: f64,
    /// `performance.now()` at the last tooltip close — the shared
    /// skip-delay window (moving between tooltips reopens instantly).
    last_close: RwSignal<Option<f64>>,
}

#[derive(Clone)]
struct TooltipCtx {
    open: RwSignal<bool>,
    /// Instant (focus / skip-window) vs delayed (hover) open, for
    /// `data-state`, mirroring Radix's `instant-open`/`delayed-open`.
    instant: RwSignal<bool>,
    content_id: String,
    trigger: NodeRef<leptos::html::Button>,
    provider: ProviderCtx,
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    timer: StoredValue<Option<send_wrapper::SendWrapper<TimeoutHandle>>>,
}

impl TooltipCtx {
    fn close(&self) {
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        self.timer.update_value(|t| {
            if let Some(handle) = t.take() {
                handle.take().clear();
            }
        });
        if self.open.get_untracked() {
            self.open.set(false);
            if let Some(perf) = web_sys_performance() {
                self.provider.last_close.set(Some(perf));
            }
        }
    }
}

fn web_sys_performance() -> Option<f64> {
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        leptos::tachys::dom::window().performance().map(|p| p.now())
    }
    #[cfg(not(any(feature = "csr", feature = "hydrate")))]
    {
        None
    }
}

#[component]
pub fn TooltipProvider(
    /// Hover-open delay in ms (Radix `delayDuration`, default 700).
    #[prop(optional, default = 700.0)] delay_duration: f64,
    /// Window after a close during which reopen is instant (Radix
    /// `skipDelayDuration`, default 300).
    #[prop(optional, default = 300.0)] skip_delay_duration: f64,
    children: Children,
) -> impl IntoView {
    let ctx = ProviderCtx {
        delay_duration,
        skip_delay_duration,
        last_close: RwSignal::new(None),
    };
    // Scoped Provider — bare provide_context would let a later sibling
    // instance shadow this ctx for lazily-built children (see select.rs).
    view! { <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider> }
}

/// Deterministic content-id counter: render order, same on server and in the
/// hydrating client (Radix's `useId` has the identical property).
static TOOLTIP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[component]
pub fn Tooltip(children: Children) -> impl IntoView {
    let provider = use_context::<ProviderCtx>().unwrap_or(ProviderCtx {
        delay_duration: 700.0,
        skip_delay_duration: 300.0,
        last_close: RwSignal::new(None),
    });
    let n = TOOLTIP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ctx = TooltipCtx {
        open: RwSignal::new(false),
        instant: RwSignal::new(false),
        content_id: format!("asy-tooltip-{n}"),
        trigger: NodeRef::new(),
        provider,
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        timer: StoredValue::new(None),
    };
    // Escape / outside-pointerdown dismissal while open, per Radix's
    // dismissable layer (a tooltip protects no subtree: any pointerdown
    // closes it, including on its own trigger).
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        use crate::behavior::dismiss::DismissGuard;
        let guard: StoredValue<Option<send_wrapper::SendWrapper<DismissGuard>>> =
            StoredValue::new(None);
        let ctx2 = ctx.clone();
        Effect::new(move |_| {
            if ctx2.open.get() {
                let document = leptos::tachys::dom::document();
                let on_dismiss = {
                    let ctx = ctx2.clone();
                    move || ctx.close()
                };
                guard.set_value(Some(send_wrapper::SendWrapper::new(DismissGuard::install(
                    &document,
                    Vec::new(),
                    on_dismiss,
                ))));
            } else {
                guard.set_value(None);
            }
        });
    }
    // Scoped Provider — bare provide_context would let a later sibling
    // instance shadow this ctx for lazily-built children (see select.rs).
    view! { <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider> }
}

/// The trigger in its only rendered form on the site: Radix
/// `Trigger asChild` merging onto a `Button` — mirrored, per the Button
/// precedent, as the trigger owning a `<button>` with Button's styling
/// props. No Slot machinery, same DOM.
#[component]
pub fn TooltipTrigger(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<TooltipCtx>().expect("invariant: TooltipTrigger inside Tooltip");
    let mut cls = format!("{BTN} {} {}", variant.class(), size.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    // Radix guards focus-after-click: a pointerdown suppresses the focus
    // open until blur.
    let from_pointer = StoredValue::new(false);
    let open_now = {
        let ctx = ctx.clone();
        move |instant: bool| {
            ctx.instant.set(instant);
            if !ctx.open.get_untracked() {
                ctx.open.set(true);
            }
        }
    };
    let ctx_enter = ctx.clone();
    let open_enter = open_now.clone();
    let ctx_leave = ctx.clone();
    let ctx_down = ctx.clone();
    let open_focus = open_now.clone();
    let ctx_blur = ctx.clone();
    let ctx_attr = ctx.clone();
    let ctx_state = ctx.clone();
    view! {
        <button
            class=cls
            node_ref=ctx.trigger
            aria-describedby=move || ctx_attr.open.get().then(|| ctx_attr.content_id.clone())
            data-state=move || {
                if ctx_state.open.get() {
                    if ctx_state.instant.get() { "instant-open" } else { "delayed-open" }
                } else {
                    "closed"
                }
            }
            on:pointerenter=move |_| {
                let in_skip_window = ctx_enter
                    .provider
                    .last_close
                    .get_untracked()
                    .zip(web_sys_performance())
                    .is_some_and(|(closed, now)| {
                        now - closed < ctx_enter.provider.skip_delay_duration
                    });
                if in_skip_window {
                    open_enter(true);
                    return;
                }
                #[cfg(any(feature = "csr", feature = "hydrate"))]
                {
                    let open_enter = open_enter.clone();
                    let handle = set_timeout_with_handle(
                        move || open_enter(false),
                        std::time::Duration::from_millis(ctx_enter.provider.delay_duration as u64),
                    );
                    if let Ok(handle) = handle {
                        ctx_enter.timer.set_value(Some(send_wrapper::SendWrapper::new(handle)));
                    }
                }
            }
            on:pointerleave=move |_| ctx_leave.close()
            on:pointerdown=move |_| {
                from_pointer.set_value(true);
                ctx_down.close();
            }
            on:focus=move |_| {
                if !from_pointer.get_value() {
                    open_focus(true);
                }
            }
            on:blur=move |_| {
                from_pointer.set_value(false);
                ctx_blur.close();
            }
        >
            {children()}
        </button>
    }
}

#[component]
pub fn TooltipContent(
    /// Gap to the anchor in px (reference default 6).
    #[prop(optional, default = 6.0)] side_offset: f64,
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let ctx = use_context::<TooltipCtx>().expect("invariant: TooltipContent inside Tooltip");
    let mut cls = TOOLTIP.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let open = ctx.open;
    let trigger = ctx.trigger;
    // Copy handles: the Show and Portal children closures are nested `move`
    // closures and must stay `Fn`, so everything they capture is `Copy`.
    let content_id = StoredValue::new(ctx.content_id.clone());
    let cls = StoredValue::new(cls);
    let children = StoredValue::new(children);
    view! {
        <Show when=move || open.get()>
            <Portal>
                {
                    let content_ref: NodeRef<leptos::html::Div> = NodeRef::new();
                    position_on_mount(content_ref, trigger, side_offset);
                    view! {
                        <div class=cls.get_value() node_ref=content_ref data-align="center">
                            {children.with_value(|c| c())}
                            <span
                                class=VISUALLY_HIDDEN
                                id=content_id.get_value()
                                role="tooltip"
                            >
                                {children.with_value(|c| c())}
                            </span>
                        </div>
                    }
                }
            </Portal>
        </Show>
    }
}

/// Claim the portal host as the popper wrapper (class + fixed placement) and
/// position it: side top → flip → shift, translate rounded to device pixels.
/// Repositions on scroll (capture), resize, and visualViewport changes.
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(unused_variables))]
fn position_on_mount(
    content_ref: NodeRef<leptos::html::Div>,
    trigger: NodeRef<leptos::html::Button>,
    side_offset: f64,
) {
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        use crate::behavior::floating::{self, Align, Side};
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        struct PositionSession {
            reposition: Closure<dyn FnMut(web_sys::Event)>,
            visual_viewport: Option<web_sys::EventTarget>,
        }

        impl Drop for PositionSession {
            fn drop(&mut self) {
                let window = leptos::tachys::dom::window();
                let cb = self.reposition.as_ref().unchecked_ref();
                let _ = window.remove_event_listener_with_callback_and_bool("scroll", cb, true);
                let _ = window.remove_event_listener_with_callback("resize", cb);
                if let Some(vv) = self.visual_viewport.take() {
                    let _ = vv.remove_event_listener_with_callback("resize", cb);
                    let _ = vv.remove_event_listener_with_callback("scroll", cb);
                }
            }
        }

        let session: StoredValue<Option<send_wrapper::SendWrapper<PositionSession>>> =
            StoredValue::new(None);
        Effect::new(move |_| {
            if session.with_value(Option::is_some) {
                return;
            }
            let (Some(content), Some(trigger_el)) = (content_ref.get(), trigger.get()) else {
                return;
            };
            let Some(host) = content.parent_element() else {
                return;
            };
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
                        Side::Top,
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
                    let _ = content.set_attribute("data-state", "delayed-open");
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

            session.set_value(Some(send_wrapper::SendWrapper::new(PositionSession {
                reposition,
                visual_viewport,
            })));
        });
        on_cleanup(move || session.set_value(None));
    }
}

#[cfg(any(feature = "csr", feature = "hydrate"))]
use wasm_bindgen::JsCast;

/// Content: the reference's utility string computed out — `z-50
/// overflow-hidden rounded-sm border bg-surface-2 px-2 py-1 text-xs` with
/// `shadow-md` (`#0000001a` layers, exactly as v4 compiles it). Popper
/// wrapper mirrors Radix's inline style; visually-hidden span mirrors
/// Radix VisuallyHidden.
pub fn css() -> String {
    format!(
        ".{POPPER}{{position:fixed;left:0;top:0;min-width:max-content;z-index:60}}\
.{TOOLTIP}{{z-index:60;overflow:hidden;border-radius:var(--radius-sm);\
border:1px solid var(--color-border);background-color:var(--color-surface-2);\
padding:.25rem .5rem;font-size:.75rem;line-height:calc(1/.75);\
color:var(--color-text);\
box-shadow:0 4px 6px -1px #0000001a,0 2px 4px -2px #0000001a}}\
.{VISUALLY_HIDDEN}{{position:absolute;border:0;width:1px;height:1px;\
padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);\
white-space:nowrap;overflow-wrap:normal}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [TOOLTIP, POPPER, VISUALLY_HIDDEN] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
