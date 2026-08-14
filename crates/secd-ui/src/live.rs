//! CSR console. Library components only.

use appsy_ui::icons as ri;
use appsy_ui::prelude::*;
use leptos::prelude::*;
use serde_json::{json, Value};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::account::{AccountPage, AccountView, PasskeyRow, SessionRow};
use crate::activity::{ActivityPage, ActivityView, AuditRow};
use crate::api;
use crate::app::Screen;
use crate::client;
use crate::crypto::{email_ok, password_ok};
use crate::gate::{resolve_gate, AuthMethod, GatePage, GateQuery, SessionInfo};
use crate::register::{FieldView, RegisterPage, RegisterView, SecretItem};
use crate::tokens::FAIL_SENTENCE;

fn nav_items() -> Vec<SideNavItem> {
    vec![
        SideNavItem {
            label: "Register".into(),
            icon: ri::RI_FILE_LIST_3_LINE,
            href: "/register".into(),
        },
        SideNavItem {
            label: "Activity".into(),
            icon: ri::RI_HISTORY_LINE,
            href: "/activity".into(),
        },
        SideNavItem {
            label: "Account".into(),
            icon: ri::RI_USER_LINE,
            href: "/account".into(),
        },
    ]
}

fn palette() -> PaletteGroup {
    PaletteGroup {
        heading: "Go to".into(),
        items: nav_items()
            .into_iter()
            .map(|i| PaletteItem {
                label: i.label,
                href: i.href,
                icon: i.icon,
            })
            .collect(),
    }
}

fn screen_from_path(path: &str) -> Screen {
    match path {
        "/activity" => Screen::Activity,
        "/account" => Screen::Account,
        "/device" => Screen::Device,
        _ => Screen::Register,
    }
}

#[component]
pub fn App() -> impl IntoView {
    let session: RwSignal<Option<SessionInfo>> = RwSignal::new(None);
    let path = RwSignal::new(client::path());
    let width = RwSignal::new(client::width_px());
    let method: RwSignal<Option<AuthMethod>> = RwSignal::new(None);
    let different = RwSignal::new(false);
    let reveal_pw = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let pending = RwSignal::new(false);
    let register = RwSignal::new(RegisterView {
        width_px: client::width_px(),
        ..RegisterView::default()
    });
    let activity = RwSignal::new(ActivityView::default());
    let account = RwSignal::new(AccountView::default());
    let (user_code, _eph) = client::query_user_code();
    let user_code = RwSignal::new(user_code);
    let remember = RwSignal::new(client::load_remember());

    {
        let path = path;
        let width = width;
        let window = web_sys::window().expect("invariant: window");
        let on_pop = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            path.set(client::path());
        });
        let _ =
            window.add_event_listener_with_callback("popstate", on_pop.as_ref().unchecked_ref());
        on_pop.forget();
        let on_resize = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            width.set(client::width_px());
        });
        let _ =
            window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
        on_resize.forget();
    }

    spawn_local({
        let session = session;
        async move {
            if let Ok(res) = client::req("GET", api::session_url(), None).await {
                if res.status == 200 {
                    session.set(Some(SessionInfo {
                        email: res
                            .data
                            .get("email")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        has_passkey: res
                            .data
                            .get("has_passkey")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        has_password: res
                            .data
                            .get("has_password")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        session_id: res
                            .data
                            .get("session_id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    }));
                }
            }
        }
    });

    let navigate = Callback::new(move |href: String| {
        client::push_path(&href);
        path.set(href);
    });

    view! {
        <Toaster />
        {move || {
            if session.get().is_some() {
                view! {
                    <LiveShell
                        session=session
                        path=path
                        width=width
                        user_code=user_code
                        register=register
                        activity=activity
                        account=account
                        error=error
                        navigate=navigate
                    />
                }.into_any()
            } else {
                view! {
                    <LiveGate
                        session=session
                        method=method
                        different=different
                        reveal_pw=reveal_pw
                        error=error
                        pending=pending
                        remember=remember
                        user_code=user_code
                    />
                }.into_any()
            }
        }}
    }
}

#[component]
fn LiveGate(
    session: RwSignal<Option<SessionInfo>>,
    method: RwSignal<Option<AuthMethod>>,
    different: RwSignal<bool>,
    reveal_pw: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    pending: RwSignal<bool>,
    remember: RwSignal<Option<crate::Remembered>>,
    user_code: RwSignal<String>,
) -> impl IntoView {
    let email = RwSignal::new(
        remember
            .get_untracked()
            .map(|r| r.email)
            .unwrap_or_default(),
    );
    let password = RwSignal::new(String::new());

    let view_now = move || {
        resolve_gate(&GateQuery {
            session: None,
            remember: remember.get(),
            method: method.get(),
            use_different_account: different.get(),
            reveal_password: reveal_pw.get(),
            user_code: {
                let c = user_code.get();
                if c.is_empty() {
                    None
                } else {
                    Some(c)
                }
            },
            now: None,
        })
    };

    let after_ok = move |addr: String, has_pk: bool| {
        spawn_local(async move {
            client::save_remember(&addr, has_pk);
            remember.set(client::load_remember());
            match client::req("GET", api::session_url(), None).await {
                Ok(res) if res.status == 200 => {
                    pending.set(false);
                    error.set(None);
                    session.set(Some(SessionInfo {
                        email: res
                            .data
                            .get("email")
                            .and_then(Value::as_str)
                            .unwrap_or(&addr)
                            .to_string(),
                        has_passkey: res
                            .data
                            .get("has_passkey")
                            .and_then(Value::as_bool)
                            .unwrap_or(has_pk),
                        has_password: res
                            .data
                            .get("has_password")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        session_id: res
                            .data
                            .get("session_id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    }));
                }
                Ok(res) => {
                    pending.set(false);
                    error.set(Some(
                        api::error_message(&res.data).unwrap_or_else(|| FAIL_SENTENCE.into()),
                    ));
                }
                Err(e) => {
                    pending.set(false);
                    error.set(Some(e));
                }
            }
        });
    };

    let on_continue = move |_| {
        if pending.get() {
            return;
        }
        let addr = email.get();
        let Some(norm) = email_ok(&addr) else {
            error.set(Some("email".into()));
            return;
        };
        pending.set(true);
        error.set(None);
        spawn_local(async move {
            if method.get_untracked().is_none() {
                match client::req("POST", api::start_url(), Some(&json!({ "email": norm }))).await {
                    Ok(res) if res.status == 200 => {
                        method.set(AuthMethod::parse(
                            res.data.get("method").and_then(Value::as_str).unwrap_or(""),
                        ));
                        pending.set(false);
                        return;
                    }
                    Ok(res) => {
                        pending.set(false);
                        error.set(Some(
                            api::error_message(&res.data).unwrap_or_else(|| FAIL_SENTENCE.into()),
                        ));
                        return;
                    }
                    Err(e) => {
                        pending.set(false);
                        error.set(Some(e));
                        return;
                    }
                }
            }
            let pw = password.get_untracked();
            if !password_ok(&pw) {
                pending.set(false);
                error.set(Some("password".into()));
                return;
            }
            let url = if method.get_untracked() == Some(AuthMethod::Register) {
                api::password_register_url()
            } else {
                api::password_login_url()
            };
            match client::req("POST", url, Some(&json!({ "email": norm, "password": pw }))).await {
                Ok(res) if res.status == 200 => after_ok(norm, false),
                Ok(res) => {
                    pending.set(false);
                    password.set(String::new());
                    error.set(Some(
                        api::error_message(&res.data).unwrap_or_else(|| FAIL_SENTENCE.into()),
                    ));
                }
                Err(e) => {
                    pending.set(false);
                    error.set(Some(e));
                }
            }
        });
    };

    let on_passkey = move |_| {
        if pending.get() {
            return;
        }
        pending.set(true);
        error.set(None);
        spawn_local(async move {
            let mut addr = email.get_untracked();
            if method.get_untracked().is_none() && !addr.is_empty() {
                if let Ok(res) =
                    client::req("POST", api::start_url(), Some(&json!({ "email": addr }))).await
                {
                    if res.status == 200 {
                        method.set(AuthMethod::parse(
                            res.data.get("method").and_then(Value::as_str).unwrap_or(""),
                        ));
                    }
                }
            }
            if addr.is_empty() {
                if let Some(r) = remember.get_untracked() {
                    addr = r.email;
                }
            }
            let res = if method.get_untracked() == Some(AuthMethod::Register) {
                client::passkey_create(&addr).await
            } else {
                client::passkey_get(if addr.is_empty() { None } else { Some(&addr) }, false).await
            };
            match res {
                Ok(http) if http.status == 200 => after_ok(addr, true),
                Ok(http) => {
                    pending.set(false);
                    error.set(Some(
                        api::error_message(&http.data).unwrap_or_else(|| FAIL_SENTENCE.into()),
                    ));
                }
                Err(e) => {
                    pending.set(false);
                    error.set(Some(e));
                }
            }
        });
    };

    view! {
        {move || {
            let g = view_now();
            view! {
                <div on:click=move |ev| {
                    let Some(t) = ev.target() else { return };
                    let Ok(el) = t.dyn_into::<web_sys::Element>() else { return };
                    let Ok(Some(act)) = el.closest("[data-action]") else { return };
                    let Some(name) = act.get_attribute("data-action") else { return };
                    match name.as_str() {
                        "continue" => on_continue(()),
                        "passkey" => on_passkey(()),
                        "use-password" => reveal_pw.set(true),
                        "different" => {
                            different.set(true);
                            method.set(None);
                        }
                        _ => {}
                    }
                }>
                    <Show when=move || error.get().is_some()>
                        <Banner tone=BannerTone::Danger title=move || error.get().unwrap_or_default() />
                    </Show>
                    <GatePage view=g />
                </div>
            }
        }}
    }
}

#[component]
fn LiveShell(
    session: RwSignal<Option<SessionInfo>>,
    path: RwSignal<String>,
    width: RwSignal<u32>,
    user_code: RwSignal<String>,
    register: RwSignal<RegisterView>,
    activity: RwSignal<ActivityView>,
    account: RwSignal<AccountView>,
    error: RwSignal<Option<String>>,
    navigate: Callback<String>,
) -> impl IntoView {
    let s = session.get_untracked().expect("invariant: session");
    let email = RwSignal::new(s.email.clone());
    let config = DashShellConfig {
        show_platform: false,
        org_name: Some("secd".into()),
        user_name: Some(email.get_untracked()),
        user_email: Some(email.get_untracked()),
        is_platform_admin: false,
        customer_items: nav_items(),
        platform_items: vec![],
        home_href: "/register".into(),
        account_href: "/account".into(),
        platform_href: None,
        notifications_href: "/account".into(),
        help_href: "/register".into(),
        unread: 0,
        orgs: vec![OrgMembership {
            org_id: "secd".into(),
            name: "secd".into(),
            is_active: true,
        }],
        on_switch_org: Callback::new(|_| {}),
        switching: Signal::from(false),
        on_sign_out: Callback::new(move |_| {
            spawn_local(async move {
                let _ = client::req("POST", api::logout_url(), Some(&json!({}))).await;
                client::clear_remember();
                session.set(None);
                client::push_path("/");
                path.set("/".into());
            });
        }),
        signing_out: Signal::from(false),
        palette_groups: vec![palette()],
        on_navigate: navigate,
    };

    Effect::new(move |_| {
        let screen = screen_from_path(&path.get());
        spawn_local(async move {
            match screen {
                Screen::Register => {
                    if let Ok(res) = client::req("GET", api::vault_url(), None).await {
                        if res.status == 200 {
                            register.set(vault_to_view(&res.data, width.get_untracked()));
                        }
                    }
                }
                Screen::Activity => {
                    if let Ok(res) = client::req("GET", api::audit_url(), None).await {
                        if res.status == 200 {
                            activity.set(audit_to_view(&res.data));
                        }
                    }
                }
                Screen::Account => {
                    let mut next = AccountView {
                        email: email.get_untracked(),
                        has_password: session
                            .get_untracked()
                            .map(|s| s.has_password)
                            .unwrap_or(false),
                        ..AccountView::default()
                    };
                    if let Ok(res) = client::req("GET", api::sessions_url(), None).await {
                        if res.status == 200 {
                            next.sessions = sessions_from(&res.data);
                        }
                    }
                    if let Ok(res) = client::req("GET", api::passkeys_url(), None).await {
                        if res.status == 200 {
                            next.passkeys = passkeys_from(&res.data);
                        }
                    }
                    account.set(next);
                }
                _ => {}
            }
        });
    });

    view! {
        <ConfiguredDashShell config=config path=Signal::derive(move || path.get())>
            <Show when=move || error.get().is_some()>
                <Banner tone=BannerTone::Danger title=move || error.get().unwrap_or_default() />
            </Show>
            {move || {
                match screen_from_path(&path.get()) {
                    Screen::Activity => view! { <ActivityPage view=activity.get() /> }.into_any(),
                    Screen::Account => view! {
                        <div on:click=move |ev| {
                            let Some(t) = ev.target() else { return };
                            let Ok(el) = t.dyn_into::<web_sys::Element>() else { return };
                            let Ok(Some(act)) = el.closest("[data-action]") else { return };
                            let Some(name) = act.get_attribute("data-action") else { return };
                            match name.as_str() {
                                "revoke" => {
                                    if let Some(id) = act.get_attribute("data-session-id") {
                                        spawn_local(async move {
                                            let url = api::session_revoke_path(&id);
                                            let _ = client::req("DELETE", &url, None).await;
                                            path.set("/account".into());
                                        });
                                    }
                                }
                                "remove" => {
                                    if let Some(id) = act.get_attribute("data-passkey-id") {
                                        spawn_local(async move {
                                            let url = api::passkey_delete_path(&id);
                                            let _ = client::req("DELETE", &url, None).await;
                                        });
                                    }
                                }
                                "add-passkey" => {
                                    let addr = email.get_untracked();
                                    spawn_local(async move {
                                        let _ = client::passkey_create(&addr).await;
                                    });
                                }
                                _ => {}
                            }
                        }>
                            <AccountPage view=account.get() />
                        </div>
                    }.into_any(),
                    Screen::Device => {
                        let g = resolve_gate(&GateQuery {
                            session: session.get(),
                            user_code: {
                                let c = user_code.get();
                                if c.is_empty() { None } else { Some(c) }
                            },
                            ..GateQuery::default()
                        });
                        view! {
                            <div on:click=move |ev| {
                                let Some(t) = ev.target() else { return };
                                let Ok(el) = t.dyn_into::<web_sys::Element>() else { return };
                                let Ok(Some(act)) = el.closest("[data-action]") else { return };
                                if act.get_attribute("data-action").as_deref() != Some("approve") {
                                    return;
                                }
                                let code = user_code.get();
                                spawn_local(async move {
                                    let body = json!({
                                        "user_code": code,
                                        "sealed_dek": { "alg": "x25519-xchacha20poly1305" }
                                    });
                                    match client::req("POST", api::device_approve_url(), Some(&body)).await {
                                        Ok(res) if res.status == 200 => {
                                            client::push_path("/register");
                                            path.set("/register".into());
                                        }
                                        Ok(res) => error.set(Some(
                                            api::error_message(&res.data).unwrap_or_else(|| FAIL_SENTENCE.into()),
                                        )),
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            }>
                                <crate::gate::DevicePage view=g />
                            </div>
                        }.into_any()
                    }
                    _ => {
                        let mut v = register.get();
                        v.width_px = width.get();
                        view! {
                            <div on:click=move |ev| {
                                let Some(t) = ev.target() else { return };
                                let Ok(el) = t.dyn_into::<web_sys::Element>() else { return };
                                let Ok(Some(act)) = el.closest("[data-action]") else { return };
                                let Some(name) = act.get_attribute("data-action") else { return };
                                match name.as_str() {
                                    "select" => {
                                        if let Some(n) = act.get_attribute("data-name") {
                                            register.update(|r| r.selected = Some(n));
                                        }
                                    }
                                    "add" => register.update(|r| r.wizard_open = true),
                                    "wizard-cancel" | "close-sheet" => {
                                        register.update(|r| {
                                            r.wizard_open = false;
                                            r.selected = None;
                                        });
                                    }
                                    "copy" => {
                                        spawn_local(async move {
                                            client::copy_text("").await;
                                        });
                                    }
                                    _ => {}
                                }
                            }>
                                <RegisterPage view=v />
                            </div>
                        }.into_any()
                    }
                }
            }}
        </ConfiguredDashShell>
    }
}

fn vault_to_view(data: &Value, width_px: u32) -> RegisterView {
    let entries = data
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut items: Vec<SecretItem> = Vec::new();
    for e in entries {
        let name = e
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let key = name.rsplit('/').next().unwrap_or(&name).to_string();
        if let Some(existing) = items.iter_mut().find(|i| i.name == name) {
            existing.fields.push(FieldView {
                key,
                secret: true,
                value: String::new(),
            });
        } else {
            items.push(SecretItem {
                name,
                fields: vec![FieldView {
                    key,
                    secret: true,
                    value: String::new(),
                }],
            });
        }
    }
    RegisterView {
        width_px,
        items,
        selected: None,
        wizard_open: false,
    }
}

fn audit_to_view(data: &Value) -> ActivityView {
    let ev = data
        .get("events")
        .or_else(|| data.get("audit"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    ActivityView {
        events: ev
            .iter()
            .map(|e| AuditRow {
                action: e
                    .get("action")
                    .or_else(|| e.get("event"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                name: e
                    .get("names")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .or_else(|| e.get("name").and_then(Value::as_str))
                    .unwrap_or("")
                    .to_string(),
                at: e
                    .get("at")
                    .or_else(|| e.get("created"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
            .collect(),
    }
}

fn sessions_from(data: &Value) -> Vec<SessionRow> {
    data.get("sessions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|s| SessionRow {
            id: s.get("id").and_then(Value::as_str).unwrap_or("").into(),
            kind: s.get("kind").and_then(Value::as_str).unwrap_or("").into(),
            label: s.get("label").and_then(Value::as_str).unwrap_or("").into(),
            created: s
                .get("created")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            last_seen: s
                .get("last_seen")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            current: s.get("current").and_then(Value::as_bool).unwrap_or(false),
        })
        .collect()
}

fn passkeys_from(data: &Value) -> Vec<PasskeyRow> {
    data.get("passkeys")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|p| PasskeyRow {
            id: p.get("id").and_then(Value::as_str).unwrap_or("").into(),
            created: p
                .get("created")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
        })
        .collect()
}
