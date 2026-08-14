//! ProfileCard — port of `dashboard/netpolicy/profile-card.tsx`: one
//! selectable netpolicy profile — icon chip, name, default pill, radio,
//! plain-English line, and the in/out FlowGlyph. `aria-pressed` tracks
//! selection; the profile definition arrives as a prop (`NPProfileDef`).

use crate::components::flow_glyph::{FlowGlyph, NpInbound, NpOutbound};
use crate::icons::{Icon, RI_CHECK_LINE};
use leptos::prelude::*;

pub const PROFILE_CARD: &str = "asy-profile-card";
pub const PROFILE_CARD_ACTIVE: &str = "asy-profile-card--active";
pub const PROFILE_CARD_IDLE: &str = "asy-profile-card--idle";
pub const PROFILE_CARD_HEAD: &str = "asy-profile-card__head";
pub const PROFILE_CARD_ID: &str = "asy-profile-card__id";
pub const PROFILE_CARD_CHIP: &str = "asy-profile-card__chip";
pub const PROFILE_CARD_CHIP_ACTIVE: &str = "asy-profile-card__chip--active";
pub const PROFILE_CARD_CHIP_IDLE: &str = "asy-profile-card__chip--idle";
pub const PROFILE_CARD_GLYPH: &str = "asy-profile-card__glyph";
pub const PROFILE_CARD_NAME: &str = "asy-profile-card__name";
pub const PROFILE_CARD_DEFAULT_PILL: &str = "asy-profile-card__default-pill";
pub const PROFILE_CARD_RADIO: &str = "asy-profile-card__radio";
pub const PROFILE_CARD_RADIO_ACTIVE: &str = "asy-profile-card__radio--active";
pub const PROFILE_CARD_RADIO_IDLE: &str = "asy-profile-card__radio--idle";
pub const PROFILE_CARD_CHECK: &str = "asy-profile-card__check";
pub const PROFILE_CARD_LINE: &str = "asy-profile-card__line";
pub const PROFILE_CARD_FLOW: &str = "asy-profile-card__flow";

/// `NPProfileDef` upstream: icon is an `icons::*` path constant.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NpProfileDef {
    pub key: String,
    pub name: String,
    pub icon: &'static str,
    pub line: String,
    pub inbound: NpInbound,
    pub outbound: NpOutbound,
    pub is_default: bool,
}

#[component]
pub fn ProfileCard(
    profile: NpProfileDef,
    #[prop(into)] active: Signal<bool>,
    on_select: Callback<()>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            aria-pressed=move || if active.get() { "true" } else { "false" }
            class=move || {
                let state = if active.get() { PROFILE_CARD_ACTIVE } else { PROFILE_CARD_IDLE };
                format!("{PROFILE_CARD} {state}")
            }
            on:click=move |_| on_select.run(())
        >
            <div class=PROFILE_CARD_HEAD>
                <div class=PROFILE_CARD_ID>
                    <span class=move || {
                        let state = if active.get() {
                            PROFILE_CARD_CHIP_ACTIVE
                        } else {
                            PROFILE_CARD_CHIP_IDLE
                        };
                        format!("{PROFILE_CARD_CHIP} {state}")
                    }>
                        <Icon d=profile.icon class=PROFILE_CARD_GLYPH />
                    </span>
                    <span class=PROFILE_CARD_NAME>
                        {profile.name.clone()}
                        {profile
                            .is_default
                            .then(|| {
                                view! {
                                    <span class=PROFILE_CARD_DEFAULT_PILL>"default"</span>
                                }
                            })}
                    </span>
                </div>
                <span class=move || {
                    let state = if active.get() {
                        PROFILE_CARD_RADIO_ACTIVE
                    } else {
                        PROFILE_CARD_RADIO_IDLE
                    };
                    format!("{PROFILE_CARD_RADIO} {state}")
                }>
                    {move || {
                        active
                            .get()
                            .then(|| view! { <Icon d=RI_CHECK_LINE class=PROFILE_CARD_CHECK /> })
                    }}
                </span>
            </div>
            <p class=PROFILE_CARD_LINE>{profile.line.clone()}</p>
            <div class=PROFILE_CARD_FLOW>
                <FlowGlyph inbound=profile.inbound outbound=profile.outbound />
            </div>
        </button>
    }
}

/// Root `relative flex cursor-pointer flex-col gap-3 rounded-lg border p-4
/// pb-[18px] text-left transition-colors`; active accent tint with 1px ring
/// shadow, idle surface with `hover:border-[oklch(35%_0_0)]` (#3a3a3a per
/// TT-2, hover-gated); 30px icon chip (accent/#fcfcfc when active); name
/// 14px/600 with 17px default pill; 18px/1.5px radio ring with check;
/// 54px-min plain-English line; centered FlowGlyph over a soft top border.
pub fn css() -> String {
    format!(
        ".{PROFILE_CARD}{{position:relative;display:flex;cursor:pointer;flex-direction:column;\
gap:.75rem;border-radius:var(--radius-lg);border-width:1px;border-style:solid;\
padding:1rem;padding-bottom:18px;text-align:left;color:var(--color-text);\
transition-property:color,background-color,border-color,outline-color,\
text-decoration-color,fill,stroke;\
transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}\
.{PROFILE_CARD_ACTIVE}{{border-color:var(--color-accent-line);\
background-color:var(--color-accent-soft);\
box-shadow:0 0 0 1px var(--color-accent-line)}}\
.{PROFILE_CARD_IDLE}{{border-color:var(--color-border);\
background-color:var(--color-surface)}}\
@media (hover: hover){{.{PROFILE_CARD_IDLE}:hover{{border-color:#3a3a3a}}}}\
.{PROFILE_CARD_HEAD}{{display:flex;align-items:flex-start;justify-content:space-between;\
gap:.75rem}}\
.{PROFILE_CARD_ID}{{display:flex;align-items:center;gap:.625rem}}\
.{PROFILE_CARD_CHIP}{{display:flex;width:30px;height:30px;flex-shrink:0;\
align-items:center;justify-content:center;border-radius:var(--radius-lg)}}\
.{PROFILE_CARD_CHIP_ACTIVE}{{background-color:var(--color-accent);color:#fcfcfc}}\
.{PROFILE_CARD_CHIP_IDLE}{{border:1px solid var(--color-border);\
background-color:var(--color-surface-2);color:var(--color-text-muted)}}\
.{PROFILE_CARD_GLYPH}{{width:1rem;height:1rem}}\
.{PROFILE_CARD_NAME}{{display:flex;align-items:center;gap:.375rem;font-size:14px;\
font-weight:600;letter-spacing:-0.01em}}\
.{PROFILE_CARD_DEFAULT_PILL}{{display:inline-flex;height:17px;align-items:center;\
border-radius:calc(infinity * 1px);border:1px solid var(--color-border);\
background-color:var(--color-surface-2);padding-left:.375rem;padding-right:.375rem;\
font-size:9.5px;text-transform:uppercase;letter-spacing:0.04em;\
color:var(--color-text-muted)}}\
.{PROFILE_CARD_RADIO}{{margin-top:1px;display:flex;width:18px;height:18px;\
flex-shrink:0;align-items:center;justify-content:center;\
border-radius:calc(infinity * 1px);border-width:1.5px;border-style:solid}}\
.{PROFILE_CARD_RADIO_ACTIVE}{{border-color:var(--color-accent);\
background-color:var(--color-accent)}}\
.{PROFILE_CARD_RADIO_IDLE}{{border-color:var(--color-border)}}\
.{PROFILE_CARD_CHECK}{{width:.75rem;height:.75rem;color:#fcfcfc}}\
.{PROFILE_CARD_LINE}{{min-height:54px;font-size:12.5px;line-height:1.5;\
text-wrap:pretty;color:var(--color-text-muted)}}\
.{PROFILE_CARD_FLOW}{{display:flex;justify-content:center;\
border-color:var(--color-border-soft);border-top-width:1px;padding-top:.625rem}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            PROFILE_CARD,
            PROFILE_CARD_ACTIVE,
            PROFILE_CARD_IDLE,
            PROFILE_CARD_HEAD,
            PROFILE_CARD_ID,
            PROFILE_CARD_CHIP,
            PROFILE_CARD_CHIP_ACTIVE,
            PROFILE_CARD_CHIP_IDLE,
            PROFILE_CARD_GLYPH,
            PROFILE_CARD_NAME,
            PROFILE_CARD_DEFAULT_PILL,
            PROFILE_CARD_RADIO,
            PROFILE_CARD_RADIO_ACTIVE,
            PROFILE_CARD_RADIO_IDLE,
            PROFILE_CARD_CHECK,
            PROFILE_CARD_LINE,
            PROFILE_CARD_FLOW,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
