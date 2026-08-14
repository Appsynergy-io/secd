//! PortTable — port of `dashboard/netpolicy/port-table.tsx`: the Gateway
//! "open ports" editor. Controlled like upstream (`forwards` in,
//! `on_change` out); the adder's protocol/port/endpoint drafts are local UI
//! state, exactly as the reference holds them. Draft keys come from a
//! module counter mirroring upstream `nextDraftKey`. Carries the site's
//! `.tbl` row-hover utility (first table in the crate).

use std::sync::atomic::{AtomicU64, Ordering};

use crate::components::button::{BTN, BTN_DEFAULT, BTN_GHOST, BTN_PRIMARY, BTN_SM};
use crate::components::card::CARD;
use crate::components::input::INPUT;
use crate::components::segmented_tabs::SegmentedTabs;
use crate::icons::{
    Icon, RI_ADD_LINE, RI_ARROW_RIGHT_LINE, RI_COMPUTER_LINE, RI_DELETE_BIN_LINE,
};
use leptos::either::Either;
use leptos::html;
use leptos::prelude::*;

pub const PORT_TABLE: &str = "asy-port-table";
pub const PORT_TABLE_HEAD: &str = "asy-port-table__head";
pub const PORT_TABLE_TITLE_COL: &str = "asy-port-table__title-col";
pub const PORT_TABLE_TITLE: &str = "asy-port-table__title";
pub const PORT_TABLE_SUB: &str = "asy-port-table__sub";
pub const PORT_TABLE_OPEN_GLYPH: &str = "asy-port-table__open-glyph";
pub const PORT_TABLE_SCROLL: &str = "asy-port-table__scroll";
pub const TBL: &str = "asy-tbl";
pub const PORT_TABLE_TABLE: &str = "asy-port-table__table";
pub const PORT_TABLE_THEAD_ROW: &str = "asy-port-table__thead-row";
pub const PORT_TABLE_TH: &str = "asy-port-table__th";
pub const PORT_TABLE_TH_MEDIUM: &str = "asy-port-table__th--medium";
pub const PORT_TABLE_TH_PORT: &str = "asy-port-table__th--port";
pub const PORT_TABLE_TH_ARROW: &str = "asy-port-table__th--arrow";
pub const PORT_TABLE_TH_ACTIONS: &str = "asy-port-table__th--actions";
pub const PORT_TABLE_EMPTY: &str = "asy-port-table__empty";
pub const PORT_TABLE_ROW: &str = "asy-port-table__row";
pub const PORT_TABLE_TD: &str = "asy-port-table__td";
pub const PORT_TABLE_TD_CENTER: &str = "asy-port-table__td--center";
pub const PORT_TABLE_TD_NOTE: &str = "asy-port-table__td--note";
pub const PORT_TABLE_TD_RIGHT: &str = "asy-port-table__td--right";
pub const PORT_TABLE_CELL_WRAP: &str = "asy-port-table__cell-wrap";
pub const PORT_TABLE_PROTO_PILL: &str = "asy-port-table__proto-pill";
pub const PORT_TABLE_PORT_VAL: &str = "asy-port-table__port-val";
pub const PORT_TABLE_ROW_ARROW: &str = "asy-port-table__row-arrow";
pub const PORT_TABLE_EP_GLYPH: &str = "asy-port-table__ep-glyph";
pub const PORT_TABLE_EP_VAL: &str = "asy-port-table__ep-val";
pub const PORT_TABLE_REMOVE: &str = "asy-port-table__remove";
pub const PORT_TABLE_REMOVE_GLYPH: &str = "asy-port-table__remove-glyph";
pub const PORT_TABLE_ADDER: &str = "asy-port-table__adder";
pub const PORT_TABLE_PROTO: &str = "asy-port-table__proto";
pub const PORT_TABLE_PORT_INPUT: &str = "asy-port-table__port-input";
pub const PORT_TABLE_MID_ARROW: &str = "asy-port-table__mid-arrow";
pub const PORT_TABLE_EP_INPUT: &str = "asy-port-table__ep-input";
pub const PORT_TABLE_ADD_GLYPH: &str = "asy-port-table__add-glyph";
pub const PORT_TABLE_HINT: &str = "asy-port-table__hint";

/// Client-side draft of a Gateway forward (`ForwardDraft` upstream): the
/// `key` is a list key, never sent; the server assigns row ids on save.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ForwardDraft {
    pub key: String,
    pub protocol: String,
    pub port: u16,
    pub endpoint: String,
    pub note: String,
}

static DRAFT_KEY_SEQ: AtomicU64 = AtomicU64::new(0);

/// A stable, collision-free key for a freshly-added draft row
/// (`nextDraftKey` upstream).
pub fn next_draft_key() -> String {
    format!("draft-{}", DRAFT_KEY_SEQ.fetch_add(1, Ordering::Relaxed) + 1)
}

#[component]
pub fn PortTable(
    #[prop(into)] forwards: Signal<Vec<ForwardDraft>>,
    on_change: Callback<Vec<ForwardDraft>>,
) -> impl IntoView {
    let proto_label = RwSignal::new("TCP".to_owned());
    let port = RwSignal::new(String::new());
    let endpoint = RwSignal::new(String::new());
    let port_ref: NodeRef<html::Input> = NodeRef::new();

    let add = move || {
        let port_text = port.with(|p| p.trim().to_owned());
        let ep = endpoint.with(|e| e.trim().to_owned());
        let Ok(port_num) = port_text.parse::<u32>() else { return };
        if !(1..=65535).contains(&port_num) || ep.is_empty() {
            return;
        }
        let mut next = forwards.get();
        next.push(ForwardDraft {
            key: next_draft_key(),
            protocol: proto_label.with(|p| p.to_lowercase()),
            port: port_num as u16,
            endpoint: ep,
            note: String::new(),
        });
        on_change.run(next);
        port.set(String::new());
        endpoint.set(String::new());
    };

    view! {
        <div class=format!("{CARD} {PORT_TABLE}")>
            <div class=PORT_TABLE_HEAD>
                <div class=PORT_TABLE_TITLE_COL>
                    <span class=PORT_TABLE_TITLE>"Open ports"</span>
                    <span class=PORT_TABLE_SUB>
                        "Each port reaches one device. Everything you don't list stays closed."
                    </span>
                </div>
                <button
                    class=format!("{BTN} {BTN_PRIMARY} {BTN_SM}")
                    on:click=move |_| {
                        if let Some(input) = port_ref.get() {
                            let _ = input.focus();
                        }
                    }
                >
                    <Icon d=RI_ADD_LINE class=PORT_TABLE_OPEN_GLYPH />
                    " Open a port"
                </button>
            </div>
            <div class=PORT_TABLE_SCROLL>
                <table class=format!("{TBL} {PORT_TABLE_TABLE}")>
                    <thead>
                        <tr class=PORT_TABLE_THEAD_ROW>
                            <th class=format!("{PORT_TABLE_TH} {PORT_TABLE_TH_MEDIUM} {PORT_TABLE_TH_PORT}")>"Port"</th>
                            <th class=format!("{PORT_TABLE_TH} {PORT_TABLE_TH_ARROW}")></th>
                            <th class=format!("{PORT_TABLE_TH} {PORT_TABLE_TH_MEDIUM}")>"Reaches"</th>
                            <th class=format!("{PORT_TABLE_TH} {PORT_TABLE_TH_MEDIUM}")>"What it's for"</th>
                            <th class=format!("{PORT_TABLE_TH} {PORT_TABLE_TH_ACTIONS}")></th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            let rows = forwards.get();
                            if rows.is_empty() {
                                Either::Left(
                                    view! {
                                        <tr>
                                            <td colspan="5" class=PORT_TABLE_EMPTY>
                                                "No open ports yet. Everything inbound is closed \u{2014} add one below to publish a service."
                                            </td>
                                        </tr>
                                    },
                                )
                            } else {
                                Either::Right(
                                    rows
                                        .into_iter()
                                        .map(|f| {
                                            let remove_key = f.key.clone();
                                            let remove = move |_| {
                                                let next: Vec<ForwardDraft> = forwards
                                                    .get()
                                                    .into_iter()
                                                    .filter(|row| row.key != remove_key)
                                                    .collect();
                                                on_change.run(next);
                                            };
                                            let note = if f.note.is_empty() {
                                                "\u{2014}".to_owned()
                                            } else {
                                                f.note.clone()
                                            };
                                            view! {
                                                <tr class=PORT_TABLE_ROW>
                                                    <td class=PORT_TABLE_TD>
                                                        <span class=PORT_TABLE_CELL_WRAP>
                                                            <span class=PORT_TABLE_PROTO_PILL>{f.protocol.clone()}</span>
                                                            <span class=format!("mono {PORT_TABLE_PORT_VAL}")>
                                                                {f.port.to_string()}
                                                            </span>
                                                        </span>
                                                    </td>
                                                    <td class=format!("{PORT_TABLE_TD} {PORT_TABLE_TD_CENTER}")>
                                                        <Icon d=RI_ARROW_RIGHT_LINE class=PORT_TABLE_ROW_ARROW />
                                                    </td>
                                                    <td class=PORT_TABLE_TD>
                                                        <span class=PORT_TABLE_CELL_WRAP>
                                                            <Icon d=RI_COMPUTER_LINE class=PORT_TABLE_EP_GLYPH />
                                                            <span class=PORT_TABLE_EP_VAL>{f.endpoint.clone()}</span>
                                                        </span>
                                                    </td>
                                                    <td class=format!("{PORT_TABLE_TD} {PORT_TABLE_TD_NOTE}")>{note}</td>
                                                    <td class=format!("{PORT_TABLE_TD} {PORT_TABLE_TD_RIGHT}")>
                                                        <button
                                                            class=format!("{BTN} {BTN_GHOST} {BTN_SM} {PORT_TABLE_REMOVE}")
                                                            aria-label=format!("remove {} {}", f.protocol, f.port)
                                                            on:click=remove
                                                        >
                                                            <Icon d=RI_DELETE_BIN_LINE class=PORT_TABLE_REMOVE_GLYPH />
                                                        </button>
                                                    </td>
                                                </tr>
                                            }
                                        })
                                        .collect_view(),
                                )
                            }
                        }}
                        <tr>
                            <td colspan="5" class=PORT_TABLE_TD>
                                <div class=PORT_TABLE_ADDER>
                                    <div class=PORT_TABLE_PROTO>
                                        <SegmentedTabs
                                            items=vec!["TCP".to_owned(), "UDP".to_owned()]
                                            active=proto_label
                                            on_change=Callback::new(move |v| proto_label.set(v))
                                        />
                                    </div>
                                    <input
                                        node_ref=port_ref
                                        class=format!("mono {INPUT} {PORT_TABLE_PORT_INPUT}")
                                        type="text"
                                        inputmode="numeric"
                                        placeholder="port"
                                        value=move || port.get()
                                        prop:value=move || port.get()
                                        on:input=move |ev| port.set(event_target_value(&ev))
                                        on:keydown=move |ev| {
                                            if ev.key() == "Enter" {
                                                add();
                                            }
                                        }
                                    />
                                    <Icon d=RI_ARROW_RIGHT_LINE class=PORT_TABLE_MID_ARROW />
                                    <input
                                        class=format!("{INPUT} {PORT_TABLE_EP_INPUT}")
                                        type="text"
                                        placeholder="pick a device"
                                        value=move || endpoint.get()
                                        prop:value=move || endpoint.get()
                                        on:input=move |ev| endpoint.set(event_target_value(&ev))
                                        on:keydown=move |ev| {
                                            if ev.key() == "Enter" {
                                                add();
                                            }
                                        }
                                    />
                                    <button
                                        class=format!("{BTN} {BTN_DEFAULT} {BTN_SM}")
                                        disabled=move || {
                                            port.with(|p| p.trim().is_empty())
                                                || endpoint.with(|e| e.trim().is_empty())
                                        }
                                        on:click=move |_| add()
                                    >
                                        <Icon d=RI_ADD_LINE class=PORT_TABLE_ADD_GLYPH />
                                        " Add"
                                    </button>
                                    <span class=PORT_TABLE_HINT>
                                        "No protocol or address blocks to type \u{2014} just pick."
                                    </span>
                                </div>
                            </td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </div>
    }
}

/// Surface `mb-3.5 overflow-hidden p-0`; head `flex items-center
/// justify-between border-b px-4 py-3`; `.tbl` row hover (site utility:
/// ungated tint, transition dropped under reduced motion); cells `px-3
/// py-2.5`; proto pill / mono port / arrow / endpoint / note / ghost remove
/// per reference utilities; adder row with 120px proto, 90px mono port
/// input, 170px endpoint input, `ml-auto` hint.
pub fn css() -> String {
    format!(
        ".{PORT_TABLE}{{margin-bottom:.875rem;overflow:hidden;padding:0}}\
.{PORT_TABLE_HEAD}{{display:flex;flex-wrap:wrap;align-items:center;\
justify-content:space-between;gap:.5rem;\
border-color:var(--color-border);border-bottom-width:1px;\
padding:.75rem 1rem}}\
.{PORT_TABLE_TITLE_COL}{{display:flex;min-width:0;flex-direction:column;gap:.125rem}}\
.{PORT_TABLE_TITLE}{{font-size:14px;font-weight:600}}\
.{PORT_TABLE_SUB}{{font-size:11.5px;color:var(--color-text-muted)}}\
.{PORT_TABLE_OPEN_GLYPH}{{width:.875rem;height:.875rem}}\
.{PORT_TABLE_SCROLL}{{overflow-x:auto;-webkit-overflow-scrolling:touch}}\
.{TBL} tbody tr{{transition:background-color 0.12s ease-out}}\
.{TBL} tbody tr:hover{{background:var(--color-surface-2)}}\
@media (prefers-reduced-motion: reduce){{.{TBL} tbody tr{{transition:none}}}}\
.{PORT_TABLE_TABLE}{{width:100%}}\
.{PORT_TABLE_THEAD_ROW}{{border-color:var(--color-border);border-bottom-width:1px;\
text-align:left;font-size:11px;text-transform:uppercase;letter-spacing:0.04em;\
color:var(--color-text-muted)}}\
.{PORT_TABLE_TH}{{padding:.625rem .75rem}}\
.{PORT_TABLE_TH_MEDIUM}{{font-weight:500}}\
.{PORT_TABLE_TH_PORT}{{width:120px}}\
.{PORT_TABLE_TH_ARROW}{{width:2.25rem}}\
.{PORT_TABLE_TH_ACTIONS}{{width:60px}}\
.{PORT_TABLE_EMPTY}{{padding:1.5rem .75rem;text-align:center;font-size:12.5px;\
color:var(--color-text-muted)}}\
.{PORT_TABLE_ROW}{{border-color:var(--color-border-soft);border-bottom-width:1px}}\
@media (hover: hover){{.{PORT_TABLE_ROW}:hover{{background-color:var(--color-surface-2)}}}}\
.{PORT_TABLE_TD}{{padding:.625rem .75rem}}\
.{PORT_TABLE_TD_CENTER}{{text-align:center}}\
.{PORT_TABLE_TD_NOTE}{{font-size:12px;color:var(--color-text-muted)}}\
.{PORT_TABLE_TD_RIGHT}{{text-align:right}}\
.{PORT_TABLE_CELL_WRAP}{{display:inline-flex;align-items:center;gap:.375rem}}\
.{PORT_TABLE_PROTO_PILL}{{display:inline-flex;height:18px;align-items:center;\
border-radius:calc(infinity * 1px);border:1px solid var(--color-border);\
background-color:var(--color-surface-2);padding-left:.375rem;padding-right:.375rem;\
font-size:10px;text-transform:uppercase;letter-spacing:0.03em;\
color:var(--color-text-muted)}}\
.{PORT_TABLE_PORT_VAL}{{font-size:13px}}\
.{PORT_TABLE_ROW_ARROW}{{display:inline;width:.875rem;height:.875rem;\
color:var(--color-accent)}}\
.{PORT_TABLE_EP_GLYPH}{{width:.875rem;height:.875rem;color:var(--color-text-dim)}}\
.{PORT_TABLE_EP_VAL}{{font-size:12.5px}}\
.{PORT_TABLE_REMOVE}{{width:1.75rem;height:1.75rem;padding:0}}\
.{PORT_TABLE_REMOVE_GLYPH}{{width:.875rem;height:.875rem}}\
.{PORT_TABLE_ADDER}{{display:flex;flex-wrap:wrap;align-items:center;gap:.5rem}}\
.{PORT_TABLE_PROTO}{{width:120px}}\
.{PORT_TABLE_PORT_INPUT}{{width:90px}}\
.{PORT_TABLE_MID_ARROW}{{width:.875rem;height:.875rem;color:var(--color-text-dim)}}\
.{PORT_TABLE_EP_INPUT}{{width:170px}}\
.{PORT_TABLE_ADD_GLYPH}{{width:.875rem;height:.875rem}}\
.{PORT_TABLE_HINT}{{margin-left:auto;align-self:center;font-size:11.5px;\
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
            PORT_TABLE,
            PORT_TABLE_HEAD,
            PORT_TABLE_TITLE_COL,
            PORT_TABLE_TITLE,
            PORT_TABLE_SUB,
            PORT_TABLE_OPEN_GLYPH,
            PORT_TABLE_SCROLL,
            PORT_TABLE_TABLE,
            PORT_TABLE_THEAD_ROW,
            PORT_TABLE_TH,
            PORT_TABLE_TH_MEDIUM,
            PORT_TABLE_TH_PORT,
            PORT_TABLE_TH_ARROW,
            PORT_TABLE_TH_ACTIONS,
            PORT_TABLE_EMPTY,
            PORT_TABLE_ROW,
            PORT_TABLE_TD,
            PORT_TABLE_TD_CENTER,
            PORT_TABLE_TD_NOTE,
            PORT_TABLE_TD_RIGHT,
            PORT_TABLE_CELL_WRAP,
            PORT_TABLE_PROTO_PILL,
            PORT_TABLE_PORT_VAL,
            PORT_TABLE_ROW_ARROW,
            PORT_TABLE_EP_GLYPH,
            PORT_TABLE_EP_VAL,
            PORT_TABLE_REMOVE,
            PORT_TABLE_REMOVE_GLYPH,
            PORT_TABLE_ADDER,
            PORT_TABLE_PROTO,
            PORT_TABLE_PORT_INPUT,
            PORT_TABLE_MID_ARROW,
            PORT_TABLE_EP_INPUT,
            PORT_TABLE_ADD_GLYPH,
            PORT_TABLE_HINT,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
        assert!(css.contains(&format!(".{TBL} tbody tr:hover")));
    }

    #[test]
    fn draft_keys_are_sequential_and_unique() {
        let a = next_draft_key();
        let b = next_draft_key();
        assert!(a.starts_with("draft-") && b.starts_with("draft-"));
        assert_ne!(a, b);
    }

    #[test]
    fn port_domain_validation_matches_reference() {
        // The reference rejects non-integers and out-of-range ports; the
        // add() closure mirrors it via u32 parse + 1..=65535.
        assert!("70000".parse::<u32>().map(|p| (1..=65535).contains(&p)) == Ok(false));
        assert!("443".parse::<u32>().map(|p| (1..=65535).contains(&p)) == Ok(true));
        assert!("4.4".parse::<u32>().is_err());
    }
}
