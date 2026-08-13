//! Account screen: sessions + Revoke, passkeys + Add/Remove.

use leptos::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRow {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub created: String,
    pub last_seen: String,
    pub current: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasskeyRow {
    pub id: String,
    pub created: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountView {
    pub email: String,
    pub sessions: Vec<SessionRow>,
    pub passkeys: Vec<PasskeyRow>,
    pub has_password: bool,
}

pub fn remove_passkey_enabled(passkey_count: usize, has_password: bool) -> bool {
    !(passkey_count <= 1 && !has_password)
}

impl AccountView {
    pub fn remove_enabled(&self) -> bool {
        remove_passkey_enabled(self.passkeys.len(), self.has_password)
    }
}

pub fn session_revoke_path(id: &str) -> String {
    format!("/api/v1/sessions/{id}")
}

pub fn passkey_delete_path(id: &str) -> String {
    format!("/api/auth/passkeys/{id}")
}

#[component]
pub fn AccountPage(view: AccountView) -> impl IntoView {
    let remove_ok = view.remove_enabled();
    view! {
        <section data-page="account">
            <h1>"Account"</h1>
            <p class="muted">{view.email.clone()}</p>
            <h2>"Sessions"</h2>
            <table data-list="sessions">
                <thead>
                    <tr>
                        <th>"Label"</th>
                        <th>"Kind"</th>
                        <th>"Created"</th>
                        <th>"Last seen"</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    {view
                        .sessions
                        .iter()
                        .map(|s| {
                            let id = s.id.clone();
                            view! {
                                <tr data-session-id=id data-current=s.current.then_some("true")>
                                    <td class="name">{s.label.clone()}</td>
                                    <td>{s.kind.clone()}</td>
                                    <td>{s.created.clone()}</td>
                                    <td>{s.last_seen.clone()}</td>
                                    <td>
                                        <button
                                            type="button"
                                            class="danger"
                                            data-action="revoke"
                                            data-session-id=s.id.clone()
                                        >
                                            "Revoke"
                                        </button>
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
            <h2>"Passkeys"</h2>
            <ul data-list="passkeys">
                {view
                    .passkeys
                    .iter()
                    .map(|p| {
                        view! {
                            <li data-passkey-id=p.id.clone()>
                                <span class="name">{p.id.clone()}</span>
                                <span class="muted">{p.created.clone()}</span>
                                <button
                                    type="button"
                                    class="danger"
                                    data-action="remove"
                                    data-passkey-id=p.id.clone()
                                    disabled=!remove_ok
                                >
                                    "Remove"
                                </button>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
            <button type="button" class="primary" data-action="add-passkey">
                "Add passkey"
            </button>
        </section>
    }
}

pub fn render_account(view: &AccountView) -> String {
    crate::html(|| view! { <AccountPage view=view.clone() /> })
}
