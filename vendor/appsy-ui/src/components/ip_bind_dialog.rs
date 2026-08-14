//! IpBindDialog — port of `dashboard/ip-bind-dialog.tsx` (T7).
//!
//! Props/callbacks split: `useBindIp` → `on_bind(IpBindRequest)` +
//! `binding`/`error` (the target ip's id never renders and stays with
//! the consumer, which also resets its mutation and closes on success);
//! `useTunnels` → the `tunnels` prop. Controlled by `target`: open while
//! `Some`, prefilled from its `current_binding` (rebind mode) per the
//! reference's `[target]` effect.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use leptos::prelude::*;

pub const IPBIND_COL: &str = "asy-ipbind__col";
pub const IPBIND_LABEL: &str = "asy-ipbind__label";
pub const IPBIND_FIELD: &str = "asy-ipbind__field";
pub const IPBIND_IP: &str = "asy-ipbind__ip";
pub const IPBIND_EMPTY: &str = "asy-ipbind__empty";
pub const IPBIND_ERR: &str = "asy-ipbind__err";

/// nft traffic direction (`PublicIpBindRequest.rule_type`) paired with a
/// plain-language label; `both` is the server default.
pub const RULE_TYPES: [(&str, &str); 3] = [
    ("both", "Both directions (inbound + outbound)"),
    ("dnat", "Inbound only (DNAT)"),
    ("snat", "Outbound only (SNAT)"),
];

/// A bindable tunnel — the fields the picker renders.
#[derive(Clone, PartialEq, Debug)]
pub struct Tunnel {
    pub id: String,
    pub profile_number: f64,
    /// `wireguard.profile_name` in the reference's nested shape.
    pub profile_name: Option<String>,
    /// `"quic"` renders QUIC; anything else WireGuard.
    pub tunnel_type: Option<String>,
}

/// The reference's `tunnelLabel`: the trimmed profile name, or
/// `profile #N`.
pub fn tunnel_label(t: &Tunnel) -> String {
    match t.profile_name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => format!("profile #{}", crate::components::acl_dialogs::fmt_num(t.profile_number)),
    }
}

/// The reference's `tunnelProtocolLabel`.
pub fn tunnel_protocol_label(t: &Tunnel) -> &'static str {
    if t.tunnel_type.as_deref() == Some("quic") {
        "QUIC"
    } else {
        "WireGuard"
    }
}

/// The dialog's target IP — display + prefill fields.
#[derive(Clone, PartialEq, Debug)]
pub struct IpBindTarget {
    pub ip_address: String,
    pub current_binding: Option<IpBinding>,
}

/// An existing binding — the prefill fields.
#[derive(Clone, PartialEq, Debug)]
pub struct IpBinding {
    pub tunnel_id: String,
    pub rule_type: Option<String>,
    pub dest_ports: Option<String>,
}

/// The bind request emitted on submit (the target id stays with the
/// consumer).
#[derive(Clone, PartialEq, Debug)]
pub struct IpBindRequest {
    pub tunnel_id: String,
    pub rule_type: String,
    /// Trimmed; `None` when blank (routes the whole IP).
    pub dest_ports: Option<String>,
}

/// Bind a reserved dedicated IP to one of the caller's tunnels, with an
/// optional routing direction and port-forward subset. Open while
/// `target` is `Some`; with an existing binding the form pre-fills and
/// the submit rebinds.
#[component]
pub fn IpBindDialog(
    #[prop(into)] target: Signal<Option<IpBindTarget>>,
    #[prop(into)] on_open_change: Callback<bool>,
    tunnels: Vec<Tunnel>,
    #[prop(into)] on_bind: Callback<IpBindRequest>,
    #[prop(optional, into)] binding: Signal<bool>,
    #[prop(optional, into)] error: Signal<Option<String>>,
) -> impl IntoView {
    let tunnels = StoredValue::new(tunnels);
    let tunnel_id = RwSignal::new(String::new());
    let rule_type = RwSignal::new("both".to_owned());
    let dest_ports = RwSignal::new(String::new());

    // The reference's `[target]` prefill effect (the consumer resets its
    // mutation alongside).
    Effect::new(move |_| {
        if let Some(t) = target.get() {
            let existing = t.current_binding;
            tunnel_id.set(
                existing
                    .as_ref()
                    .map(|b| b.tunnel_id.clone())
                    .or_else(|| tunnels.with_value(|ts| ts.first().map(|t| t.id.clone())))
                    .unwrap_or_default(),
            );
            rule_type.set(
                existing
                    .as_ref()
                    .and_then(|b| b.rule_type.clone())
                    .unwrap_or_else(|| "both".to_owned()),
            );
            dest_ports
                .set(existing.and_then(|b| b.dest_ports).unwrap_or_default());
        }
    });

    let rebinding = move || target.get().is_some_and(|t| t.current_binding.is_some());
    let submit = move |_| {
        if target.get_untracked().is_none() || tunnel_id.get_untracked().is_empty() {
            return;
        }
        let ports = dest_ports.get_untracked().trim().to_owned();
        on_bind.run(IpBindRequest {
            tunnel_id: tunnel_id.get_untracked(),
            rule_type: rule_type.get_untracked(),
            dest_ports: (!ports.is_empty()).then_some(ports),
        });
    };

    view! {
        <DialogControlled
            open=Signal::derive(move || target.get().is_some())
            on_open_change=on_open_change
        >
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>
                        {move || if rebinding() { "Edit binding" } else { "Bind to tunnel" }}
                    </DialogTitle>
                    <DialogDescription>
                        "Route "
                        {move || {
                            let ip = target.get().map(|t| t.ip_address).unwrap_or_default();
                            if ip.is_empty() {
                                leptos::either::Either::Left("this IP")
                            } else {
                                leptos::either::Either::Right(
                                    view! {
                                        <span class=format!("mono {IPBIND_IP}")>{ip}</span>
                                    },
                                )
                            }
                        }}
                        " to one of your tunnels. Optionally restrict the direction and forward only specific ports."
                    </DialogDescription>
                </DialogHeader>
                {move || {
                    if tunnels.with_value(|ts| ts.is_empty()) {
                        leptos::either::Either::Left(
                            view! {
                                <p class=IPBIND_EMPTY>
                                    "You have no tunnels yet. Create a tunnel first, then bind an IP to it."
                                </p>
                            },
                        )
                    } else {
                        let options = tunnels
                            .get_value()
                            .into_iter()
                            .map(|t| {
                                view! {
                                    <option value=t.id.clone()>
                                        {tunnel_label(&t)}
                                        " · "
                                        {tunnel_protocol_label(&t)}
                                    </option>
                                }
                            })
                            .collect_view();
                        leptos::either::Either::Right(
                            view! {
                                <div class=IPBIND_COL>
                                    <label class=IPBIND_LABEL>
                                        "Tunnel"
                                        <select
                                            class=IPBIND_FIELD
                                            prop:value=move || tunnel_id.get()
                                            on:change=move |ev| {
                                                tunnel_id.set(event_target_value(&ev))
                                            }
                                        >
                                            {options}
                                        </select>
                                    </label>
                                    <label class=IPBIND_LABEL>
                                        "Direction"
                                        <select
                                            class=IPBIND_FIELD
                                            prop:value=move || rule_type.get()
                                            on:change=move |ev| {
                                                rule_type.set(event_target_value(&ev))
                                            }
                                        >
                                            {RULE_TYPES
                                                .iter()
                                                .map(|(v, l)| {
                                                    view! { <option value=*v>{*l}</option> }
                                                })
                                                .collect_view()}
                                        </select>
                                    </label>
                                    <label class=IPBIND_LABEL>
                                        "Forwarded ports (optional)"
                                        <input
                                            class=IPBIND_FIELD
                                            value=move || dest_ports.get()
                                            prop:value=move || dest_ports.get()
                                            on:input=move |ev| {
                                                dest_ports.set(event_target_value(&ev))
                                            }
                                            placeholder="tcp/443,udp/53 — blank routes the whole IP"
                                        />
                                    </label>
                                    {move || {
                                        error
                                            .get()
                                            .map(|msg| {
                                                view! {
                                                    <p
                                                        class=IPBIND_ERR
                                                        style="color: var(--color-danger)"
                                                    >
                                                        "Could not bind: "
                                                        {msg}
                                                    </p>
                                                }
                                            })
                                    }}
                                </div>
                            },
                        )
                    }
                }}
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || binding.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || {
                            (tunnels.with_value(|ts| ts.is_empty())
                                || tunnel_id.get().is_empty() || binding.get())
                                .then_some("")
                        }
                        on:click=submit
                    >
                        {move || {
                            if binding.get() {
                                "Binding…"
                            } else if rebinding() {
                                "Save binding"
                            } else {
                                "Bind"
                            }
                        }}
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
            ".{label}{{display:flex;flex-direction:column;gap:.25rem;",
            "font-size:11.5px;font-weight:500;color:var(--color-text-muted)}}",
            ".{field}{{height:2rem;width:100%;border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);padding-inline:.5rem;",
            "font-size:12.5px;color:var(--color-text)}}",
            ".{ip}{{color:var(--color-text)}}",
            ".{empty}{{font-size:12.5px;color:var(--color-text-muted)}}",
            ".{err}{{font-size:12px}}",
        ),
        col = IPBIND_COL,
        label = IPBIND_LABEL,
        field = IPBIND_FIELD,
        ip = IPBIND_IP,
        empty = IPBIND_EMPTY,
        err = IPBIND_ERR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tunnel(name: Option<&str>, n: f64, ty: Option<&str>) -> Tunnel {
        Tunnel {
            id: "t".into(),
            profile_number: n,
            profile_name: name.map(Into::into),
            tunnel_type: ty.map(Into::into),
        }
    }

    #[test]
    fn tunnel_label_prefers_trimmed_name() {
        assert_eq!(tunnel_label(&tunnel(Some("ci-runner-3"), 3.0, None)), "ci-runner-3");
        assert_eq!(tunnel_label(&tunnel(Some("  "), 3.0, None)), "profile #3");
        assert_eq!(tunnel_label(&tunnel(None, 7.0, None)), "profile #7");
    }

    #[test]
    fn protocol_label_quic_or_wireguard() {
        assert_eq!(tunnel_protocol_label(&tunnel(None, 1.0, Some("quic"))), "QUIC");
        assert_eq!(tunnel_protocol_label(&tunnel(None, 1.0, Some("wireguard"))), "WireGuard");
        assert_eq!(tunnel_protocol_label(&tunnel(None, 1.0, None)), "WireGuard");
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [IPBIND_COL, IPBIND_LABEL, IPBIND_FIELD, IPBIND_IP, IPBIND_EMPTY, IPBIND_ERR]
        {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
