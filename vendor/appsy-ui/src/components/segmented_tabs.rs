//! SegmentedTabs — port of `components/dashboard/segmented-tabs.tsx`:
//! inline segmented control used by the ACL rule builder. Controlled like
//! the reference: `active` selects, `on_change` reports clicks; plain
//! buttons, no roving focus (none upstream).

use leptos::prelude::*;

pub const SEG_TABS: &str = "asy-seg-tabs";
pub const SEG_TABS_BTN: &str = "asy-seg-tabs__btn";
pub const SEG_TABS_BTN_ACTIVE: &str = "asy-seg-tabs__btn--active";
pub const SEG_TABS_BTN_IDLE: &str = "asy-seg-tabs__btn--idle";

#[component]
pub fn SegmentedTabs(
    #[prop(into)] items: Vec<String>,
    #[prop(into)] active: Signal<String>,
    #[prop(optional)] on_change: Option<Callback<String>>,
) -> impl IntoView {
    view! {
        <div class=SEG_TABS>
            {items
                .into_iter()
                .map(|item| {
                    let label = item.clone();
                    let click_value = item.clone();
                    view! {
                        <button
                            type="button"
                            class=move || {
                                let state = if active.get() == item {
                                    SEG_TABS_BTN_ACTIVE
                                } else {
                                    SEG_TABS_BTN_IDLE
                                };
                                format!("{SEG_TABS_BTN} {state}")
                            }
                            on:click=move |_| {
                                if let Some(cb) = on_change {
                                    cb.run(click_value.clone());
                                }
                            }
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// Shell `inline-flex rounded-sm border border-border bg-surface-2 p-0.5`;
/// buttons `rounded px-2.5 py-1 text-[12px] font-medium`; active adds
/// `border-border bg-surface text-text`, idle `border-transparent
/// text-muted hover:text-text` (hover gated like Tailwind's variant).
pub fn css() -> String {
    format!(
        ".{SEG_TABS}{{display:inline-flex;border-radius:var(--radius-sm);\
border:1px solid var(--color-border);background-color:var(--color-surface-2);\
padding:.125rem}}\
.{SEG_TABS_BTN}{{border-radius:.25rem;padding-left:.625rem;padding-right:.625rem;\
padding-top:.25rem;padding-bottom:.25rem;font-size:12px;font-weight:500}}\
.{SEG_TABS_BTN_ACTIVE}{{border:1px solid var(--color-border);\
background-color:var(--color-surface);color:var(--color-text)}}\
.{SEG_TABS_BTN_IDLE}{{border:1px solid transparent;color:var(--color-text-muted)}}\
@media (hover: hover){{.{SEG_TABS_BTN_IDLE}:hover{{color:var(--color-text)}}}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [SEG_TABS, SEG_TABS_BTN, SEG_TABS_BTN_ACTIVE, SEG_TABS_BTN_IDLE] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
    }

    #[test]
    fn idle_hover_is_gated_like_tailwind() {
        assert!(css().contains(&format!(
            "@media (hover: hover){{.{SEG_TABS_BTN_IDLE}:hover{{color:var(--color-text)}}}}"
        )));
    }
}
