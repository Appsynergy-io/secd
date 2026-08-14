//! FamilyProfilePicker — port of `dashboard/family-profile-picker.tsx`
//! (T10). Card-style radio picker over the DNS filter-list catalog;
//! fully catalog-driven — no category names are hardcoded. The catalog
//! arrives as the `filters` prop (`useDnsFilterLists` stays with the
//! consumer); selection is controlled (`value` + `on_change`).
//!
//! The reference scopes DOM ids per mounted picker with `React.useId`;
//! here a per-instance counter provides the same collision-freedom
//! deterministically.

use crate::components::label::LABEL;
use crate::components::radio_group::{RadioGroup, RadioGroupItem};
use crate::icons::{Icon, RI_SHIELD_CHECK_LINE, RI_SHIELD_LINE};
use leptos::prelude::*;

pub const FPP_GROUP: &str = "asy-fpp";
pub const FPP_OPTION: &str = "asy-fpp__option";
pub const FPP_OPTION_ACTIVE: &str = "asy-fpp__option--active";
pub const FPP_RADIO: &str = "asy-fpp__radio";
pub const FPP_ICON: &str = "asy-fpp__icon";
pub const FPP_COL: &str = "asy-fpp__col";
pub const FPP_NAME: &str = "asy-fpp__name";
pub const FPP_DESC: &str = "asy-fpp__desc";

/// Sentinel `filter_list_id` value meaning "run unfiltered".
pub const UNFILTERED_PROFILE: &str = "";

/// One catalog filter list — the fields the picker renders.
#[derive(Clone, PartialEq, Debug)]
pub struct DnsFilterList {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

static FPP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Card-style radio picker over the filter-list SKU catalog, presented
/// as selectable content profiles ("Families").
#[component]
pub fn FamilyProfilePicker(
    filters: Vec<DnsFilterList>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_change: Callback<String>,
) -> impl IntoView {
    let scope = FPP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    view! {
        <RadioGroup
            value=value
            on_value_change=Callback::new(move |v: String| on_change.run(v))
            class=FPP_GROUP
        >
            <ProfileOption
                dom_id=format!("asy-fpp-{scope}-unfiltered")
                value=UNFILTERED_PROFILE
                active=Signal::derive(move || value.get() == UNFILTERED_PROFILE)
                name="Unfiltered"
                description="No filtering — every query is forwarded as-is."
                icon=RI_SHIELD_LINE
            />
            {filters
                .into_iter()
                .map(|f| {
                    let id = f.id.clone();
                    let active =
                        Signal::derive(move || value.get() == id);
                    view! {
                        <ProfileOption
                            dom_id=format!("asy-fpp-{scope}-{}", f.id)
                            value=f.id
                            active=active
                            name=f.name
                            description=f.description.unwrap_or(f.slug)
                            icon=RI_SHIELD_CHECK_LINE
                        />
                    }
                })
                .collect_view()}
        </RadioGroup>
    }
}

#[component]
fn ProfileOption(
    #[prop(into)] dom_id: String,
    #[prop(into)] value: String,
    #[prop(into)] active: Signal<bool>,
    #[prop(into)] name: String,
    #[prop(into)] description: String,
    icon: &'static str,
) -> impl IntoView {
    let for_id = dom_id.clone();
    view! {
        <label
            for=for_id
            class=move || {
                if active.get() {
                    format!("{LABEL} {FPP_OPTION} {FPP_OPTION_ACTIVE}")
                } else {
                    format!("{LABEL} {FPP_OPTION}")
                }
            }
        >
            <RadioGroupItem id=dom_id value=value class=FPP_RADIO />
            <Icon d=icon class=FPP_ICON />
            <span class=FPP_COL>
                <span class=FPP_NAME>{name}</span>
                <span class=FPP_DESC>{description}</span>
            </span>
        </label>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{group}{{gap:.375rem}}",
            // Label overrides: normal-case body text, card chrome, the
            // active accent state, and the hover ring (hover-gated).
            ".{option}{{display:flex;cursor:pointer;align-items:flex-start;",
            "gap:.625rem;border-radius:var(--radius-md);border:1px solid;",
            "padding:.625rem;text-align:left;font-weight:400;",
            "text-transform:none;letter-spacing:normal;",
            "transition-property:color,background-color,border-color,",
            "outline-color,text-decoration-color,fill,stroke;",
            "transition-timing-function:cubic-bezier(.4,0,.2,1);",
            "transition-duration:.15s;",
            "border-color:var(--color-border);",
            "background-color:var(--color-surface-2)}}",
            "@media (hover:hover){{.{option}:hover{{",
            "border-color:var(--color-accent-line)}}}}",
            ".{active}{{border-color:var(--color-accent);",
            "background-color:var(--color-accent-soft)}}",
            "@media (hover:hover){{.{active}:hover{{",
            "border-color:var(--color-accent)}}}}",
            ".{radio}{{margin-top:.125rem}}",
            ".{icon}{{margin-top:.125rem;width:1rem;height:1rem;",
            "flex-shrink:0;color:var(--color-text-muted)}}",
            ".{col}{{display:flex;flex-direction:column;gap:.125rem}}",
            ".{name}{{font-size:12.5px;font-weight:500;",
            "color:var(--color-text)}}",
            ".{desc}{{font-size:11px;color:var(--color-text-muted)}}",
        ),
        group = FPP_GROUP,
        option = FPP_OPTION,
        active = FPP_OPTION_ACTIVE,
        radio = FPP_RADIO,
        icon = FPP_ICON,
        col = FPP_COL,
        name = FPP_NAME,
        desc = FPP_DESC,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfiltered_sentinel_is_empty() {
        assert_eq!(UNFILTERED_PROFILE, "");
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in
            [FPP_GROUP, FPP_OPTION, FPP_OPTION_ACTIVE, FPP_RADIO, FPP_ICON, FPP_COL, FPP_NAME,
                FPP_DESC]
        {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
