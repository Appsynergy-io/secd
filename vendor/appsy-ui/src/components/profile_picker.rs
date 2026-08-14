//! ProfilePicker — port of `dashboard/netpolicy/profile-picker.tsx`: the
//! network-policy hero (short header, the four profile cards, and the
//! "Customize rules" affordance). Controlled: `active_key`/`advanced` in,
//! `on_select`/`on_toggle_advanced` out — the rule-builder itself lives in
//! the consumer.
//!
//! The four-profile catalog mirrors the reference's `NP_PROFILES`
//! verbatim: a fixed set tied to the backend's `net_policy` Profile enum,
//! customer-facing copy only — content the reference hardcodes, so the
//! crate hardcodes it (the navigation-is-props exception does not apply).

use crate::components::flow_glyph::{NpInbound, NpOutbound};
use crate::components::profile_card::{NpProfileDef, ProfileCard};
use crate::icons::{Icon, RI_ARROW_DOWN_S_LINE, RI_ARROW_RIGHT_S_LINE, RI_EQUALIZER_LINE};
use leptos::prelude::*;

pub const PICKER_HEAD: &str = "asy-profile-picker__head";
pub const PICKER_TITLE: &str = "asy-profile-picker__title";
pub const PICKER_SUB: &str = "asy-profile-picker__sub";
pub const PICKER_GRID: &str = "asy-profile-picker__grid";
pub const PICKER_TOGGLE: &str = "asy-profile-picker__toggle";
pub const PICKER_TOGGLE_ON: &str = "asy-profile-picker__toggle--on";
pub const PICKER_TOGGLE_OFF: &str = "asy-profile-picker__toggle--off";
pub const PICKER_TOGGLE_ICON: &str = "asy-profile-picker__toggle-icon";
pub const PICKER_TOGGLE_ICON_ON: &str = "asy-profile-picker__toggle-icon--on";
pub const PICKER_TOGGLE_ICON_OFF: &str = "asy-profile-picker__toggle-icon--off";
pub const PICKER_TOGGLE_COL: &str = "asy-profile-picker__toggle-col";
pub const PICKER_TOGGLE_TITLE: &str = "asy-profile-picker__toggle-title";
pub const PICKER_TOGGLE_SUB: &str = "asy-profile-picker__toggle-sub";
pub const PICKER_TOGGLE_ARROW: &str = "asy-profile-picker__toggle-arrow";

/// The reference's `NP_PROFILES` catalog, verbatim.
pub fn np_profiles() -> Vec<NpProfileDef> {
    vec![
        NpProfileDef {
            key: "private".to_owned(),
            name: "Private".to_owned(),
            icon: crate::icons::RI_LOCK_2_LINE,
            line: "Your devices reach each other and browse freely. The internet can't start a connection in.".to_owned(),
            inbound: NpInbound::Block,
            outbound: NpOutbound::Open,
            is_default: true,
        },
        NpProfileDef {
            key: "gateway".to_owned(),
            name: "Gateway".to_owned(),
            icon: crate::icons::RI_DOOR_OPEN_LINE,
            line: "Open specific ports to specific devices \u{2014} everything else stays closed.".to_owned(),
            inbound: NpInbound::Ports,
            outbound: NpOutbound::Open,
            is_default: false,
        },
        NpProfileDef {
            key: "forward".to_owned(),
            name: "Forward-all".to_owned(),
            icon: crate::icons::RI_INBOX_ARCHIVE_LINE,
            line: "Send everything to one device, except the few things you choose to block.".to_owned(),
            inbound: NpInbound::All,
            outbound: NpOutbound::Open,
            is_default: false,
        },
        NpProfileDef {
            key: "passthrough".to_owned(),
            name: "Passthrough".to_owned(),
            icon: crate::icons::RI_SHIELD_KEYHOLE_LINE,
            line: "Your device owns the public IP. Every port and protocol passes through \u{2014} you run your own firewall.".to_owned(),
            inbound: NpInbound::All,
            outbound: NpOutbound::All,
            is_default: false,
        },
    ]
}

#[component]
pub fn ProfilePicker(
    /// The selected profile key (`"private" | "gateway" | "forward" |
    /// "passthrough"`).
    #[prop(into)]
    active_key: Signal<String>,
    /// Whether the advanced rule builder is open (the Customize toggle's
    /// expanded state).
    #[prop(into)]
    advanced: Signal<bool>,
    on_select: Callback<String>,
    on_toggle_advanced: Callback<()>,
) -> impl IntoView {
    view! {
        <div class=PICKER_HEAD>
            <span class=PICKER_TITLE>"Choose how traffic reaches your network"</span>
            <span class=PICKER_SUB>
                "Pick one. It takes effect immediately \u{2014} you can change it any time."
            </span>
        </div>

        <div class=PICKER_GRID>
            {np_profiles()
                .into_iter()
                .map(|p| {
                    let key = p.key.clone();
                    let select_key = p.key.clone();
                    view! {
                        <ProfileCard
                            profile=p
                            active=Signal::derive(move || active_key.get() == key)
                            on_select=Callback::new(move |_| on_select.run(select_key.clone()))
                        />
                    }
                })
                .collect_view()}
        </div>

        <button
            type="button"
            aria-expanded=move || if advanced.get() { "true" } else { "false" }
            class=move || {
                let state = if advanced.get() { PICKER_TOGGLE_ON } else { PICKER_TOGGLE_OFF };
                format!("{PICKER_TOGGLE} {state}")
            }
            on:click=move |_| on_toggle_advanced.run(())
        >
            // Inline svg in `Icon`'s exact element shape — the class here
            // is reactive (accent while advanced), which the static Icon
            // prop can't express.
            <svg
                viewBox="0 0 24 24"
                xmlns="http://www.w3.org/2000/svg"
                width="24"
                height="24"
                fill="currentColor"
                class=move || {
                    let state = if advanced.get() {
                        PICKER_TOGGLE_ICON_ON
                    } else {
                        PICKER_TOGGLE_ICON_OFF
                    };
                    format!("{PICKER_TOGGLE_ICON} {state}")
                }
            >
                <path d=RI_EQUALIZER_LINE></path>
            </svg>
            <div class=PICKER_TOGGLE_COL>
                <span class=PICKER_TOGGLE_TITLE>"Customize rules"</span>
                <span class=PICKER_TOGGLE_SUB>
                    "Build your own allow / block list with friendly pickers. For when you know exactly what you want."
                </span>
            </div>
            <Show
                when=move || advanced.get()
                fallback=|| view! { <Icon d=RI_ARROW_RIGHT_S_LINE class=PICKER_TOGGLE_ARROW /> }
            >
                <Icon d=RI_ARROW_DOWN_S_LINE class=PICKER_TOGGLE_ARROW />
            </Show>
        </button>
    }
}

/// Header `mb-3 flex flex-col gap-1` with 13px/500 muted title and 12px
/// muted sub; grid `mb-3 grid grid-cols-4 gap-3` collapsing to 2 columns
/// at 1100px and 1 at 640px; toggle button per its utility string with the
/// accent-soft advanced state; icon/arrow `size-4`.
pub fn css() -> String {
    format!(
        ".{PICKER_HEAD}{{margin-bottom:.75rem;display:flex;flex-direction:column;\
gap:.25rem}}\
.{PICKER_TITLE}{{font-size:13px;font-weight:500;color:var(--color-text-muted)}}\
.{PICKER_SUB}{{font-size:12px;color:var(--color-text-muted)}}\
.{PICKER_GRID}{{margin-bottom:.75rem;display:grid;\
grid-template-columns:repeat(4,minmax(0,1fr));gap:.75rem}}\
@media (width <= 1100px){{.{PICKER_GRID}{{\
grid-template-columns:repeat(2,minmax(0,1fr))}}}}\
@media (width <= 640px){{.{PICKER_GRID}{{\
grid-template-columns:repeat(1,minmax(0,1fr))}}}}\
.{PICKER_TOGGLE}{{margin-bottom:.875rem;display:flex;width:100%;\
cursor:pointer;align-items:center;gap:.625rem;\
border-radius:var(--radius-md);border-width:1px;border-style:solid;\
padding:.75rem .875rem;text-align:left;color:var(--color-text)}}\
.{PICKER_TOGGLE_ON}{{border-color:var(--color-accent-line);\
background-color:var(--color-accent-soft)}}\
.{PICKER_TOGGLE_OFF}{{border-color:var(--color-border);\
background-color:var(--color-surface)}}\
.{PICKER_TOGGLE_ICON}{{width:1rem;height:1rem}}\
.{PICKER_TOGGLE_ICON_ON}{{color:var(--color-accent)}}\
.{PICKER_TOGGLE_ICON_OFF}{{color:var(--color-text-muted)}}\
.{PICKER_TOGGLE_COL}{{display:flex;flex:1;flex-direction:column;gap:1px}}\
.{PICKER_TOGGLE_TITLE}{{font-size:13px;font-weight:500}}\
.{PICKER_TOGGLE_SUB}{{font-size:11.5px;color:var(--color-text-muted)}}\
.{PICKER_TOGGLE_ARROW}{{width:1rem;height:1rem;color:var(--color-text-dim)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            PICKER_HEAD,
            PICKER_TITLE,
            PICKER_SUB,
            PICKER_GRID,
            PICKER_TOGGLE,
            PICKER_TOGGLE_ON,
            PICKER_TOGGLE_OFF,
            PICKER_TOGGLE_ICON,
            PICKER_TOGGLE_ICON_ON,
            PICKER_TOGGLE_ICON_OFF,
            PICKER_TOGGLE_COL,
            PICKER_TOGGLE_TITLE,
            PICKER_TOGGLE_SUB,
            PICKER_TOGGLE_ARROW,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn catalog_mirrors_the_reference_set() {
        let profiles = np_profiles();
        assert_eq!(
            profiles.iter().map(|p| p.key.as_str()).collect::<Vec<_>>(),
            ["private", "gateway", "forward", "passthrough"]
        );
        assert!(profiles[0].is_default && profiles.iter().filter(|p| p.is_default).count() == 1);
    }
}
