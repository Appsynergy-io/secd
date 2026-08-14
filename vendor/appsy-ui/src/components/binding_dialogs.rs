//! Binding dialog suite — port of `platform/binding-dialogs.tsx` (P2).
//!
//! Props/callbacks split: the reference calls two data hooks inside the
//! component; both leave for the consumer.
//! - `usePlatformServers` → the `servers` prop ([`RelayServer`]): the query
//!   (and its loading/error lifecycle) stays in the consumer, the dialog
//!   renders whatever list it is handed — exactly the reference's
//!   `serversQuery.data?.items ?? []` behavior with the fetch removed.
//! - `useSetBindingExitServer` → `on_save(Option<String>)` (`None` clears
//!   the relay — the reference's `""` sentinel mapping to JSON `null`),
//!   with `saving`/`error` mirroring `isPending`/`isError`+message. The
//!   consumer holds the binding id and closes on success.
//!
//! `binding` carries the two fields the dialog renders from
//! (`exit_server_id` seeds the select, `external_exit_id` disables the
//! form); the consumer keeps the full API record.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use leptos::prelude::*;

pub const BINDR_FIELD: &str = "asy-bindr__field";
pub const BINDR_LABEL: &str = "asy-bindr__label";
pub const BINDR_COL: &str = "asy-bindr__col";
pub const BINDR_NOTE: &str = "asy-bindr__note";
pub const BINDR_ERROR: &str = "asy-bindr__error";

/// The fields of `PlatformBinding` the relay dialog renders from.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct RelayBinding {
    pub exit_server_id: Option<String>,
    pub external_exit_id: Option<String>,
}

/// One selectable relay target — the fields of `PlatformServerRead` the
/// option row renders (`name (region ?? location)`).
#[derive(Clone, PartialEq, Debug)]
pub struct RelayServer {
    pub id: String,
    pub name: String,
    pub region: Option<String>,
    pub location: String,
}

/// Set or clear a binding's inter-hub egress relay. A binding pinned to a
/// third-party `external_exit_id` cannot carry an internal relay, so the
/// form is disabled in that case.
#[component]
pub fn BindingExitRelayDialog(
    #[prop(into)] binding: Signal<Option<RelayBinding>>,
    servers: Vec<RelayServer>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useSetBindingExitServer.mutate` — `None` clears the
    /// relay (egress back at the ingress PoP); the consumer closes on
    /// success.
    #[prop(into)]
    on_save: Callback<Option<String>>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    saving: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    // The reference's CLEAR sentinel: "" selects "no relay".
    let choice = RwSignal::new(String::new());
    let servers = StoredValue::new(servers);

    // Seed the select from the binding on (re)open — the reference's effect.
    Effect::new(move |_| {
        if let Some(b) = binding.get() {
            choice.set(b.exit_server_id.unwrap_or_default());
        }
    });

    let externally_pinned =
        Signal::derive(move || binding.get().is_some_and(|b| b.external_exit_id.is_some()));

    let submit = move |_| {
        if binding.get_untracked().is_none() {
            return;
        }
        let c = choice.get_untracked();
        on_save.run((!c.is_empty()).then_some(c));
    };

    view! {
        <DialogControlled open=Signal::derive(move || binding.get().is_some()) on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Set exit relay"</DialogTitle>
                    <DialogDescription>
                        "Route this binding's egress through another hub, or clear it so egress stays at the ingress PoP. The server validates reachability (joined agent, cross-hub carrier)."
                    </DialogDescription>
                </DialogHeader>
                {move || {
                    if externally_pinned.get() {
                        leptos::either::Either::Left(view! {
                            <p class=BINDR_NOTE>
                                "This binding egresses via a third-party exit (external_exit_id). Clear that first to assign an internal relay."
                            </p>
                        })
                    } else {
                        let options = servers
                            .get_value()
                            .into_iter()
                            .map(|s| {
                                view! {
                                    <option value=s.id.clone()>
                                        {s.name}
                                        " ("
                                        {s.region.unwrap_or(s.location)}
                                        ")"
                                    </option>
                                }
                            })
                            .collect_view();
                        leptos::either::Either::Right(view! {
                            <div class=BINDR_COL>
                                <label class=BINDR_LABEL>
                                    "Exit relay"
                                    <select
                                        class=BINDR_FIELD
                                        prop:value=move || choice.get()
                                        on:change=move |ev| choice.set(event_target_value(&ev))
                                    >
                                        <option value="">"None — egress at ingress PoP"</option>
                                        {options}
                                    </select>
                                </label>
                                {move || {
                                    error
                                        .get()
                                        .map(|msg| {
                                            view! {
                                                <p class=BINDR_ERROR style="color: var(--color-danger)">
                                                    "Could not set relay: "
                                                    {msg}
                                                </p>
                                            }
                                        })
                                }}
                            </div>
                        })
                    }
                }}
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || saving.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || {
                            (externally_pinned.get() || saving.get()).then_some("")
                        }
                        on:click=submit
                    >
                        {move || if saving.get() { "Saving…" } else { "Save relay" }}
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
            ".{note}{{font-size:12.5px;color:var(--color-text-muted)}}",
            ".{error}{{font-size:12px}}",
        ),
        field = BINDR_FIELD,
        label = BINDR_LABEL,
        col = BINDR_COL,
        note = BINDR_NOTE,
        error = BINDR_ERROR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [BINDR_FIELD, BINDR_LABEL, BINDR_COL, BINDR_NOTE, BINDR_ERROR] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
