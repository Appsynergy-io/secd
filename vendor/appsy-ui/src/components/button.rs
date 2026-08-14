//! Button — port of `components/ui/button.tsx` (cva base + variant/size
//! matrix, `asChild` via Radix Slot). Variant and size values, defaults, and
//! the class-per-state mapping mirror the reference exactly.
//!
//! `asChild` in the reference merges button styling onto an arbitrary child;
//! its only rendered use is an anchor styled as a button. The Rust mirror is
//! the `href` prop: present → renders `<a href …>`, absent → `<button>`.
//! Same DOM, same styling, no Slot machinery.

use leptos::either::Either;
use leptos::prelude::*;

pub const BTN: &str = "asy-btn";
pub const BTN_DEFAULT: &str = "asy-btn--default";
pub const BTN_PRIMARY: &str = "asy-btn--primary";
pub const BTN_GHOST: &str = "asy-btn--ghost";
pub const BTN_DANGER: &str = "asy-btn--danger";
pub const BTN_SM: &str = "asy-btn--sm";
pub const BTN_MD: &str = "asy-btn--md";
pub const BTN_LG: &str = "asy-btn--lg";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Primary,
    Ghost,
    Danger,
}

impl ButtonVariant {
    pub(crate) fn class(self) -> &'static str {
        match self {
            Self::Default => BTN_DEFAULT,
            Self::Primary => BTN_PRIMARY,
            Self::Ghost => BTN_GHOST,
            Self::Danger => BTN_DANGER,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl ButtonSize {
    pub(crate) fn class(self) -> &'static str {
        match self {
            Self::Sm => BTN_SM,
            Self::Md => BTN_MD,
            Self::Lg => BTN_LG,
        }
    }
}

#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    /// `bool` still compiles; pass a signal and pending states toggle
    /// without re-rendering the button (approved API change, 2026-08-07;
    /// frozen again).
    #[prop(optional, into)]
    disabled: Signal<bool>,
    /// Renders `<a href …>` instead of `<button>` — the reference's
    /// `asChild` anchor form.
    #[prop(optional, into)] href: Option<String>,
    /// Extra classes appended after the component's own (the reference's
    /// `className` passthrough position).
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = format!("{BTN} {} {}", variant.class(), size.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    match href {
        Some(href) => Either::Left(view! {
            <a class=cls href=href>{children()}</a>
        }),
        None => Either::Right(view! {
            <button class=cls disabled=move || disabled.get()>{children()}</button>
        }),
    }
}

/// Styles for the base class, states, and every variant/size — computed-value
/// translation of the reference's utility strings. Sizes own `font-size`
/// exactly as the reference resolves after `tailwind-merge`: `sm`/`md`
/// replace the base `text-sm` (so `md`'s line-height falls back to
/// inheritance), `lg` restates it. Hover rules sit under
/// `@media (hover:hover)` because Tailwind's `hover:` variant does — on
/// touch devices the reference's hover styling never applies, so neither
/// does the port's.
pub fn css() -> String {
    format!(
        ".{BTN}{{display:inline-flex;align-items:center;justify-content:center;\
gap:.5rem;white-space:nowrap;border-radius:var(--radius-sm);font-weight:500;\
transition-property:color,background-color,border-color,text-decoration-color,fill,stroke;\
transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s;cursor:pointer}}\
.{BTN}:focus-visible{{outline:none;box-shadow:0 0 0 2px var(--color-accent-line)}}\
.{BTN}:disabled{{pointer-events:none;opacity:.5}}\
.{BTN_DEFAULT}{{background-color:var(--color-surface-2);\
border:1px solid var(--color-border);color:var(--color-text)}}\
.{BTN_PRIMARY}{{background-color:var(--color-accent);\
border:1px solid var(--color-accent);color:#fff}}\
.{BTN_GHOST}{{background-color:transparent;border:1px solid transparent;\
color:var(--color-text-muted)}}\
.{BTN_DANGER}{{background-color:transparent;\
border:1px solid color-mix(in oklab,var(--color-danger)40%,transparent);\
color:var(--color-danger)}}\
@media(hover:hover){{\
.{BTN_DEFAULT}:hover{{background-color:var(--color-surface);border-color:oklch(35% 0 0)}}\
.{BTN_PRIMARY}:hover{{filter:brightness(1.1)}}\
.{BTN_GHOST}:hover{{background-color:var(--color-surface-2);color:var(--color-text)}}\
.{BTN_DANGER}:hover{{background-color:var(--color-danger-soft)}}\
}}\
.{BTN_SM}{{height:2.75rem;padding-left:.5rem;padding-right:.5rem;\
font-size:.75rem;line-height:1rem}}\
.{BTN_MD}{{height:2rem;padding-left:.75rem;padding-right:.75rem;font-size:13px}}\
.{BTN_LG}{{height:2.5rem;padding-left:1rem;padding-right:1rem;\
font-size:.875rem;line-height:1.25rem}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [BTN, BTN_DEFAULT, BTN_PRIMARY, BTN_GHOST, BTN_DANGER, BTN_SM, BTN_MD, BTN_LG]
        {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn variant_and_size_map_to_their_classes() {
        assert_eq!(ButtonVariant::default().class(), BTN_DEFAULT);
        assert_eq!(ButtonVariant::Primary.class(), BTN_PRIMARY);
        assert_eq!(ButtonSize::default().class(), BTN_MD);
        assert_eq!(ButtonSize::Lg.class(), BTN_LG);
    }
}
