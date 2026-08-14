//! ConnectedToast — port of `onboarding/connected-toast.tsx`: the W-42
//! success state, rendered inline at the wizard's bottom (not a floating
//! overlay) so it survives until the user navigates on.

use crate::icons::{Icon, RI_CHECKBOX_CIRCLE_FILL};
use leptos::prelude::*;

pub const CONNECTED_TOAST: &str = "asy-connected-toast";
pub const CONNECTED_TOAST_GLYPH: &str = "asy-connected-toast__glyph";
pub const CONNECTED_TOAST_BODY: &str = "asy-connected-toast__body";
pub const CONNECTED_TOAST_TITLE: &str = "asy-connected-toast__title";
pub const CONNECTED_TOAST_DETAIL: &str = "asy-connected-toast__detail";

#[component]
pub fn ConnectedToast(#[prop(into)] device_name: String) -> impl IntoView {
    view! {
        <div role="status" class=CONNECTED_TOAST>
            <Icon d=RI_CHECKBOX_CIRCLE_FILL class=CONNECTED_TOAST_GLYPH />
            <div class=CONNECTED_TOAST_BODY>
                <span class=CONNECTED_TOAST_TITLE>"Connected"</span>
                <span class=CONNECTED_TOAST_DETAIL>
                    <span class="mono">{device_name}</span>
                    " is linked to your org and visible under Devices."
                </span>
            </div>
        </div>
    }
}

/// Row `flex items-center gap-2.5 rounded-sm border accent-line
/// bg-accent-soft px-4 py-3`; `size-5 shrink-0 text-accent` glyph; column
/// body with 13px/600 title and 12px muted detail (mono device name).
pub fn css() -> String {
    format!(
        ".{CONNECTED_TOAST}{{display:flex;align-items:center;gap:.625rem;\
border-radius:var(--radius-sm);border:1px solid var(--color-accent-line);\
background-color:var(--color-accent-soft);padding:.75rem 1rem}}\
.{CONNECTED_TOAST_GLYPH}{{width:1.25rem;height:1.25rem;flex-shrink:0;\
color:var(--color-accent)}}\
.{CONNECTED_TOAST_BODY}{{display:flex;min-width:0;flex-direction:column;\
overflow-wrap:anywhere}}\
.{CONNECTED_TOAST_TITLE}{{font-size:13px;font-weight:600;color:var(--color-text)}}\
.{CONNECTED_TOAST_DETAIL}{{font-size:12px;color:var(--color-text-muted)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            CONNECTED_TOAST,
            CONNECTED_TOAST_GLYPH,
            CONNECTED_TOAST_BODY,
            CONNECTED_TOAST_TITLE,
            CONNECTED_TOAST_DETAIL,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
