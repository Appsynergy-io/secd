//! CustomizePanel — port of `dashboard/netpolicy/customize-panel.tsx`: the
//! advanced rule builder (drag glyph, enable checkbox, Allow/Block
//! segmented tabs, three friendly Selects, remove button per row) beside
//! the live plain-English summary card. Controlled: the rule list is owned
//! by the consumer (`rules` in, `on_change` out); persistence is the
//! consumer's business.
//!
//! The option catalogs, effect labels, and sentence templates mirror the
//! reference verbatim (ALLOW-HARDCODE content tied to the `net_policy`
//! backend enums). Draft keys come from a render-order counter exactly
//! like the reference's module-level `nextDraftKey` sequence.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::card::Card;
use crate::components::ip_chip::{Chip, ChipTone};
use crate::components::segmented_tabs::SegmentedTabs;
use crate::components::select::{Select, SelectContent, SelectItem, SelectTrigger, SelectValue};
use crate::icons::{
    Icon, RI_ADD_LINE, RI_CHECK_LINE, RI_DELETE_BIN_LINE, RI_DRAGGABLE, RI_FORBID_LINE,
    RI_INFORMATION_LINE,
};
use leptos::prelude::*;

pub const CPANEL: &str = "asy-cpanel";
pub const CPANEL_CARD: &str = "asy-cpanel__card";
pub const CPANEL_CARD_HEAD: &str = "asy-cpanel__card-head";
pub const CPANEL_HEAD_COL: &str = "asy-cpanel__head-col";
pub const CPANEL_HEAD_TITLE: &str = "asy-cpanel__head-title";
pub const CPANEL_HEAD_SUB: &str = "asy-cpanel__head-sub";
pub const CPANEL_EMPTY: &str = "asy-cpanel__empty";
pub const CPANEL_ROW: &str = "asy-cpanel__row";
pub const CPANEL_ROW_DIVIDED: &str = "asy-cpanel__row--divided";
pub const CPANEL_DRAG: &str = "asy-cpanel__drag";
pub const CPANEL_CHECKBOX: &str = "asy-cpanel__checkbox";
pub const CPANEL_W150: &str = "asy-cpanel__w150";
pub const CPANEL_PREP: &str = "asy-cpanel__prep";
pub const CPANEL_DEL: &str = "asy-cpanel__del";
pub const CPANEL_DEL_GLYPH: &str = "asy-cpanel__del-glyph";
pub const CPANEL_FOOT: &str = "asy-cpanel__foot";
pub const CPANEL_ADD_GLYPH: &str = "asy-cpanel__add-glyph";
pub const CPANEL_SUM_BODY: &str = "asy-cpanel__sum-body";
pub const CPANEL_SUM_EMPTY: &str = "asy-cpanel__sum-empty";
pub const CPANEL_SENT_ROW: &str = "asy-cpanel__sent-row";
pub const CPANEL_SENT_ICON: &str = "asy-cpanel__sent-icon";
pub const CPANEL_SENT_ICON_OK: &str = "asy-cpanel__sent-icon--ok";
pub const CPANEL_SENT_ICON_BAD: &str = "asy-cpanel__sent-icon--bad";
pub const CPANEL_SENT_ICON_INFO: &str = "asy-cpanel__sent-icon--info";
pub const CPANEL_SENT: &str = "asy-cpanel__sent";
pub const CPANEL_SENT_BAD: &str = "asy-cpanel__sent--bad";
pub const CPANEL_SEP: &str = "asy-cpanel__sep";
pub const CPANEL_NOTE: &str = "asy-cpanel__note";

/// Client-side draft of a custom rule (`RuleDraft` upstream).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RuleDraft {
    pub key: String,
    pub priority: usize,
    pub enabled: bool,
    pub effect: String,
    pub src: String,
    pub dst: String,
    pub protocol: String,
    pub dst_port: String,
}

/// The reference's module-level `draftKeySeq`.
static DRAFT_KEY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_draft_key() -> String {
    let n = DRAFT_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    format!("draft-{n}")
}

fn new_rule(priority: usize) -> RuleDraft {
    RuleDraft {
        key: next_draft_key(),
        priority,
        enabled: true,
        effect: "allow".to_owned(),
        src: "my_devices".to_owned(),
        dst: "internet".to_owned(),
        protocol: "any".to_owned(),
        dst_port: "any".to_owned(),
    }
}

const EFFECT_LABELS: [&str; 2] = ["Allow", "Block"];
const ENDPOINT_OPTIONS: [(&str, &str); 2] =
    [("my_devices", "My devices"), ("internet", "The internet")];
const PORT_OPTIONS: [(&str, &str); 5] = [
    ("any", "Any port"),
    ("443", "443 (HTTPS)"),
    ("80", "80 (HTTP)"),
    ("5432", "5432 (Postgres)"),
    ("22", "22 (SSH)"),
];

fn label_of(options: &[(&str, &str)], value: &str) -> String {
    options
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, l)| (*l).to_owned())
        .unwrap_or_else(|| value.to_owned())
}

/// The one un-storable rule shape (internet → internet; the API 400s it).
fn shape_invalid(r: &RuleDraft) -> bool {
    r.src == "internet" && r.dst == "internet"
}

/// One rule as the plain-English summary line.
fn sentence(r: &RuleDraft) -> String {
    if shape_invalid(r) {
        return "The internet to the internet isn't a rule about your network \u{2014} pick My devices on one side.".to_owned();
    }
    let verb = if r.effect == "allow" { "can reach" } else { "can't reach" };
    let on_port = if r.dst_port == "any" {
        String::new()
    } else {
        format!(" on {}", label_of(&PORT_OPTIONS, &r.dst_port))
    };
    let src = label_of(&ENDPOINT_OPTIONS, &r.src);
    let dst = label_of(&ENDPOINT_OPTIONS, &r.dst).to_lowercase();
    format!("{src} {verb} {dst}{on_port}.")
}

#[component]
fn FriendlySelect(
    #[prop(into)] value: Signal<String>,
    options: &'static [(&'static str, &'static str)],
    on_change: Callback<String>,
) -> impl IntoView {
    view! {
        <Select value=value on_value_change=on_change>
            <SelectTrigger
                class=CPANEL_W150
                attr:aria-label=move || label_of(options, &value.get())
            >
                <SelectValue label=Signal::derive(move || label_of(options, &value.get())) />
            </SelectTrigger>
            <SelectContent>
                {options
                    .iter()
                    .map(|(v, l)| view! { <SelectItem value=*v>{*l}</SelectItem> })
                    .collect_view()}
            </SelectContent>
        </Select>
    }
}

#[component]
pub fn CustomizePanel(
    #[prop(into)] rules: Signal<Vec<RuleDraft>>,
    on_change: Callback<Vec<RuleDraft>>,
) -> impl IntoView {
    let count = Memo::new(move |_| rules.with(Vec::len));
    let count_ref: NodeRef<leptos::html::Span> = NodeRef::new();
    // The count renders as three text nodes ("1", " ", "rule") like React's
    // three JSX children. Chrome's AX tree keeps the whitespace run between
    // plain text siblings but drops it when flanked by comment nodes, so the
    // hydration markers are stripped once mounted — the text bindings hold
    // the text nodes themselves and stay live.
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    Effect::new(move |_| {
        if let Some(span) = count_ref.get() {
            crate::components::strip_comment_children(span.as_ref());
        }
    });
    let active = Memo::new(move |_| {
        rules.with(|rs| rs.iter().filter(|r| r.enabled).cloned().collect::<Vec<_>>())
    });
    let any_invalid = Memo::new(move |_| active.with(|rs| rs.iter().any(shape_invalid)));
    let add = move |_| {
        let mut next = rules.get_untracked();
        next.push(new_rule(next.len()));
        on_change.run(next);
    };

    view! {
        <div class=CPANEL>
            <Card class=CPANEL_CARD>
                <div class=CPANEL_CARD_HEAD>
                    <div class=CPANEL_HEAD_COL>
                        <span class=CPANEL_HEAD_TITLE>"Your rules"</span>
                        <span class=CPANEL_HEAD_SUB>
                            "Evaluated top to bottom \u{2014} the first match wins."
                        </span>
                    </div>
                    <span class=CPANEL_HEAD_SUB node_ref=count_ref>
                        {move || {
                            (
                                count.get().to_string(),
                                " ",
                                if count.get() == 1 { "rule" } else { "rules" },
                            )
                        }}
                    </span>
                </div>

                <Show
                    when=move || !rules.with(Vec::is_empty)
                    fallback=|| {
                        view! {
                            <div class=CPANEL_EMPTY>
                                "No custom rules yet. Your selected profile is in effect. Add a rule to start tailoring it."
                            </div>
                        }
                    }
                >
                    // For interpolated as a dynamic child: written inline,
                    // the macro emits a literal space before the first row
                    // that breaks hydration ("expected div, found Text").
                    <div>{rule_rows(rules, on_change)}</div>
                </Show>

                <div class=CPANEL_FOOT>
                    <Button size=ButtonSize::Sm on:click=add>
                        <Icon d=RI_ADD_LINE class=CPANEL_ADD_GLYPH />
                        " Add a rule"
                    </Button>
                </div>
            </Card>

            <Card class=CPANEL_CARD>
                <div class=CPANEL_CARD_HEAD>
                    <span class=CPANEL_HEAD_TITLE>"In plain English"</span>
                    <Show
                        when=move || any_invalid.get()
                        fallback=|| view! { <Chip tone=ChipTone::Ok>"valid"</Chip> }
                    >
                        <Chip tone=ChipTone::Bad>"fix a rule"</Chip>
                    </Show>
                </div>
                <div class=CPANEL_SUM_BODY>
                    <Show
                        when=move || !active.with(Vec::is_empty)
                        fallback=|| {
                            view! {
                                <span class=CPANEL_SUM_EMPTY>
                                    "Your rules appear here in plain language as you build them."
                                </span>
                            }
                        }
                    >
                        {sentence_rows(active)}
                    </Show>
                    <div class=CPANEL_SEP></div>
                    <div class=CPANEL_SENT_ROW>
                        <Icon
                            d=RI_INFORMATION_LINE
                            class=format!("{CPANEL_SENT_ICON} {CPANEL_SENT_ICON_INFO}")
                        />
                        <span class=CPANEL_NOTE>
                            "Tenant isolation and anti-spoofing still apply underneath these rules \u{2014} they can't be turned off."
                        </span>
                    </div>
                </div>
            </Card>
        </div>
    }
}

/// The summary lines (enabled rules only). Plain reactive closure — a
/// keyed `<For>` emits a stray leading text node under SSR that breaks
/// hydration (the calendar/port-table precedent), so the list morphs in
/// place; each line renders from the current snapshot.
fn sentence_rows(active: Memo<Vec<RuleDraft>>) -> impl IntoView {
    move || {
        active
            .get()
            .into_iter()
            .map(|r| {
                let (d, tone) = if shape_invalid(&r) {
                    (RI_INFORMATION_LINE, CPANEL_SENT_ICON_BAD)
                } else if r.effect == "allow" {
                    (RI_CHECK_LINE, CPANEL_SENT_ICON_OK)
                } else {
                    (RI_FORBID_LINE, CPANEL_SENT_ICON_BAD)
                };
                let sent_class = if shape_invalid(&r) {
                    format!("{CPANEL_SENT} {CPANEL_SENT_BAD}")
                } else {
                    CPANEL_SENT.to_owned()
                };
                view! {
                    <div class=CPANEL_SENT_ROW>
                        <Icon d=d class=format!("{CPANEL_SENT_ICON} {tone}") />
                        <span class=sent_class>{sentence(&r)}</span>
                    </div>
                }
            })
            .collect_view()
    }
}

/// The builder rows. Same morphing-closure shape as `sentence_rows` (and
/// port_table): tachys reuses same-position nodes across re-renders, which
/// keeps focus on the control being edited exactly like React's keyed
/// row-diff does.
fn rule_rows(rules: Signal<Vec<RuleDraft>>, on_change: Callback<Vec<RuleDraft>>) -> impl IntoView {
    move || {
        rules
            .get()
            .into_iter()
            .map(|r| rule_row(r, rules, on_change))
            .collect_view()
    }
}

/// One builder row. Reads its rule reactively by key so field edits update
/// in place (the keyed `<For>` reuses the row's DOM, like React's diff).
fn rule_row(
    initial: RuleDraft,
    rules: Signal<Vec<RuleDraft>>,
    on_change: Callback<Vec<RuleDraft>>,
) -> impl IntoView {
    let key = StoredValue::new(initial.key.clone());
    let rule = Memo::new(move |_| {
        rules
            .with(|rs| key.with_value(|k| rs.iter().find(|x| &x.key == k).cloned()))
            .unwrap_or_else(|| initial.clone())
    });
    let last = Memo::new(move |_| {
        rules.with(|rs| key.with_value(|k| rs.last().is_some_and(|x| &x.key == k)))
    });
    let patch = move |f: &dyn Fn(&mut RuleDraft)| {
        let mut next = rules.get_untracked();
        if let Some(x) = next.iter_mut().find(|x| key.with_value(|k| &x.key == k)) {
            f(x);
        }
        on_change.run(next);
    };
    let remove = move |_| {
        let next: Vec<RuleDraft> = rules
            .get_untracked()
            .into_iter()
            .filter(|x| key.with_value(|k| &x.key != k))
            .collect();
        on_change.run(next);
    };

    view! {
        <div class=move || {
            if last.get() { CPANEL_ROW.to_owned() } else { format!("{CPANEL_ROW} {CPANEL_ROW_DIVIDED}") }
        }>
            <Icon d=RI_DRAGGABLE class=CPANEL_DRAG />
            {
                // React freezes the `checked` *attribute* at its mount value and
                // moves only the property afterwards. The rows are morphing
                // closures, so a plain attribute binding would track state through
                // rebuilds; instead the mount value is stamped on the reused DOM
                // node (data-* is outside the comparison vocabulary) and
                // re-enforced after every morph.
                let cb_ref: NodeRef<leptos::html::Input> = NodeRef::new();
                #[cfg(any(feature = "csr", feature = "hydrate"))]
                Effect::new(move |_| {
                    rule.with(|r| r.enabled);
                    let Some(input) = cb_ref.get() else { return };
                    let el: &web_sys::Element = input.as_ref();
                    let init = match el.get_attribute("data-asy-checked-init") {
                        Some(v) => v == "1",
                        None => {
                            let mounted = el.has_attribute("checked");
                            let _ = el.set_attribute(
                                "data-asy-checked-init",
                                if mounted { "1" } else { "0" },
                            );
                            mounted
                        }
                    };
                    if init {
                        let _ = el.set_attribute("checked", "");
                    } else {
                        let _ = el.remove_attribute("checked");
                    }
                });
                view! {
            <input
                node_ref=cb_ref
                type="checkbox"
                checked=rule.with_untracked(|r| r.enabled).then_some("")
                prop:checked=move || rule.with(|r| r.enabled)
                aria-label="rule enabled"
                class=CPANEL_CHECKBOX
                on:change:target=move |ev| {
                    let checked = ev.target().checked();
                    patch(&move |r| r.enabled = checked);
                }
            />
                }
            }
            <div class=CPANEL_W150>
                <SegmentedTabs
                    items=EFFECT_LABELS.map(str::to_owned).to_vec()
                    active=Signal::derive(move || {
                        rule.with(|r| if r.effect == "allow" { "Allow" } else { "Block" }.to_owned())
                    })
                    on_change=Callback::new(move |v: String| {
                        let effect = v.to_lowercase();
                        patch(&move |r| r.effect = effect.clone());
                    })
                />
            </div>
            <span class=CPANEL_PREP>"from"</span>
            <FriendlySelect
                value=Signal::derive(move || rule.with(|r| r.src.clone()))
                options=&ENDPOINT_OPTIONS
                on_change=Callback::new(move |v: String| patch(&move |r| r.src = v.clone()))
            />
            <span class=CPANEL_PREP>"to"</span>
            <FriendlySelect
                value=Signal::derive(move || rule.with(|r| r.dst.clone()))
                options=&ENDPOINT_OPTIONS
                on_change=Callback::new(move |v: String| patch(&move |r| r.dst = v.clone()))
            />
            <span class=CPANEL_PREP>"on"</span>
            <FriendlySelect
                value=Signal::derive(move || rule.with(|r| r.dst_port.clone()))
                options=&PORT_OPTIONS
                on_change=Callback::new(move |v: String| patch(&move |r| r.dst_port = v.clone()))
            />
            <Button
                variant=ButtonVariant::Ghost
                size=ButtonSize::Sm
                class=CPANEL_DEL
                attr:aria-label="remove rule"
                on:click=remove
            >
                <Icon d=RI_DELETE_BIN_LINE class=CPANEL_DEL_GLYPH />
            </Button>
        </div>
    }
}

/// The panel grid and both cards' utility strings translated; the delete
/// button's `size-7 p-0` overrides win over the Button sm sizing by rule
/// order (this module's css is appended after button's).
pub fn css() -> String {
    format!(
        ".{CPANEL}{{margin-bottom:.875rem;display:grid;\
grid-template-columns:1.55fr 1fr;align-items:flex-start;gap:.75rem}}\
@media (width <= 900px){{.{CPANEL}{{grid-template-columns:1fr}}}}\
.{CPANEL_CARD}{{overflow:hidden;padding:0}}\
.{CPANEL_CARD_HEAD}{{display:flex;align-items:center;\
justify-content:space-between;border-bottom-width:1px;\
border-color:var(--color-border);padding:.75rem 1rem}}\
.{CPANEL_HEAD_COL}{{display:flex;flex-direction:column;gap:.125rem}}\
.{CPANEL_HEAD_TITLE}{{font-size:14px;font-weight:600}}\
.{CPANEL_HEAD_SUB}{{font-size:11.5px;color:var(--color-text-muted)}}\
.{CPANEL_EMPTY}{{padding:1.5rem 1rem;font-size:12.5px;\
color:var(--color-text-muted)}}\
.{CPANEL_ROW}{{display:flex;flex-wrap:wrap;align-items:center;\
gap:.625rem;padding:.625rem .75rem}}\
.{CPANEL_ROW_DIVIDED}{{border-bottom-width:1px;\
border-color:var(--color-border-soft)}}\
.{CPANEL_DRAG}{{width:1rem;height:1rem;flex-shrink:0;\
color:var(--color-text-dim)}}\
.{CPANEL_CHECKBOX}{{width:15px;height:15px;flex-shrink:0;\
accent-color:var(--color-accent)}}\
.{CPANEL_W150}{{width:150px}}\
@media (width < 40rem){{.{CPANEL_W150}{{width:100%;max-width:none}}}}\
.{CPANEL_PREP}{{font-size:12px;color:var(--color-text-muted)}}\
.{CPANEL_DEL}{{margin-left:auto;width:1.75rem;height:1.75rem;\
flex-shrink:0;padding:0}}\
.{CPANEL_DEL_GLYPH}{{width:.875rem;height:.875rem}}\
.{CPANEL_FOOT}{{border-top-width:1px;\
border-color:var(--color-border);padding:.75rem .875rem}}\
.{CPANEL_ADD_GLYPH}{{width:.875rem;height:.875rem}}\
.{CPANEL_SUM_BODY}{{display:flex;flex-direction:column;gap:.625rem;\
padding:1rem}}\
.{CPANEL_SUM_EMPTY}{{font-size:12.5px;color:var(--color-text-muted)}}\
.{CPANEL_SENT_ROW}{{display:flex;align-items:flex-start;gap:.625rem}}\
.{CPANEL_SENT_ICON}{{margin-top:1px;width:15px;height:15px;\
flex-shrink:0}}\
.{CPANEL_SENT_ICON_OK}{{color:var(--color-success)}}\
.{CPANEL_SENT_ICON_BAD}{{color:var(--color-danger)}}\
.{CPANEL_SENT_ICON_INFO}{{color:var(--color-text-dim)}}\
.{CPANEL_SENT}{{font-size:12.5px;line-height:1.5;text-wrap:pretty}}\
.{CPANEL_SENT_BAD}{{color:var(--color-danger)}}\
.{CPANEL_SEP}{{margin:.125rem 0;height:1px;\
background-color:var(--color-border-soft)}}\
.{CPANEL_NOTE}{{font-size:11.5px;line-height:1.5;\
color:var(--color-text-muted)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            CPANEL,
            CPANEL_CARD,
            CPANEL_CARD_HEAD,
            CPANEL_HEAD_COL,
            CPANEL_HEAD_TITLE,
            CPANEL_HEAD_SUB,
            CPANEL_EMPTY,
            CPANEL_ROW,
            CPANEL_ROW_DIVIDED,
            CPANEL_DRAG,
            CPANEL_CHECKBOX,
            CPANEL_W150,
            CPANEL_PREP,
            CPANEL_DEL,
            CPANEL_DEL_GLYPH,
            CPANEL_FOOT,
            CPANEL_ADD_GLYPH,
            CPANEL_SUM_BODY,
            CPANEL_SUM_EMPTY,
            CPANEL_SENT_ROW,
            CPANEL_SENT_ICON,
            CPANEL_SENT_ICON_OK,
            CPANEL_SENT_ICON_BAD,
            CPANEL_SENT_ICON_INFO,
            CPANEL_SENT,
            CPANEL_SENT_BAD,
            CPANEL_SEP,
            CPANEL_NOTE,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn sentences_mirror_the_reference_templates() {
        let mut r = new_rule(0);
        assert_eq!(sentence(&r), "My devices can reach the internet.");
        r.effect = "block".into();
        r.dst_port = "443".into();
        assert_eq!(sentence(&r), "My devices can't reach the internet on 443 (HTTPS).");
        r.src = "internet".into();
        r.dst = "internet".into();
        assert!(shape_invalid(&r));
        assert!(sentence(&r).starts_with("The internet to the internet isn't"));
    }

    #[test]
    fn draft_keys_are_a_stable_sequence() {
        let a = next_draft_key();
        let b = next_draft_key();
        assert!(a.starts_with("draft-") && b.starts_with("draft-") && a != b);
    }
}
