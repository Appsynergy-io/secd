//! Fleet dialog suite — port of `platform/fleet-dialogs.tsx` (P3).
//!
//! Props/callbacks split, per component (all four reference dialogs call
//! mutation hooks; every mutation leaves for the consumer):
//! - `ServerProvisionDialog`: `useCreateServer` → `on_create(ServerProvision)`
//!   with `creating`/`error` for `isPending`/`isError`+message. Form state
//!   (reset-on-open, validity gate) is presentation and stays here.
//! - `AgentRotateKeyDialog`: `useRotateAgentKey` → `on_rotate(())`; the
//!   once-only bearer comes back as the `issued` prop (the consumer sets it
//!   from the mutation's `onSuccess` and clears it on reopen, the
//!   reference's local `issued` state).
//! - `AgentVersionDialog`: `usePatchAgent` → `on_save(VersionPins)`; blank
//!   fields clear the respective pin (`None`).
//! - `AgentRetireDialog`: `useRetireAgent` → `on_retire(())`.
//!
//! All four are controlled dialogs over the crate-internal
//! `DialogControlled` root; the consumer holds ids and closes on success.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use leptos::prelude::*;

pub const FLEET_FIELD: &str = "asy-fleet__field";
pub const FLEET_LABEL: &str = "asy-fleet__label";
pub const FLEET_COL: &str = "asy-fleet__col";
pub const FLEET_GRID2: &str = "asy-fleet__grid2";
pub const FLEET_GRID2_END: &str = "asy-fleet__grid2--end";
pub const FLEET_CHECK: &str = "asy-fleet__check";
pub const FLEET_KEYBOX: &str = "asy-fleet__keybox";
pub const FLEET_KEY: &str = "asy-fleet__key";
pub const FLEET_ERROR: &str = "asy-fleet__error";
pub const FLEET_BTN_DANGER: &str = "asy-fleet__btn-danger";

/// The fields of `PlatformAgent` the dialogs render; ids, status, and the
/// rest of the record stay with the consumer.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FleetAgent {
    pub hostname: Option<String>,
    pub name: Option<String>,
    pub expected_version: Option<String>,
    pub reported_version: Option<String>,
    pub expected_image_version: Option<String>,
    pub reported_image_version: Option<String>,
}

impl FleetAgent {
    /// `agent?.hostname ?? agent?.name ?? "this agent"`.
    fn identity(agent: &Option<FleetAgent>) -> String {
        agent
            .as_ref()
            .and_then(|a| a.hostname.clone().or_else(|| a.name.clone()))
            .unwrap_or_else(|| "this agent".to_owned())
    }
}

/// What `ServerProvisionDialog` emits — the reference's POST body.
#[derive(Clone, PartialEq, Debug)]
pub struct ServerProvision {
    pub name: String,
    pub location: String,
    pub region: Option<String>,
    pub endpoint: String,
    pub public_key: String,
    pub private_key: String,
    pub server_type: String,
    pub max_users: Option<f64>,
    pub allow_static_ips: bool,
}

/// What `AgentVersionDialog` emits — `None` clears that pin.
#[derive(Clone, PartialEq, Debug)]
pub struct VersionPins {
    pub expected_version: Option<String>,
    pub expected_image_version: Option<String>,
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

/// Provision a new WG / QUIC infra server. The private key is sealed
/// server-side and never round-trips.
#[component]
pub fn ServerProvisionDialog(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useCreateServer.mutate`; the consumer closes on
    /// success.
    #[prop(into)]
    on_create: Callback<ServerProvision>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    creating: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let location = RwSignal::new(String::new());
    let region = RwSignal::new(String::new());
    let endpoint = RwSignal::new(String::new());
    let public_key = RwSignal::new(String::new());
    let private_key = RwSignal::new(String::new());
    let server_type = RwSignal::new("wireguard".to_owned());
    let max_users = RwSignal::new(String::new());
    let allow_static_ips = RwSignal::new(false);

    // Blank the whole form on (re)open — the reference's effect.
    Effect::new(move |_| {
        if open.get() {
            name.set(String::new());
            location.set(String::new());
            region.set(String::new());
            endpoint.set(String::new());
            public_key.set(String::new());
            private_key.set(String::new());
            server_type.set("wireguard".to_owned());
            max_users.set(String::new());
            allow_static_ips.set(false);
        }
    });

    let valid = Signal::derive(move || {
        !name.with(|v| v.trim().is_empty())
            && !location.with(|v| v.trim().is_empty())
            && !endpoint.with(|v| v.trim().is_empty())
            && !public_key.with(|v| v.trim().is_empty())
            && !private_key.with(|v| v.trim().is_empty())
            && !server_type.with(|v| v.trim().is_empty())
    });

    let submit = move |_| {
        if !valid.get_untracked() {
            return;
        }
        let r = region.get_untracked().trim().to_owned();
        on_create.run(ServerProvision {
            name: name.get_untracked().trim().to_owned(),
            location: location.get_untracked().trim().to_owned(),
            region: (!r.is_empty()).then_some(r),
            endpoint: endpoint.get_untracked().trim().to_owned(),
            public_key: public_key.get_untracked().trim().to_owned(),
            private_key: private_key.get_untracked().trim().to_owned(),
            server_type: server_type.get_untracked().trim().to_owned(),
            max_users: num(&max_users.get_untracked()),
            allow_static_ips: allow_static_ips.get_untracked(),
        });
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Provision server"</DialogTitle>
                    <DialogDescription>
                        "Register a WG / QUIC infra server. The private key is sealed server-side and never shown again. The server starts offline until its agent checks in."
                    </DialogDescription>
                </DialogHeader>
                <div class=FLEET_COL>
                    <div class=FLEET_GRID2>
                        <label class=FLEET_LABEL>
                            "Name"
                            <input
                                class=FLEET_FIELD
                                value=move || name.get()
                                prop:value=move || name.get()
                                on:input=move |ev| name.set(event_target_value(&ev))
                            />
                        </label>
                        <label class=FLEET_LABEL>
                            "Type"
                            <select
                                class=FLEET_FIELD
                                prop:value=move || server_type.get()
                                on:change=move |ev| server_type.set(event_target_value(&ev))
                            >
                                <option value="wireguard">"wireguard"</option>
                                <option value="quic">"quic"</option>
                            </select>
                        </label>
                    </div>
                    <div class=FLEET_GRID2>
                        <label class=FLEET_LABEL>
                            "Location"
                            <input
                                class=FLEET_FIELD
                                value=move || location.get()
                                prop:value=move || location.get()
                                on:input=move |ev| location.set(event_target_value(&ev))
                                placeholder="city / DC"
                            />
                        </label>
                        <label class=FLEET_LABEL>
                            "Region (optional)"
                            <input
                                class=FLEET_FIELD
                                value=move || region.get()
                                prop:value=move || region.get()
                                on:input=move |ev| region.set(event_target_value(&ev))
                                placeholder="continent"
                            />
                        </label>
                    </div>
                    <label class=FLEET_LABEL>
                        "Endpoint"
                        <input
                            class=format!("{FLEET_FIELD} mono")
                            value=move || endpoint.get()
                            prop:value=move || endpoint.get()
                            on:input=move |ev| endpoint.set(event_target_value(&ev))
                            placeholder="host:port"
                        />
                    </label>
                    <label class=FLEET_LABEL>
                        "WG public key"
                        <input
                            class=format!("{FLEET_FIELD} mono")
                            value=move || public_key.get()
                            prop:value=move || public_key.get()
                            on:input=move |ev| public_key.set(event_target_value(&ev))
                            placeholder="base64"
                        />
                    </label>
                    <label class=FLEET_LABEL>
                        "WG private key"
                        <input
                            class=format!("{FLEET_FIELD} mono")
                            type="password"
                            value=move || private_key.get()
                            prop:value=move || private_key.get()
                            on:input=move |ev| private_key.set(event_target_value(&ev))
                            placeholder="base64 — sealed server-side"
                        />
                    </label>
                    <div class=FLEET_GRID2_END>
                        <label class=FLEET_LABEL>
                            "Max users (optional)"
                            <input
                                class=FLEET_FIELD
                                inputmode="numeric"
                                value=move || max_users.get()
                                prop:value=move || max_users.get()
                                on:input=move |ev| max_users.set(event_target_value(&ev))
                            />
                        </label>
                        <label class=FLEET_CHECK>
                            <input
                                type="checkbox"
                                prop:checked=move || allow_static_ips.get()
                                on:change=move |ev| allow_static_ips.set(event_target_checked(&ev))
                            />
                            "Allow static IPs"
                        </label>
                    </div>
                    {move || {
                        error
                            .get()
                            .map(|msg| {
                                view! {
                                    <p class=FLEET_ERROR style="color: var(--color-danger)">
                                        "Could not provision: "
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
                        {move || if creating.get() { "Provisioning…" } else { "Provision" }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Rotate a fleet-agent's bearer credential. The new key (the `issued`
/// prop) is shown exactly once.
#[component]
pub fn AgentRotateKeyDialog(
    #[prop(into)] agent: Signal<Option<FleetAgent>>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useRotateAgentKey.mutate` for the agent's id.
    #[prop(into)]
    on_rotate: Callback<()>,
    /// The minted bearer from the mutation's success — the consumer sets
    /// it once and clears it on reopen (the reference's `issued` state).
    #[prop(optional, into)]
    issued: Signal<Option<String>>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    rotating: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let confirm = move |_| {
        if agent.get_untracked().is_some() {
            on_rotate.run(());
        }
    };
    view! {
        <DialogControlled open=Signal::derive(move || agent.get().is_some()) on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Rotate agent key"</DialogTitle>
                    <DialogDescription>
                        {move || {
                            if issued.get().is_some() {
                                "Copy this bearer into the node's config now — it will not be shown again."
                                    .to_owned()
                            } else {
                                format!(
                                    "Mint a fresh bearer for {}. The previous credential is invalidated immediately.",
                                    FleetAgent::identity(&agent.get()),
                                )
                            }
                        }}
                    </DialogDescription>
                </DialogHeader>
                {move || match issued.get() {
                    Some(key) => leptos::either::Either::Left(view! {
                        <div class=FLEET_KEYBOX>
                            <code class=format!("mono {FLEET_KEY}")>{key}</code>
                        </div>
                    }),
                    None => leptos::either::Either::Right({
                        move || {
                            error
                                .get()
                                .map(|msg| {
                                    view! {
                                        <p class=FLEET_ERROR style="color: var(--color-danger)">
                                            "Could not rotate: "
                                            {msg}
                                        </p>
                                    }
                                })
                        }
                    }),
                }}
                <DialogFooter>
                    {move || {
                        if issued.get().is_some() {
                            leptos::either::Either::Left(view! {
                                <Button
                                    variant=ButtonVariant::Primary
                                    size=ButtonSize::Sm
                                    on:click=move |_| on_open_change.run(false)
                                >
                                    "Done"
                                </Button>
                            })
                        } else {
                            leptos::either::Either::Right(view! {
                                <Button
                                    variant=ButtonVariant::Ghost
                                    size=ButtonSize::Sm
                                    attr:disabled=move || rotating.get().then_some("")
                                    on:click=move |_| on_open_change.run(false)
                                >
                                    "Cancel"
                                </Button>
                                <Button
                                    variant=ButtonVariant::Primary
                                    size=ButtonSize::Sm
                                    attr:disabled=move || rotating.get().then_some("")
                                    on:click=confirm
                                >
                                    {move || if rotating.get() { "Rotating…" } else { "Rotate key" }}
                                </Button>
                            })
                        }
                    }}
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Pin the target versions a fleet agent should converge to. Blank fields
/// clear the respective pin.
#[component]
pub fn AgentVersionDialog(
    #[prop(into)] agent: Signal<Option<FleetAgent>>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `usePatchAgent.mutate` for the agent's id; the
    /// consumer closes on success.
    #[prop(into)]
    on_save: Callback<VersionPins>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    saving: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let version = RwSignal::new(String::new());
    let image_version = RwSignal::new(String::new());

    // Prefill each field with its current target, falling back to the
    // reported value — the reference's effect.
    Effect::new(move |_| {
        if let Some(a) = agent.get() {
            version.set(a.expected_version.or(a.reported_version).unwrap_or_default());
            image_version
                .set(a.expected_image_version.or(a.reported_image_version).unwrap_or_default());
        }
    });

    let submit = move |_| {
        if agent.get_untracked().is_none() {
            return;
        }
        let v = version.get_untracked().trim().to_owned();
        let iv = image_version.get_untracked().trim().to_owned();
        on_save.run(VersionPins {
            expected_version: (!v.is_empty()).then_some(v),
            expected_image_version: (!iv.is_empty()).then_some(iv),
        });
    };

    view! {
        <DialogControlled open=Signal::derive(move || agent.get().is_some()) on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Set target versions"</DialogTitle>
                    <DialogDescription>
                        "Pin the builds "
                        {move || FleetAgent::identity(&agent.get())}
                        " should converge to. An OS-image pin holds this node out of staged image rollouts until cleared."
                    </DialogDescription>
                </DialogHeader>
                <div class=FLEET_COL>
                    <label class=FLEET_LABEL>
                        "Agent binary version — reported: "
                        {move || {
                            agent
                                .get()
                                .and_then(|a| a.reported_version)
                                .unwrap_or_else(|| "unknown".to_owned())
                        }}
                        <input
                            class=FLEET_FIELD
                            value=move || version.get()
                            prop:value=move || version.get()
                            on:input=move |ev| version.set(event_target_value(&ev))
                            placeholder="e.g. 0.4.0 (blank = unpin)"
                        />
                    </label>
                    <label class=FLEET_LABEL>
                        "OS-image version — reported: "
                        {move || {
                            agent
                                .get()
                                .and_then(|a| a.reported_image_version)
                                .unwrap_or_else(|| "unknown".to_owned())
                        }}
                        <input
                            class=FLEET_FIELD
                            value=move || image_version.get()
                            prop:value=move || image_version.get()
                            on:input=move |ev| image_version.set(event_target_value(&ev))
                            placeholder="e.g. v0.1.1 (blank = unpin, rejoins rollouts)"
                        />
                    </label>
                </div>
                {move || {
                    error
                        .get()
                        .map(|msg| {
                            view! {
                                <p class=FLEET_ERROR style="color: var(--color-danger)">
                                    "Could not update: "
                                    {msg}
                                </p>
                            }
                        })
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
                        attr:disabled=move || saving.get().then_some("")
                        on:click=submit
                    >
                        {move || if saving.get() { "Saving…" } else { "Set versions" }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Retire a fleet agent — a soft transition to `retired`, confirmed by node
/// identity.
#[component]
pub fn AgentRetireDialog(
    #[prop(into)] agent: Signal<Option<FleetAgent>>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useRetireAgent.mutate` for the agent's id; the
    /// consumer closes on success.
    #[prop(into)]
    on_retire: Callback<()>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    retiring: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let confirm = move |_| {
        if agent.get_untracked().is_some() {
            on_retire.run(());
        }
    };
    view! {
        <DialogControlled open=Signal::derive(move || agent.get().is_some()) on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Retire node"</DialogTitle>
                    <DialogDescription>
                        "Retire "
                        {move || FleetAgent::identity(&agent.get())}
                        "? It stops counting toward fleet health and no longer receives version or rollout targets. Linked capability rows are kept for audit."
                    </DialogDescription>
                </DialogHeader>
                {move || {
                    error
                        .get()
                        .map(|msg| {
                            view! {
                                <p class=FLEET_ERROR style="color: var(--color-danger)">
                                    "Could not retire: "
                                    {msg}
                                </p>
                            }
                        })
                }}
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || retiring.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        class=FLEET_BTN_DANGER
                        attr:disabled=move || retiring.get().then_some("")
                        on:click=confirm
                    >
                        {move || if retiring.get() { "Retiring…" } else { "Retire node" }}
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
            ".{grid2}{{display:grid;grid-template-columns:1fr;gap:.75rem}}",
            "@media (width >= 40rem){{.{grid2}{{",
            "grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            ".{grid2_end}{{display:grid;grid-template-columns:1fr;",
            "align-items:flex-end;gap:.75rem}}",
            "@media (width >= 40rem){{.{grid2_end}{{",
            "grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            ".{check}{{display:flex;align-items:center;gap:.5rem;font-size:12px;",
            "color:var(--color-text-muted)}}",
            ".{keybox}{{border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);padding:.5rem}}",
            ".{key}{{display:block;word-break:break-all;font-size:12px;",
            "color:var(--color-accent)}}",
            ".{error}{{font-size:12px}}",
            ".{btn_danger}{{background-color:var(--color-danger)}}",
        ),
        field = FLEET_FIELD,
        label = FLEET_LABEL,
        col = FLEET_COL,
        grid2 = FLEET_GRID2,
        grid2_end = FLEET_GRID2_END,
        check = FLEET_CHECK,
        keybox = FLEET_KEYBOX,
        key = FLEET_KEY,
        error = FLEET_ERROR,
        btn_danger = FLEET_BTN_DANGER,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            FLEET_FIELD, FLEET_LABEL, FLEET_COL, FLEET_GRID2, FLEET_GRID2_END, FLEET_CHECK,
            FLEET_KEYBOX, FLEET_KEY, FLEET_ERROR, FLEET_BTN_DANGER,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    /// The reference's `num()`: blank → null, `Number(t)`, finite gate.
    #[test]
    fn num_mirrors_reference() {
        assert_eq!(num(""), None);
        assert_eq!(num("   "), None);
        assert_eq!(num("2000"), Some(2000.0));
        assert_eq!(num(" 1.5 "), Some(1.5));
        assert_eq!(num("12x"), None);
    }

    #[test]
    fn identity_fallback_chain() {
        let a = FleetAgent { hostname: Some("h".into()), name: Some("n".into()), ..Default::default() };
        assert_eq!(FleetAgent::identity(&Some(a)), "h");
        let b = FleetAgent { hostname: None, name: Some("n".into()), ..Default::default() };
        assert_eq!(FleetAgent::identity(&Some(b)), "n");
        assert_eq!(FleetAgent::identity(&None), "this agent");
    }
}
