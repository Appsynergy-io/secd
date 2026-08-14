//! Rollout dialog suite — port of `platform/fleet-rollout-dialogs.tsx` (P5).
//!
//! Props/callbacks split: both dialogs call mutation hooks; both leave for
//! the consumer.
//! - `RolloutCreateDialog`: `useCreateImageRollout` →
//!   `on_create(RolloutCreate)` with `creating`/`error`.
//! - `RolloutAdvanceDialog`: `usePatchImageRollout` → `on_advance(f64)`
//!   (the new wave percent; the consumer holds the rollout id) with
//!   `advancing`/`error`. `open` and `rollout` are separate props exactly
//!   like the reference — the dialog can be open with no rollout selected.
//!
//! The wave floor is advisory client-side (the server 409s an actual
//! decrease); the reference's `num()` and validity arithmetic are mirrored
//! op-for-op.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use leptos::prelude::*;

pub const ROLL_FIELD: &str = "asy-roll__field";
pub const ROLL_LABEL: &str = "asy-roll__label";
pub const ROLL_COL: &str = "asy-roll__col";
pub const ROLL_ERROR: &str = "asy-roll__error";

/// The fields of `ImageRollout` the advance dialog renders; id and status
/// stay with the consumer.
#[derive(Clone, PartialEq, Debug)]
pub struct ImageRollout {
    pub channel: String,
    pub target_version: String,
    pub wave_percent: f64,
}

/// What `RolloutCreateDialog` emits — the reference's POST body.
#[derive(Clone, PartialEq, Debug)]
pub struct RolloutCreate {
    pub channel: String,
    pub target_version: String,
    pub wave_percent: f64,
}

/// The reference's `num()`: blank → `None`, `Number(t)`, non-finite → `None`.
fn num(v: &str) -> Option<f64> {
    let t = v.trim();
    if t.is_empty() {
        return None;
    }
    let n = t.parse::<f64>().unwrap_or(f64::NAN);
    n.is_finite().then_some(n)
}

/// JS `String(number)` (shortest-roundtrip; integers bare).
fn fmt_num(v: f64) -> String {
    format!("{v}")
}

/// Open a new staged OS-image rollout: `(channel, target_version)` plus an
/// initial wave size.
#[component]
pub fn RolloutCreateDialog(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useCreateImageRollout.mutate`; the consumer closes
    /// on success.
    #[prop(into)]
    on_create: Callback<RolloutCreate>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    creating: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let channel = RwSignal::new(String::new());
    let target_version = RwSignal::new(String::new());
    let wave_percent = RwSignal::new("10".to_owned());

    // Reset to the empty (default 10%) state on (re)open.
    Effect::new(move |_| {
        if open.get() {
            channel.set(String::new());
            target_version.set(String::new());
            wave_percent.set("10".to_owned());
        }
    });

    let valid = Signal::derive(move || {
        let wave = num(&wave_percent.get());
        !channel.with(|v| v.trim().is_empty())
            && !target_version.with(|v| v.trim().is_empty())
            && wave.is_some_and(|w| (0.0..=100.0).contains(&w))
    });

    let submit = move |_| {
        let Some(wave) = num(&wave_percent.get_untracked()) else { return };
        if !valid.get_untracked() {
            return;
        }
        on_create.run(RolloutCreate {
            channel: channel.get_untracked().trim().to_owned(),
            target_version: target_version.get_untracked().trim().to_owned(),
            wave_percent: wave,
        });
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"New OS-image rollout"</DialogTitle>
                    <DialogDescription>
                        "Stage a channel + target-version wave. The `(channel, target_version)` pair must already be catalogued in `release_images`. Start small and advance the wave once adoption proves out."
                    </DialogDescription>
                </DialogHeader>
                <div class=ROLL_COL>
                    <label class=ROLL_LABEL>
                        "Channel"
                        <input
                            class=ROLL_FIELD
                            value=move || channel.get()
                            prop:value=move || channel.get()
                            on:input=move |ev| channel.set(event_target_value(&ev))
                            placeholder="e.g. stable"
                        />
                    </label>
                    <label class=ROLL_LABEL>
                        "Target version"
                        <input
                            class=format!("{ROLL_FIELD} mono")
                            value=move || target_version.get()
                            prop:value=move || target_version.get()
                            on:input=move |ev| target_version.set(event_target_value(&ev))
                            placeholder="e.g. 0.5.0"
                        />
                    </label>
                    <label class=ROLL_LABEL>
                        "Initial wave (%)"
                        <input
                            class=ROLL_FIELD
                            inputmode="numeric"
                            value=move || wave_percent.get()
                            prop:value=move || wave_percent.get()
                            on:input=move |ev| wave_percent.set(event_target_value(&ev))
                            placeholder="0–100"
                        />
                    </label>
                    {move || {
                        error
                            .get()
                            .map(|msg| {
                                view! {
                                    <p class=ROLL_ERROR style="color: var(--color-danger)">
                                        "Could not open rollout: "
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
                        attr:disabled=move || creating.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || (!valid.get() || creating.get()).then_some("")
                        on:click=submit
                    >
                        {move || if creating.get() { "Opening…" } else { "Open rollout" }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Advance a rollout's wave size (never decreases; the server enforces).
#[component]
pub fn RolloutAdvanceDialog(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    #[prop(into)] rollout: Signal<Option<ImageRollout>>,
    /// The consumer's `usePatchImageRollout.mutate` with the new wave
    /// percent (it holds the rollout id); the consumer closes on success.
    #[prop(into)]
    on_advance: Callback<f64>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    advancing: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let wave_percent = RwSignal::new(String::new());

    // Seed from the rollout's current wave on (re)open — the reference's
    // `[open, rollout]` effect.
    Effect::new(move |_| {
        if open.get() {
            if let Some(r) = rollout.get() {
                wave_percent.set(fmt_num(r.wave_percent));
            }
        }
    });

    let floor = Signal::derive(move || rollout.get().map(|r| r.wave_percent).unwrap_or(0.0));
    let valid = Signal::derive(move || {
        rollout.get().is_some()
            && num(&wave_percent.get()).is_some_and(|w| w >= floor.get() && w <= 100.0)
    });

    let submit = move |_| {
        if rollout.get_untracked().is_none() {
            return;
        }
        let Some(wave) = num(&wave_percent.get_untracked()) else { return };
        on_advance.run(wave);
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Advance wave"</DialogTitle>
                    <DialogDescription>
                        {move || match rollout.get() {
                            Some(r) => format!(
                                "Raise the wave for {} @ {} — currently {}% of the un-pinned cohort.",
                                r.channel,
                                r.target_version,
                                fmt_num(r.wave_percent),
                            ),
                            None => "Select a rollout to advance.".to_owned(),
                        }}
                    </DialogDescription>
                </DialogHeader>
                <label class=ROLL_LABEL>
                    "New wave (%)"
                    <input
                        class=ROLL_FIELD
                        inputmode="numeric"
                        value=move || wave_percent.get()
                        prop:value=move || wave_percent.get()
                        on:input=move |ev| wave_percent.set(event_target_value(&ev))
                        placeholder=move || format!("{}\u{2013}100", fmt_num(floor.get()))
                    />
                </label>
                {move || {
                    error
                        .get()
                        .map(|msg| {
                            view! {
                                <p class=ROLL_ERROR style="color: var(--color-danger)">
                                    "Could not advance: "
                                    {msg}
                                </p>
                            }
                        })
                }}
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || advancing.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || (!valid.get() || advancing.get()).then_some("")
                        on:click=submit
                    >
                        {move || if advancing.get() { "Advancing…" } else { "Advance" }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{field}{{height:2rem;width:100%;border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);padding-inline:.5rem;",
            "font-size:12.5px;color:var(--color-text)}}",
            ".{label}{{display:flex;flex-direction:column;gap:.25rem;",
            "font-size:11.5px;font-weight:500;color:var(--color-text-muted)}}",
            ".{col}{{display:flex;flex-direction:column;gap:.75rem}}",
            ".{error}{{font-size:12px}}",
        ),
        field = ROLL_FIELD,
        label = ROLL_LABEL,
        col = ROLL_COL,
        error = ROLL_ERROR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [ROLL_FIELD, ROLL_LABEL, ROLL_COL, ROLL_ERROR] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn wave_validity_mirrors_reference() {
        assert_eq!(num("10"), Some(10.0));
        assert_eq!(num(""), None);
        assert_eq!(num("abc"), None);
        // Create bounds: 0..=100.
        assert!((0.0..=100.0).contains(&0.0));
        assert!((0.0..=100.0).contains(&100.0));
        assert!(!(0.0..=100.0).contains(&100.5));
        assert_eq!(fmt_num(65.0), "65");
        assert_eq!(fmt_num(12.5), "12.5");
    }
}
