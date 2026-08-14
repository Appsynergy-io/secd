//! FilterSkuEditorDialog — port of `platform/dns-filter-dialogs.tsx`
//! (T10). Create / edit a DNS filter-list SKU: `sku` `None` → create
//! blank; `Some` → patch in place (slug immutable on edit).
//!
//! Props/callbacks split: `useCreateDnsFilterList` /
//! `useUpdateDnsFilterList` → `on_create(SkuCreate)` /
//! `on_update(SkuUpdate)` + the shared `busy`/`error` the reference
//! derives from whichever mutation ran (the sku id and close-on-success
//! stay with the consumer). Blank `description`/`source_url` clear to
//! `None`, the double-option semantics.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use leptos::prelude::*;

pub const DNSF_COL: &str = "asy-dnsf__col";
pub const DNSF_GRID2: &str = "asy-dnsf__grid2";
pub const DNSF_LABEL: &str = "asy-dnsf__label";
pub const DNSF_FIELD: &str = "asy-dnsf__field";
pub const DNSF_CHECK_ROW: &str = "asy-dnsf__check-row";
pub const DNSF_ERR: &str = "asy-dnsf__err";

/// A filter-list SKU — the fields the editor renders/prefills.
#[derive(Clone, PartialEq, Debug)]
pub struct PlatformDnsFilterList {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub source_url: Option<String>,
    pub is_active: bool,
}

/// `POST /platform/dns/filter-lists` payload.
#[derive(Clone, PartialEq, Debug)]
pub struct SkuCreate {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub source_url: Option<String>,
    pub is_active: bool,
}

/// `PATCH` payload — slug immutable; the sku id stays with the consumer.
#[derive(Clone, PartialEq, Debug)]
pub struct SkuUpdate {
    pub name: String,
    pub description: Option<String>,
    pub source_url: Option<String>,
    pub is_active: bool,
}

/// Create / edit a DNS filter-list SKU.
#[component]
pub fn FilterSkuEditorDialog(
    /// `None` → create blank; `Some` → edit (prefilled, slug locked).
    sku: Option<PlatformDnsFilterList>,
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    #[prop(into)] on_create: Callback<SkuCreate>,
    #[prop(into)] on_update: Callback<SkuUpdate>,
    /// Either mutation in flight.
    #[prop(optional, into)]
    busy: Signal<bool>,
    #[prop(optional, into)] error: Signal<Option<String>>,
) -> impl IntoView {
    let editing = sku.is_some();
    let sku = StoredValue::new(sku);
    let name = RwSignal::new(String::new());
    let slug = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let source_url = RwSignal::new(String::new());
    let is_active = RwSignal::new(true);

    let reset = move || {
        sku.with_value(|sku| {
            name.set(sku.as_ref().map(|s| s.name.clone()).unwrap_or_default());
            slug.set(sku.as_ref().map(|s| s.slug.clone()).unwrap_or_default());
            description.set(
                sku.as_ref().and_then(|s| s.description.clone()).unwrap_or_default(),
            );
            source_url.set(
                sku.as_ref().and_then(|s| s.source_url.clone()).unwrap_or_default(),
            );
            is_active.set(sku.as_ref().map(|s| s.is_active).unwrap_or(true));
        })
    };
    reset();
    // The reference's `[open, sku]` reset effect.
    Effect::new(move |_| {
        if open.get() {
            reset();
        }
    });

    let valid =
        move || !name.get().trim().is_empty() && !slug.get().trim().is_empty();
    let submit = move |_| {
        if !valid() {
            return;
        }
        let d = description.get_untracked().trim().to_owned();
        let u = source_url.get_untracked().trim().to_owned();
        let d = (!d.is_empty()).then_some(d);
        let u = (!u.is_empty()).then_some(u);
        if editing {
            on_update.run(SkuUpdate {
                name: name.get_untracked().trim().to_owned(),
                description: d,
                source_url: u,
                is_active: is_active.get_untracked(),
            });
        } else {
            on_create.run(SkuCreate {
                name: name.get_untracked().trim().to_owned(),
                slug: slug.get_untracked().trim().to_owned(),
                description: d,
                source_url: u,
                is_active: is_active.get_untracked(),
            });
        }
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>
                        {if editing { "Edit filter SKU" } else { "New filter SKU" }}
                    </DialogTitle>
                    <DialogDescription>
                        "A filter SKU is a named blocklist customers can attach to a DNS assignment. Syncs are append-only; clearing the source URL stops future syncs."
                    </DialogDescription>
                </DialogHeader>
                <div class=DNSF_COL>
                    <div class=DNSF_GRID2>
                        <label class=DNSF_LABEL>
                            "Name"
                            <input
                                class=DNSF_FIELD
                                value=move || name.get()
                                prop:value=move || name.get()
                                on:input=move |ev| name.set(event_target_value(&ev))
                            />
                        </label>
                        <label class=DNSF_LABEL>
                            "Slug"
                            <input
                                class=DNSF_FIELD
                                value=move || slug.get()
                                prop:value=move || slug.get()
                                disabled=editing
                                on:input=move |ev| slug.set(event_target_value(&ev))
                            />
                        </label>
                    </div>
                    <label class=DNSF_LABEL>
                        "Description"
                        <input
                            class=DNSF_FIELD
                            value=move || description.get()
                            prop:value=move || description.get()
                            on:input=move |ev| description.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=DNSF_LABEL>
                        "Source feed URL"
                        <input
                            class=DNSF_FIELD
                            inputmode="url"
                            placeholder="https://…"
                            value=move || source_url.get()
                            prop:value=move || source_url.get()
                            on:input=move |ev| source_url.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=DNSF_CHECK_ROW>
                        <input
                            type="checkbox"
                            checked=move || is_active.get()
                            prop:checked=move || is_active.get()
                            on:change=move |ev| is_active.set(event_target_checked(&ev))
                        />
                        "Active (selectable by customers)"
                    </label>
                    {move || {
                        error
                            .get()
                            .map(|msg| {
                                view! {
                                    <p class=DNSF_ERR style="color: var(--color-danger)">
                                        "Could not save: "
                                        {msg}
                                    </p>
                                }
                            })
                    }}
                </div>
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || busy.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || (!valid() || busy.get()).then_some("")
                        on:click=submit
                    >
                        {move || {
                            if busy.get() {
                                "Saving…"
                            } else if editing {
                                "Save changes"
                            } else {
                                "Create SKU"
                            }
                        }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{col}{{display:flex;flex-direction:column;gap:.75rem}}",
            ".{grid2}{{display:grid;grid-template-columns:1fr;gap:.75rem}}",
            "@media (width >= 40rem){{.{grid2}{{",
            "grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            ".{label}{{display:flex;flex-direction:column;gap:.25rem;",
            "font-size:11.5px;font-weight:500;color:var(--color-text-muted)}}",
            ".{field}{{height:2rem;width:100%;border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);padding-inline:.5rem;",
            "font-size:12.5px;color:var(--color-text)}}",
            ".{check_row}{{display:flex;align-items:center;gap:.5rem;",
            "font-size:12.5px}}",
            ".{err}{{font-size:12px}}",
        ),
        col = DNSF_COL,
        grid2 = DNSF_GRID2,
        label = DNSF_LABEL,
        field = DNSF_FIELD,
        check_row = DNSF_CHECK_ROW,
        err = DNSF_ERR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [DNSF_COL, DNSF_GRID2, DNSF_LABEL, DNSF_FIELD, DNSF_CHECK_ROW, DNSF_ERR] {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
