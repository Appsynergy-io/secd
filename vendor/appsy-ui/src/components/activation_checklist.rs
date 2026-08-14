//! ActivationChecklist + ChecklistItem — port of
//! `dashboard/activation-checklist.tsx` (T4).
//!
//! Already props/callbacks-shaped upstream: `steps` in, `on_dismiss` out,
//! `dismissing` flag; the consumer owns `useActivationStatus` /
//! `useDismissActivation` and the show/hide-on-`dismissed_at` decision.
//! `primary_cta_url` is step data supplied by the caller, per
//! navigation-is-props.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::card::Card;
use crate::icons::{Icon, RI_ARROW_RIGHT_LINE, RI_CHECK_LINE};
use leptos::prelude::*;

pub const CHECK_CARD: &str = "asy-checklist";
pub const CHECK_HEAD: &str = "asy-checklist__head";
pub const CHECK_H2: &str = "asy-checklist__h2";
pub const CHECK_SUB: &str = "asy-checklist__sub";
pub const CHECK_LIST: &str = "asy-checklist__list";
pub const CHECK_ROW: &str = "asy-checklist__row";
pub const CHECK_MARKER: &str = "asy-checklist__marker";
pub const CHECK_MARKER_DONE: &str = "asy-checklist__marker--done";
pub const CHECK_MARKER_ICON: &str = "asy-checklist__marker-icon";
pub const CHECK_LABEL: &str = "asy-checklist__label";
pub const CHECK_LABEL_DONE: &str = "asy-checklist__label--done";
pub const CHECK_CTA_ICON: &str = "asy-checklist__cta-icon";

/// One activation step — display fields plus the CTA deep-link target.
#[derive(Clone, PartialEq, Debug)]
pub struct ActivationStep {
    pub id: String,
    pub label: String,
    pub completed: bool,
    pub primary_cta_url: String,
}

/// The id of the first still-incomplete step — the reference's
/// `steps.find((s) => !s.completed)?.id`; the CTA lights on it alone.
pub fn first_todo_id(steps: &[ActivationStep]) -> Option<String> {
    steps.iter().find(|s| !s.completed).map(|s| s.id.clone())
}

/// One checklist row: completion marker, label (muted + struck through
/// once done), and — on the first still-incomplete step — a primary CTA
/// deep-linking to where the user completes it.
#[component]
pub fn ChecklistItem(
    step: ActivationStep,
    /// Render the CTA button (the parent lights it on the first todo only).
    show_cta: bool,
) -> impl IntoView {
    let marker_cls = if step.completed {
        format!("{CHECK_MARKER} {CHECK_MARKER_DONE}")
    } else {
        CHECK_MARKER.to_owned()
    };
    let label_cls = if step.completed {
        format!("{CHECK_LABEL} {CHECK_LABEL_DONE}")
    } else {
        CHECK_LABEL.to_owned()
    };
    view! {
        <li class=CHECK_ROW>
            <span class=marker_cls aria-hidden="true">
                {step
                    .completed
                    .then(|| view! { <Icon d=RI_CHECK_LINE class=CHECK_MARKER_ICON /> })}
            </span>
            <span class=label_cls>{step.label}</span>
            {(show_cta && !step.completed)
                .then(|| {
                    view! {
                        <Button
                            variant=ButtonVariant::Default
                            size=ButtonSize::Sm
                            href=step.primary_cta_url
                        >
                            "Start"
                            <Icon d=RI_ARROW_RIGHT_LINE class=CHECK_CTA_ICON />
                        </Button>
                    }
                })}
        </li>
    }
}

/// The post-signup activation checklist card. Shows the onboarding steps
/// with completion state and a CTA on the first still-incomplete step;
/// when every step is done it switches to an "all set" acknowledgement.
#[component]
pub fn ActivationChecklist(
    steps: Vec<ActivationStep>,
    /// The consumer's dismiss mutation (persists server-side).
    #[prop(into)]
    on_dismiss: Callback<()>,
    /// Disable the dismiss control while the mutation is in flight.
    #[prop(optional, into)]
    dismissing: Signal<bool>,
) -> impl IntoView {
    let done = steps.iter().filter(|s| s.completed).count();
    let total = steps.len();
    let all_done = done == total;
    let first_todo = first_todo_id(&steps);

    view! {
        <Card class=CHECK_CARD>
            <div class=CHECK_HEAD>
                <div>
                    <h2 class=CHECK_H2>
                        {if all_done { "You're all set" } else { "Finish setting up AppSynergy" }}
                    </h2>
                    <p class=CHECK_SUB>
                        {if all_done {
                            "Every onboarding step is complete.".to_owned()
                        } else {
                            format!("{done} of {total} steps complete")
                        }}
                    </p>
                </div>
                <Button
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Sm
                    attr:r#type="button"
                    attr:disabled=move || dismissing.get().then_some("")
                    on:click=move |_| on_dismiss.run(())
                >
                    "Dismiss"
                </Button>
            </div>
            <ul class=CHECK_LIST>
                {steps
                    .into_iter()
                    .map(|step| {
                        let show_cta = first_todo.as_deref() == Some(step.id.as_str());
                        view! { <ChecklistItem step=step show_cta=show_cta /> }
                    })
                    .collect_view()}
            </ul>
        </Card>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{card}{{margin-bottom:1rem;padding:1rem}}",
            ".{head}{{display:flex;align-items:center;",
            "justify-content:space-between;gap:.75rem}}",
            ".{h2}{{font-size:14px;font-weight:500}}",
            ".{sub}{{font-size:12.5px;color:var(--color-text-muted)}}",
            // divide-y divide-[var(--color-border-soft)] (Tailwind v4
            // between-row borders; color applies to all sides).
            ".{list}{{margin-top:.25rem}}",
            ".{list}>:not(:last-child){{border-color:var(--color-border-soft);",
            "border-top-style:solid;border-bottom-style:solid;",
            "border-top-width:0;border-bottom-width:1px}}",
            ".{row}{{display:flex;align-items:center;gap:.75rem;",
            "padding-block:.5rem}}",
            ".{marker}{{display:flex;width:1.25rem;height:1.25rem;",
            "flex-shrink:0;align-items:center;justify-content:center;",
            "border-radius:calc(infinity * 1px);",
            "border:1px solid var(--color-border);font-size:11px;",
            "background:transparent;color:var(--color-text-muted)}}",
            ".{marker_done}{{border-color:var(--color-success);",
            "background:var(--color-success);color:white}}",
            ".{marker_icon}{{width:.875rem;height:.875rem}}",
            ".{label}{{flex:1 1 0%;min-width:0;font-size:13px}}",
            ".{label_done}{{color:var(--color-text-muted);",
            "text-decoration-line:line-through}}",
            ".{cta_icon}{{width:.875rem;height:.875rem}}",
        ),
        card = CHECK_CARD,
        head = CHECK_HEAD,
        h2 = CHECK_H2,
        sub = CHECK_SUB,
        list = CHECK_LIST,
        row = CHECK_ROW,
        marker = CHECK_MARKER,
        marker_done = CHECK_MARKER_DONE,
        marker_icon = CHECK_MARKER_ICON,
        label = CHECK_LABEL,
        label_done = CHECK_LABEL_DONE,
        cta_icon = CHECK_CTA_ICON,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, completed: bool) -> ActivationStep {
        ActivationStep {
            id: id.into(),
            label: format!("Step {id}"),
            completed,
            primary_cta_url: format!("/go/{id}"),
        }
    }

    /// The CTA lights on the first incomplete step only — one clear next
    /// action, regardless of later incomplete steps.
    #[test]
    fn first_todo_is_first_incomplete_step() {
        let steps = [step("a", true), step("b", false), step("c", false)];
        assert_eq!(first_todo_id(&steps), Some("b".to_owned()));
    }

    #[test]
    fn first_todo_none_when_all_done() {
        let steps = [step("a", true), step("b", true)];
        assert_eq!(first_todo_id(&steps), None);
    }

    #[test]
    fn first_todo_first_when_none_done() {
        let steps = [step("a", false), step("b", false)];
        assert_eq!(first_todo_id(&steps), Some("a".to_owned()));
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            CHECK_CARD, CHECK_HEAD, CHECK_H2, CHECK_SUB, CHECK_LIST, CHECK_ROW, CHECK_MARKER,
            CHECK_MARKER_DONE, CHECK_MARKER_ICON, CHECK_LABEL, CHECK_LABEL_DONE, CHECK_CTA_ICON,
        ] {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
