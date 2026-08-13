//! Web console. Humans see values here; agents never do.

pub mod account;
pub mod activity;
pub mod api;
pub mod app;
pub mod crypto;
pub mod gate;
pub mod layout;
pub mod providers;
pub mod register;
pub mod remember;
pub mod tokens;

use leptos::prelude::*;

pub use account::{
    passkey_delete_path, remove_passkey_enabled, render_account, session_revoke_path, AccountView,
    PasskeyRow, SessionRow,
};
pub use activity::{render_activity, ActivityView, AuditRow};
pub use app::{render_console, App, Chrome, ConsoleState, Screen};
pub use crypto::{check_name, email_ok, password_ok};
pub use gate::{
    render_device, render_gate, resolve_gate, AuthMethod, GateKind, GateQuery, GateView,
    SessionInfo,
};
pub use layout::{layout_mode, LayoutMode};
pub use register::{primary_field_action, render_register, FieldAction, RegisterView, SecretItem};
pub use remember::{parse_remembered, remember_is_fresh, remember_is_fresh_unix, Remembered};
pub use tokens::{
    BREAKPOINT_PX, CANVAS, COBALT, DANGER, EMAIL_AUTOCOMPLETE, INK, LAST_KEY, LINE, MUTED, OK,
    REMEMBER_DAYS, SURFACE,
};

pub const INDEX_HTML: &str = include_str!("../index.html");
pub const APP_CSS: &str = include_str!("style.css");
pub const APP_JS: &str = include_str!("app.js");

pub fn html<F, V>(f: F) -> String
where
    F: FnOnce() -> V,
    V: IntoView,
{
    let owner = Owner::new();
    owner.with(|| f().into_view().to_html())
}

pub fn render_app() -> String {
    html(|| view! { <App /> })
}

pub fn index_html() -> String {
    INDEX_HTML.replace("__LEPTOS__", &render_app())
}

#[cfg(all(target_arch = "wasm32", feature = "hydrate"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn hydrate() {
    leptos::mount::hydrate_body(App);
}

#[cfg(all(target_arch = "wasm32", feature = "csr"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn mount() {
    leptos::mount::mount_to_body(App);
}
