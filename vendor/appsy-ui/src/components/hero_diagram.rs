//! HeroDiagram — port of `marketing/hero-diagram.tsx`: the abstract topology
//! SVG anchored top-right of the marketing hero. Eight spoke devices around
//! a central PoP (fra-1), devices 2 and 5 highlighted (device 2 is a
//! Dedicated IP with its address label), and a dashed Tor hand-off upper
//! right. Pure SVG, no props.
//!
//! Spoke coordinates are the reference's `cos`/`sin` results serialized by
//! the JS runtime, embedded verbatim (`486.06601717798213`,
//! `180.00000000000003`) — recomputing them in Rust would drift the
//! attribute strings by float-formatting rules, not geometry.

use leptos::prelude::*;

pub const HERO_DIAGRAM: &str = "asy-hero-diagram";

/// (x, y, highlighted) per spoke, in the reference's index order.
/// Only spoke 2 draws solid (`stroke-dasharray="0"`) and carries the
/// Dedicated-IP label; spoke 5 is highlighted but keeps the dash.
const SPOKES: [(&str, &str, bool); 8] = [
    ("380", "30", false),
    ("486.06601717798213", "73.93398282201788", false),
    ("530", "180", true),
    ("486.06601717798213", "286.06601717798213", false),
    ("380", "330", false),
    ("273.93398282201787", "286.06601717798213", true),
    ("230", "180.00000000000003", false),
    ("273.93398282201787", "73.93398282201788", false),
];

#[component]
pub fn HeroDiagram() -> impl IntoView {
    view! {
        <svg
            width="540"
            height="380"
            viewBox="0 0 540 380"
            class=HERO_DIAGRAM
            aria-hidden="true"
        >
            <defs>
                <radialGradient id="hero-diagram-fade" cx="50%" cy="50%" r="50%">
                    <stop offset="0%" stop-color="var(--color-accent)" stop-opacity="0.18"></stop>
                    <stop offset="100%" stop-color="var(--color-accent)" stop-opacity="0"></stop>
                </radialGradient>
            </defs>
            <circle cx="380" cy="180" r="170" fill="url(#hero-diagram-fade)"></circle>
            {SPOKES
                .iter()
                .enumerate()
                .map(|(i, (x, y, highlighted))| {
                    let highlighted = *highlighted;
                    view! {
                        <g>
                            <line
                                x1="380"
                                y1="180"
                                x2=*x
                                y2=*y
                                stroke="var(--color-accent)"
                                stroke-width="0.8"
                                opacity=if highlighted { "0.7" } else { "0.25" }
                                stroke-dasharray=if i == 2 { "0" } else { "3 4" }
                            ></line>
                            <circle
                                cx=*x
                                cy=*y
                                r=if highlighted { "5" } else { "3.5" }
                                fill="var(--color-bg)"
                                stroke="var(--color-accent)"
                                stroke-width=if highlighted { "1.6" } else { "1" }
                            ></circle>
                            {(i == 2)
                                .then(|| {
                                    view! {
                                        <text
                                            x="520"
                                            y="183"
                                            text-anchor="end"
                                            font-size="10"
                                            fill="var(--color-text-muted)"
                                            font-family="var(--font-mono)"
                                        >
                                            "203.0.113.41"
                                        </text>
                                    }
                                })}
                        </g>
                    }
                })
                .collect_view()}
            <circle
                cx="380"
                cy="180"
                r="22"
                fill="var(--color-bg)"
                stroke="var(--color-accent)"
                stroke-width="1.6"
            ></circle>
            <circle cx="380" cy="180" r="6" fill="var(--color-accent)"></circle>
            <text
                x="380"
                y="216"
                font-size="10"
                fill="var(--color-text-muted)"
                text-anchor="middle"
                font-family="var(--font-mono)"
            >
                "fra-1"
            </text>
            <line
                x1="402"
                y1="180"
                x2="500"
                y2="120"
                stroke="var(--color-accent)"
                stroke-width="0.8"
                stroke-dasharray="3 3"
                opacity="0.5"
            ></line>
            <text
                x="450"
                y="115"
                font-size="9"
                fill="var(--color-text-dim)"
                font-family="var(--font-mono)"
            >
                "→ tor"
            </text>
        </svg>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{d}{{pointer-events:none;position:absolute;right:-1.25rem;top:1.75rem;",
            "opacity:.8;max-width:min(540px,100%);height:auto}}",
            "@media (width < 48rem){{.{d}{{display:none}}}}",
        ),
        d = HERO_DIAGRAM,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoke_coordinates_are_the_reference_serializations() {
        // The trig identities the strings encode: 380±150cos, 180±150sin.
        assert_eq!(SPOKES.len(), 8);
        assert_eq!(SPOKES[2], ("530", "180", true));
        assert_eq!(SPOKES[5].2, true);
        assert!(SPOKES[6].1.starts_with("180.0000000000000"));
    }
}
