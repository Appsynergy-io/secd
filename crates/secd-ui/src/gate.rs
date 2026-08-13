//! Auth gate table. Live session → Approve only. Remembered passkey ≤30d → no email.

use leptos::prelude::*;
use time::OffsetDateTime;

use crate::remember::{remember_is_fresh, Remembered};
use crate::tokens::EMAIL_AUTOCOMPLETE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMethod {
    Register,
    Passkey,
    Password,
    Either,
}

impl AuthMethod {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "register" => Some(Self::Register),
            "passkey" => Some(Self::Passkey),
            "password" => Some(Self::Password),
            "either" => Some(Self::Either),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionInfo {
    pub email: String,
    pub has_passkey: bool,
    pub has_password: bool,
    pub session_id: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GateQuery {
    pub session: Option<SessionInfo>,
    pub remember: Option<Remembered>,
    pub now: Option<OffsetDateTime>,
    pub method: Option<AuthMethod>,
    pub use_different_account: bool,
    pub reveal_password: bool,
    pub user_code: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateKind {
    ApproveOnly,
    RememberedPasskey,
    RememberedPassword,
    Cold,
    Identity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateView {
    pub kind: GateKind,
    pub show_email: bool,
    pub show_password: bool,
    pub show_passkey: bool,
    pub show_approve: bool,
    pub email_autocomplete: Option<&'static str>,
    pub email_prefill: Option<String>,
    pub show_use_different_account: bool,
    pub show_use_password_instead: bool,
    pub user_code: Option<String>,
}

pub fn resolve_gate(q: &GateQuery) -> GateView {
    if q.session.is_some() {
        return GateView {
            kind: GateKind::ApproveOnly,
            show_email: false,
            show_password: false,
            show_passkey: false,
            show_approve: true,
            email_autocomplete: None,
            email_prefill: None,
            show_use_different_account: false,
            show_use_password_instead: false,
            user_code: q.user_code.clone(),
        };
    }

    let now = q.now.unwrap_or_else(OffsetDateTime::now_utc);
    let remembered = q
        .remember
        .as_ref()
        .filter(|r| !q.use_different_account && remember_is_fresh(&r.at, now));

    if let Some(r) = remembered {
        if r.has_passkey {
            return GateView {
                kind: GateKind::RememberedPasskey,
                show_email: false,
                show_password: false,
                show_passkey: true,
                show_approve: false,
                email_autocomplete: None,
                email_prefill: Some(r.email.clone()),
                show_use_different_account: true,
                show_use_password_instead: false,
                user_code: q.user_code.clone(),
            };
        }
        return GateView {
            kind: GateKind::RememberedPassword,
            show_email: false,
            show_password: true,
            show_passkey: false,
            show_approve: false,
            email_autocomplete: None,
            email_prefill: Some(r.email.clone()),
            show_use_different_account: true,
            show_use_password_instead: false,
            user_code: q.user_code.clone(),
        };
    }

    if let Some(method) = q.method {
        return identity_gate(method, q);
    }

    GateView {
        kind: GateKind::Cold,
        show_email: true,
        show_password: false,
        show_passkey: true,
        show_approve: false,
        email_autocomplete: Some(EMAIL_AUTOCOMPLETE),
        email_prefill: q.remember.as_ref().map(|r| r.email.clone()),
        show_use_different_account: false,
        show_use_password_instead: false,
        user_code: q.user_code.clone(),
    }
}

fn identity_gate(method: AuthMethod, q: &GateQuery) -> GateView {
    let (show_password, show_passkey, show_use_password) = match method {
        AuthMethod::Passkey => (false, true, false),
        AuthMethod::Password => (true, false, false),
        AuthMethod::Either => (q.reveal_password, true, !q.reveal_password),
        AuthMethod::Register => (true, true, false),
    };
    GateView {
        kind: GateKind::Identity,
        show_email: true,
        show_password,
        show_passkey,
        show_approve: false,
        email_autocomplete: Some(EMAIL_AUTOCOMPLETE),
        email_prefill: q.remember.as_ref().map(|r| r.email.clone()),
        show_use_different_account: q.remember.is_some(),
        show_use_password_instead: show_use_password,
        user_code: q.user_code.clone(),
    }
}

#[component]
pub fn DevicePage(view: GateView) -> impl IntoView {
    let code = view.user_code.clone().unwrap_or_default();
    view! {
        <section data-page="device">
            <h1>"Approve this machine"</h1>
            <label class="field-label">
                "Device code"
                <input
                    type="text"
                    name="user_code"
                    autocomplete="off"
                    value=code
                />
            </label>
            <button type="button" class="primary" data-action="approve">
                "Approve"
            </button>
        </section>
    }
}

#[component]
pub fn GatePage(view: GateView) -> impl IntoView {
    let prefill = view.email_prefill.clone().unwrap_or_default();
    let ac = view.email_autocomplete.unwrap_or("username");
    view! {
        <section data-page="gate">
            <h1>"secd"</h1>
            {view.show_email.then(|| {
                let prefill = prefill.clone();
                view! {
                    <label class="field-label">
                        "Email"
                        <input
                            type="email"
                            name="email"
                            autocomplete=ac
                            value=prefill
                        />
                    </label>
                }
            })}
            {view.show_password.then(|| {
                view! {
                    <label class="field-label">
                        "Password"
                        <input type="password" name="password" autocomplete="current-password" />
                    </label>
                }
            })}
            {view.show_passkey.then(|| {
                view! {
                    <button type="button" class="primary" data-action="passkey">
                        "Use a passkey"
                    </button>
                }
            })}
            {view.show_password.then(|| {
                view! {
                    <button type="button" class="primary" data-action="continue">
                        "Continue"
                    </button>
                }
            })}
            {view.show_use_password_instead.then(|| {
                view! {
                    <button type="button" class="ghost" data-action="use-password">
                        "Use a password instead"
                    </button>
                }
            })}
            {view.show_use_different_account.then(|| {
                view! {
                    <button type="button" class="ghost" data-action="different">
                        "Use a different account"
                    </button>
                }
            })}
        </section>
    }
}

pub fn render_gate(view: &GateView) -> String {
    crate::html(|| {
        if view.kind == GateKind::ApproveOnly {
            view! { <DevicePage view=view.clone() /> }.into_any()
        } else {
            view! { <GatePage view=view.clone() /> }.into_any()
        }
    })
}

pub fn render_device(view: &GateView) -> String {
    crate::html(|| view! { <DevicePage view=view.clone() /> })
}
