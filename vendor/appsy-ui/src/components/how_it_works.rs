//! HowItWorksSection — port of `marketing/how-it-works-section.tsx`: eyebrow
//! + headline over a three-card step grid (install / claim IP / write ACL).
//! No props; all copy is the reference's hardcoded marketing content
//! (ALLOW-HARDCODE).

use crate::icons::{Icon, RI_DOWNLOAD_CLOUD_2_LINE, RI_ROUTER_LINE, RI_SHIELD_KEYHOLE_LINE};
use leptos::prelude::*;

pub const HIW: &str = "asy-hiw";
pub const HIW_HEAD: &str = "asy-hiw__head";
pub const HIW_EYEBROW: &str = "asy-hiw__eyebrow";
pub const HIW_H2: &str = "asy-hiw__h2";
pub const HIW_GRID: &str = "asy-hiw__grid";
pub const HIW_CARD: &str = "asy-hiw__card";
pub const HIW_CARD_HEAD: &str = "asy-hiw__card-head";
pub const HIW_NUM: &str = "asy-hiw__num";
pub const HIW_ICON: &str = "asy-hiw__icon";
pub const HIW_H3: &str = "asy-hiw__h3";
pub const HIW_P: &str = "asy-hiw__p";

const STEPS: [(&str, &str, &str, &str); 3] = [
    (
        "01",
        "Install the agent",
        "One-liner on macOS, Linux, Windows. Mobile via the App Store / Play. The agent registers, terminates a WireGuard tunnel at the nearest PoP, and stays connected.",
        RI_DOWNLOAD_CLOUD_2_LINE,
    ),
    (
        "02",
        "Claim a dedicated IP",
        "Pick one from your reserved pool, bind it to a tunnel. Port-forward subsets if your plan allows. The IP survives reconnects, restarts, and device swaps.",
        RI_ROUTER_LINE,
    ),
    (
        "03",
        "Write an ACL",
        "Say once: \"anyone in Engineering reaches staging.internal:443.\" The policy is enforced in-path at the PoP, not in the app. Audit logged.",
        RI_SHIELD_KEYHOLE_LINE,
    ),
];

#[component]
pub fn HowItWorksSection() -> impl IntoView {
    view! {
        <section class=HIW>
            <div class=HIW_HEAD>
                <span class=HIW_EYEBROW>"How it works"</span>
                <h2 class=HIW_H2>"One agent. One control plane. Routing that decides."</h2>
            </div>
            <div class=HIW_GRID>
                {STEPS
                    .iter()
                    .map(|(n, h, p, d)| {
                        view! {
                            <div class=HIW_CARD>
                                <div class=HIW_CARD_HEAD>
                                    <span class=HIW_NUM>{*n}</span>
                                    <Icon d=*d class=HIW_ICON />
                                </div>
                                <h3 class=HIW_H3>{*h}</h3>
                                <p class=HIW_P>{*p}</p>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </section>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{hiw}{{margin-inline:auto;max-width:1180px;padding-inline:1rem;",
            "padding-bottom:3rem;padding-top:72px}}",
            "@media (width >= 40rem){{.{hiw}{{padding-inline:2rem}}}}",
            ".{head}{{margin-bottom:2.25rem;display:flex;max-width:640px;",
            "flex-direction:column;gap:.5rem}}",
            ".{eyebrow}{{font-size:11px;font-weight:600;text-transform:uppercase;",
            "letter-spacing:.08em;color:var(--color-accent)}}",
            ".{h2}{{font-size:32px;font-weight:600;letter-spacing:-0.02em}}",
            ".{grid}{{display:grid;grid-template-columns:repeat(1,minmax(0,1fr));gap:.875rem}}",
            "@media (width >= 40rem){{.{grid}{{grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            "@media (width >= 48rem){{.{grid}{{grid-template-columns:repeat(3,minmax(0,1fr))}}}}",
            ".{card}{{position:relative;display:flex;flex-direction:column;gap:.75rem;",
            "border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:1.25rem}}",
            ".{card_head}{{display:flex;align-items:center;justify-content:space-between}}",
            ".{num}{{font-family:var(--font-mono);font-feature-settings:\"ss01\";",
            "font-size:11px;letter-spacing:.04em;color:var(--color-text-dim)}}",
            ".{icon}{{width:18px;height:18px;color:var(--color-accent)}}",
            ".{h3}{{font-size:17px;font-weight:600;letter-spacing:-0.01em}}",
            ".{p}{{font-size:13.5px;line-height:1.55;color:var(--color-text-muted)}}",
        ),
        hiw = HIW,
        head = HIW_HEAD,
        eyebrow = HIW_EYEBROW,
        h2 = HIW_H2,
        grid = HIW_GRID,
        card = HIW_CARD,
        card_head = HIW_CARD_HEAD,
        num = HIW_NUM,
        icon = HIW_ICON,
        h3 = HIW_H3,
        p = HIW_P,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            HIW, HIW_HEAD, HIW_EYEBROW, HIW_H2, HIW_GRID, HIW_CARD, HIW_CARD_HEAD, HIW_NUM,
            HIW_ICON, HIW_H3, HIW_P,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn steps_mirror_reference() {
        assert_eq!(STEPS.len(), 3);
        assert_eq!(STEPS[2].1, "Write an ACL");
        assert!(STEPS[2].2.contains("staging.internal:443"));
    }
}
