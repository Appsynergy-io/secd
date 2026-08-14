//! FlowGlyph — port of `dashboard/netpolicy/flow-glyph.tsx`: tiny in/out
//! diagram (internet ↔ perimeter ↔ your network). Inbound lane renders
//! blocked / specific-ports / all; outbound open / all. Purely decorative —
//! `aria-hidden`, geometry mirrored literally.

use leptos::either::Either;
use leptos::prelude::*;

pub const FLOW_GLYPH: &str = "asy-flow-glyph";

/// `NPInbound` upstream.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NpInbound {
    #[default]
    Block,
    Ports,
    All,
}

/// `NPOutbound` upstream.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NpOutbound {
    #[default]
    Open,
    All,
}

#[component]
pub fn FlowGlyph(
    #[prop(optional)] inbound: NpInbound,
    #[prop(optional)] outbound: NpOutbound,
) -> impl IntoView {
    const ACCENT: &str = "var(--color-accent)";
    const MUTED: &str = "var(--color-text-dim)";
    const DANGER: &str = "var(--color-danger)";
    let in_color = if inbound == NpInbound::Block { MUTED } else { ACCENT };

    let inbound_lane = if inbound == NpInbound::Block {
        Either::Left(view! {
            <line x1="28" y1="20" x2="56" y2="20" stroke=MUTED stroke-width="1.4" stroke-dasharray="3 3" />
            <circle cx="60" cy="20" r="5.4" fill="var(--color-danger-soft)" stroke=DANGER stroke-width="1.2" />
            <path
                d="M57.6 17.6 l4.8 4.8 M62.4 17.6 l-4.8 4.8"
                stroke=DANGER
                stroke-width="1.2"
                stroke-linecap="round"
            />
        })
    } else {
        Either::Right(view! {
            <line
                x1="28"
                y1="20"
                x2="92"
                y2="20"
                stroke=in_color
                stroke-width=if inbound == NpInbound::All { "2.2" } else { "1.6" }
                stroke-dasharray=if inbound == NpInbound::Ports { "4 3" } else { "0" }
            />
            <path d="M92 20 l-6 -3.4 v6.8 z" fill=in_color />
        })
    };

    view! {
        <svg viewBox="0 0 132 60" width="132" height="60" class=FLOW_GLYPH aria-hidden="true">
            <circle cx="16" cy="30" r="9" fill="none" stroke="var(--color-text-muted)" stroke-width="1.3" />
            <ellipse cx="16" cy="30" rx="3.6" ry="9" fill="none" stroke="var(--color-text-muted)" stroke-width="1" />
            <line x1="7" y1="30" x2="25" y2="30" stroke="var(--color-text-muted)" stroke-width="1" />
            <line x1="66" y1="9" x2="66" y2="51" stroke="var(--color-border)" stroke-width="1.3" stroke-dasharray="3 3" />
            <path
                d="M66 22 l5 2.4 v4.2 c0 3.2 -2.4 5.3 -5 6.4 c-2.6 -1.1 -5 -3.2 -5 -6.4 v-4.2 z"
                fill="var(--color-surface-2)"
                stroke="var(--color-accent-line)"
                stroke-width="1.1"
            />
            <path
                d="M63.8 29 l1.4 1.5 l2.6 -2.8"
                fill="none"
                stroke=ACCENT
                stroke-width="1.2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
            <rect x="98" y="13" width="12" height="10" rx="2.5" fill="var(--color-surface-2)" stroke="var(--color-text-muted)" stroke-width="1.1" />
            <rect x="112" y="25" width="12" height="10" rx="2.5" fill="var(--color-surface-2)" stroke="var(--color-text-muted)" stroke-width="1.1" />
            <rect x="98" y="37" width="12" height="10" rx="2.5" fill="var(--color-surface-2)" stroke="var(--color-text-muted)" stroke-width="1.1" />
            <line x1="104" y1="18" x2="118" y2="30" stroke="var(--color-border)" stroke-width="1" />
            <line x1="104" y1="42" x2="118" y2="30" stroke="var(--color-border)" stroke-width="1" />
            {inbound_lane}
            <line
                x1="92"
                y1="40"
                x2="32"
                y2="40"
                stroke=ACCENT
                stroke-width=if outbound == NpOutbound::All { "2.2" } else { "1.6" }
            />
            <path d="M32 40 l6 -3.4 v6.8 z" fill=ACCENT />
        </svg>
    }
}

/// `block shrink-0` on the svg.
pub fn css() -> String {
    format!(".{FLOW_GLYPH}{{display:block;flex-shrink:0;max-width:100%;height:auto}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        assert!(css().contains(&format!(".{FLOW_GLYPH}{{")));
    }

    #[test]
    fn defaults_mirror_reference() {
        assert_eq!(NpInbound::default(), NpInbound::Block);
        assert_eq!(NpOutbound::default(), NpOutbound::Open);
    }
}
