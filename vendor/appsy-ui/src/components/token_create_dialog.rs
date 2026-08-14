//! TokenCreateDialog — port of `team/token-create-dialog.tsx` (T12).
//! Create an API token scoped to a subset of the caller's own actions;
//! area master toggles delegate `<area>:*` wildcards under the 32-entry
//! cap; a one-time SecretReveal pane shows the minted plaintext.
//!
//! Props/callbacks split: `useOrgEffectiveActions` → `actions` +
//! `actions_loading`/`actions_error`; `useCreateOrgToken` →
//! `on_create(TokenCreateRequest)` + `creating`/`create_error`, with the
//! mutation result flowing back in as `secret` (the consumer sets it on
//! success — the dialog switches to the reveal pane while `Some`);
//! `orgId` presence → `org_selected` (gates the trigger and the loading
//! copy). `on_close` fires after any dismissal, where the consumer
//! clears `secret` and resets its mutation (the reference's `reset()`
//! tail); the dialog's own fields reset internally.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::checkbox::Checkbox;
use crate::components::dialog::{
    Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
    DialogTrigger,
};
use crate::components::input::Input;
use crate::components::label::Label;
use crate::icons::{Icon, RI_CHECK_LINE, RI_FILE_COPY_LINE, RI_KEY_2_LINE};
use leptos::prelude::*;

pub const TOKC_CONTENT: &str = "asy-tokc__content";
pub const TOKC_BODY: &str = "asy-tokc__body";
pub const TOKC_FIELD: &str = "asy-tokc__field";
pub const TOKC_SCOPE_HEAD: &str = "asy-tokc__scope-head";
pub const TOKC_COUNT: &str = "asy-tokc__count";
pub const TOKC_LIST: &str = "asy-tokc__list";
pub const TOKC_GROUP: &str = "asy-tokc__group";
pub const TOKC_AREA: &str = "asy-tokc__area";
pub const TOKC_AREA_NAME: &str = "asy-tokc__area-name";
pub const TOKC_WILDCARD: &str = "asy-tokc__wildcard";
pub const TOKC_ACTIONS: &str = "asy-tokc__actions";
pub const TOKC_ACTION: &str = "asy-tokc__action";
pub const TOKC_SLUG: &str = "asy-tokc__slug";
pub const TOKC_NOTE: &str = "asy-tokc__note";
pub const TOKC_EMPTY: &str = "asy-tokc__empty";
pub const TOKC_CAP: &str = "asy-tokc__cap";
pub const TOKC_ERR: &str = "asy-tokc__err";
pub const TOKC_REVEAL: &str = "asy-tokc__reveal";
pub const TOKC_SECRET: &str = "asy-tokc__secret";
pub const TOKC_KEY_ICO: &str = "asy-tokc__key-ico";
pub const TOKC_PLAINTEXT: &str = "asy-tokc__plaintext";
pub const TOKC_TOKEN_ID: &str = "asy-tokc__token-id";
pub const TOKC_TRIGGER_ICO: &str = "asy-tokc__trigger-ico";
pub const TOKC_COPY_ICO: &str = "asy-tokc__copy-ico";

/// Maximum `granted_actions` entries the backend accepts on one token.
pub const MAX_ACTIONS: usize = 32;

/// One grantable action from `GET /iam/orgs/{org_id}/effective-actions`.
#[derive(Clone, PartialEq, Debug)]
pub struct GrantableAction {
    /// The action slug (e.g. `vpn:tunnel:create`).
    pub name: String,
    pub description: Option<String>,
}

/// A freshly minted token — shown exactly once.
#[derive(Clone, PartialEq, Debug)]
pub struct TokenSecret {
    pub id: String,
    pub plaintext: String,
}

/// `POST /iam/orgs/{org_id}/tokens` payload (the org id stays with the
/// consumer).
#[derive(Clone, PartialEq, Debug)]
pub struct TokenCreateRequest {
    pub name: String,
    /// Concrete slugs and/or `<area>:*` wildcards, selection order.
    pub granted_actions: Vec<String>,
    /// UTC RFC3339 from the local datetime field; `None` when blank.
    pub expires_at: Option<String>,
}

/// The area segment of an action slug (`vpn:tunnel:create` → `vpn`).
pub fn area_of(slug: &str) -> &str {
    slug.split_once(':').map_or(slug, |(area, _)| area)
}

/// Title-case an area slug for display (`static_ips` → `Static ips`).
pub fn area_label(area: &str) -> String {
    let spaced = area.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

/// The `<area>:*` wildcard string a group's master toggle delegates.
pub fn wildcard_of(area: &str) -> String {
    format!("{area}:*")
}

/// One area's slice of the grantable catalog.
#[derive(Clone, PartialEq, Debug)]
pub struct AreaGroup {
    pub area: String,
    pub wildcard: String,
    pub actions: Vec<GrantableAction>,
}

/// Group the grantable actions by area, sorted by area then slug.
pub fn group_by_area(actions: &[GrantableAction]) -> Vec<AreaGroup> {
    // First-seen insertion order, then sorted — the reference's Map
    // round-trip.
    let mut order: Vec<String> = Vec::new();
    let mut by_area: std::collections::HashMap<String, Vec<GrantableAction>> =
        std::collections::HashMap::new();
    for a in actions {
        let area = area_of(&a.name).to_owned();
        if !by_area.contains_key(&area) {
            order.push(area.clone());
        }
        by_area.entry(area).or_default().push(a.clone());
    }
    let mut groups: Vec<AreaGroup> = order
        .into_iter()
        .map(|area| {
            let mut list = by_area.remove(&area).expect("invariant: area recorded on insert");
            list.sort_by(|x, y| x.name.cmp(&y.name));
            AreaGroup { wildcard: wildcard_of(&area), area, actions: list }
        })
        .collect();
    groups.sort_by(|x, y| x.area.cmp(&y.area));
    groups
}

/// The reference's filter: actions whose slug contains the query
/// (case-insensitive) survive; a group also survives wholesale when its
/// area contains the query.
pub fn filter_groups(groups: &[AreaGroup], query: &str) -> Vec<AreaGroup> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return groups.to_vec();
    }
    groups
        .iter()
        .map(|g| AreaGroup {
            area: g.area.clone(),
            wildcard: g.wildcard.clone(),
            actions: g
                .actions
                .iter()
                .filter(|a| a.name.to_lowercase().contains(&q) || g.area.contains(&q))
                .cloned()
                .collect(),
        })
        .filter(|g| !g.actions.is_empty() || g.area.contains(&q))
        .collect()
}

/// Local `datetime-local` value → UTC RFC3339 via the browser's own
/// Date (TZ semantics identical to the reference); blank → `None`.
fn expires_iso(local: &str) -> Option<String> {
    if local.is_empty() {
        return None;
    }
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(local));
        if d.get_time().is_nan() {
            return None;
        }
        Some(String::from(d.to_iso_string()))
    }
    #[cfg(not(any(feature = "csr", feature = "hydrate")))]
    {
        // Submission is a client-side event; the server never converts.
        Some(local.to_owned())
    }
}

/// Create an API token scoped to a subset of the caller's own actions.
#[component]
pub fn TokenCreateDialog(
    /// The grantable catalog (`None` while loading).
    #[prop(into)]
    actions: Signal<Option<Vec<GrantableAction>>>,
    #[prop(optional, into)] actions_error: Signal<Option<String>>,
    /// Whether an org id is known — gates the trigger and loading copy.
    org_selected: bool,
    #[prop(into)] on_create: Callback<TokenCreateRequest>,
    #[prop(optional, into)] creating: Signal<bool>,
    #[prop(optional, into)] create_error: Signal<Option<String>>,
    /// The minted token from the consumer's mutation success — the
    /// dialog shows the one-time reveal pane while `Some`.
    #[prop(into)]
    secret: Signal<Option<TokenSecret>>,
    /// Fires after any dismissal: clear `secret` and reset the mutation.
    #[prop(into)]
    on_close: Callback<()>,
) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let query = RwSignal::new(String::new());
    let expires_local = RwSignal::new(String::new());
    // Selection-ordered (JS `Set` preserves insertion order).
    let selected = RwSignal::new(Vec::<String>::new());

    let reset = move || {
        name.set(String::new());
        query.set(String::new());
        expires_local.set(String::new());
        selected.set(Vec::new());
        on_close.run(());
    };

    let count = move || selected.with(|s| s.len());
    let too_many = move || count() > MAX_ACTIONS;

    let toggle_wildcard = move |group: &AreaGroup, on: bool| {
        let wildcard = group.wildcard.clone();
        let names: Vec<String> = group.actions.iter().map(|a| a.name.clone()).collect();
        selected.update(|sel| {
            // The wildcard subsumes the area; drop any concrete picks.
            sel.retain(|e| !names.contains(e));
            if on {
                if !sel.contains(&wildcard) {
                    sel.push(wildcard);
                }
            } else {
                sel.retain(|e| e != &wildcard);
            }
        });
    };
    let toggle_action = move |group: &AreaGroup, slug: &str, on: bool| {
        let wildcard = group.wildcard.clone();
        let slug = slug.to_owned();
        selected.update(|sel| {
            // A concrete pick is mutually exclusive with the wildcard.
            sel.retain(|e| e != &wildcard);
            if on {
                if !sel.contains(&slug) {
                    sel.push(slug);
                }
            } else {
                sel.retain(|e| e != &slug);
            }
        });
    };

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        on_create.run(TokenCreateRequest {
            name: name.get_untracked().trim().to_owned(),
            granted_actions: selected.get_untracked(),
            expires_at: expires_iso(&expires_local.get_untracked()),
        });
    };
    let submit_disabled = move || {
        name.get().trim().is_empty() || count() == 0 || too_many() || creating.get()
    };

    view! {
        <Dialog>
            <ResetOnClose on_reset=Callback::new(move |_| reset()) />
            <DialogTrigger size=ButtonSize::Sm attr:disabled=(!org_selected).then_some("")>
                <Icon d=RI_KEY_2_LINE class=TOKC_TRIGGER_ICO />
                " New token"
            </DialogTrigger>
            <DialogContent class=TOKC_CONTENT>
                {move || {
                    if let Some(sec) = secret.get() {
                        leptos::either::Either::Left(view! { <SecretReveal secret=sec /> })
                    } else {
                        leptos::either::Either::Right(
                            view! {
                                <form on:submit=submit>
                                    <DialogHeader>
                                        <DialogTitle>"New API token"</DialogTitle>
                                        <DialogDescription>
                                            "Tokens delegate a subset of your own permissions to automation. Pick a name and the actions it may take — it can never exceed what you can do."
                                        </DialogDescription>
                                    </DialogHeader>
                                    <div class=TOKC_BODY>
                                        <div class=TOKC_FIELD>
                                            <Label r#for="token-name">"Name"</Label>
                                            <Input
                                                id="token-name"
                                                value=""
                                                placeholder="ci-deploy"
                                                attr:required=""
                                                prop:value=move || name.get()
                                                on:input=move |ev| {
                                                    name.set(event_target_value(&ev))
                                                }
                                            />
                                        </div>
                                        <div class=TOKC_FIELD>
                                            <div class=TOKC_SCOPE_HEAD>
                                                <Label>"Scope"</Label>
                                                <span
                                                    class=TOKC_COUNT
                                                    style:color=move || {
                                                        if too_many() {
                                                            "var(--color-danger)"
                                                        } else {
                                                            "var(--color-text-muted)"
                                                        }
                                                    }
                                                >
                                                    {count} " / " {MAX_ACTIONS} " selected"
                                                </span>
                                            </div>
                                            <Input
                                                value=""
                                                placeholder="Filter actions…"
                                                prop:value=move || query.get()
                                                on:input=move |ev| {
                                                    query.set(event_target_value(&ev))
                                                }
                                            />
                                            {move || {
                                                scope_list(
                                                    actions.get(),
                                                    actions_error.get(),
                                                    org_selected,
                                                    &query.get(),
                                                    selected,
                                                    toggle_wildcard,
                                                    toggle_action,
                                                )
                                            }}
                                            {move || {
                                                too_many()
                                                    .then(|| {
                                                        view! {
                                                            <p
                                                                class=TOKC_CAP
                                                                style="color: var(--color-danger)"
                                                            >
                                                                "A token carries at most "
                                                                {MAX_ACTIONS}
                                                                " entries — collapse an area into its wildcard or narrow the scope."
                                                            </p>
                                                        }
                                                    })
                                            }}
                                        </div>
                                        <div class=TOKC_FIELD>
                                            <Label r#for="token-expiry">
                                                "Expires (optional)"
                                            </Label>
                                            <Input
                                                id="token-expiry"
                                                value=""
                                                r#type="datetime-local"
                                                prop:value=move || expires_local.get()
                                                on:input=move |ev| {
                                                    expires_local.set(event_target_value(&ev))
                                                }
                                            />
                                        </div>
                                        {move || {
                                            create_error
                                                .get()
                                                .map(|msg| {
                                                    view! {
                                                        <p
                                                            class=TOKC_ERR
                                                            style="color: var(--color-danger)"
                                                        >
                                                            "Could not create token: "
                                                            {msg}
                                                        </p>
                                                    }
                                                })
                                        }}
                                    </div>
                                    <DialogFooter>
                                        <Button
                                            attr:r#type="submit"
                                            attr:disabled=move || {
                                                submit_disabled().then_some("")
                                            }
                                        >
                                            {move || {
                                                if creating.get() {
                                                    "Creating…"
                                                } else {
                                                    "Create token"
                                                }
                                            }}
                                        </Button>
                                    </DialogFooter>
                                </form>
                            },
                        )
                    }
                }}
            </DialogContent>
        </Dialog>
    }
}

/// The reference's list branch: loading / error / grouped catalog.
fn scope_list(
    actions: Option<Vec<GrantableAction>>,
    error: Option<String>,
    org_selected: bool,
    query: &str,
    selected: RwSignal<Vec<String>>,
    toggle_wildcard: impl Fn(&AreaGroup, bool) + Copy + Send + Sync + 'static,
    toggle_action: impl Fn(&AreaGroup, &str, bool) + Copy + Send + Sync + 'static,
) -> AnyView {
    if let Some(msg) = error {
        return view! {
            <p class=TOKC_NOTE style="color: var(--color-danger)">
                "Could not load actions: "
                {msg}
            </p>
        }
        .into_any();
    }
    let Some(actions) = actions else {
        if org_selected {
            return view! {
                <p class=TOKC_NOTE>"Loading your grantable actions…"</p>
            }
            .into_any();
        }
        // No org yet: the reference falls through to an empty list box.
        return view! { <div class=TOKC_LIST></div> }.into_any();
    };
    let groups = filter_groups(&group_by_area(&actions), query);
    let query = query.to_owned();
    let empty = groups.is_empty();
    view! {
        <div class=TOKC_LIST>
            {groups
                .into_iter()
                .map(|g| {
                    let wildcard = g.wildcard.clone();
                    let wild_on = Signal::derive(move || {
                        selected.with(|s| s.contains(&wildcard))
                    });
                    let group = StoredValue::new(g.clone());
                    view! {
                        <div class=TOKC_GROUP>
                            <label class=TOKC_AREA>
                                <Checkbox
                                    checked=wild_on
                                    on_checked_change=Callback::new(move |on| {
                                        group.with_value(|g| toggle_wildcard(g, on))
                                    })
                                />
                                <span class=TOKC_AREA_NAME>{area_label(&g.area)}</span>
                                <code class=format!("mono {TOKC_WILDCARD}")>
                                    {g.wildcard.clone()}
                                </code>
                            </label>
                            <div class=TOKC_ACTIONS>
                                {g
                                    .actions
                                    .iter()
                                    .map(|a| {
                                        let name = a.name.clone();
                                        let checked = Signal::derive(move || {
                                            wild_on.get()
                                                || selected.with(|s| s.contains(&name))
                                        });
                                        let slug = StoredValue::new(a.name.clone());
                                        view! {
                                            <label
                                                class=TOKC_ACTION
                                                title=a.description.clone()
                                            >
                                                <Checkbox
                                                    checked=checked
                                                    disabled=wild_on
                                                    on_checked_change=Callback::new(move |on| {
                                                        group
                                                            .with_value(|g| {
                                                                slug.with_value(|s| toggle_action(g, s, on))
                                                            })
                                                    })
                                                />
                                                <code class=format!(
                                                    "mono {TOKC_SLUG}",
                                                )>{a.name.clone()}</code>
                                            </label>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                    }
                })
                .collect_view()}
            {empty
                .then(|| {
                    view! {
                        <p class=TOKC_EMPTY>
                            "No actions match \u{201c}"
                            {query}
                            "\u{201d}."
                        </p>
                    }
                })}
        </div>
    }
    .into_any()
}

/// Resets the port's field state (and notifies the consumer) whenever
/// the surrounding uncontrolled Dialog closes — the reference's
/// `onOpenChange(false)` tail. Renders nothing.
#[component]
fn ResetOnClose(#[prop(into)] on_reset: Callback<()>) -> impl IntoView {
    let ctx = use_context::<crate::components::dialog::DialogCtx>()
        .expect("invariant: ResetOnClose inside Dialog");
    Effect::new(move |prev: Option<bool>| {
        let open = ctx.open.get();
        if prev == Some(true) && !open {
            on_reset.run(());
        }
        open
    });
}

/// One-time reveal of a freshly minted token.
#[component]
fn SecretReveal(secret: TokenSecret) -> impl IntoView {
    let ctx = use_context::<crate::components::dialog::DialogCtx>()
        .expect("invariant: SecretReveal inside Dialog");
    let copied = RwSignal::new(false);
    let plaintext = StoredValue::new(secret.plaintext.clone());
    let copy = move |_| {
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        {
            use wasm_bindgen::JsCast;
            let Some(window) = web_sys::window() else { return };
            let promise = window.navigator().clipboard().write_text(&plaintext.get_value());
            let on_ok = wasm_bindgen::closure::Closure::once(
                move |_: wasm_bindgen::JsValue| {
                    copied.set(true);
                    if let Some(window) = web_sys::window() {
                        let revert = wasm_bindgen::closure::Closure::once(move || {
                            copied.set(false);
                        });
                        let _ = window
                            .set_timeout_with_callback_and_timeout_and_arguments_0(
                                revert.as_ref().unchecked_ref(),
                                1500,
                            );
                        revert.forget();
                    }
                },
            );
            let on_err = wasm_bindgen::closure::Closure::once(
                move |_: wasm_bindgen::JsValue| { /* ignore denial */ },
            );
            let _ = promise.then(&on_ok).catch(&on_err);
            on_ok.forget();
            on_err.forget();
        }
        #[cfg(not(any(feature = "csr", feature = "hydrate")))]
        {
            let _ = &plaintext;
        }
    };
    view! {
        <DialogHeader>
            <DialogTitle>"Token created"</DialogTitle>
            <DialogDescription>
                "Copy this secret now — it is shown once and never again. Store it somewhere safe; you can revoke it any time from this page."
            </DialogDescription>
        </DialogHeader>
        <div class=TOKC_REVEAL>
            <div class=TOKC_SECRET>
                <Icon d=RI_KEY_2_LINE class=TOKC_KEY_ICO />
                <code class=format!("mono {TOKC_PLAINTEXT}")>{secret.plaintext}</code>
                <Button
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Sm
                    attr:r#type="button"
                    on:click=copy
                >
                    {move || {
                        if copied.get() {
                            leptos::either::Either::Left(
                                view! { <Icon d=RI_CHECK_LINE class=TOKC_COPY_ICO /> },
                            )
                        } else {
                            leptos::either::Either::Right(
                                view! {
                                    <Icon d=RI_FILE_COPY_LINE class=TOKC_COPY_ICO />
                                },
                            )
                        }
                    }}
                    {move || if copied.get() { "Copied" } else { "Copy" }}
                </Button>
            </div>
            <p class=TOKC_TOKEN_ID>
                "Token id " <span class="mono">{secret.id}</span>
            </p>
        </div>
        <DialogFooter>
            <Button attr:r#type="button" on:click=move |_| ctx.open.set(false)>
                "Done"
            </Button>
        </DialogFooter>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{content}{{max-width:560px}}",
            ".{body}{{display:grid;gap:1rem;padding-block:1rem}}",
            ".{field}{{display:grid;gap:.375rem}}",
            ".{scope_head}{{display:flex;align-items:center;",
            "justify-content:space-between}}",
            ".{count}{{font-size:11px}}",
            ".{list}{{max-height:280px;overflow-y:auto;padding:.25rem;",
            "border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border)}}",
            ".{group}{{border:0 solid var(--color-border-soft);border-bottom-width:1px}}",
            ".{group}:last-child{{border-bottom-width:0}}",
            ".{area}{{display:flex;cursor:pointer;align-items:center;",
            "gap:.625rem;background-color:var(--color-surface-2);",
            "padding:.5rem .75rem}}",
            ".{area_name}{{flex:1 1 0%;font-size:12.5px;font-weight:500}}",
            ".{wildcard}{{font-size:11px;color:var(--color-text-muted)}}",
            ".{actions}{{display:flex;flex-direction:column}}",
            ".{action}{{display:flex;cursor:pointer;align-items:center;",
            "gap:.625rem;padding:.375rem .75rem .375rem 2rem}}",
            "@media (hover:hover){{.{action}:hover{{",
            "background-color:var(--color-surface-2)}}}}",
            ".{slug}{{font-size:11.5px}}",
            ".{note}{{padding-block:.75rem;font-size:12px;",
            "color:var(--color-text-muted)}}",
            ".{empty}{{padding:.75rem;font-size:12px;",
            "color:var(--color-text-muted)}}",
            ".{cap}{{font-size:11.5px}}",
            ".{err}{{font-size:12px}}",
            ".{reveal}{{display:grid;gap:.75rem;padding-block:1rem}}",
            ".{secret}{{display:flex;align-items:center;gap:.5rem;",
            "border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);",
            "padding:.5rem .75rem}}",
            ".{key_ico}{{width:1rem;height:1rem;flex-shrink:0;",
            "color:var(--color-accent)}}",
            ".{plaintext}{{flex:1 1 0%;overflow:hidden;",
            "text-overflow:ellipsis;white-space:nowrap;font-size:12px}}",
            ".{token_id}{{font-size:11.5px;color:var(--color-text-muted)}}",
            ".{trigger_ico}{{width:.875rem;height:.875rem}}",
            ".{copy_ico}{{width:.875rem;height:.875rem}}",
        ),
        content = TOKC_CONTENT,
        body = TOKC_BODY,
        field = TOKC_FIELD,
        scope_head = TOKC_SCOPE_HEAD,
        count = TOKC_COUNT,
        list = TOKC_LIST,
        group = TOKC_GROUP,
        area = TOKC_AREA,
        area_name = TOKC_AREA_NAME,
        wildcard = TOKC_WILDCARD,
        actions = TOKC_ACTIONS,
        action = TOKC_ACTION,
        slug = TOKC_SLUG,
        note = TOKC_NOTE,
        empty = TOKC_EMPTY,
        cap = TOKC_CAP,
        err = TOKC_ERR,
        reveal = TOKC_REVEAL,
        secret = TOKC_SECRET,
        key_ico = TOKC_KEY_ICO,
        plaintext = TOKC_PLAINTEXT,
        token_id = TOKC_TOKEN_ID,
        trigger_ico = TOKC_TRIGGER_ICO,
        copy_ico = TOKC_COPY_ICO,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(name: &str) -> GrantableAction {
        GrantableAction { name: name.into(), description: None }
    }

    #[test]
    fn area_helpers_mirror_reference() {
        assert_eq!(area_of("vpn:tunnel:create"), "vpn");
        assert_eq!(area_of("nocolon"), "nocolon");
        assert_eq!(area_label("static_ips"), "Static ips");
        assert_eq!(area_label("vpn"), "Vpn");
        assert_eq!(wildcard_of("vpn"), "vpn:*");
    }

    #[test]
    fn group_by_area_sorts_area_then_slug() {
        let groups = group_by_area(&[
            act("vpn:tunnel:delete"),
            act("static_ips:list"),
            act("vpn:tunnel:create"),
        ]);
        assert_eq!(
            groups.iter().map(|g| g.area.as_str()).collect::<Vec<_>>(),
            ["static_ips", "vpn"]
        );
        assert_eq!(groups[1].wildcard, "vpn:*");
        assert_eq!(
            groups[1].actions.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            ["vpn:tunnel:create", "vpn:tunnel:delete"]
        );
    }

    #[test]
    fn filter_matches_slug_or_area() {
        let groups = group_by_area(&[act("vpn:tunnel:create"), act("dns:assignment:list")]);
        let hit = filter_groups(&groups, "tunnel");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].area, "vpn");
        // An area hit keeps the group wholesale even with no slug match.
        let area_hit = filter_groups(&groups, "dns");
        assert_eq!(area_hit.len(), 1);
        assert_eq!(area_hit[0].actions.len(), 1);
        assert!(filter_groups(&groups, "zzz").is_empty());
        assert_eq!(filter_groups(&groups, "").len(), 2);
    }

    #[test]
    fn expires_blank_is_none() {
        assert_eq!(expires_iso(""), None);
        // Native/SSR passes the raw value through — submission is a
        // client-side event.
        assert_eq!(expires_iso("2026-08-07T10:00"), Some("2026-08-07T10:00".into()));
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            TOKC_CONTENT, TOKC_BODY, TOKC_FIELD, TOKC_SCOPE_HEAD, TOKC_COUNT, TOKC_LIST,
            TOKC_GROUP, TOKC_AREA, TOKC_AREA_NAME, TOKC_WILDCARD, TOKC_ACTIONS, TOKC_ACTION,
            TOKC_SLUG, TOKC_NOTE, TOKC_EMPTY, TOKC_CAP, TOKC_ERR, TOKC_REVEAL, TOKC_SECRET,
            TOKC_KEY_ICO, TOKC_PLAINTEXT, TOKC_TOKEN_ID, TOKC_TRIGGER_ICO, TOKC_COPY_ICO,
        ] {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
