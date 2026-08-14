//! ACL dialog suite — port of `dashboard/acl-dialogs.tsx` (P1).
//!
//! Props/callbacks split, per component:
//! - `AclTestDialog`: the reference is already props-only — `rules` comes in
//!   as data and evaluation is the client-side literal/`*` preview grammar
//!   from `data/acl.ts` (explicitly not backend-coupled: the API treats the
//!   matchers as opaque). The evaluator ports with the component as
//!   [`evaluate_acl_flow`] / [`acl_glob_match`].
//! - `AclEditDialog`: `useUpdateAclRule` stays in the consumer. The visual
//!   form takes `rule` (open = `Some`), `saving` (isPending) and `error`
//!   (isError + message) as props and emits `on_save(AclRulePatch)`; the
//!   consumer owns the mutation, its `reset()` on reopen, and closing on
//!   success.
//! - `AclDeleteDialog`: `useDeleteAclRule` stays in the consumer. Props
//!   `rule`/`deleting`/`error`, callback `on_delete` (the consumer already
//!   holds the rule id).
//!
//! All three are controlled dialogs (`open`/`onOpenChange`, no trigger) over
//! the crate-internal `DialogControlled` root.

use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ip_chip::{Chip, ChipTone};
use leptos::prelude::*;

pub const ACL_FIELD: &str = "asy-acl__field";
pub const ACL_LABEL: &str = "asy-acl__label";
pub const ACL_COL: &str = "asy-acl__col";
pub const ACL_GRID2: &str = "asy-acl__grid2";
pub const ACL_RESULT: &str = "asy-acl__result";
pub const ACL_RESULT_HEAD: &str = "asy-acl__result-head";
pub const ACL_RESULT_LABEL: &str = "asy-acl__result-label";
pub const ACL_PREVIEW: &str = "asy-acl__preview";
pub const ACL_RESULT_P: &str = "asy-acl__result-p";
pub const ACL_ERROR: &str = "asy-acl__error";

/// The fields of `OrgAclRuleSummary` the dialogs render. The consumer keeps
/// the full API record; `priority` is a JS number upstream, so it is `f64`
/// here and serializes JS-style (integers print bare).
#[derive(Clone, PartialEq, Debug)]
pub struct AclRule {
    pub name: String,
    pub src_match: String,
    pub dst_match: String,
    pub action: String,
    pub priority: f64,
}

/// What `AclEditDialog` emits on save — the reference's PATCH body.
#[derive(Clone, PartialEq, Debug)]
pub struct AclRulePatch {
    pub name: String,
    pub src_match: String,
    pub dst_match: String,
    pub action: String,
    pub priority: f64,
}

/// The dashboard's literal/`*` preview matcher, op-for-op from
/// `aclGlobMatch` in `data/acl.ts`: no `*` → exact; otherwise the first
/// part anchors as a prefix, the last as a suffix, interior parts must
/// appear in order between them.
pub fn acl_glob_match(pattern: &str, candidate: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == candidate;
    }
    let mut pos: usize = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !candidate.starts_with(part) {
                return false;
            }
            pos = part.len();
            continue;
        }
        if i == parts.len() - 1 {
            return candidate.len() as i64 - part.len() as i64 >= pos as i64
                && candidate.ends_with(part);
        }
        match candidate[pos.min(candidate.len())..].find(part) {
            Some(found) => pos = pos + found + part.len(),
            None => return false,
        }
    }
    true
}

/// First rule whose matchers both match, in list order (the API delivers
/// `priority ASC, id ASC`), or `None` for default-deny. Preview only — the
/// in-path agent is the authoritative enforcer.
pub fn evaluate_acl_flow(rules: &[AclRule], src: &str, dst: &str) -> Option<AclRule> {
    rules
        .iter()
        .find(|r| acl_glob_match(&r.src_match, src) && acl_glob_match(&r.dst_match, dst))
        .cloned()
}

/// JS `String(number)` for `priority` (shortest-roundtrip; integers bare).
pub(crate) fn fmt_num(v: f64) -> String {
    format!("{v}")
}

/// JS `Number(string)`: whitespace-only → 0, unparseable → NaN.
pub(crate) fn js_number(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// Dry-run a hypothetical flow against the org's ACL rules (preview).
#[component]
pub fn AclTestDialog(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    rules: Vec<AclRule>,
) -> impl IntoView {
    let src = RwSignal::new("role:engineering".to_owned());
    let dst = RwSignal::new("staging.internal:443".to_owned());
    // None = not evaluated; Some(None) = evaluated, default-deny.
    let result = RwSignal::new(None::<Option<AclRule>>);
    let rules = StoredValue::new(rules);

    Effect::new(move |_| {
        if open.get() {
            result.set(None);
        }
    });

    let evaluate = move |_| {
        result.set(Some(rules.with_value(|r| {
            evaluate_acl_flow(r, src.get_untracked().trim(), dst.get_untracked().trim())
        })));
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Test against flow"</DialogTitle>
                    <DialogDescription>
                        "Preview which rule a flow would hit. Matching uses literal text and "
                        <code>"*"</code>
                        " "
                        "wildcards over your rules in priority order. The in-path agent enforces the authoritative result."
                    </DialogDescription>
                </DialogHeader>
                <div class=ACL_COL>
                    <label class=ACL_LABEL>
                        "Source (subject)"
                        <input
                            class=format!("{ACL_FIELD} mono")
                            value=move || src.get()
                            prop:value=move || src.get()
                            on:input=move |ev| src.set(event_target_value(&ev))
                            placeholder="role:engineering"
                        />
                    </label>
                    <label class=ACL_LABEL>
                        "Destination (resource)"
                        <input
                            class=format!("{ACL_FIELD} mono")
                            value=move || dst.get()
                            prop:value=move || dst.get()
                            on:input=move |ev| dst.set(event_target_value(&ev))
                            placeholder="host:port"
                        />
                    </label>
                    {move || {
                        result
                            .get()
                            .map(|matched| {
                                let effect = matched
                                    .as_ref()
                                    .map(|m| m.action.clone())
                                    .unwrap_or_else(|| "deny".to_owned());
                                view! {
                                    <div class=ACL_RESULT>
                                        <div class=ACL_RESULT_HEAD>
                                            <span class=ACL_RESULT_LABEL>"Result:"</span>
                                            {if effect == "deny" {
                                                view! { <Chip tone=ChipTone::Bad>"deny"</Chip> }
                                            } else {
                                                view! { <Chip tone=ChipTone::Ok>"allow"</Chip> }
                                            }}
                                            <span class=ACL_PREVIEW>"preview"</span>
                                        </div>
                                        {match matched {
                                            Some(m) => leptos::either::Either::Left(view! {
                                                <p class=ACL_RESULT_P>
                                                    "Matched rule "
                                                    <b>{m.name.clone()}</b>
                                                    " (priority "
                                                    {fmt_num(m.priority)}
                                                    "):"
                                                    " "
                                                    <span class="mono">{m.src_match.clone()}</span>
                                                    " →"
                                                    " "
                                                    <span class="mono">{m.dst_match.clone()}</span>
                                                    "."
                                                </p>
                                            }),
                                            None => leptos::either::Either::Right(view! {
                                                <p class=ACL_RESULT_P>
                                                    "No rule matched — default deny."
                                                </p>
                                            }),
                                        }}
                                    </div>
                                }
                            })
                    }}
                </div>
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Close"
                    </Button>
                    <Button variant=ButtonVariant::Primary size=ButtonSize::Sm on:click=evaluate>
                        "Evaluate"
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Edit an existing ACL rule (name / matchers / effect / priority).
#[component]
pub fn AclEditDialog(
    #[prop(into)] rule: Signal<Option<AclRule>>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useUpdateAclRule.mutate` — receives the trimmed
    /// PATCH body; the consumer closes on success.
    #[prop(into)]
    on_save: Callback<AclRulePatch>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    saving: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let src_match = RwSignal::new(String::new());
    let dst_match = RwSignal::new(String::new());
    let action = RwSignal::new("allow".to_owned());
    let priority = RwSignal::new("100".to_owned());

    // Seed the form from the rule on (re)open — the reference's effect.
    Effect::new(move |_| {
        if let Some(r) = rule.get() {
            name.set(r.name);
            src_match.set(r.src_match);
            dst_match.set(r.dst_match);
            action.set(r.action);
            priority.set(fmt_num(r.priority));
        }
    });

    let submit = move |_| {
        let Some(r) = rule.get_untracked() else { return };
        let p = js_number(&priority.get_untracked());
        on_save.run(AclRulePatch {
            name: name.get_untracked().trim().to_owned(),
            src_match: src_match.get_untracked().trim().to_owned(),
            dst_match: dst_match.get_untracked().trim().to_owned(),
            action: action.get_untracked(),
            priority: if p.is_finite() { p } else { r.priority },
        });
    };

    view! {
        <DialogControlled open=Signal::derive(move || rule.get().is_some()) on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Edit ACL rule"</DialogTitle>
                    <DialogDescription>
                        "Update the matchers, effect, or priority for this rule. Lower priority runs first."
                    </DialogDescription>
                </DialogHeader>
                <div class=ACL_COL>
                    <label class=ACL_LABEL>
                        "Name"
                        <input
                            class=ACL_FIELD
                            value=move || name.get()
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=ACL_LABEL>
                        "Source match"
                        <input
                            class=format!("{ACL_FIELD} mono")
                            value=move || src_match.get()
                            prop:value=move || src_match.get()
                            on:input=move |ev| src_match.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=ACL_LABEL>
                        "Destination match"
                        <input
                            class=format!("{ACL_FIELD} mono")
                            value=move || dst_match.get()
                            prop:value=move || dst_match.get()
                            on:input=move |ev| dst_match.set(event_target_value(&ev))
                        />
                    </label>
                    <div class=ACL_GRID2>
                        <label class=ACL_LABEL>
                            "Effect"
                            <select
                                class=ACL_FIELD
                                prop:value=move || action.get()
                                on:change=move |ev| action.set(event_target_value(&ev))
                            >
                                <option value="allow">"allow"</option>
                                <option value="deny">"deny"</option>
                            </select>
                        </label>
                        <label class=ACL_LABEL>
                            "Priority"
                            <input
                                class=ACL_FIELD
                                inputmode="numeric"
                                value=move || priority.get()
                                prop:value=move || priority.get()
                                on:input=move |ev| priority.set(event_target_value(&ev))
                            />
                        </label>
                    </div>
                    {move || {
                        error
                            .get()
                            .map(|msg| {
                                view! {
                                    <p class=ACL_ERROR style="color: var(--color-danger)">
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
                        attr:disabled=move || saving.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || {
                            (saving.get() || name.with(|n| n.trim().is_empty())).then_some("")
                        }
                        on:click=submit
                    >
                        {move || if saving.get() { "Saving…" } else { "Save changes" }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Confirm + delete an ACL rule.
#[component]
pub fn AclDeleteDialog(
    #[prop(into)] rule: Signal<Option<AclRule>>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's `useDeleteAclRule.mutate` — it already holds the
    /// rule id; the consumer closes on success.
    #[prop(into)]
    on_delete: Callback<()>,
    /// The mutation's `isPending`.
    #[prop(optional, into)]
    deleting: Signal<bool>,
    /// The mutation's error message when `isError`.
    #[prop(optional, into)]
    error: Signal<Option<String>>,
) -> impl IntoView {
    let confirm = move |_| {
        if rule.get_untracked().is_some() {
            on_delete.run(());
        }
    };
    view! {
        <DialogControlled open=Signal::derive(move || rule.get().is_some()) on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Delete ACL rule"</DialogTitle>
                    <DialogDescription>
                        "Remove "
                        <b>{move || rule.get().map(|r| r.name)}</b>
                        "? This cannot be undone; flows it governed fall through to the remaining rules."
                    </DialogDescription>
                </DialogHeader>
                {move || {
                    error
                        .get()
                        .map(|msg| {
                            view! {
                                <p class=ACL_ERROR style="color: var(--color-danger)">
                                    "Could not delete: "
                                    {msg}
                                </p>
                            }
                        })
                }}
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || deleting.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Danger
                        size=ButtonSize::Sm
                        attr:disabled=move || deleting.get().then_some("")
                        on:click=confirm
                    >
                        {move || if deleting.get() { "Deleting…" } else { "Delete rule" }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{field}{{height:2rem;width:100%;border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);padding-inline:.5rem;",
            "font-size:12.5px;color:var(--color-text)}}",
            ".{label}{{display:flex;flex-direction:column;gap:.25rem;",
            "font-size:11.5px;font-weight:500;color:var(--color-text-muted)}}",
            ".{col}{{display:flex;flex-direction:column;gap:.75rem}}",
            ".{grid2}{{display:grid;grid-template-columns:1fr;gap:.75rem}}",
            "@media (width >= 40rem){{.{grid2}{{",
            "grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            ".{result}{{border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);padding-inline:.75rem;",
            "padding-block:.625rem;font-size:12.5px}}",
            ".{result_head}{{margin-bottom:.25rem;display:flex;align-items:center;",
            "gap:.5rem}}",
            ".{result_label}{{font-weight:500}}",
            ".{preview}{{font-size:11px;color:var(--color-text-muted)}}",
            ".{result_p}{{color:var(--color-text-muted)}}",
            ".{error}{{font-size:12px}}",
        ),
        field = ACL_FIELD,
        label = ACL_LABEL,
        col = ACL_COL,
        grid2 = ACL_GRID2,
        result = ACL_RESULT,
        result_head = ACL_RESULT_HEAD,
        result_label = ACL_RESULT_LABEL,
        preview = ACL_PREVIEW,
        result_p = ACL_RESULT_P,
        error = ACL_ERROR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            ACL_FIELD, ACL_LABEL, ACL_COL, ACL_GRID2, ACL_RESULT, ACL_RESULT_HEAD,
            ACL_RESULT_LABEL, ACL_PREVIEW, ACL_RESULT_P, ACL_ERROR,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    /// Mirrors `aclGlobMatch`'s JS semantics case-for-case.
    #[test]
    fn glob_match_mirrors_reference() {
        // No wildcard → exact.
        assert!(acl_glob_match("role:engineering", "role:engineering"));
        assert!(!acl_glob_match("role:engineering", "role:eng"));
        // Bare star matches everything (["",""], both parts empty).
        assert!(acl_glob_match("*", "anything"));
        assert!(acl_glob_match("*", ""));
        // Prefix anchor.
        assert!(acl_glob_match("role:*", "role:engineering"));
        assert!(!acl_glob_match("role:*", "user:role:x"));
        // Suffix anchor.
        assert!(acl_glob_match("*.internal", "prod.internal"));
        assert!(!acl_glob_match("*.internal", "internal"));
        // Prefix + suffix without overlap: "aba" cannot satisfy "ab*ba"
        // (the JS length arithmetic, not plain starts/ends).
        assert!(!acl_glob_match("ab*ba", "aba"));
        assert!(acl_glob_match("ab*ba", "abba"));
        // Interior parts in order.
        assert!(acl_glob_match("a*b*c", "aXbYc"));
        assert!(!acl_glob_match("a*b*c", "aXcYb"));
        // Empty-string candidate against exact empty pattern.
        assert!(acl_glob_match("", ""));
        // Reference sample-rule pairs.
        assert!(acl_glob_match("prod.internal:*", "prod.internal:5432"));
        assert!(!acl_glob_match("prod.internal:*", "staging.internal:443"));
    }

    #[test]
    fn evaluate_takes_first_match_in_list_order() {
        let rules = vec![
            AclRule {
                name: "engineering → staging".into(),
                src_match: "role:engineering".into(),
                dst_match: "staging.internal:443".into(),
                action: "allow".into(),
                priority: 60.0,
            },
            AclRule {
                name: "deny prod".into(),
                src_match: "role:*".into(),
                dst_match: "prod.internal:*".into(),
                action: "deny".into(),
                priority: 10.0,
            },
        ];
        let hit = evaluate_acl_flow(&rules, "role:engineering", "staging.internal:443");
        assert_eq!(hit.as_ref().map(|r| r.name.as_str()), Some("engineering → staging"));
        let deny = evaluate_acl_flow(&rules, "role:ops", "prod.internal:5432");
        assert_eq!(deny.as_ref().map(|r| r.action.as_str()), Some("deny"));
        assert!(evaluate_acl_flow(&rules, "role:ops", "elsewhere:1").is_none());
    }

    #[test]
    fn js_number_semantics() {
        assert_eq!(js_number(""), 0.0);
        assert_eq!(js_number("  "), 0.0);
        assert_eq!(js_number("100"), 100.0);
        assert_eq!(js_number("1.5"), 1.5);
        assert!(js_number("12x").is_nan());
        assert_eq!(fmt_num(60.0), "60");
        assert_eq!(fmt_num(1.5), "1.5");
    }
}
