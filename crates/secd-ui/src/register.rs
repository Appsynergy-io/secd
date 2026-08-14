//! Vault register: list|inspector at ≥900, list→sheet below. Copy is the default action.

use appsy_ui::prelude::*;
use leptos::prelude::*;

use crate::layout::{layout_mode, LayoutMode};
use crate::providers::PROVIDERS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldAction {
    Copy,
    Show,
}

pub fn primary_field_action() -> FieldAction {
    FieldAction::Copy
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldView {
    pub key: String,
    pub secret: bool,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretItem {
    pub name: String,
    pub fields: Vec<FieldView>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegisterView {
    pub width_px: u32,
    pub items: Vec<SecretItem>,
    pub selected: Option<String>,
    pub wizard_open: bool,
}

impl RegisterView {
    pub fn layout(&self) -> LayoutMode {
        layout_mode(self.width_px)
    }
}

#[component]
pub fn FieldRow(field: FieldView) -> impl IntoView {
    let key = field.key.clone();
    view! {
        <div class="secd-stack" data-field=key>
            <KeyVal label=field.key.clone() value=|| "••••••••" mono=true />
            <div class="secd-field-actions">
                <span class="asy-btn--primary"><span data-action="copy">
                    <Button variant=ButtonVariant::Primary size=ButtonSize::Sm>"Copy"</Button>
                </span></span>
                <span data-action="show" data-hold="1">
                    <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm>"Show"</Button>
                </span>
            </div>
        </div>
    }
}

#[component]
pub fn Inspector(item: Option<SecretItem>) -> impl IntoView {
    let title = match &item {
        None => "Secret".to_owned(),
        Some(i) => i.name.clone(),
    };
    view! {
        <div data-pane="inspector">
            <Card>
                <CardHeader>
                    <CardTitle>{title}</CardTitle>
                    <CardDescription>"Field values stay on this machine."</CardDescription>
                </CardHeader>
                <CardContent class="secd-stack">
                    {match item {
                        None => view! {
                            <EmptyState
                                title="Select a secret"
                                body=|| "Choose a name from the list."
                            />
                        }.into_any(),
                        Some(item) => view! {
                            {item
                                .fields
                                .into_iter()
                                .map(|f| view! { <FieldRow field=f /> })
                                .collect_view()}
                        }.into_any(),
                    }}
                </CardContent>
            </Card>
        </div>
    }
}

#[component]
pub fn Sheet(item: SecretItem) -> impl IntoView {
    view! {
        <div class="sheet" data-sheet="open" data-pane="sheet">
            <Card>
                <CardHeader>
                    <CardTitle>{item.name.clone()}</CardTitle>
                </CardHeader>
                <CardContent class="secd-stack">
                    {item
                        .fields
                        .into_iter()
                        .map(|f| view! { <FieldRow field=f /> })
                        .collect_view()}
                    <span data-action="close-sheet">
                        <Button variant=ButtonVariant::Ghost>"Close"</Button>
                    </span>
                </CardContent>
            </Card>
        </div>
    }
}

#[component]
pub fn AddWizard() -> impl IntoView {
    view! {
        <div class="wizard" data-wizard="open">
            <Card>
                <CardHeader>
                    <CardTitle>"Add"</CardTitle>
                    <CardDescription>"Name a secret and pick a provider."</CardDescription>
                </CardHeader>
                <CardContent class="secd-stack">
                    <div>
                        <Label>"Provider"</Label>
                        <Select default_value=PROVIDERS[0].name.to_owned()>
                            <SelectTrigger>
                                <SelectValue placeholder="Provider" />
                            </SelectTrigger>
                            <SelectContent>
                                {PROVIDERS
                                    .iter()
                                    .map(|p| {
                                        view! {
                                            <SelectItem value=p.name>{p.title}</SelectItem>
                                        }
                                    })
                                    .collect_view()}
                            </SelectContent>
                        </Select>
                    </div>
                    <LabeledInput
                        id="secret_name"
                        label="Name"
                        autocomplete="off"
                        mono=true
                    />
                    <div data-wizard-step="fields"></div>
                    <div class="secd-row">
                        <span class="asy-btn--primary" data-action="wizard-save">
                            <Button variant=ButtonVariant::Primary>"Save"</Button>
                        </span>
                        <span data-action="wizard-cancel">
                            <Button variant=ButtonVariant::Ghost>"Cancel"</Button>
                        </span>
                    </div>
                </CardContent>
            </Card>
        </div>
    }
}

#[component]
pub fn RegisterPage(view: RegisterView) -> impl IntoView {
    let layout = view.layout();
    let selected = view
        .selected
        .as_ref()
        .and_then(|n| view.items.iter().find(|i| i.name == *n).cloned());
    let show_inspector = layout.shows_inspector();
    let show_sheet = layout.uses_sheet() && selected.is_some();
    view! {
        <section data-page="register" data-layout=layout.as_str()>
            <PageHead
                title=ViewFn::from(|| "Register")
                subtitle=ViewFn::from(|| "Secrets stored on this LAN. Copy is the default.")
                actions=ViewFn::from(|| view! {
                    <span class="asy-btn--primary" data-action="add">
                        <Button variant=ButtonVariant::Primary>"Add"</Button>
                    </span>
                })
            />
            <div class="secd-grid-2">
                <div data-pane="list">
                    <Card>
                        <CardContent class="secd-list">
                            {if view.items.is_empty() {
                                view! {
                                    <EmptyState
                                        title="No secrets yet"
                                        body=|| "Add a name to start the register."
                                    />
                                }.into_any()
                            } else {
                                view! {
                                    {view
                                        .items
                                        .iter()
                                        .map(|item| {
                                            let name = item.name.clone();
                                            let name_btn = name.clone();
                                            let name_txt = name.clone();
                                            view! {
                                                <div data-name=name>
                                                    <span data-action="select" data-name=name_btn>
                                                        <Button variant=ButtonVariant::Ghost class="secd-btn-block">
                                                            {name_txt}
                                                        </Button>
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                }.into_any()
                            }}
                        </CardContent>
                    </Card>
                </div>
                {show_inspector.then(|| view! { <Inspector item=selected.clone() /> })}
            </div>
            {show_sheet.then(|| {
                let item = selected
                    .clone()
                    .expect("invariant: sheet only when a secret is selected");
                view! { <Sheet item=item /> }
            })}
            {view.wizard_open.then(|| view! { <AddWizard /> })}
        </section>
    }
}

pub fn render_register(view: &RegisterView) -> String {
    crate::html(|| view! { <RegisterPage view=view.clone() /> })
}
