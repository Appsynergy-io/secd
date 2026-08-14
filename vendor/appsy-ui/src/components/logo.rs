//! Logo — port of `components/brand/logo.tsx`: the `[ appsynergy ]`
//! lock-up wordmark (Direction C). Brackets take the accent token unless
//! `mono` forces `currentColor`; wordmark inherits.

use leptos::prelude::*;

pub const LOGO: &str = "asy-logo";
pub const LOGO_BRACKET: &str = "asy-logo__bracket";
pub const LOGO_BRACKET_SM: &str = "asy-logo__bracket--sm";
pub const LOGO_BRACKET_MD: &str = "asy-logo__bracket--md";
pub const LOGO_BRACKET_LG: &str = "asy-logo__bracket--lg";
pub const LOGO_BRACKET_ACCENT: &str = "asy-logo__bracket--accent";
pub const LOGO_BRACKET_MONO: &str = "asy-logo__bracket--mono";
pub const LOGO_WORDMARK: &str = "asy-logo__wordmark";
pub const LOGO_WORDMARK_SM: &str = "asy-logo__wordmark--sm";
pub const LOGO_WORDMARK_MD: &str = "asy-logo__wordmark--md";
pub const LOGO_WORDMARK_LG: &str = "asy-logo__wordmark--lg";

/// `sm` ≈ 13px wordmark, `md` ≈ 16px, `lg` ≈ 22px.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LogoSize {
    Sm,
    #[default]
    Md,
    Lg,
}

#[component]
pub fn Logo(
    #[prop(optional)] size: LogoSize,
    /// Force a mono colour (header on solid accent etc.).
    #[prop(optional)] mono: bool,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let (bracket_size, wordmark_size) = match size {
        LogoSize::Sm => (LOGO_BRACKET_SM, LOGO_WORDMARK_SM),
        LogoSize::Md => (LOGO_BRACKET_MD, LOGO_WORDMARK_MD),
        LogoSize::Lg => (LOGO_BRACKET_LG, LOGO_WORDMARK_LG),
    };
    let bracket_color = if mono { LOGO_BRACKET_MONO } else { LOGO_BRACKET_ACCENT };
    let bracket_class = format!("{LOGO_BRACKET} {bracket_size} {bracket_color}");
    let mut cls = LOGO.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <span class=cls aria-label="appsynergy">
            <span class=bracket_class.clone() aria-hidden="true">"["</span>
            <span class=format!("{LOGO_WORDMARK} {wordmark_size}")>"appsynergy"</span>
            <span class=bracket_class aria-hidden="true">"]"</span>
        </span>
    }
}

/// Root `inline-flex items-center gap-1.5 select-none font-medium
/// tracking-tight`; brackets `font-light leading-none` at 18/22/30px in
/// accent or currentColor; wordmark `font-semibold leading-none
/// tracking-tight` at 13px/1rem/22px.
pub fn css() -> String {
    format!(
        ".{LOGO}{{display:inline-flex;align-items:center;gap:.375rem;user-select:none;\
font-weight:500;letter-spacing:-.025em}}\
.{LOGO_BRACKET}{{font-weight:300;line-height:1}}\
.{LOGO_BRACKET_SM}{{font-size:18px}}\
.{LOGO_BRACKET_MD}{{font-size:22px}}\
.{LOGO_BRACKET_LG}{{font-size:30px}}\
.{LOGO_BRACKET_ACCENT}{{color:var(--color-accent)}}\
.{LOGO_BRACKET_MONO}{{color:currentColor}}\
.{LOGO_WORDMARK}{{font-weight:600;line-height:1;letter-spacing:-.025em}}\
.{LOGO_WORDMARK_SM}{{font-size:13px}}\
.{LOGO_WORDMARK_MD}{{font-size:1rem}}\
.{LOGO_WORDMARK_LG}{{font-size:22px}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            LOGO,
            LOGO_BRACKET,
            LOGO_BRACKET_SM,
            LOGO_BRACKET_MD,
            LOGO_BRACKET_LG,
            LOGO_BRACKET_ACCENT,
            LOGO_BRACKET_MONO,
            LOGO_WORDMARK,
            LOGO_WORDMARK_SM,
            LOGO_WORDMARK_MD,
            LOGO_WORDMARK_LG,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
