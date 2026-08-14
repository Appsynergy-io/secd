//! Checkbox — port of `components/ui/checkbox.tsx` (Radix Checkbox).
//! Renders Radix's DOM: `<button type="button" role="checkbox"
//! aria-checked value="on" data-state>` with the indicator span mounted
//! only while checked. Machine (WAI-ARIA checkbox pattern):
//!
//! ```text
//! unchecked --click/Space--> checked --click/Space--> unchecked
//! Enter: prevented (Radix suppresses it); disabled: inert
//! ```
//!
//! Controlled via `checked` or uncontrolled via `default_checked`, like
//! Radix. Carries the `asy-peer` marker so sibling labels can dim on
//! `:disabled` (see label.rs).

use crate::components::label::PEER;
use crate::icons::{Icon, RI_CHECK_LINE};
use leptos::prelude::*;

pub const CHECKBOX: &str = "asy-checkbox";
pub const CHECKBOX_CHECKED: &str = "asy-checkbox--checked";
pub const CHECKBOX_INDICATOR: &str = "asy-checkbox__indicator";
pub const CHECKBOX_GLYPH: &str = "asy-checkbox__glyph";

#[component]
pub fn Checkbox(
    /// Controlled checked state; omit for uncontrolled.
    #[prop(optional, into)] checked: Option<Signal<bool>>,
    /// Uncontrolled initial state (Radix `defaultChecked`).
    #[prop(optional)] default_checked: bool,
    #[prop(optional)] on_checked_change: Option<Callback<bool>>,
    #[prop(optional, into)] disabled: Signal<bool>,
    #[prop(optional, into)] id: Option<String>,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let internal = RwSignal::new(default_checked);
    let is_checked = move || checked.map_or_else(|| internal.get(), |c| c.get());
    // Radix renders a visually-hidden native input beside the control when
    // the checkbox sits inside a form (BubbleInput; `closest('form')`
    // measured after mount, optimistically true before — SSR includes it).
    let btn_ref: NodeRef<leptos::html::Button> = NodeRef::new();
    let is_form_control = RwSignal::new(true);
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    Effect::new(move |_| {
        if let Some(btn) = btn_ref.get() {
            let el: &web_sys::Element = btn.as_ref();
            is_form_control.set(el.closest("form").ok().flatten().is_some());
        }
    });
    let initial_checked = is_checked();
    view! {
        <button
            node_ref=btn_ref
            type="button"
            role="checkbox"
            aria-checked=move || if is_checked() { "true" } else { "false" }
            data-state=move || if is_checked() { "checked" } else { "unchecked" }
            value="on"
            disabled=move || disabled.get()
            id=id
            class=move || {
                let mut cls = format!("{PEER} {CHECKBOX}");
                if is_checked() {
                    cls.push(' ');
                    cls.push_str(CHECKBOX_CHECKED);
                }
                if let Some(extra) = &class {
                    cls.push(' ');
                    cls.push_str(extra);
                }
                cls
            }
            on:click=move |_| {
                let next = !is_checked();
                if checked.is_none() {
                    internal.set(next);
                }
                if let Some(cb) = on_checked_change {
                    cb.run(next);
                }
            }
            on:keydown=move |ev| {
                // WAI-ARIA: Enter does not activate a checkbox.
                if ev.key() == "Enter" {
                    ev.prevent_default();
                }
            }
        >
            {move || {
                is_checked()
                    .then(|| {
                        view! {
                            <span class=CHECKBOX_INDICATOR data-state="checked">
                                <Icon d=RI_CHECK_LINE class=CHECKBOX_GLYPH />
                            </span>
                        }
                    })
            }}
        </button>
        {move || {
            is_form_control
                .get()
                .then(|| {
                    view! {
                        <input
                            type="checkbox"
                            aria-hidden="true"
                            tabindex="-1"
                            value="on"
                            checked=initial_checked.then_some("")
                            prop:checked=is_checked
                            disabled=move || disabled.get()
                            style="transform: translateX(-100%); position: absolute; pointer-events: none; opacity: 0; margin: 0px; width: 16px; height: 16px;"
                        />
                    }
                })
        }}
    }
}

/// `peer inline-flex size-4 shrink-0 cursor-pointer items-center
/// justify-center rounded-sm border transition-colors border-border
/// bg-surface-2 text-white`, focus-visible accent ring, disabled dim,
/// checked accent fill; indicator `flex items-center justify-center
/// text-current` with a `size-3` check.
pub fn css() -> String {
    format!(
        ".{CHECKBOX}{{display:inline-flex;width:1.25rem;height:1.25rem;flex-shrink:0;\
cursor:pointer;align-items:center;justify-content:center;\
border-radius:var(--radius-sm);border-width:1px;border-style:solid;\
border-color:var(--color-border);background-color:var(--color-surface-2);\
color:#fff;transition-property:color,background-color,border-color,\
outline-color,text-decoration-color,fill,stroke;\
transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}\
.{CHECKBOX}:focus-visible{{outline-style:none;\
box-shadow:0 0 0 2px var(--color-accent-line)}}\
.{CHECKBOX}:disabled{{cursor:not-allowed;opacity:.5}}\
.{CHECKBOX_CHECKED}{{background-color:var(--color-accent);\
border-color:var(--color-accent)}}\
.{CHECKBOX_INDICATOR}{{display:flex;align-items:center;justify-content:center;\
color:currentColor}}\
.{CHECKBOX_GLYPH}{{width:.75rem;height:.75rem}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [CHECKBOX, CHECKBOX_CHECKED, CHECKBOX_INDICATOR, CHECKBOX_GLYPH] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
        assert!(css.contains(&format!(".{CHECKBOX}:focus-visible")));
        assert!(css.contains(&format!(".{CHECKBOX}:disabled")));
    }
}
