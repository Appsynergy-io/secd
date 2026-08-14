//! FinalCTA — port of `marketing/final-cta.tsx`: the closing gradient card
//! with headline, lead, and dual CTA. Copy is the reference's hardcoded
//! marketing content (ALLOW-HARDCODE); the two route targets are props per
//! navigation-is-props (the reference hardcodes /sign-up and /contact).

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::icons::{Icon, RI_ARROW_RIGHT_LINE};
use leptos::prelude::*;

pub const FCTA: &str = "asy-fcta";
pub const FCTA_CARD: &str = "asy-fcta__card";
pub const FCTA_H2: &str = "asy-fcta__h2";
pub const FCTA_P: &str = "asy-fcta__p";
pub const FCTA_ROW: &str = "asy-fcta__row";
pub const FCTA_QUIET: &str = "asy-fcta__quiet";
pub const FCTA_ICON: &str = "asy-fcta__icon";

#[component]
pub fn FinalCTA(
    /// Route target of the primary "Start the trial" CTA.
    #[prop(into)] trial_href: String,
    /// Route target of the "Talk to us" CTA.
    #[prop(into)] contact_href: String,
) -> impl IntoView {
    view! {
        <section class=FCTA>
            <div
                class=FCTA_CARD
                style="background: linear-gradient(135deg, var(--color-accent-soft) 0%, transparent 65%), var(--color-surface); border-color: var(--color-accent-line);"
            >
                <h2 class=FCTA_H2>"Install the agent. Watch it stay connected."</h2>
                <p class=FCTA_P>
                    "Seven-day trial. Stripe or crypto. Cancel any day from the dashboard; your dedicated IP returns to the reserved pool, not the public one."
                </p>
                <div class=FCTA_ROW>
                    <Button variant=ButtonVariant::Primary size=ButtonSize::Lg href=trial_href>
                        "Start the trial"
                        <Icon d=RI_ARROW_RIGHT_LINE class=FCTA_ICON />
                    </Button>
                    <Button
                        variant=ButtonVariant::Default
                        size=ButtonSize::Lg
                        class=FCTA_QUIET
                        href=contact_href
                    >
                        "Talk to us"
                    </Button>
                </div>
            </div>
        </section>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{fcta}{{margin-inline:auto;margin-top:1rem;max-width:1180px;",
            "padding-inline:1rem;padding-block:3rem}}",
            "@media (width >= 40rem){{.{fcta}{{padding-inline:2rem}}}}",
            ".{card}{{display:flex;flex-direction:column;align-items:flex-start;gap:.875rem;",
            "border-radius:var(--radius-md);border-width:1px;padding:3rem}}",
            "@media (width < 40rem){{.{card}{{padding:1.5rem}}}}",
            ".{h2}{{max-width:600px;text-wrap:balance;font-size:28px;font-weight:600;",
            "letter-spacing:-0.02em}}",
            ".{p}{{max-width:560px;font-size:14.5px;color:var(--color-text-muted)}}",
            ".{row}{{display:flex;flex-wrap:wrap;align-items:center;gap:.625rem}}",
            ".{quiet}{{background-color:transparent}}",
            ".{icon}{{width:.875rem;height:.875rem}}",
        ),
        fcta = FCTA,
        card = FCTA_CARD,
        h2 = FCTA_H2,
        p = FCTA_P,
        row = FCTA_ROW,
        quiet = FCTA_QUIET,
        icon = FCTA_ICON,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [FCTA, FCTA_CARD, FCTA_H2, FCTA_P, FCTA_ROW, FCTA_QUIET, FCTA_ICON] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
