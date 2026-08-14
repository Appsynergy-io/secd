//! GrantSubscriptionDialog — port of `platform/subscription-dialogs.tsx`
//! (T11). Manual comp-subscription grant: operator supplies the org
//! UUID, picks a plan + billing cycle, optionally overrides the period
//! length.
//!
//! Props/callbacks split: `useGrantSubscription` →
//! `on_grant(SubscriptionGrant)` + `granting`/`error` (mutation reset on
//! open and close-on-success stay with the consumer);
//! `usePlatformPlans` → the `plans` prop. The `[open]` reset effect is
//! ported; the effective plan falls back to the first catalog entry
//! while none is picked, exactly like the reference.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use crate::components::settings_forms::num_or_null;
use leptos::prelude::*;

pub const SUBG_COL: &str = "asy-subg__col";
pub const SUBG_GRID2: &str = "asy-subg__grid2";
pub const SUBG_LABEL: &str = "asy-subg__label";
pub const SUBG_FIELD: &str = "asy-subg__field";
pub const SUBG_ERR: &str = "asy-subg__err";

/// Billing-cycle options in reference render order.
pub const BILLING_CYCLES: [(&str, &str); 3] =
    [("monthly", "Monthly"), ("biweekly", "Biweekly"), ("annual", "Annual")];

/// A grantable plan — the fields the picker renders.
#[derive(Clone, PartialEq, Debug)]
pub struct PlatformPlan {
    pub id: String,
    pub name: String,
    pub slug: String,
}

/// `POST /platform/subscriptions` payload.
#[derive(Clone, PartialEq, Debug)]
pub struct SubscriptionGrant {
    pub org_id: String,
    pub plan_id: String,
    pub billing_cycle: String,
    /// Parsed from the days field; `None` when blank/invalid (server
    /// default 365).
    pub period_days: Option<f64>,
}

/// Grant a comp / manual subscription to an org.
#[component]
pub fn GrantSubscriptionDialog(
    plans: Vec<PlatformPlan>,
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    #[prop(into)] on_grant: Callback<SubscriptionGrant>,
    /// The grant mutation in flight.
    #[prop(optional, into)]
    granting: Signal<bool>,
    #[prop(optional, into)] error: Signal<Option<String>>,
) -> impl IntoView {
    let plans = StoredValue::new(plans);
    let org_id = RwSignal::new(String::new());
    let plan_id = RwSignal::new(String::new());
    let cycle = RwSignal::new("annual".to_owned());
    let period_days = RwSignal::new("365".to_owned());

    // The reference's `[open]` reset effect (the consumer resets its
    // mutation alongside).
    Effect::new(move |_| {
        if open.get() {
            org_id.set(String::new());
            plan_id.set(String::new());
            cycle.set("annual".to_owned());
            period_days.set("365".to_owned());
        }
    });

    let effective_plan = move || {
        let picked = plan_id.get();
        if picked.is_empty() {
            plans.with_value(|ps| ps.first().map(|p| p.id.clone()).unwrap_or_default())
        } else {
            picked
        }
    };
    let valid = move || !org_id.get().trim().is_empty() && !effective_plan().is_empty();
    let submit = move |_| {
        if !valid() {
            return;
        }
        on_grant.run(SubscriptionGrant {
            org_id: org_id.get_untracked().trim().to_owned(),
            plan_id: effective_plan(),
            billing_cycle: cycle.get_untracked(),
            period_days: num_or_null(&period_days.get_untracked()),
        });
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Manual grant"</DialogTitle>
                    <DialogDescription>
                        "Create a comp subscription for an org without a payment processor. The org must not already have an active subscription."
                    </DialogDescription>
                </DialogHeader>
                <div class=SUBG_COL>
                    <label class=SUBG_LABEL>
                        "Org ID"
                        <input
                            class=format!("{SUBG_FIELD} mono")
                            placeholder="org UUID"
                            value=move || org_id.get()
                            prop:value=move || org_id.get()
                            on:input=move |ev| org_id.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=SUBG_LABEL>
                        "Plan"
                        <select
                            class=SUBG_FIELD
                            prop:value=effective_plan
                            on:change=move |ev| plan_id.set(event_target_value(&ev))
                        >
                            {plans
                                .get_value()
                                .into_iter()
                                .map(|p| {
                                    view! {
                                        <option value=p.id.clone()>
                                            {p.name} " (" {p.slug} ")"
                                        </option>
                                    }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    <div class=SUBG_GRID2>
                        <label class=SUBG_LABEL>
                            "Billing cycle"
                            <select
                                class=SUBG_FIELD
                                prop:value=move || cycle.get()
                                on:change=move |ev| cycle.set(event_target_value(&ev))
                            >
                                {BILLING_CYCLES
                                    .iter()
                                    .map(|(v, l)| {
                                        view! { <option value=*v>{*l}</option> }
                                    })
                                    .collect_view()}
                            </select>
                        </label>
                        <label class=SUBG_LABEL>
                            "Period (days)"
                            <input
                                class=SUBG_FIELD
                                inputmode="numeric"
                                value=move || period_days.get()
                                prop:value=move || period_days.get()
                                on:input=move |ev| period_days.set(event_target_value(&ev))
                            />
                        </label>
                    </div>
                    {move || {
                        error
                            .get()
                            .map(|msg| {
                                view! {
                                    <p class=SUBG_ERR style="color: var(--color-danger)">
                                        "Could not grant: "
                                        {msg}
                                    </p>
                                }
                            })
                    }}
                </div>
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || granting.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || (!valid() || granting.get()).then_some("")
                        on:click=submit
                    >
                        {move || if granting.get() { "Granting…" } else { "Grant subscription" }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{col}{{display:flex;flex-direction:column;gap:.75rem}}",
            ".{grid2}{{display:grid;grid-template-columns:1fr;gap:.75rem}}",
            "@media (width >= 40rem){{.{grid2}{{",
            "grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            ".{label}{{display:flex;flex-direction:column;gap:.25rem;",
            "font-size:11.5px;font-weight:500;color:var(--color-text-muted)}}",
            ".{field}{{height:2rem;width:100%;border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);padding-inline:.5rem;",
            "font-size:12.5px;color:var(--color-text)}}",
            ".{err}{{font-size:12px}}",
        ),
        col = SUBG_COL,
        grid2 = SUBG_GRID2,
        label = SUBG_LABEL,
        field = SUBG_FIELD,
        err = SUBG_ERR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billing_cycles_reference_order() {
        assert_eq!(
            BILLING_CYCLES.map(|(v, _)| v),
            ["monthly", "biweekly", "annual"]
        );
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [SUBG_COL, SUBG_GRID2, SUBG_LABEL, SUBG_FIELD, SUBG_ERR] {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
