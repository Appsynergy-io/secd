//! Hero — port of `marketing/hero.tsx`: the marketing home hero. Headline +
//! lead + dual CTA + trust bullets, with the topology diagram floated to the
//! right. Headline and lead are caller props exactly like the reference;
//! the CTA copy and trust bullets are the reference's hardcoded content
//! (ALLOW-HARDCODE: static marketing copy). The two CTA route targets are
//! props per the navigation-is-props boundary — the reference hardcodes
//! `/sign-up` and `/pricing`, the crate never does.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::badge::{Badge, BadgeTone};
use crate::components::hero_diagram::HeroDiagram;
use crate::icons::{Icon, RI_ARROW_RIGHT_LINE, RI_CHECK_LINE};
use leptos::prelude::*;

pub const HERO: &str = "asy-hero";
pub const HERO_CONTENT: &str = "asy-hero__content";
pub const HERO_BADGE: &str = "asy-hero__badge";
pub const HERO_BADGE_SEP: &str = "asy-hero__badge-sep";
pub const HERO_H1: &str = "asy-hero__h1";
pub const HERO_SUB: &str = "asy-hero__sub";
pub const HERO_CTAS: &str = "asy-hero__ctas";
pub const HERO_CTA_QUIET: &str = "asy-hero__cta-quiet";
pub const HERO_CTA_ICON: &str = "asy-hero__cta-icon";
pub const HERO_BULLETS: &str = "asy-hero__bullets";
pub const HERO_BULLET_ICON: &str = "asy-hero__bullet-icon";

/// One trust bullet: inline check glyph + text, exactly the reference's
/// span shape.
fn bullet(label: &'static str) -> impl IntoView {
    view! {
        <span>
            <Icon d=RI_CHECK_LINE class=HERO_BULLET_ICON attr:style="color: var(--color-success);" />
            {label}
        </span>
    }
}

#[component]
pub fn Hero(
    /// Single-line headline.
    #[prop(into)] h1: String,
    /// Lead paragraph below the headline.
    #[prop(into)] sub: String,
    /// Route target of the primary "Start 7-day trial" CTA.
    #[prop(into)] trial_href: String,
    /// Route target of the "See pricing" CTA.
    #[prop(into)] pricing_href: String,
) -> impl IntoView {
    view! {
        <section class=HERO>
            <HeroDiagram />
            <div class=HERO_CONTENT>
                <Badge tone=BadgeTone::Default with_dot=true class=HERO_BADGE>
                    <span style="color: var(--color-success);">"Live"</span>
                    <span class=HERO_BADGE_SEP>"·"</span>
                    "99.98% over the last 90 days"
                </Badge>
                <h1 class=HERO_H1>{h1}</h1>
                <p class=HERO_SUB>{sub}</p>
                <div class=HERO_CTAS>
                    <Button variant=ButtonVariant::Primary size=ButtonSize::Lg href=trial_href>
                        "Start 7-day trial"
                        <Icon d=RI_ARROW_RIGHT_LINE class=HERO_CTA_ICON />
                    </Button>
                    <Button
                        variant=ButtonVariant::Default
                        size=ButtonSize::Lg
                        class=HERO_CTA_QUIET
                        href=pricing_href
                    >
                        "See pricing"
                    </Button>
                </div>
                <div class=HERO_BULLETS>
                    {bullet("No card for trial")}
                    {bullet("Cancel any day")}
                    {bullet("Stripe + BTC / ETH")}
                </div>
            </div>
        </section>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{hero}{{position:relative;margin-inline:auto;max-width:1180px;",
            "padding-inline:1rem;padding-bottom:3.5rem;padding-top:72px}}",
            "@media (width >= 40rem){{.{hero}{{padding-inline:2rem}}}}",
            ".{content}{{position:relative;z-index:1;max-width:720px}}",
            ".{badge}{{margin-bottom:1.25rem;border-color:var(--color-border);",
            "color:var(--color-text-muted)}}",
            ".{sep}{{color:var(--color-text-dim)}}",
            ".{h1}{{margin-bottom:1rem;max-width:680px;text-wrap:balance;",
            "font-size:clamp(38px,5vw,64px);font-weight:600;line-height:1.05;",
            "letter-spacing:-0.03em}}",
            ".{sub}{{margin-bottom:1.75rem;max-width:580px;font-size:17px;",
            "line-height:1.55;color:var(--color-text-muted)}}",
            ".{ctas}{{display:flex;flex-wrap:wrap;align-items:center;gap:.75rem}}",
            ".{quiet}{{background-color:transparent}}",
            ".{cta_icon}{{width:.875rem;height:.875rem}}",
            ".{bullets}{{margin-top:1rem;display:flex;flex-wrap:wrap;align-items:center;",
            "gap:18px;font-size:.75rem;line-height:calc(1/.75);color:var(--color-text-dim)}}",
            ".{bullet_icon}{{margin-right:.25rem;display:inline;width:.75rem;height:.75rem}}",
        ),
        hero = HERO,
        content = HERO_CONTENT,
        badge = HERO_BADGE,
        sep = HERO_BADGE_SEP,
        h1 = HERO_H1,
        sub = HERO_SUB,
        ctas = HERO_CTAS,
        quiet = HERO_CTA_QUIET,
        cta_icon = HERO_CTA_ICON,
        bullets = HERO_BULLETS,
        bullet_icon = HERO_BULLET_ICON,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            HERO,
            HERO_CONTENT,
            HERO_BADGE,
            HERO_BADGE_SEP,
            HERO_H1,
            HERO_SUB,
            HERO_CTAS,
            HERO_CTA_QUIET,
            HERO_CTA_ICON,
            HERO_BULLETS,
            HERO_BULLET_ICON,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
