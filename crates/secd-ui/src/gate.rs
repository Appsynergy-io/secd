//! Auth gate table. Email first. Live session → Approve only. Remembered passkey ≤30d → no email.

use appsy_ui::prelude::*;
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
        show_passkey: false,
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
    let challenge = if code.is_empty() {
        view! {
            <DeviceCodeChallenge
                expires_in=300u32
                minting=false
                on_expired=Callback::new(|_| {})
                on_remint=Callback::new(|_| {})
            />
        }
        .into_any()
    } else {
        view! {
            <DeviceCodeChallenge
                code=code.clone()
                expires_in=300u32
                minting=false
                on_expired=Callback::new(|_| {})
                on_remint=Callback::new(|_| {})
            />
        }
        .into_any()
    };
    view! {
        <div data-page="device">
            <AuthShell home_href="/" links=Vec::new()>
                <AuthHead title="Approve this machine." />
                <div class="secd-auth-form">
                    {challenge}
                    <LabeledInput
                        id="user_code"
                        label="Device code"
                        autocomplete="off"
                        value=code
                    />
                    <span class="asy-btn--primary" data-action="approve">
                        <Button variant=ButtonVariant::Primary size=ButtonSize::Lg class="secd-btn-block">
                            "Approve"
                        </Button>
                    </span>
                </div>
            </AuthShell>
        </div>
    }
}

#[component]
pub fn GatePage(view: GateView) -> impl IntoView {
    let prefill = view.email_prefill.clone().unwrap_or_default();
    let ac = view.email_autocomplete.unwrap_or("username");
    let title = match view.kind {
        GateKind::RememberedPasskey | GateKind::RememberedPassword => "Welcome back.",
        GateKind::Identity => "Continue.",
        _ => "Sign in.",
    };
    let sub = match view.kind {
        GateKind::Cold => "Enter your email to continue.",
        GateKind::RememberedPasskey => "Use your passkey.",
        GateKind::RememberedPassword => "Enter your password.",
        GateKind::Identity => "Choose a factor.",
        GateKind::ApproveOnly => "",
    };
    view! {
        <div data-page="gate">
            <AuthShell home_href="/" links=Vec::new()>
                <AuthHead
                    title=title
                    sub=ViewFn::from(move || sub)
                />
                <div class="secd-auth-form">
                    {view.show_email.then(|| {
                        let prefill = prefill.clone();
                        view! {
                            <LabeledInput
                                id="email"
                                label="Email"
                                r#type="email"
                                autocomplete=ac
                                required=true
                                mono=true
                                value=prefill
                            />
                        }
                    })}
                    {view.show_password.then(|| {
                        view! {
                            <LabeledInput
                                id="password"
                                label="Password"
                                r#type="password"
                                autocomplete="current-password"
                                required=true
                            />
                        }
                    })}
                    {view.show_passkey.then(|| {
                        view! {
                            <span class="asy-btn--primary" data-action="passkey">
                                <Button variant=ButtonVariant::Primary size=ButtonSize::Lg class="secd-btn-block">
                                    "Use a passkey"
                                </Button>
                            </span>
                        }
                    })}
                    {(view.show_email || view.show_password).then(|| {
                        view! {
                            <span class="asy-btn--primary" data-action="continue">
                                <Button variant=ButtonVariant::Primary size=ButtonSize::Lg class="secd-btn-block">
                                    "Continue"
                                </Button>
                            </span>
                        }
                    })}
                    {view.show_use_password_instead.then(|| {
                        view! {
                            <span data-action="use-password">
                                <Button variant=ButtonVariant::Ghost class="secd-btn-block">
                                    "Use a password instead"
                                </Button>
                            </span>
                        }
                    })}
                    {view.show_use_different_account.then(|| {
                        view! {
                            <span data-action="different">
                                <Button variant=ButtonVariant::Ghost class="secd-btn-block">
                                    "Use a different account"
                                </Button>
                            </span>
                        }
                    })}
                </div>
            </AuthShell>
        </div>
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
