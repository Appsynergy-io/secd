//! Badge — port of `components/ui/badge.tsx` (cva base + tone matrix,
//! optional status dot). Tone values, defaults, and the class-per-state
//! mapping mirror the reference exactly.

use leptos::prelude::*;

pub const BADGE: &str = "asy-badge";
pub const BADGE_DEFAULT: &str = "asy-badge--default";
pub const BADGE_OK: &str = "asy-badge--ok";
pub const BADGE_WARN: &str = "asy-badge--warn";
pub const BADGE_BAD: &str = "asy-badge--bad";
pub const BADGE_ACCENT: &str = "asy-badge--accent";
pub const BADGE_BETA: &str = "asy-badge--beta";
pub const BADGE_DOT: &str = "asy-badge__dot";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BadgeTone {
    #[default]
    Default,
    Ok,
    Warn,
    Bad,
    Accent,
    Beta,
}

impl BadgeTone {
    fn class(self) -> &'static str {
        match self {
            Self::Default => BADGE_DEFAULT,
            Self::Ok => BADGE_OK,
            Self::Warn => BADGE_WARN,
            Self::Bad => BADGE_BAD,
            Self::Accent => BADGE_ACCENT,
            Self::Beta => BADGE_BETA,
        }
    }
}

#[component]
pub fn Badge(
    #[prop(optional)] tone: BadgeTone,
    #[prop(optional)] with_dot: bool,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = format!("{BADGE} {}", tone.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <span class=cls>
            {with_dot.then(|| view! { <span class=BADGE_DOT aria-hidden="true"></span> })}
            {children()}
        </span>
    }
}

/// Base: `inline-flex h-[22px] items-center gap-1.5 whitespace-nowrap
/// rounded-full border px-2 text-[11.5px] font-medium leading-none`.
/// Dot: `size-1.5 rounded-full bg-current`. Tone borders are the
/// reference's literal alpha colors; Tailwind compiles arbitrary literal
/// oklch to rgba at build time (TT-2 rule), so the rgba serializations
/// below are what the reference actually computes:
/// ok = oklch(70% .15 145 / .3), warn = oklch(78% .13 80 / .3),
/// bad = oklch(64% .18 25 / .3).
pub fn css() -> String {
    format!(
        ".{BADGE}{{display:inline-flex;height:22px;align-items:center;gap:.375rem;\
white-space:nowrap;border-radius:calc(infinity * 1px);border-width:1px;\
padding-left:.5rem;padding-right:.5rem;font-size:11.5px;font-weight:500;line-height:1}}\
.{BADGE_DEFAULT}{{background-color:var(--color-surface-2);\
border-color:var(--color-border);color:var(--color-text-muted)}}\
.{BADGE_OK}{{background-color:var(--color-success-soft);\
border-color:rgba(91,182,97,.3);color:var(--color-success)}}\
.{BADGE_WARN}{{background-color:var(--color-warning-soft);\
border-color:rgba(227,173,75,.3);color:var(--color-warning)}}\
.{BADGE_BAD}{{background-color:var(--color-danger-soft);\
border-color:rgba(229,85,81,.3);color:var(--color-danger)}}\
.{BADGE_ACCENT}{{background-color:var(--color-accent-soft);\
border-color:var(--color-accent-line);color:var(--color-accent)}}\
.{BADGE_BETA}{{background-color:transparent;\
border-color:var(--color-accent-line);color:var(--color-accent)}}\
.{BADGE_DOT}{{width:.375rem;height:.375rem;border-radius:calc(infinity * 1px);\
background-color:currentcolor}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [BADGE, BADGE_DEFAULT, BADGE_OK, BADGE_WARN, BADGE_BAD, BADGE_ACCENT, BADGE_BETA, BADGE_DOT]
        {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn tone_maps_to_its_class() {
        assert_eq!(BadgeTone::default().class(), BADGE_DEFAULT);
        assert_eq!(BadgeTone::Beta.class(), BADGE_BETA);
    }
}
