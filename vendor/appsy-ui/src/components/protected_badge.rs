//! ProtectedBadge — port of `dashboard/netpolicy/protected-badge.tsx`: the
//! non-editable "floor" reassurance under every netpolicy profile. Copy is
//! hardcoded upstream, so it is hardcoded here (zero-drift: content mirrors
//! the reference; only navigation is always props).

use crate::icons::{Icon, RI_LOCK_LINE, RI_SHIELD_CHECK_FILL};
use leptos::prelude::*;

pub const PROTECTED_BADGE: &str = "asy-protected-badge";
pub const PROTECTED_BADGE_SHIELD: &str = "asy-protected-badge__shield";
pub const PROTECTED_BADGE_BODY: &str = "asy-protected-badge__body";
pub const PROTECTED_BADGE_TITLE: &str = "asy-protected-badge__title";
pub const PROTECTED_BADGE_DETAIL: &str = "asy-protected-badge__detail";
pub const PROTECTED_BADGE_TAG: &str = "asy-protected-badge__tag";
pub const PROTECTED_BADGE_LOCK: &str = "asy-protected-badge__lock";

#[component]
pub fn ProtectedBadge() -> impl IntoView {
    view! {
        <div class=PROTECTED_BADGE>
            <Icon d=RI_SHIELD_CHECK_FILL class=PROTECTED_BADGE_SHIELD />
            <div class=PROTECTED_BADGE_BODY>
                <span class=PROTECTED_BADGE_TITLE>
                    "Always on, no matter which profile you choose"
                </span>
                <span class=PROTECTED_BADGE_DETAIL>
                    "Tenant isolation and anti-spoofing are enforced on our side. Other customers can never reach your network, and traffic can't pretend to be you."
                </span>
            </div>
            <span class=PROTECTED_BADGE_TAG>
                <Icon d=RI_LOCK_LINE class=PROTECTED_BADGE_LOCK />
                " Enforced"
            </span>
        </div>
    }
}

/// Row `flex items-center gap-3 rounded-md border
/// border-[oklch(70%_0.15_145_/_0.30)] (rgba per TT-2) bg-success-soft
/// px-3.5 py-2.5`; shield `size-[17px] shrink-0 text-success`; body
/// `flex flex-1 flex-col gap-px`; title 12.5px/500; detail 11.5px muted;
/// tag `flex shrink-0 items-center gap-1.5 text-[11px] text-dim` with a
/// `size-3` lock.
pub fn css() -> String {
    format!(
        ".{PROTECTED_BADGE}{{display:flex;align-items:center;gap:.75rem;\
border-radius:var(--radius-md);border:1px solid rgba(91,182,97,.3);\
background-color:var(--color-success-soft);padding-left:.875rem;\
padding-right:.875rem;padding-top:.625rem;padding-bottom:.625rem}}\
.{PROTECTED_BADGE_SHIELD}{{width:17px;height:17px;flex-shrink:0;\
color:var(--color-success)}}\
.{PROTECTED_BADGE_BODY}{{display:flex;flex:1 1 0%;flex-direction:column;gap:1px}}\
.{PROTECTED_BADGE_TITLE}{{font-size:12.5px;font-weight:500;color:var(--color-text)}}\
.{PROTECTED_BADGE_DETAIL}{{font-size:11.5px;color:var(--color-text-muted)}}\
.{PROTECTED_BADGE_TAG}{{display:flex;flex-shrink:0;align-items:center;gap:.375rem;\
font-size:11px;color:var(--color-text-dim)}}\
.{PROTECTED_BADGE_LOCK}{{width:.75rem;height:.75rem}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            PROTECTED_BADGE,
            PROTECTED_BADGE_SHIELD,
            PROTECTED_BADGE_BODY,
            PROTECTED_BADGE_TITLE,
            PROTECTED_BADGE_DETAIL,
            PROTECTED_BADGE_TAG,
            PROTECTED_BADGE_LOCK,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
