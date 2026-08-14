//! ProgressRail — port of `components/onboarding/progress-rail.tsx`: the
//! onboarding step rail. Grid columns follow the step count (inline style,
//! like upstream); per-state bar tints and label colors are the reference's
//! inline/conditional styles as state classes with identical computed values.

use crate::icons::{Icon, RI_CHECK_LINE};
use leptos::either::Either;
use leptos::prelude::*;

pub const PROGRESS_RAIL: &str = "asy-progress-rail";
pub const PROGRESS_RAIL_GRID: &str = "asy-progress-rail__grid";
pub const PROGRESS_RAIL_STEP: &str = "asy-progress-rail__step";
pub const PROGRESS_RAIL_BAR: &str = "asy-progress-rail__bar";
pub const PROGRESS_RAIL_BAR_DONE: &str = "asy-progress-rail__bar--done";
pub const PROGRESS_RAIL_BAR_ACTIVE: &str = "asy-progress-rail__bar--active";
pub const PROGRESS_RAIL_BAR_TODO: &str = "asy-progress-rail__bar--todo";
pub const PROGRESS_RAIL_META: &str = "asy-progress-rail__meta";
pub const PROGRESS_RAIL_CHECK: &str = "asy-progress-rail__check";
pub const PROGRESS_RAIL_NUM: &str = "asy-progress-rail__num";
pub const PROGRESS_RAIL_NUM_ACTIVE: &str = "asy-progress-rail__num--active";
pub const PROGRESS_RAIL_NUM_TODO: &str = "asy-progress-rail__num--todo";
pub const PROGRESS_RAIL_LABEL_ACTIVE: &str = "asy-progress-rail__label--active";
pub const PROGRESS_RAIL_LABEL_DONE: &str = "asy-progress-rail__label--done";
pub const PROGRESS_RAIL_LABEL_TODO: &str = "asy-progress-rail__label--todo";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProgressStepState {
    Done,
    Active,
    Todo,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProgressRailStep {
    pub label: String,
    pub state: ProgressStepState,
}

#[component]
pub fn ProgressRail(steps: Vec<ProgressRailStep>) -> impl IntoView {
    let columns = format!("repeat({}, minmax(0,1fr))", steps.len());
    view! {
        <div class=PROGRESS_RAIL>
            <div class=PROGRESS_RAIL_GRID style:grid-template-columns=columns>
                {steps
                    .into_iter()
                    .enumerate()
                    .map(|(i, step)| {
                        let bar = match step.state {
                            ProgressStepState::Done => PROGRESS_RAIL_BAR_DONE,
                            ProgressStepState::Active => PROGRESS_RAIL_BAR_ACTIVE,
                            ProgressStepState::Todo => PROGRESS_RAIL_BAR_TODO,
                        };
                        let label_class = match step.state {
                            ProgressStepState::Active => PROGRESS_RAIL_LABEL_ACTIVE,
                            ProgressStepState::Done => PROGRESS_RAIL_LABEL_DONE,
                            ProgressStepState::Todo => PROGRESS_RAIL_LABEL_TODO,
                        };
                        let glyph = match step.state {
                            ProgressStepState::Done => Either::Left(
                                view! { <Icon d=RI_CHECK_LINE class=PROGRESS_RAIL_CHECK /> },
                            ),
                            ProgressStepState::Active | ProgressStepState::Todo => {
                                let num = if step.state == ProgressStepState::Active {
                                    PROGRESS_RAIL_NUM_ACTIVE
                                } else {
                                    PROGRESS_RAIL_NUM_TODO
                                };
                                Either::Right(
                                    view! {
                                        <span class=format!(
                                            "{PROGRESS_RAIL_NUM} {num}",
                                        )>{(i + 1).to_string()}</span>
                                    },
                                )
                            }
                        };
                        view! {
                            <div class=PROGRESS_RAIL_STEP>
                                <div class=format!("{PROGRESS_RAIL_BAR} {bar}")></div>
                                <div class=PROGRESS_RAIL_META>
                                    {glyph} <span class=label_class>{step.label}</span>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

/// Rail `border-b border-border bg-surface px-8 py-4`; grid `mx-auto
/// max-w-[760px] gap-2` (columns inline); bars `h-[3px] rounded-sm` tinted
/// accent / accent→border gradient / surface-2; meta `flex items-center
/// gap-1.5 text-[11.5px]` with `size-3.5` accent check or `size-[13px]`
/// numbered ring; labels text / muted / dim by state.
pub fn css() -> String {
    format!(
        ".{PROGRESS_RAIL}{{border-color:var(--color-border);border-bottom-width:1px;\
background-color:var(--color-surface);padding-left:1rem;padding-right:1rem;\
padding-top:1rem;padding-bottom:1rem}}\
@media (width >= 40rem){{.{PROGRESS_RAIL}{{padding-left:2rem;padding-right:2rem}}}}\
.{PROGRESS_RAIL_GRID}{{margin-left:auto;margin-right:auto;display:grid;\
max-width:760px;gap:.5rem}}\
.{PROGRESS_RAIL_STEP}{{display:flex;flex-direction:column;gap:.25rem;min-width:0}}\
.{PROGRESS_RAIL_BAR}{{height:3px;border-radius:var(--radius-sm)}}\
.{PROGRESS_RAIL_BAR_DONE}{{background:var(--color-accent)}}\
.{PROGRESS_RAIL_BAR_ACTIVE}{{background:linear-gradient(90deg, var(--color-accent) 60%, var(--color-border))}}\
.{PROGRESS_RAIL_BAR_TODO}{{background:var(--color-surface-2)}}\
.{PROGRESS_RAIL_META}{{display:flex;align-items:center;gap:.375rem;font-size:11.5px;\
min-width:0}}\
.{PROGRESS_RAIL_CHECK}{{width:.875rem;height:.875rem;flex-shrink:0;color:var(--color-accent)}}\
.{PROGRESS_RAIL_NUM}{{display:inline-flex;width:13px;height:13px;flex-shrink:0;align-items:center;\
justify-content:center;border-radius:calc(infinity * 1px);border-width:1px;\
border-style:solid;font-size:9px}}\
.{PROGRESS_RAIL_NUM_ACTIVE}{{border-color:var(--color-accent);color:var(--color-accent)}}\
.{PROGRESS_RAIL_NUM_TODO}{{border-color:var(--color-text-dim);color:var(--color-text-dim)}}\
.{PROGRESS_RAIL_LABEL_ACTIVE}{{min-width:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap;color:var(--color-text)}}\
.{PROGRESS_RAIL_LABEL_DONE}{{min-width:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap;color:var(--color-text-muted)}}\
.{PROGRESS_RAIL_LABEL_TODO}{{min-width:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap;color:var(--color-text-dim)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            PROGRESS_RAIL,
            PROGRESS_RAIL_GRID,
            PROGRESS_RAIL_STEP,
            PROGRESS_RAIL_BAR,
            PROGRESS_RAIL_BAR_DONE,
            PROGRESS_RAIL_BAR_ACTIVE,
            PROGRESS_RAIL_BAR_TODO,
            PROGRESS_RAIL_META,
            PROGRESS_RAIL_CHECK,
            PROGRESS_RAIL_NUM,
            PROGRESS_RAIL_NUM_ACTIVE,
            PROGRESS_RAIL_NUM_TODO,
            PROGRESS_RAIL_LABEL_ACTIVE,
            PROGRESS_RAIL_LABEL_DONE,
            PROGRESS_RAIL_LABEL_TODO,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
