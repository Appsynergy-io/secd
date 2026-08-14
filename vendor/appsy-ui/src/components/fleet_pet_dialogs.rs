//! Pet dialog suite — port of `platform/fleet-pet-dialogs.tsx` (P4).
//!
//! Props/callbacks split: both dialogs call mutation hooks; both leave for
//! the consumer.
//! - `PetCreateDialog`: `useCreateAgentPet` → `on_create(PetCreate)` with
//!   `creating`/`error`. The reference's `agentId` prop feeds only the
//!   mutation, so it stays with the consumer entirely — the dialog renders
//!   nothing from it.
//! - `PetRemoveDialog`: `useDeleteAgentPet` → `on_remove(())`; `agentId`
//!   and the pet id likewise stay outside. `pet` carries the one field the
//!   dialog renders (`name`).

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use leptos::prelude::*;

pub const PET_FIELD: &str = "asy-pet__field";
pub const PET_LABEL: &str = "asy-pet__label";
pub const PET_COL: &str = "asy-pet__col";
pub const PET_ERROR: &str = "asy-pet__error";

/// The field of `AgentPet` the remove dialog renders; ids and state stay
/// with the consumer.
#[derive(Clone, PartialEq, Debug)]
pub struct AgentPet {
    pub name: String,
}

/// What `PetCreateDialog` emits — the reference's create body
/// (`AgentPetCreateRequest`).
#[derive(Clone, PartialEq, Debug)]
pub struct PetCreate {
    pub name: String,
    pub image: String,
    pub desired_state: String,
}

/// Declare a new pet on a fleet node; the agent provisions it from the
/// named seed image and converges it on its next reconcile pass.
#[component]
pub fn PetCreateDialog(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useCreateAgentPet.mutate` (it holds the agent id);
    /// the consumer closes on success.
    #[prop(into)]
    on_create: Callback<PetCreate>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    creating: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let image = RwSignal::new(String::new());
    let desired_state = RwSignal::new("running".to_owned());

    // Blank the form on (re)open — the reference's effect.
    Effect::new(move |_| {
        if open.get() {
            name.set(String::new());
            image.set(String::new());
            desired_state.set("running".to_owned());
        }
    });

    let valid = Signal::derive(move || {
        !name.with(|v| v.trim().is_empty()) && !image.with(|v| v.trim().is_empty())
    });

    let submit = move |_| {
        if !valid.get_untracked() {
            return;
        }
        on_create.run(PetCreate {
            name: name.get_untracked().trim().to_owned(),
            image: image.get_untracked().trim().to_owned(),
            desired_state: desired_state.get_untracked(),
        });
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Add pet container"</DialogTitle>
                    <DialogDescription>
                        "Declare a new pet on this node. The agent provisions it from the named seed image and converges it to the desired state on its next reconcile pass."
                    </DialogDescription>
                </DialogHeader>
                <div class=PET_COL>
                    <label class=PET_LABEL>
                        "Name"
                        <input
                            class=format!("{PET_FIELD} mono")
                            value=move || name.get()
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                            placeholder="e.g. wg-router-1 (max 12 chars)"
                        />
                    </label>
                    <label class=PET_LABEL>
                        "Image"
                        <input
                            class=format!("{PET_FIELD} mono")
                            value=move || image.get()
                            prop:value=move || image.get()
                            on:input=move |ev| image.set(event_target_value(&ev))
                            placeholder="seed name, e.g. appsynergy-pet"
                        />
                    </label>
                    <label class=PET_LABEL>
                        "Desired state"
                        <select
                            class=PET_FIELD
                            prop:value=move || desired_state.get()
                            on:change=move |ev| desired_state.set(event_target_value(&ev))
                        >
                            <option value="running">"running"</option>
                            <option value="stopped">"stopped"</option>
                        </select>
                    </label>
                    {move || {
                        error
                            .get()
                            .map(|msg| {
                                view! {
                                    <p class=PET_ERROR style="color: var(--color-danger)">
                                        "Could not create pet: "
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
                        {move || if creating.get() { "Creating…" } else { "Add pet" }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Confirm removal of a declared pet container.
#[component]
pub fn PetRemoveDialog(
    #[prop(into)] pet: Signal<Option<AgentPet>>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useDeleteAgentPet.mutate` (it holds both ids); the
    /// consumer closes on success.
    #[prop(into)]
    on_remove: Callback<()>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    removing: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let confirm = move |_| {
        if pet.get_untracked().is_some() {
            on_remove.run(());
        }
    };
    view! {
        <DialogControlled open=Signal::derive(move || pet.get().is_some()) on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>
                        "Remove "
                        {move || {
                            pet.get().map(|p| p.name).unwrap_or_else(|| "this pet".to_owned())
                        }}
                        "?"
                    </DialogTitle>
                    <DialogDescription>
                        "If the agent has already reported this container, it becomes a pending removal until the agent confirms it destroyed the matching container; otherwise it is deleted immediately."
                    </DialogDescription>
                </DialogHeader>
                {move || {
                    error
                        .get()
                        .map(|msg| {
                            view! {
                                <p class=PET_ERROR style="color: var(--color-danger)">
                                    "Could not remove: "
                                    {msg}
                                </p>
                            }
                        })
                }}
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || removing.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Danger
                        size=ButtonSize::Sm
                        attr:disabled=move || removing.get().then_some("")
                        on:click=confirm
                    >
                        {move || if removing.get() { "Removing…" } else { "Remove pet" }}
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
        field = PET_FIELD,
        label = PET_LABEL,
        col = PET_COL,
        error = PET_ERROR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [PET_FIELD, PET_LABEL, PET_COL, PET_ERROR] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
