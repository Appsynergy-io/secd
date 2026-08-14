//! Account screen: sessions + Revoke, passkeys + Add/Remove.

use appsy_ui::prelude::*;
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
    crate::api::session_revoke_path(id)
}

pub fn passkey_delete_path(id: &str) -> String {
    crate::api::passkey_delete_path(id)
}

#[component]
pub fn AccountPage(view: AccountView) -> impl IntoView {
    let remove_ok = view.remove_enabled();
    let email = view.email.clone();
    view! {
        <section data-page="account">
            <PageHead
                title=ViewFn::from(|| "Account")
                subtitle=ViewFn::from(move || email.clone())
            />
            <div class="secd-grid-2">
                <Card>
                    <CardHeader>
                        <CardTitle>"Sessions"</CardTitle>
                        <CardDescription>"Revoke a row to end that session."</CardDescription>
                    </CardHeader>
                    <CardContent class="secd-list">
                        <div data-list="sessions" class="secd-list">
                        {view
                            .sessions
                            .iter()
                            .map(|s| {
                                let id = s.id.clone();
                                let current = s.current.then_some("true");
                                view! {
                                    <div data-session-id=id.clone() data-current=current>
                                        <KeyVal
                                            label=s.label.clone()
                                            value={
                                                let kind = s.kind.clone();
                                                let last = s.last_seen.clone();
                                                move || format!("{kind} · {last}")
                                            }
                                        />
                                        <span class="asy-btn--danger" data-action="revoke" data-session-id=s.id.clone()>
                                            <Button variant=ButtonVariant::Danger size=ButtonSize::Sm>
                                                "Revoke"
                                            </Button>
                                        </span>
                                    </div>
                                }
                            })
                            .collect_view()}
                        </div>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader>
                        <CardTitle>"Passkeys"</CardTitle>
                        <CardDescription>"At least one factor must remain."</CardDescription>
                    </CardHeader>
                    <CardContent class="secd-list">
                        <div data-list="passkeys" class="secd-list">
                        {view
                            .passkeys
                            .iter()
                            .map(|p| {
                                view! {
                                    <div data-passkey-id=p.id.clone()>
                                        <KeyVal
                                            label=p.id.clone()
                                            value={
                                                let created = p.created.clone();
                                                move || created.clone()
                                            }
                                            mono=true
                                        />
                                        <span data-action="remove" data-passkey-id=p.id.clone()>
                                            <Button
                                                variant=ButtonVariant::Danger
                                                size=ButtonSize::Sm
                                                disabled=Signal::from(!remove_ok)
                                            >
                                                "Remove"
                                            </Button>
                                        </span>
                                    </div>
                                }
                            })
                            .collect_view()}
                        <span class="asy-btn--primary" data-action="add-passkey">
                            <Button variant=ButtonVariant::Primary>"Add passkey"</Button>
                        </span>
                        </div>
                    </CardContent>
                </Card>
            </div>
        </section>
    }
}

pub fn render_account(view: &AccountView) -> String {
    crate::html(|| view! { <AccountPage view=view.clone() /> })
}
