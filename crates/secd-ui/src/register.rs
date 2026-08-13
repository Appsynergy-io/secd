//! Vault register: list|inspector at ≥900, list→sheet below. Copy is the default action.

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
    view! {
        <div class="field" data-field=field.key.clone()>
            <span class="name">{field.key.clone()}</span>
            <button type="button" class="primary" data-action="copy">
                "Copy"
            </button>
            <button type="button" class="ghost" data-action="show" data-hold="1">
                "Show"
            </button>
        </div>
    }
}

#[component]
pub fn Inspector(item: Option<SecretItem>) -> impl IntoView {
    view! {
        <aside class="inspector" data-pane="inspector">
            {match item {
                None => view! { <p class="muted">"Select a secret"</p> }.into_any(),
                Some(item) => view! {
                    <h2 class="name">{item.name.clone()}</h2>
                    {item
                        .fields
                        .into_iter()
                        .map(|f| view! { <FieldRow field=f /> })
                        .collect_view()}
                }
                .into_any(),
            }}
        </aside>
    }
}

#[component]
pub fn Sheet(item: SecretItem) -> impl IntoView {
    view! {
        <div class="sheet" data-sheet="open" data-pane="sheet">
            <h2 class="name">{item.name.clone()}</h2>
            {item
                .fields
                .into_iter()
                .map(|f| view! { <FieldRow field=f /> })
                .collect_view()}
            <button type="button" class="ghost" data-action="close-sheet">
                "Close"
            </button>
        </div>
    }
}

#[component]
pub fn AddWizard() -> impl IntoView {
    view! {
        <section class="wizard" data-wizard="open">
            <h1>"Add"</h1>
            <label class="field-label">
                "Provider"
                <select name="provider">
                    {PROVIDERS
                        .iter()
                        .map(|p| {
                            view! {
                                <option value=p.name>{p.title}</option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <label class="field-label">
                "Name"
                <input type="text" name="secret_name" class="name" autocomplete="off" />
            </label>
            <div data-wizard-step="fields"></div>
            <button type="button" class="primary" data-action="wizard-save">
                "Save"
            </button>
            <button type="button" class="ghost" data-action="wizard-cancel">
                "Cancel"
            </button>
        </section>
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
            <header class="toolbar">
                <h1>"Register"</h1>
                <button type="button" class="primary" data-action="add">
                    "Add"
                </button>
            </header>
            <div class="workspace">
                <div class="list" data-pane="list">
                    <ul>
                        {view
                            .items
                            .iter()
                            .map(|item| {
                                let name = item.name.clone();
                                let name_btn = name.clone();
                                let name_txt = name.clone();
                                view! {
                                    <li data-name=name>
                                        <button
                                            type="button"
                                            class="row name"
                                            data-action="select"
                                            data-name=name_btn
                                        >
                                            {name_txt}
                                        </button>
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
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
