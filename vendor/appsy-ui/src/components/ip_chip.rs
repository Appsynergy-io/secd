//! IPChip family — port of `components/dashboard/ip-chip.tsx`: four small
//! status primitives. `IPChip` (mono IP beside a bound/off dot with accent
//! glow), `LiveDot` (success glow halo + `live-pulse` heartbeat, flat dim
//! dot when off), `BetaPill`, and `Chip` (toned mini-badge). The
//! `live-pulse` heartbeat ports here from the app.css motion layer (first
//! consumer), reduced-motion inert; its resting glow is a static box-shadow
//! so the live state still reads under reduced motion, like the reference.

use leptos::prelude::*;

pub const IP_CHIP: &str = "asy-ip-chip";
pub const IP_CHIP_DOT: &str = "asy-ip-chip__dot";
pub const IP_CHIP_DOT_BOUND: &str = "asy-ip-chip__dot--bound";
pub const IP_CHIP_DOT_OFF: &str = "asy-ip-chip__dot--off";
pub const IP_CHIP_IP: &str = "asy-ip-chip__ip";
pub const LIVE_DOT: &str = "asy-live-dot";
pub const LIVE_DOT_ON: &str = "asy-live-dot--on";
pub const LIVE_DOT_OFF: &str = "asy-live-dot--off";
pub const LIVE_PULSE: &str = "asy-live-pulse";
pub const BETA_PILL: &str = "asy-beta-pill";
pub const CHIP: &str = "asy-chip";
pub const CHIP_DEFAULT: &str = "asy-chip--default";
pub const CHIP_WARN: &str = "asy-chip--warn";
pub const CHIP_OK: &str = "asy-chip--ok";
pub const CHIP_BAD: &str = "asy-chip--bad";
pub const CHIP_ACCENT: &str = "asy-chip--accent";

/// IP address chip with a live-dot indicator. `bound=true` (default) shows
/// the accent dot; `false` the muted "off" variant.
#[component]
pub fn IpChip(
    #[prop(into)] ip: String,
    #[prop(optional, default = true)] bound: bool,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let mut cls = IP_CHIP.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let dot = if bound { IP_CHIP_DOT_BOUND } else { IP_CHIP_DOT_OFF };
    view! {
        <span class=cls>
            <span class=format!("{IP_CHIP_DOT} {dot}")></span>
            <span class=format!("mono {IP_CHIP_IP}")>{ip}</span>
        </span>
    }
}

/// Live-status indicator: success dot with glow halo + heartbeat when on,
/// flat dim dot when off.
#[component]
pub fn LiveDot(#[prop(optional)] off: bool) -> impl IntoView {
    let state = if off {
        LIVE_DOT_OFF.to_owned()
    } else {
        format!("{LIVE_DOT_ON} {LIVE_PULSE}")
    };
    view! { <span class=format!("{LIVE_DOT} {state}")></span> }
}

#[component]
pub fn BetaPill() -> impl IntoView {
    view! { <span class=BETA_PILL>"beta"</span> }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChipTone {
    Warn,
    Ok,
    Bad,
    Accent,
}

/// Toned mini-badge (`Chip` upstream); no tone = neutral surface chip.
#[component]
pub fn Chip(#[prop(optional)] tone: Option<ChipTone>, children: Children) -> impl IntoView {
    let tone_class = match tone {
        None => CHIP_DEFAULT,
        Some(ChipTone::Warn) => CHIP_WARN,
        Some(ChipTone::Ok) => CHIP_OK,
        Some(ChipTone::Bad) => CHIP_BAD,
        Some(ChipTone::Accent) => CHIP_ACCENT,
    };
    view! { <span class=format!("{CHIP} {tone_class}")>{children()}</span> }
}

/// Dots are `inline-block size-2 rounded-full`; bound/on carry the static
/// glow halo box-shadow (inline upstream, so raw oklch) and the heartbeat
/// (`live-pulse 2s ease-out infinite`, reduced-motion inert). BetaPill
/// `h-[18px] rounded-[3px] border bg-surface-2 px-1.5 text-[10px] uppercase
/// tracking-[0.04em]`; Chip `h-5 rounded border px-1.5 text-[11px]` with
/// tone tints (arbitrary oklch borders compile to rgba — TT-2).
pub fn css() -> String {
    format!(
        ".{IP_CHIP}{{display:inline-flex;align-items:center;gap:.375rem}}\
.{IP_CHIP_DOT}{{display:inline-block;width:.5rem;height:.5rem;\
border-radius:calc(infinity * 1px)}}\
.{IP_CHIP_DOT_BOUND}{{background-color:var(--color-accent);\
box-shadow:0 0 0 3px oklch(62% 0.12 220 / 0.18)}}\
.{IP_CHIP_DOT_OFF}{{background-color:var(--color-text-dim)}}\
.{IP_CHIP_IP}{{font-size:12.5px}}\
.{LIVE_DOT}{{display:inline-block;width:.5rem;height:.5rem;\
border-radius:calc(infinity * 1px)}}\
.{LIVE_DOT_ON}{{background-color:var(--color-success);\
box-shadow:0 0 0 3px oklch(70% 0.15 145 / 0.18)}}\
.{LIVE_DOT_OFF}{{background-color:var(--color-text-dim)}}\
.{LIVE_PULSE}{{animation:asy-live-pulse 2s ease-out infinite}}\
@keyframes asy-live-pulse{{0%{{box-shadow:0 0 0 0 oklch(70% 0.15 145 / 0.35)}}\
70%{{box-shadow:0 0 0 5px oklch(70% 0.15 145 / 0)}}\
100%{{box-shadow:0 0 0 0 oklch(70% 0.15 145 / 0)}}}}\
@media (prefers-reduced-motion: reduce){{.{LIVE_PULSE}{{animation:none}}}}\
.{BETA_PILL}{{display:inline-flex;height:18px;align-items:center;border-radius:3px;\
border:1px solid var(--color-border);background-color:var(--color-surface-2);\
padding-left:.375rem;padding-right:.375rem;font-size:10px;font-weight:500;\
text-transform:uppercase;letter-spacing:0.04em;color:var(--color-text-muted)}}\
.{CHIP}{{display:inline-flex;height:1.25rem;align-items:center;border-radius:.25rem;\
border-width:1px;border-style:solid;padding-left:.375rem;padding-right:.375rem;\
font-size:11px}}\
.{CHIP_DEFAULT}{{border-color:var(--color-border);\
background-color:var(--color-surface-2);color:var(--color-text-muted)}}\
.{CHIP_WARN}{{border-color:rgba(227,173,75,.3);\
background-color:var(--color-warning-soft);color:var(--color-warning)}}\
.{CHIP_OK}{{border-color:var(--color-border);\
background-color:var(--color-success-soft);color:var(--color-success)}}\
.{CHIP_BAD}{{border-color:rgba(229,85,81,.35);\
background-color:var(--color-danger-soft);color:var(--color-danger)}}\
.{CHIP_ACCENT}{{border-color:var(--color-accent-line);\
background-color:var(--color-accent-soft);color:var(--color-accent)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            IP_CHIP,
            IP_CHIP_DOT,
            IP_CHIP_DOT_BOUND,
            IP_CHIP_DOT_OFF,
            IP_CHIP_IP,
            LIVE_DOT,
            LIVE_DOT_ON,
            LIVE_DOT_OFF,
            LIVE_PULSE,
            BETA_PILL,
            CHIP,
            CHIP_DEFAULT,
            CHIP_WARN,
            CHIP_OK,
            CHIP_BAD,
            CHIP_ACCENT,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn live_pulse_is_reduced_motion_inert_but_glow_is_static() {
        let css = css();
        assert!(css.contains(&format!(
            "@media (prefers-reduced-motion: reduce){{.{LIVE_PULSE}{{animation:none}}}}"
        )));
        // The halo must not live inside the animation-only class.
        assert!(css.contains(&format!(
            ".{LIVE_DOT_ON}{{background-color:var(--color-success);\
box-shadow:0 0 0 3px oklch(70% 0.15 145 / 0.18)}}"
        )));
    }
}
