//! Web console. Humans see values here; agents never do. All Rust.

#![recursion_limit = "512"]

pub mod account;
pub mod activity;
pub mod api;
pub mod app;
pub mod crypto;
pub mod css;
pub mod gate;
pub mod layout;
pub mod providers;
pub mod register;
pub mod remember;
pub mod tokens;

#[cfg(target_arch = "wasm32")]
pub mod client;
#[cfg(target_arch = "wasm32")]
pub mod live;

use leptos::prelude::*;

pub use account::{
    passkey_delete_path, remove_passkey_enabled, render_account, session_revoke_path, AccountView,
    PasskeyRow, SessionRow,
};
pub use activity::{render_activity, ActivityView, AuditRow};
pub use api::session_revoke_delete;
pub use app::{render_console, App, ConsoleState, Screen};
pub use crypto::{check_name, email_ok, password_ok};
pub use gate::{
    render_device, render_gate, resolve_gate, AuthMethod, GateKind, GateQuery, GateView,
    SessionInfo,
};
pub use layout::{layout_mode, LayoutMode};
pub use register::{primary_field_action, render_register, FieldAction, RegisterView, SecretItem};
pub use remember::{parse_remembered, remember_is_fresh_unix, Remembered};
pub use tokens::{BREAKPOINT_PX, EMAIL_AUTOCOMPLETE, LAST_KEY};

pub fn html<F, V>(f: F) -> String
where
    F: FnOnce() -> V,
    V: IntoView,
{
    let owner = Owner::new();
    owner.with(|| f().into_view().to_html())
}

#[cfg(target_arch = "wasm32")]
pub fn inject_styles() {
    let document = web_sys::window()
        .expect("invariant: window exists")
        .document()
        .expect("invariant: document exists");
    let head = document.head().expect("invariant: head exists");
    let sheet = document
        .create_element("style")
        .expect("invariant: style element");
    sheet.set_text_content(Some(&appsy_ui::STYLESHEET));
    head.append_child(&sheet)
        .expect("invariant: head accepts a style");
    let app = document
        .create_element("style")
        .expect("invariant: style element");
    app.set_text_content(Some(css::APP_CSS));
    head.append_child(&app)
        .expect("invariant: head accepts a style");
}
