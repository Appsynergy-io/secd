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
pub fn FieldRow(
    field: FieldView,
    #[prop(optional, default = Callback::new(|_| {}))] on_copy: Callback<String>,
) -> impl IntoView {
    let key = field.key.clone();
    let copy_key = field.key.clone();
    view! {
        <div class="secd-stack" data-field=key>
            <KeyVal label=field.key.clone() value=|| "••••••••" mono=true />
            <div class="secd-field-actions">
                <span class="asy-btn--primary"><span data-action="copy">
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        on:click=move |_| {
                            on_copy.run(copy_key.clone());
                        }
                    >
                        "Copy"
                    </Button>
                </span></span>
                <span data-action="show" data-hold="1">
                    <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm>"Show"</Button>
                </span>
            </div>
        </div>
    }
}

#[component]
pub fn Inspector(
    item: Option<SecretItem>,
    #[prop(optional, default = Callback::new(|_| {}))] on_copy: Callback<String>,
) -> impl IntoView {
    let title = match &item {
        None => "Secret".to_owned(),
        Some(i) => i.name.clone(),
    };
    view! {
        <div data-pane="inspector">
            <Card>
                <CardHeader>
                    <CardTitle><span class="mono">{title}</span></CardTitle>
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
                                .map(|f| view! { <FieldRow field=f on_copy=on_copy /> })
                                .collect_view()}
                        }.into_any(),
                    }}
                </CardContent>
            </Card>
        </div>
    }
}

#[component]
pub fn Sheet(
    item: SecretItem,
    #[prop(optional, default = Callback::new(|_| {}))] on_close: Callback<()>,
    #[prop(optional, default = Callback::new(|_| {}))] on_copy: Callback<String>,
) -> impl IntoView {
    view! {
        <div class="secd-overlay" data-sheet="open" data-pane="sheet">
            <div class="secd-modal">
                <Card>
                    <CardHeader>
                        <CardTitle><span class="mono">{item.name.clone()}</span></CardTitle>
                    </CardHeader>
                    <CardContent class="secd-stack">
                        {item
                            .fields
                            .into_iter()
                            .map(|f| view! { <FieldRow field=f on_copy=on_copy /> })
                            .collect_view()}
                        <Button
                            variant=ButtonVariant::Ghost
                            on:click=move |_| {
                                on_close.run(());
                            }
                        >
                            "Close"
                        </Button>
                    </CardContent>
                </Card>
            </div>
        </div>
    }
}

#[component]
pub fn AddWizard(
    #[prop(optional, default = Callback::new(|_| {}))] on_save: Callback<(String, String)>,
    #[prop(optional, default = Callback::new(|_| {}))] on_cancel: Callback<()>,
) -> impl IntoView {
    let provider = RwSignal::new(PROVIDERS[0].name.to_owned());
    let name = RwSignal::new(String::new());
    view! {
        <div class="secd-overlay" data-wizard="open">
            <div class="secd-modal">
                <Card>
                    <CardHeader>
                        <CardTitle>"Add a secret"</CardTitle>
                        <CardDescription>"Name a secret and pick a provider."</CardDescription>
                    </CardHeader>
                    <CardContent class="secd-stack">
                        <div>
                            <Label>"Provider"</Label>
                            <Select
                                value=Signal::derive(move || provider.get())
                                on_value_change=Callback::new(move |v: String| provider.set(v))
                            >
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
                            placeholder="kv/service/credential"
                            value=name
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                        <div class="secd-row">
                            <Button
                                variant=ButtonVariant::Primary
                                on:click=move |_| {
                                    on_save.run((provider.get(), name.get()));
                                }
                            >
                                "Save"
                            </Button>
                            <Button
                                variant=ButtonVariant::Ghost
                                on:click=move |_| {
                                    on_cancel.run(());
                                }
                            >
                                "Cancel"
                            </Button>
                        </div>
                    </CardContent>
                </Card>
            </div>
        </div>
    }
}

#[component]
pub fn RegisterPage(
    view: RegisterView,
    /// Live list filter; a page-local signal when absent (SSR render).
    #[prop(optional)]
    filter: Option<RwSignal<String>>,
    #[prop(optional, default = Callback::new(|_| {}))] on_select: Callback<String>,
    #[prop(optional, default = Callback::new(|_| {}))] on_add: Callback<()>,
    #[prop(optional, default = Callback::new(|_| {}))] on_close: Callback<()>,
    #[prop(optional, default = Callback::new(|_| {}))] on_copy: Callback<String>,
    #[prop(optional, default = Callback::new(|_| {}))] on_save: Callback<(String, String)>,
) -> impl IntoView {
    let layout = view.layout();
    let filter = filter.unwrap_or_else(|| RwSignal::new(String::new()));
    let selected = view
        .selected
        .as_ref()
        .and_then(|n| view.items.iter().find(|i| i.name == *n).cloned());
    let show_inspector = layout.shows_inspector();
    let show_sheet = layout.uses_sheet() && selected.is_some();
    let items = view.items.clone();
    let has_items = !items.is_empty();
    view! {
        <section data-page="register" data-layout=layout.as_str()>
            <PageHead
                title=ViewFn::from(|| "Register")
                subtitle=ViewFn::from(|| "Secrets stored on this LAN. Copy is the default.")
                actions=ViewFn::from(move || view! {
                    <Button
                        variant=ButtonVariant::Primary
                        on:click=move |_| {
                            on_add.run(());
                        }
                    >
                        "Add"
                    </Button>
                })
            />
            <div class="secd-grid-2">
                <div data-pane="list" class="secd-stack">
                    {has_items.then(|| view! {
                        <Input
                            class="mono"
                            placeholder="Filter names…"
                            value=filter
                            on:input=move |ev| filter.set(event_target_value(&ev))
                        />
                    })}
                    <Card>
                        <CardContent class="secd-list">
                            {if !has_items {
                                view! {
                                    <EmptyState
                                        title="No secrets yet"
                                        body=|| "Add a name to start the register."
                                    />
                                }.into_any()
                            } else {
                                view! {
                                    {move || items
                                        .iter()
                                        .filter(|item| {
                                            let f = filter.get();
                                            f.is_empty() || item.name.contains(f.as_str())
                                        })
                                        .map(|item| {
                                            let name = item.name.clone();
                                            let name_cb = name.clone();
                                            let name_txt = name.clone();
                                            view! {
                                                <div data-name=name>
                                                    <Button
                                                        variant=ButtonVariant::Ghost
                                                        class="secd-btn-block mono secd-name"
                                                        on:click=move |_| {
                                                            on_select.run(name_cb.clone());
                                                        }
                                                    >
                                                        {name_txt}
                                                    </Button>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                }.into_any()
                            }}
                        </CardContent>
                    </Card>
                </div>
                {show_inspector.then(|| view! { <Inspector item=selected.clone() on_copy=on_copy /> })}
            </div>
            {show_sheet.then(|| {
                let item = selected
                    .clone()
                    .expect("invariant: sheet only when a secret is selected");
                view! { <Sheet item=item on_close=on_close on_copy=on_copy /> }
            })}
            {view.wizard_open.then(|| view! { <AddWizard on_save=on_save on_cancel=on_close /> })}
        </section>
    }
}

pub fn render_register(view: &RegisterView) -> String {
    crate::html(|| view! { <RegisterPage view=view.clone() /> })
}
