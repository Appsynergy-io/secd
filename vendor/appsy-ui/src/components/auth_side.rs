//! AuthSide — port of `auth/auth-side.tsx`: the auth shell's decoration
//! column. Hub-spoke topology SVG bleeding off the top-right plus the
//! value-prop block bottom-left. Hidden below `lg` exactly like the
//! reference. No props; copy is the reference's hardcoded content
//! (ALLOW-HARDCODE).
//!
//! Spoke coordinates embed the JS runtime's serialized trig results
//! verbatim (`112.15390309173472`, `439.99999999999994`) — recomputing at
//! render time would drift the attribute strings across runtimes.

use crate::icons::{Icon, RI_EYE_OFF_LINE, RI_FINGERPRINT_LINE, RI_SHIELD_CHECK_LINE};
use leptos::prelude::*;

pub const ASIDE: &str = "asy-aside";
pub const ASIDE_SVG: &str = "asy-aside__svg";
pub const ASIDE_CONTENT: &str = "asy-aside__content";
pub const ASIDE_KICKER: &str = "asy-aside__kicker";
pub const ASIDE_H2: &str = "asy-aside__h2";
pub const ASIDE_P: &str = "asy-aside__p";
pub const ASIDE_CHIPS: &str = "asy-aside__chips";
pub const ASIDE_CHIP: &str = "asy-aside__chip";
pub const ASIDE_CHIP_ICON: &str = "asy-aside__chip-icon";

/// (x, y, opacity, dasharray, r) per spoke, reference index order:
/// opacity `0.2 + (i%3)*0.2` (the `0.6000000000000001` is the JS float
/// sum serialized), dash solid every 4th, radius 6 every 3rd.
const SPOKES: [(&str, &str, &str, &str, &str); 12] = [
    ("320", "80", "0.2", "0", "6"),
    ("440", "112.15390309173472", "0.4", "3 4", "4"),
    ("527.8460969082653", "200", "0.6000000000000001", "3 4", "4"),
    ("560", "320", "0.2", "3 4", "6"),
    ("527.8460969082653", "439.99999999999994", "0.4", "0", "4"),
    ("440", "527.8460969082653", "0.6000000000000001", "3 4", "4"),
    ("320", "560", "0.2", "3 4", "6"),
    ("199.99999999999994", "527.8460969082653", "0.4", "3 4", "4"),
    ("112.15390309173478", "440.0000000000001", "0.6000000000000001", "0", "4"),
    ("80", "320.00000000000006", "0.2", "3 4", "6"),
    ("112.15390309173475", "199.99999999999997", "0.4", "3 4", "4"),
    ("199.9999999999999", "112.15390309173478", "0.6000000000000001", "3 4", "4"),
];

fn chip(d: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <span class=ASIDE_CHIP>
            <Icon d=d class=ASIDE_CHIP_ICON />
            {label}
        </span>
    }
}

#[component]
pub fn AuthSide() -> impl IntoView {
    view! {
        <div class=ASIDE>
            <svg
                width="640"
                height="640"
                viewBox="0 0 640 640"
                class=ASIDE_SVG
                aria-hidden="true"
            >
                <defs>
                    <radialGradient id="auth-side-fade" cx="50%" cy="50%" r="50%">
                        <stop offset="0%" stop-color="var(--color-accent)" stop-opacity="0.22"></stop>
                        <stop offset="100%" stop-color="var(--color-accent)" stop-opacity="0"></stop>
                    </radialGradient>
                </defs>
                <circle cx="320" cy="320" r="280" fill="url(#auth-side-fade)"></circle>
                {SPOKES
                    .iter()
                    .map(|(x, y, opacity, dash, r)| {
                        view! {
                            <g>
                                <line
                                    x1="320"
                                    y1="320"
                                    x2=*x
                                    y2=*y
                                    stroke="var(--color-accent)"
                                    stroke-width="0.7"
                                    opacity=*opacity
                                    stroke-dasharray=*dash
                                ></line>
                                <circle
                                    cx=*x
                                    cy=*y
                                    r=*r
                                    fill="var(--color-bg)"
                                    stroke="var(--color-accent)"
                                    stroke-width="1.4"
                                ></circle>
                            </g>
                        }
                    })
                    .collect_view()}
                <circle
                    cx="320"
                    cy="320"
                    r="28"
                    fill="var(--color-bg)"
                    stroke="var(--color-accent)"
                    stroke-width="1.6"
                ></circle>
                <circle cx="320" cy="320" r="9" fill="var(--color-accent)"></circle>
            </svg>
            <div class=ASIDE_CONTENT>
                <span class=ASIDE_KICKER>"What you get"</span>
                <h2 class=ASIDE_H2>"One agent. One control plane. Routing that decides."</h2>
                <p class=ASIDE_P>
                    "Install on macOS, Linux, Windows, iOS, Android. Bind a dedicated IP. Write an ACL in plain English. Cancel any day."
                </p>
                <div class=ASIDE_CHIPS>
                    {chip(RI_FINGERPRINT_LINE, "Passkey-first")}
                    {chip(RI_SHIELD_CHECK_LINE, "99.98% / 90d")}
                    {chip(RI_EYE_OFF_LINE, "No tracking pixels")}
                </div>
            </div>
        </div>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{aside}{{position:relative;display:none;flex:1;align-items:flex-end;",
            "overflow:hidden;border-left-width:1px;border-color:var(--color-border);",
            "background-color:var(--color-surface);padding:3rem}}",
            "@media (width >= 64rem){{.{aside}{{display:flex}}}}",
            ".{svg}{{pointer-events:none;position:absolute;right:-6rem;top:-8rem;",
            "opacity:.6}}",
            ".{content}{{position:relative;z-index:1;display:flex;max-width:480px;",
            "flex-direction:column;gap:.5rem}}",
            ".{kicker}{{font-size:11px;font-weight:600;text-transform:uppercase;",
            "letter-spacing:.08em;color:var(--color-accent)}}",
            ".{h2}{{font-size:26px;font-weight:600;line-height:1.15;",
            "letter-spacing:-0.02em}}",
            ".{p}{{font-size:13.5px;line-height:1.55;color:var(--color-text-muted)}}",
            ".{chips}{{margin-top:.75rem;display:flex;flex-wrap:wrap;align-items:center;",
            "gap:.75rem;font-size:.75rem;line-height:calc(1/.75);",
            "color:var(--color-text-muted)}}",
            ".{chip}{{display:inline-flex;align-items:center;gap:.25rem}}",
            ".{chip_icon}{{width:.875rem;height:.875rem;color:var(--color-accent)}}",
        ),
        aside = ASIDE,
        svg = ASIDE_SVG,
        content = ASIDE_CONTENT,
        kicker = ASIDE_KICKER,
        h2 = ASIDE_H2,
        p = ASIDE_P,
        chips = ASIDE_CHIPS,
        chip = ASIDE_CHIP,
        chip_icon = ASIDE_CHIP_ICON,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            ASIDE, ASIDE_SVG, ASIDE_CONTENT, ASIDE_KICKER, ASIDE_H2, ASIDE_P, ASIDE_CHIPS,
            ASIDE_CHIP, ASIDE_CHIP_ICON,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn spokes_mirror_the_reference_pattern() {
        assert_eq!(SPOKES.len(), 12);
        for (i, s) in SPOKES.iter().enumerate() {
            assert_eq!(s.3 == "0", i % 4 == 0, "dash at {i}");
            assert_eq!(s.4 == "6", i % 3 == 0, "radius at {i}");
        }
    }
}
