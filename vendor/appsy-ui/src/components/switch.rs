//! Switch — port of `components/ui/switch.tsx` (Radix Switch). DOM:
//! `<button type="button" role="switch" aria-checked value="on"
//! data-state>` with an always-mounted thumb span. Machine mirrors the
//! WAI-ARIA switch pattern (click/Space toggle; Enter suppressed like
//! Radix; disabled inert). Thumb travel uses the `translate` property,
//! exactly as Tailwind v4 compiles `translate-x-*`. Carries `asy-peer`.
//!
//! Inside a `<form>` Radix additionally renders its hidden BubbleInput
//! checkbox (absolute, opacity 0, sized to the control, translateX
//! -100%) so native form plumbing sees the state — mirrored here with
//! the same `closest("form")` detection: rendered on the server (Radix
//! defaults `isFormControl` true before the ref lands), removed after
//! mount when no form ancestor exists. Its `checked` attribute tracks
//! state reactively, which lands on the same settled DOM the reference
//! shows (the reference renders it with the already-loaded value).

use crate::components::label::PEER;
use leptos::prelude::*;

pub const SWITCH: &str = "asy-switch";
pub const SWITCH_CHECKED: &str = "asy-switch--checked";
pub const SWITCH_UNCHECKED: &str = "asy-switch--unchecked";
pub const SWITCH_THUMB: &str = "asy-switch__thumb";
pub const SWITCH_THUMB_CHECKED: &str = "asy-switch__thumb--checked";
pub const SWITCH_THUMB_UNCHECKED: &str = "asy-switch__thumb--unchecked";

#[component]
pub fn Switch(
    /// Controlled checked state; omit for uncontrolled.
    #[prop(optional, into)] checked: Option<Signal<bool>>,
    /// Uncontrolled initial state (Radix `defaultChecked`).
    #[prop(optional)] default_checked: bool,
    #[prop(optional)] on_checked_change: Option<Callback<bool>>,
    #[prop(optional)] disabled: bool,
    #[prop(optional, into)] id: Option<String>,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let internal = RwSignal::new(default_checked);
    let is_checked = move || checked.map_or_else(|| internal.get(), |c| c.get());
    let btn_ref = NodeRef::<leptos::html::Button>::new();
    // Radix: `isFormControl = button ? !!button.closest("form") : true`.
    let is_form_control = RwSignal::new(true);
    // Radix `useSize(control)` — measured after mount.
    let control_size = RwSignal::new(None::<(f64, f64)>);
    Effect::new(move |_| {
        if let Some(btn) = btn_ref.get() {
            is_form_control.set(btn.closest("form").ok().flatten().is_some());
            control_size.set(Some((btn.offset_width() as f64, btn.offset_height() as f64)));
        }
    });
    view! {
        <button
            node_ref=btn_ref
            type="button"
            role="switch"
            aria-checked=move || if is_checked() { "true" } else { "false" }
            data-state=move || if is_checked() { "checked" } else { "unchecked" }
            value="on"
            disabled=disabled
            id=id
            class=move || {
                let state = if is_checked() { SWITCH_CHECKED } else { SWITCH_UNCHECKED };
                let mut cls = format!("{PEER} {SWITCH} {state}");
                if let Some(extra) = &class {
                    cls.push(' ');
                    cls.push_str(extra);
                }
                cls
            }
            on:click=move |ev| {
                let next = !is_checked();
                if checked.is_none() {
                    internal.set(next);
                }
                if let Some(cb) = on_checked_change {
                    cb.run(next);
                }
                if is_form_control.get_untracked() {
                    ev.stop_propagation();
                }
            }
            on:keydown=move |ev| {
                if ev.key() == "Enter" {
                    ev.prevent_default();
                }
            }
        >
            <span
                class=move || {
                    let state = if is_checked() {
                        SWITCH_THUMB_CHECKED
                    } else {
                        SWITCH_THUMB_UNCHECKED
                    };
                    format!("{SWITCH_THUMB} {state}")
                }
                data-state=move || if is_checked() { "checked" } else { "unchecked" }
            ></span>
        </button>
        {move || {
            is_form_control
                .get()
                .then(|| {
                    view! {
                        <input
                            type="checkbox"
                            aria-hidden="true"
                            checked=move || is_checked()
                            prop:checked=move || is_checked()
                            value="on"
                            disabled=disabled
                            tabindex="-1"
                            style=move || {
                                let size = control_size
                                    .get()
                                    .map(|(w, h)| format!("width: {w}px; height: {h}px; "))
                                    .unwrap_or_default();
                                format!(
                                    "transform: translateX(-100%); {size}position: absolute; \
                                     pointer-events: none; opacity: 0; margin: 0px;"
                                )
                            }
                        />
                    }
                })
        }}
    }
}

/// Track `peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center
/// rounded-full border border-transparent transition-colors` with accent
/// fill when checked, surface-2 + border when unchecked, focus-visible
/// accent ring, disabled dim. Thumb `pointer-events-none block size-4
/// rounded-full bg-white shadow transition-transform` travelling
/// `translate-x-0.5` → `translate-x-4` (the `translate` property, per v4).
pub fn css() -> String {
    format!(
        ".{SWITCH}{{display:inline-flex;height:1.25rem;width:2.25rem;flex-shrink:0;\
cursor:pointer;align-items:center;border-radius:calc(infinity * 1px);\
border:1px solid transparent;transition-property:color,background-color,\
border-color,outline-color,text-decoration-color,fill,stroke;\
transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}\
.{SWITCH}:focus-visible{{outline-style:none;\
box-shadow:0 0 0 2px var(--color-accent-line)}}\
.{SWITCH}:disabled{{cursor:not-allowed;opacity:.5}}\
.{SWITCH_CHECKED}{{background-color:var(--color-accent)}}\
.{SWITCH_UNCHECKED}{{background-color:var(--color-surface-2);\
border-color:var(--color-border)}}\
.{SWITCH_THUMB}{{pointer-events:none;display:block;width:1rem;height:1rem;\
border-radius:calc(infinity * 1px);background-color:#fff;\
box-shadow:0 0 0 0 currentcolor,0 1px 3px 0 rgba(0,0,0,.1),\
0 1px 2px -1px rgba(0,0,0,.1);\
transition-property:transform,translate,scale,rotate;\
transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}\
.{SWITCH_THUMB_CHECKED}{{translate:1rem 0}}\
.{SWITCH_THUMB_UNCHECKED}{{translate:.125rem 0}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            SWITCH,
            SWITCH_CHECKED,
            SWITCH_UNCHECKED,
            SWITCH_THUMB,
            SWITCH_THUMB_CHECKED,
            SWITCH_THUMB_UNCHECKED,
        ] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
        assert!(css.contains(&format!(".{SWITCH}:focus-visible")));
    }
}
