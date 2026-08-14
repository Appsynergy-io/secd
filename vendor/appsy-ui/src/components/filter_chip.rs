//! FilterChip — port of `components/dashboard/filter-chip.tsx`: pill filter
//! affordance above tabular views. This lands the plain-button mode (all
//! three reference stories); the dropdown mode (`options`/`selected`/
//! `on_select`) rides on DropdownMenu and extends this component in that
//! overlay's unit — an entangled reference file is never ported atomically.

use crate::icons::{Icon, RI_ARROW_DOWN_S_LINE};
use leptos::prelude::*;

pub const FILTER_CHIP: &str = "asy-filter-chip";
pub const FILTER_CHIP_LABEL: &str = "asy-filter-chip__label";
pub const FILTER_CHIP_VALUE: &str = "asy-filter-chip__value";
pub const FILTER_CHIP_ARROW: &str = "asy-filter-chip__arrow";

#[component]
pub fn FilterChip(
    #[prop(into)] label: String,
    /// The selected option's display text (e.g. `"all"`).
    #[prop(into)] value: String,
    /// Plain-button mode: a bare click handler, no menu.
    #[prop(optional)] on_click: Option<Callback<()>>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class=FILTER_CHIP
            on:click=move |_| {
                if let Some(cb) = on_click {
                    cb.run(());
                }
            }
        >
            // Two text nodes ({label} then ":"), exactly as the JSX emits —
            // AX text-run granularity is part of the comparison.
            <span class=FILTER_CHIP_LABEL>{label}":"</span>
            <span class=FILTER_CHIP_VALUE>{value}</span>
            <Icon d=RI_ARROW_DOWN_S_LINE class=FILTER_CHIP_ARROW />
        </button>
    }
}

/// `inline-flex h-[30px] items-center gap-1.5 rounded-sm border
/// border-border bg-surface px-2.5 text-[12.5px]`; muted label, medium
/// value, `size-3.5 text-dim` arrow.
pub fn css() -> String {
    format!(
        ".{FILTER_CHIP}{{display:inline-flex;min-height:2.75rem;align-items:center;gap:.375rem;\
border-radius:var(--radius-sm);border:1px solid var(--color-border);\
background-color:var(--color-surface);padding-left:.625rem;padding-right:.625rem;\
font-size:12.5px}}\
.{FILTER_CHIP_LABEL}{{color:var(--color-text-muted)}}\
.{FILTER_CHIP_VALUE}{{font-weight:500}}\
.{FILTER_CHIP_ARROW}{{width:.875rem;height:.875rem;color:var(--color-text-dim)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [FILTER_CHIP, FILTER_CHIP_LABEL, FILTER_CHIP_VALUE, FILTER_CHIP_ARROW] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
