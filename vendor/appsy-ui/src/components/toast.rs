//! Toast — port of `components/ui/sonner.tsx`: the site's sonner `Toaster`
//! pinned to `theme="dark"` / `position="top-right"` with tokenized
//! `toastOptions`, plus the `toast` dispatch API. Machine (sonner's, in the
//! configuration the site ships):
//!
//! ```text
//! (empty)  --toast()/toast_success()/toast_error()--> toast prepended (front)
//! mounted  --4000 ms--> removed --200 ms--> unmounted
//! list hover: expands the stack and pauses every dismiss timer
//! Alt+T: focuses the list; toasts are Tab stops (tabindex=0)
//! ```
//!
//! DOM mirrors sonner: `<section aria-live=polite>` always present, `<ol>`
//! only while toasts exist, newest toast first (`data-index=0`, front).
//! Toast `<li>`s are keyed — lifecycle lands as attribute flips on the same
//! element (mount measures height in the pre-mount frame, then flips
//! `data-mounted`, which is what drives sonner's enter transition), and the
//! same CSS custom properties sonner writes carry the geometry (`--offset`,
//! `--initial-height`, `--front-toast-height`). Dismiss timers are
//! wall-clock, paused while the pointer is over the list.

use leptos::prelude::*;

pub const TOASTER: &str = "asy-toaster";
pub const TOAST: &str = "asy-toast";
pub const TOAST_ICON: &str = "asy-toast__icon";
pub const TOAST_CONTENT: &str = "asy-toast__content";
pub const TOAST_TITLE: &str = "asy-toast__title";
pub const TOAST_DESC: &str = "asy-toast__desc";

/// sonner's success icon (20×20 circle-check).
const SUCCESS_PATH: &str = "M10 18a8 8 0 100-16 8 8 0 000 16zm3.857-9.809a.75.75 0 00-1.214-.882l-3.483 4.79-1.88-1.88a.75.75 0 10-1.06 1.061l2.5 2.5a.75.75 0 001.137-.089l4-5.5z";
/// sonner's error icon (20×20 circle-exclamation).
const ERROR_PATH: &str = "M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-5a.75.75 0 01.75.75v4.5a.75.75 0 01-1.5 0v-4.5A.75.75 0 0110 5zm0 10a1 1 0 100-2 1 1 0 000 2z";

#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(dead_code))]
const DURATION_MS: f64 = 4000.0;
#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(dead_code))]
const UNMOUNT_DELAY_MS: u64 = 200;
const GAP_PX: f64 = 14.0;
const VISIBLE_TOASTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Normal,
    Success,
    Error,
}

#[derive(Clone, PartialEq)]
struct Entry {
    id: u64,
    kind: ToastKind,
    title: String,
    description: Option<String>,
    /// Measured `offsetHeight`, set in the pre-mount frame.
    height: RwSignal<f64>,
    measured: RwSignal<bool>,
    mounted: RwSignal<bool>,
    removed: RwSignal<bool>,
}

#[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(dead_code))]
static TOAST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(any(feature = "csr", feature = "hydrate"))]
thread_local! {
    static SINK: std::cell::RefCell<Option<RwSignal<Vec<Entry>>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg_attr(
    not(any(feature = "csr", feature = "hydrate")),
    allow(unused_variables)
)]
fn dispatch(kind: ToastKind, title: String, description: Option<String>) {
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    SINK.with(|sink| {
        if let Some(toasts) = *sink.borrow() {
            let entry = Entry {
                id: TOAST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                kind,
                title,
                description,
                height: RwSignal::new(0.0),
                measured: RwSignal::new(false),
                mounted: RwSignal::new(false),
                removed: RwSignal::new(false),
            };
            toasts.update(|list| list.insert(0, entry));
        }
    });
}

/// `toast("…")` — a plain message toast.
pub fn toast(title: impl Into<String>) {
    dispatch(ToastKind::Normal, title.into(), None);
}

/// `toast.success(title, { description })`.
pub fn toast_success(title: impl Into<String>, description: Option<String>) {
    dispatch(ToastKind::Success, title.into(), description);
}

/// `toast.error(title, { description })`.
pub fn toast_error(title: impl Into<String>, description: Option<String>) {
    dispatch(ToastKind::Error, title.into(), description);
}

#[component]
pub fn Toaster(
    /// Offset from the viewport edges at >=600px — the reference sonner's
    /// scalar `offset`, expanded to all four sides. Default matches the
    /// reference (`24px`); pass e.g. `"64px"` to clear a fixed topbar
    /// (additive prop, 2026-08-07).
    #[prop(optional, into, default = "24px".into())]
    offset: String,
    /// Below-600px offset — sonner's `mobileOffset`; default `16px`.
    #[prop(optional, into, default = "16px".into())]
    mobile_offset: String,
) -> impl IntoView {
    let toasts: RwSignal<Vec<Entry>> = RwSignal::new(Vec::new());
    let expanded = RwSignal::new(false);
    let list_ref: NodeRef<leptos::html::Ol> = NodeRef::new();

    #[cfg(any(feature = "csr", feature = "hydrate"))]
    toaster_effects(toasts, expanded, list_ref);

    view! {
        <section
            aria-label="Notifications alt+T"
            tabindex="-1"
            aria-live="polite"
            aria-relevant="additions text"
            aria-atomic="false"
        >
            <Show when=move || toasts.with(|list| !list.is_empty())>
                <ol
                    class=TOASTER
                    node_ref=list_ref
                    dir="ltr"
                    tabindex="-1"
                    data-sonner-toaster="true"
                    data-sonner-theme="dark"
                    data-y-position="top"
                    data-x-position="right"
                    // CSSOM bindings, not a style attribute: the console ships a
                    // style-src without 'unsafe-hashes', which blocks attributes.
                    style=("--front-toast-height", move || {
                        let front = toasts
                            .with(|list| list.first().map(|e| e.height.get()))
                            .unwrap_or(0.0);
                        format!("{front}px")
                    })
                    style=("--width", "356px")
                    style=("--gap", format!("{GAP_PX}px"))
                    style=("--offset-top", offset.clone())
                    style=("--offset-right", offset.clone())
                    style=("--offset-bottom", offset.clone())
                    style=("--offset-left", offset.clone())
                    style=("--mobile-offset-top", mobile_offset.clone())
                    style=("--mobile-offset-right", mobile_offset.clone())
                    style=("--mobile-offset-bottom", mobile_offset.clone())
                    style=("--mobile-offset-left", mobile_offset.clone())
                    on:pointerenter=move |_| expanded.set(true)
                    on:pointerleave=move |_| expanded.set(false)
                >
                    <For
                        each=move || toasts.get()
                        key=|entry| entry.id
                        children=move |entry| toast_li(entry, toasts, expanded)
                    />
                </ol>
            </Show>
        </section>
    }
}

/// One `<li data-sonner-toast>` in sonner's vocabulary: state carried in
/// `data-*` attributes and CSS custom properties, style overrides inline
/// exactly as `toastOptions.style` lands in the reference.
fn toast_li(
    entry: Entry,
    toasts: RwSignal<Vec<Entry>>,
    expanded: RwSignal<bool>,
) -> impl IntoView {
    let id = entry.id;
    let index = Memo::new(move |_| {
        toasts.with(|list| list.iter().position(|e| e.id == id).unwrap_or(0))
    });
    // The reference lands these as one inline style attribute; the console's
    // style-src forbids attributes, so state rides CSSOM custom properties and
    // the constant declarations live in the `.asy-toast` stylesheet rule.
    let height = entry.height;
    let icon = match entry.kind {
        ToastKind::Normal => None,
        ToastKind::Success => Some(SUCCESS_PATH),
        ToastKind::Error => Some(ERROR_PATH),
    };
    let kind_attr = match entry.kind {
        ToastKind::Normal => None,
        ToastKind::Success => Some("success"),
        ToastKind::Error => Some("error"),
    };
    let mounted = entry.mounted;
    let removed = entry.removed;
    let flag = |b: bool| if b { "true" } else { "false" };
    view! {
        <li
            class=TOAST
            tabindex="0"
            data-sonner-toast=""
            data-styled="true"
            data-mounted=move || flag(mounted.get())
            data-promise="false"
            data-swiped="false"
            data-removed=move || flag(removed.get())
            data-visible=move || flag(index.get() < VISIBLE_TOASTS)
            data-y-position="top"
            data-x-position="right"
            data-index=move || index.get().to_string()
            data-front=move || flag(index.get() == 0)
            data-swiping="false"
            data-dismissible="true"
            data-type=kind_attr
            data-swipe-out="false"
            data-expanded=move || flag(expanded.get())
            style=("--index", move || index.get().to_string())
            style=("--toasts-before", move || index.get().to_string())
            style=("--z-index", move || {
                toasts.with(|list| (list.len() - index.get()).to_string())
            })
            style=("--offset", move || {
                let px: f64 = toasts.with(|list| {
                    list.iter()
                        .take(index.get())
                        .map(|e| e.height.get() + GAP_PX)
                        .sum()
                });
                format!("{px}px")
            })
            style=("--initial-height", move || format!("{}px", height.get()))
        >
            {icon.map(|path| {
                view! {
                    <div class=TOAST_ICON data-icon="">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 20 20"
                            fill="currentColor"
                            height="20"
                            width="20"
                        >
                            <path fill-rule="evenodd" d=path clip-rule="evenodd"></path>
                        </svg>
                    </div>
                }
            })}
            <div class=TOAST_CONTENT data-content="">
                <div class=TOAST_TITLE data-title="">{entry.title.clone()}</div>
                {entry
                    .description
                    .clone()
                    .map(|d| view! { <div class=TOAST_DESC data-description="">{d}</div> })}
            </div>
        </li>
    }
}

/// Client lifecycle: register the sink for the `toast*` dispatchers, measure
/// mounted toasts, run the dismiss timers (paused while the pointer is over
/// the list), and the Alt+T focus hotkey.
#[cfg(any(feature = "csr", feature = "hydrate"))]
fn toaster_effects(
    toasts: RwSignal<Vec<Entry>>,
    expanded: RwSignal<bool>,
    list_ref: NodeRef<leptos::html::Ol>,
) {
    use leptos::leptos_dom::helpers::{set_timeout_with_handle, TimeoutHandle};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    SINK.with(|sink| *sink.borrow_mut() = Some(toasts));
    on_cleanup(|| SINK.with(|sink| *sink.borrow_mut() = None));

    // Pre-mount measurement: a fresh toast renders unmounted, its
    // `offsetHeight` is recorded, then `data-mounted` flips on the same
    // element — the flip drives sonner's enter transition.
    Effect::new(move |_| {
        toasts.track();
        let Some(list) = list_ref.get_untracked() else { return };
        let list: &web_sys::Element = list.as_ref();
        let children = list.children();
        let pending: Vec<(usize, Entry)> = toasts.with_untracked(|all| {
            all.iter()
                .enumerate()
                .filter(|(_, e)| !e.measured.get_untracked())
                .map(|(i, e)| (i, e.clone()))
                .collect()
        });
        for (i, entry) in pending {
            let Some(el) = children.item(i as u32) else { continue };
            // Fractional height, exactly as sonner measures (bounding rect,
            // not the integer offsetHeight).
            entry.height.set(el.get_bounding_client_rect().height());
            entry.measured.set(true);
            entry.mounted.set(true);
        }
    });

    // Dismiss timers: one per live toast, armed on first sight, cleared and
    // re-armed with the remaining time around list hover.
    struct Timer {
        handle: TimeoutHandle,
        deadline: f64,
        remaining: f64,
    }
    let timers: Rc<RefCell<HashMap<u64, Timer>>> = Rc::new(RefCell::new(HashMap::new()));
    let now =
        || leptos::tachys::dom::window().performance().map(|p| p.now()).unwrap_or(0.0);
    let remove_toast = move |id: u64| {
        let entry = toasts.with_untracked(|all| all.iter().find(|e| e.id == id).cloned());
        if let Some(entry) = entry {
            entry.removed.set(true);
        }
        let _ = set_timeout_with_handle(
            move || toasts.update(|all| all.retain(|e| e.id != id)),
            std::time::Duration::from_millis(UNMOUNT_DELAY_MS),
        );
    };
    let arm = {
        let timers = Rc::clone(&timers);
        move |id: u64, ms: f64| {
            let Ok(handle) = set_timeout_with_handle(
                move || remove_toast(id),
                std::time::Duration::from_millis(ms.max(0.0) as u64),
            ) else {
                return;
            };
            timers
                .borrow_mut()
                .insert(id, Timer { handle, deadline: now() + ms, remaining: ms });
        }
    };
    Effect::new({
        let timers = Rc::clone(&timers);
        let arm = arm.clone();
        move |_| {
            let live: Vec<u64> = toasts.with(|all| {
                all.iter().filter(|e| !e.removed.get_untracked()).map(|e| e.id).collect()
            });
            for id in &live {
                if !timers.borrow().contains_key(id) && !expanded.get_untracked() {
                    arm(*id, DURATION_MS);
                }
            }
            timers.borrow_mut().retain(|id, timer| {
                let keep = live.contains(id);
                if !keep {
                    timer.handle.clear();
                }
                keep
            });
        }
    });
    // Hover pause/resume (sonner pauses every timer while interacting).
    Effect::new({
        let timers = Rc::clone(&timers);
        move |_| {
            if expanded.get() {
                let current = now();
                for timer in timers.borrow_mut().values_mut() {
                    timer.remaining = (timer.deadline - current).max(0.0);
                    timer.handle.clear();
                }
            } else {
                let paused: Vec<(u64, f64)> =
                    timers.borrow_mut().drain().map(|(id, t)| (id, t.remaining)).collect();
                for (id, remaining) in paused {
                    arm(id, remaining);
                }
            }
        }
    });

    // Alt+T focuses the list (sonner's hotkey, spelled in the aria-label).
    let hotkey = Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
        if ev.alt_key() && ev.code() == "KeyT" {
            if let Some(list) = list_ref.get_untracked() {
                let list: &web_sys::HtmlElement = list.as_ref();
                let _ = list.focus();
            }
        }
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
    let document = leptos::tachys::dom::document();
    let _ =
        document.add_event_listener_with_callback("keydown", hotkey.as_ref().unchecked_ref());
    let hotkey = send_wrapper::SendWrapper::new(hotkey);
    on_cleanup(move || {
        let document = leptos::tachys::dom::document();
        let _ = document
            .remove_event_listener_with_callback("keydown", hotkey.as_ref().unchecked_ref());
    });
}

/// sonner's stylesheet, translated for the one configuration the site uses
/// (dark theme, top-right, ltr) and scoped to `asy-` classes; the utility
/// classNames from `toastOptions` are collapsed into the title/description
/// rules at their computed-winning values.
pub fn css() -> String {
    format!(
        ".{TOASTER}{{position:fixed;width:var(--width);\
font-family:ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,\
\"Segoe UI\",Roboto,\"Helvetica Neue\",Arial,\"Noto Sans\",sans-serif,\
\"Apple Color Emoji\",\"Segoe UI Emoji\",\"Segoe UI Symbol\",\
\"Noto Color Emoji\";box-sizing:border-box;padding:0;margin:0;\
list-style:none;outline:0;z-index:999999999;transition:transform .4s ease;\
--toast-icon-margin-start:-3px;--toast-icon-margin-end:4px;\
--toast-svg-margin-start:-1px;--toast-svg-margin-end:0px}}\
.{TOASTER}[data-x-position=right]{{right:var(--offset-right)}}\
.{TOASTER}[data-y-position=top]{{top:var(--offset-top)}}\
.{TOAST}{{--y:translateY(100%);--lift-amount:calc(var(--lift) * var(--gap));\
z-index:var(--z-index);position:absolute;opacity:0;transform:var(--y);\
touch-action:none;\
transition:transform .4s,opacity .4s,height .4s,box-shadow .2s;\
box-sizing:border-box;outline:0;overflow-wrap:anywhere;padding:16px;\
background:var(--color-surface);border:1px solid var(--color-border);\
color:var(--color-text);border-radius:var(--radius-md);\
box-shadow:0 4px 12px rgba(0,0,0,.1);width:var(--width);font-size:13px;\
font-family:var(--font-sans);display:flex;align-items:center;gap:6px}}\
.{TOAST}>*{{transition:opacity .4s}}\
.{TOAST}[data-y-position=top]{{top:0;--y:translateY(-100%);--lift:1;\
--lift-amount:calc(1 * var(--gap))}}\
.{TOAST}[data-x-position=right]{{right:0}}\
.{TOAST}[data-mounted=true]{{--y:translateY(0);opacity:1}}\
.{TOAST}[data-expanded=false][data-front=false]{{\
--scale:var(--toasts-before) * 0.05 + 1;\
--y:translateY(calc(var(--lift-amount) * var(--toasts-before))) \
scale(calc(-1 * var(--scale)));height:var(--front-toast-height)}}\
.{TOAST}[data-expanded=false][data-front=false]>*{{opacity:0}}\
.{TOAST}[data-visible=false]{{opacity:0;pointer-events:none}}\
.{TOAST}[data-mounted=true][data-expanded=true]{{\
--y:translateY(calc(var(--lift) * var(--offset)));\
height:var(--initial-height)}}\
.{TOAST}[data-expanded=true]::after{{content:'';position:absolute;left:0;\
height:calc(var(--gap) + 1px);bottom:100%;width:100%}}\
.{TOAST}[data-removed=true][data-front=true][data-swipe-out=false]{{\
--y:translateY(calc(var(--lift) * -100%));opacity:0}}\
.{TOAST}[data-removed=true][data-front=false][data-swipe-out=false][data-expanded=true]{{\
--y:translateY(calc(var(--lift) * var(--offset) + var(--lift) * -100%));\
opacity:0}}\
.{TOAST}[data-removed=true][data-front=false][data-swipe-out=false][data-expanded=false]{{\
--y:translateY(40%);opacity:0;transition:transform .5s,opacity .2s}}\
.{TOAST_ICON}{{display:flex;height:16px;width:16px;position:relative;\
justify-content:flex-start;align-items:center;flex-shrink:0;\
margin-left:var(--toast-icon-margin-start);\
margin-right:var(--toast-icon-margin-end)}}\
.{TOAST_ICON}>*{{flex-shrink:0}}\
.{TOAST_ICON} svg{{margin-left:var(--toast-svg-margin-start);\
margin-right:var(--toast-svg-margin-end)}}\
.{TOAST_CONTENT}{{display:flex;flex-direction:column;gap:2px}}\
.{TOAST_TITLE}{{font-size:.875rem;font-weight:500;line-height:1.5;\
color:inherit}}\
.{TOAST_DESC}{{font-size:.75rem;font-weight:400;line-height:1.4;\
color:#e8e8e8}}\
@media (max-width:600px){{\
.{TOASTER}{{position:fixed;right:var(--mobile-offset-right);\
left:var(--mobile-offset-left);width:100%}}\
.{TOASTER} .{TOAST}{{left:0;right:0;\
width:calc(100% - var(--mobile-offset-left) * 2)}}\
.{TOASTER}[data-y-position=top]{{top:var(--mobile-offset-top)}}}}\
@media (prefers-reduced-motion){{\
.{TOAST},.{TOAST}>*{{transition:none!important;animation:none!important}}}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default offsets must reproduce the reference's inline style exactly;
    /// custom offsets land on all four sides of both breakpoints.
    #[test]
    fn offset_vars_default_and_custom() {
        assert_eq!(
            offset_vars("24px", "16px"),
            "--width: 356px; --gap: 14px; --offset-top: 24px; \
             --offset-right: 24px; --offset-bottom: 24px; \
             --offset-left: 24px; --mobile-offset-top: 16px; \
             --mobile-offset-right: 16px; \
             --mobile-offset-bottom: 16px; \
             --mobile-offset-left: 16px;"
        );
        let custom = offset_vars("64px", "72px");
        for side in ["top", "right", "bottom", "left"] {
            assert!(custom.contains(&format!("--offset-{side}: 64px;")), "{custom}");
            assert!(custom.contains(&format!("--mobile-offset-{side}: 72px;")), "{custom}");
        }
    }

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [TOASTER, TOAST, TOAST_ICON, TOAST_CONTENT, TOAST_TITLE, TOAST_DESC] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
    }

    #[test]
    fn icon_paths_are_sonners() {
        assert!(SUCCESS_PATH.starts_with("M10 18a8 8 0 100-16"));
        assert!(ERROR_PATH.starts_with("M18 10a8 8 0 11-16 0"));
    }
}
