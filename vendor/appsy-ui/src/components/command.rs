//! Command — port of `components/ui/command.tsx` (cmdk wrapped with the
//! site's utility classNames and a Remix search icon). Machine (cmdk, the
//! defaults the site ships):
//!
//! ```text
//! typing --> every item scores against the search (command-score);
//!            score 0 unmounts the item; a group with no visible items gets
//!            `hidden`; the empty slot renders when nothing matches;
//!            after each filter pass the visible items are stable-sorted by
//!            score (desc) within their group wrap via appendChild, and
//!            selection resets to the first item in DOM order
//! clearing --> no sort runs and moved nodes are never restored: the sorted
//!            order persists; an unmounted item remounts before its nearest
//!            still-mounted successor (React's placement anchor), so the
//!            post-clear order interleaves deterministically
//! groups  --> never reorder (cmdk's group sort queries
//!            data-value="encodeURIComponent(id)" against the raw attribute
//!            — a silent no-op; measured on the reference: "PoPs" top-scores
//!            for "pop" yet "Go to" stays first)
//! ArrowDown/ArrowUp (and vim ctrl-n/j / ctrl-p/k) --> move selection
//!            through visible items in DOM order, no wrap;
//!            Home/End first/last
//! Enter --> runs the selected item's on_select
//! pointer over an item --> selection follows
//! ```
//!
//! DOM mirrors cmdk: root `cmdk-root` with a visually-hidden label, the
//! input wrapper (icon + combobox input), `cmdk-list` (`role=listbox`,
//! "Suggestions") over a `cmdk-list-sizer`, groups as
//! `presentation`/heading/`role=group` triples, items as `role=option`
//! with `data-selected`/`aria-selected`. Item values derive from rendered
//! text when no `value` prop is given, exactly as cmdk reads
//! `textContent`.

use leptos::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::icons::{Icon, RI_SEARCH_LINE};

pub const CMD: &str = "asy-cmd";
pub const CMD_INPUT_WRAP: &str = "asy-cmd__input-wrap";
pub const CMD_INPUT_ICON: &str = "asy-cmd__input-icon";
pub const CMD_INPUT: &str = "asy-cmd__input";
pub const CMD_LIST: &str = "asy-cmd__list";
pub const CMD_EMPTY: &str = "asy-cmd__empty";
pub const CMD_GROUP: &str = "asy-cmd__group";
pub const CMD_GROUP_HEADING: &str = "asy-cmd__group-heading";
pub const CMD_ITEM: &str = "asy-cmd__item";
pub const CMD_SEP: &str = "asy-cmd__sep";

/// cmdk's visually-hidden label style, verbatim.
const SR_ONLY: &str = "position: absolute; width: 1px; height: 1px; padding: 0px; margin: -1px; overflow: hidden; clip: rect(0px, 0px, 0px, 0px); white-space: nowrap; border-width: 0px;";

// --- command-score (Superhuman's algorithm, as vendored by cmdk) ---

const SCORE_CONTINUE_MATCH: f64 = 1.0;
const SCORE_SPACE_WORD_JUMP: f64 = 0.9;
const SCORE_NON_SPACE_WORD_JUMP: f64 = 0.8;
const SCORE_CHARACTER_JUMP: f64 = 0.17;
const SCORE_TRANSPOSITION: f64 = 0.1;
const PENALTY_SKIPPED: f64 = 0.999;
const PENALTY_CASE_MISMATCH: f64 = 0.9999;
const PENALTY_NOT_COMPLETE: f64 = 0.99;

fn is_gap(c: char) -> bool {
    matches!(c, '\\' | '/' | '_' | '+' | '.' | '#' | '"' | '@' | '[' | '(' | '{' | '&')
}

fn is_space(c: char) -> bool {
    c.is_whitespace() || c == '-'
}

/// Lowercase with space-class characters normalized to plain spaces
/// (command-score's `formatInput`).
fn normalize(s: &str) -> Vec<char> {
    s.to_lowercase()
        .chars()
        .map(|c| if is_space(c) { ' ' } else { c })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn score_inner(
    string: &[char],
    abbr: &[char],
    lower_string: &[char],
    lower_abbr: &[char],
    string_index: usize,
    abbr_index: usize,
    memo: &mut std::collections::HashMap<(usize, usize), f64>,
) -> f64 {
    if abbr_index == abbr.len() {
        return if string_index == string.len() {
            SCORE_CONTINUE_MATCH
        } else {
            PENALTY_NOT_COMPLETE
        };
    }
    if let Some(&hit) = memo.get(&(string_index, abbr_index)) {
        return hit;
    }
    let abbr_char = lower_abbr[abbr_index];
    let mut high_score: f64 = 0.0;
    let mut index = lower_string[string_index..]
        .iter()
        .position(|&c| c == abbr_char)
        .map(|p| p + string_index);
    while let Some(i) = index {
        let mut score =
            score_inner(string, abbr, lower_string, lower_abbr, i + 1, abbr_index + 1, memo);
        if score > high_score {
            if i == string_index {
                score *= SCORE_CONTINUE_MATCH;
            } else if is_gap(string[i - 1]) {
                score *= SCORE_NON_SPACE_WORD_JUMP;
                if string_index > 0 && i >= 1 {
                    let breaks =
                        string[string_index..i - 1].iter().filter(|c| is_gap(**c)).count();
                    if breaks > 0 {
                        score *= PENALTY_SKIPPED.powi(breaks as i32);
                    }
                }
            } else if is_space(string[i - 1]) {
                score *= SCORE_SPACE_WORD_JUMP;
                if string_index > 0 && i >= 1 {
                    let breaks =
                        string[string_index..i - 1].iter().filter(|c| is_space(**c)).count();
                    if breaks > 0 {
                        score *= PENALTY_SKIPPED.powi(breaks as i32);
                    }
                }
            } else {
                score *= SCORE_CHARACTER_JUMP;
                if string_index > 0 {
                    score *= PENALTY_SKIPPED.powi((i - string_index) as i32);
                }
            }
            if string[i] != abbr[abbr_index] {
                score *= PENALTY_CASE_MISMATCH;
            }
        }
        let prev_lower = if i > 0 { Some(lower_string[i - 1]) } else { None };
        let next_abbr = lower_abbr.get(abbr_index + 1).copied();
        let transpose_a = score < SCORE_TRANSPOSITION && prev_lower.is_some() && prev_lower == next_abbr;
        let transpose_b = next_abbr == Some(abbr_char) && prev_lower != Some(abbr_char);
        if (transpose_a || transpose_b) && next_abbr.is_some() {
            let transposed =
                score_inner(string, abbr, lower_string, lower_abbr, i + 1, abbr_index + 2, memo);
            if transposed * SCORE_TRANSPOSITION > score {
                score = transposed * SCORE_TRANSPOSITION;
            }
        }
        if score > high_score {
            high_score = score;
        }
        index = lower_string[i + 1..]
            .iter()
            .position(|&c| c == abbr_char)
            .map(|p| p + i + 1);
    }
    memo.insert((string_index, abbr_index), high_score);
    high_score
}

/// `commandScore(string, abbreviation)` — 0 means no match.
pub fn command_score(string: &str, abbreviation: &str) -> f64 {
    let s: Vec<char> = string.chars().collect();
    let a: Vec<char> = abbreviation.chars().collect();
    let ls = normalize(string);
    let la = normalize(abbreviation);
    score_inner(&s, &a, &ls, &la, 0, 0, &mut std::collections::HashMap::new())
}

// --- component ---

static CMD_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct ItemReg {
    /// Effective value; empty until derived from the DOM when no `value`
    /// prop was given.
    value: RwSignal<String>,
    disabled: bool,
    on_select: Option<Callback<String>>,
    /// The item's element — the sort pass and remount placement move it.
    #[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(dead_code))]
    node: NodeRef<leptos::html::Div>,
}

#[derive(Clone, Copy)]
struct CommandCtx {
    /// Base for every generated id. Hydration re-derives it from the
    /// server-rendered input's id (the SSR and client counters diverge),
    /// so it is reactive and every id attribute reads it.
    base_id: RwSignal<String>,
    search: RwSignal<String>,
    /// Selected item's registry index.
    selected: RwSignal<Option<usize>>,
    /// The `aria-activedescendant` value on input and list. cmdk sets it
    /// from a deferred callback on every selection-value change, and the
    /// callback's timing decides what it sees: from a keyboard/pointer
    /// event it runs post-commit and stamps the NEW selection's id; from
    /// the filter path it runs inside the same layout-effect flush
    /// (Map.forEach visits entries added mid-iteration) against pre-emit
    /// DOM — stamping the PREVIOUSLY-selected element's id if that element
    /// survived the filter commit, else clearing. Pristine state: absent.
    /// Measured on the reference: after "te" the attribute names Tunnels
    /// while Team carries aria-selected.
    active_id: RwSignal<Option<String>>,
    items: RwSignal<Vec<ItemReg>>,
}

impl CommandCtx {
    fn matches(&self, value: &str) -> bool {
        self.search.with(|s| s.is_empty() || command_score(value, s) > 0.0)
    }

    fn visible_indices(&self) -> Vec<usize> {
        self.items.with(|items| {
            items
                .iter()
                .enumerate()
                .filter(|(_, it)| {
                    !it.disabled && it.value.with(|v| {
                        self.search.with(|s| s.is_empty() || command_score(v, s) > 0.0)
                    })
                })
                .map(|(i, _)| i)
                .collect()
        })
    }
}

/// Registry index encoded in an item element's id (`{base}-item-{index}`).
#[cfg(any(feature = "csr", feature = "hydrate"))]
fn item_index_of(el: &web_sys::Element, base: &str) -> Option<usize> {
    el.id().strip_prefix(&format!("{base}-item-"))?.parse().ok()
}

/// Visible, non-disabled registry indices in DOM order — cmdk's
/// `getValidItems()` (a live querySelectorAll over the list).
#[cfg(any(feature = "csr", feature = "hydrate"))]
fn dom_visible_indices(ctx: &CommandCtx) -> Vec<usize> {
    use wasm_bindgen::JsCast;
    let document = leptos::tachys::dom::document();
    let base = ctx.base_id.get_untracked();
    let Some(list) = document.get_element_by_id(&format!("{base}-list")) else {
        return ctx.visible_indices();
    };
    let Ok(nodes) = list.query_selector_all("[cmdk-item]:not([aria-disabled=\"true\"])") else {
        return ctx.visible_indices();
    };
    let mut out = Vec::new();
    for i in 0..nodes.length() {
        if let Some(el) = nodes.item(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok()) {
            if let Some(idx) = item_index_of(&el, &base) {
                out.push(idx);
            }
        }
    }
    out
}

/// Non-wasm targets never dispatch events; registration order stands in.
#[cfg(not(any(feature = "csr", feature = "hydrate")))]
fn dom_visible_indices(ctx: &CommandCtx) -> Vec<usize> {
    ctx.visible_indices()
}

/// cmdk's sort pass: stable-sort the mounted items by score against the
/// *incoming* search (desc) within each parent wrap via appendChild — a
/// real DOM move that persists across searches. Runs synchronously in the
/// input event, BEFORE the filter commits: items about to unmount are
/// sorted (then removed), items about to remount are absent and later
/// append unsorted — the reference's measured backspace interleaving
/// depends on exactly this ordering. Groups are deliberately not reordered
/// (the reference's group sort is a measured no-op; see the module docs).
#[cfg(any(feature = "csr", feature = "hydrate"))]
fn sort_items(ctx: &CommandCtx, search: &str) {
    use wasm_bindgen::JsCast;
    if search.is_empty() {
        return;
    }
    let document = leptos::tachys::dom::document();
    let base = ctx.base_id.get_untracked();
    let Some(list) = document.get_element_by_id(&format!("{base}-list")) else { return };
    {
        if let Ok(nodes) = list.query_selector_all("[cmdk-item]") {
            let mut by_parent: Vec<(web_sys::Element, Vec<(web_sys::Element, f64)>)> = Vec::new();
            for i in 0..nodes.length() {
                let Some(el) = nodes.item(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok())
                else {
                    continue;
                };
                let Some(parent) = el.parent_element() else { continue };
                let score = item_index_of(&el, &base)
                    .and_then(|idx| {
                        ctx.items.with_untracked(|items| {
                            items
                                .get(idx)
                                .map(|it| it.value.with_untracked(|v| command_score(v, search)))
                        })
                    })
                    .unwrap_or(0.0);
                match by_parent.iter_mut().find(|(p, _)| *p == parent) {
                    Some((_, v)) => v.push((el, score)),
                    None => by_parent.push((parent, vec![(el, score)])),
                }
            }
            for (parent, mut scored) in by_parent {
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                for (el, _) in scored {
                    let _ = parent.append_child(el.as_ref());
                }
            }
        }
    }
}

/// cmdk's `scrollSelectedIntoView`: when the element is the first item of
/// its wrap, bring the group heading into view first, then the element
/// (`block: nearest` both). Beyond the visual, the scroll resets Chrome's
/// sequential-focus starting point — trace-relevant even when no scrolling
/// occurs.
#[cfg(any(feature = "csr", feature = "hydrate"))]
fn scroll_item_into_view(el: &web_sys::Element) {
    let opts = web_sys::ScrollIntoViewOptions::new();
    opts.set_block(web_sys::ScrollLogicalPosition::Nearest);
    let is_first = el
        .parent_element()
        .and_then(|p| p.first_element_child())
        .is_some_and(|first| &first == el);
    if is_first {
        if let Some(heading) = el
            .closest("[cmdk-group]")
            .ok()
            .flatten()
            .and_then(|g| g.query_selector("[cmdk-group-heading]").ok().flatten())
        {
            heading.scroll_into_view_with_scroll_into_view_options(&opts);
        }
    }
    el.scroll_into_view_with_scroll_into_view_options(&opts);
}

/// cmdk's deferred selectFirstItem — runs after the filter commit (also on
/// a cleared search), over the post-commit DOM. On a selection-value
/// change the deferred stamp and scroll callbacks run in the same
/// layout-effect flush against pre-emit DOM, so both target the
/// PREVIOUSLY-selected element when it survived the commit:
/// `aria-activedescendant` takes its id (else clears — see `active_id`)
/// and it is scrolled into view (resetting the sequential-focus starting
/// point). No-op reselects leave both untouched.
#[cfg(any(feature = "csr", feature = "hydrate"))]
fn select_first(ctx: CommandCtx) {
    let document = leptos::tachys::dom::document();
    let base = ctx.base_id.get_untracked();
    let Some(list) = document.get_element_by_id(&format!("{base}-list")) else { return };
    let first = list.query_selector("[cmdk-item]:not([aria-disabled=\"true\"])").ok().flatten();
    let new_sel = first.and_then(|el| item_index_of(&el, &base));
    let prev = ctx.selected.get_untracked();
    if selection_value(&ctx, new_sel) != selection_value(&ctx, prev) {
        // getElementById only reaches connected nodes: Some ⟺ the previous
        // selection's element is still mounted.
        let prev_el =
            prev.and_then(|i| document.get_element_by_id(&format!("{base}-item-{i}")));
        ctx.active_id.set(prev_el.as_ref().map(web_sys::Element::id));
        if let Some(prev_el) = prev_el {
            scroll_item_into_view(&prev_el);
        }
    }
    ctx.selected.set(new_sel);
}

/// The selected item's effective value — cmdk's change detection compares
/// value strings, not positions.
fn selection_value(ctx: &CommandCtx, index: Option<usize>) -> Option<String> {
    index.and_then(|i| {
        ctx.items.with_untracked(|items| items.get(i).map(|it| it.value.get_untracked()))
    })
}

#[component]
pub fn Command(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let n = CMD_ID.fetch_add(1, Ordering::Relaxed);
    let ctx = CommandCtx {
        base_id: RwSignal::new(format!("asy-cmd-{n}")),
        search: RwSignal::new(String::new()),
        selected: RwSignal::new(Some(0)),
        active_id: RwSignal::new(None),
        items: RwSignal::new(Vec::new()),
    };
    let mut cls = CMD.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let keydown = move |ev: web_sys::KeyboardEvent| {
        // Navigation walks the *DOM order* (cmdk's getValidItems is a
        // querySelectorAll) — after a sort pass that differs from
        // registration order.
        let visible = dom_visible_indices(&ctx);
        if visible.is_empty() {
            return;
        }
        let pos = ctx
            .selected
            .get_untracked()
            .and_then(|sel| visible.iter().position(|i| *i == sel));
        let key = ev.key();
        let ctrl = ev.ctrl_key();
        let next: Option<usize> = match key.as_str() {
            "ArrowDown" => Some(pos.map_or(0, |p| (p + 1).min(visible.len() - 1))),
            "ArrowUp" => Some(pos.map_or(visible.len() - 1, |p| p.saturating_sub(1))),
            "n" | "j" if ctrl => Some(pos.map_or(0, |p| (p + 1).min(visible.len() - 1))),
            "p" | "k" if ctrl => {
                Some(pos.map_or(visible.len() - 1, |p| p.saturating_sub(1)))
            }
            "Home" => Some(0),
            "End" => Some(visible.len() - 1),
            "Enter" => {
                ev.prevent_default();
                if let Some(sel) = ctx.selected.get_untracked() {
                    let item = ctx.items.with_untracked(|items| items.get(sel).cloned());
                    if let Some(item) = item {
                        if let Some(cb) = item.on_select {
                            cb.run(item.value.get_untracked());
                        }
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(p) = next {
            ev.prevent_default();
            // cmdk's setState no-ops when the value is unchanged (an
            // Object.is guard): a boundary arrow press moves nothing —
            // no focus reset, no scroll, no aria-activedescendant stamp.
            if selection_value(&ctx, Some(visible[p]))
                == selection_value(&ctx, ctx.selected.get_untracked())
            {
                return;
            }
            ctx.selected.set(Some(visible[p]));
            // Event-context stamp: cmdk's deferred callback runs
            // post-commit here and captures the NEW selection's id.
            ctx.active_id
                .set(Some(format!("{}-item-{}", ctx.base_id.get_untracked(), visible[p])));
            #[cfg(any(feature = "csr", feature = "hydrate"))]
            {
                use wasm_bindgen::JsCast;
                let document = leptos::tachys::dom::document();
                // cmdk re-focuses its input when navigation moves the
                // value (a same-element focus() — no events, but it resets
                // the browser's sequential-focus starting point) and
                // scrolls the newly navigated-to item into view.
                let input_id = format!("{}-input", ctx.base_id.get_untracked());
                if let Some(input) = document
                    .get_element_by_id(&input_id)
                    .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = input.focus();
                }
                let id = format!("{}-item-{}", ctx.base_id.get_untracked(), visible[p]);
                if let Some(el) = document.get_element_by_id(&id) {
                    scroll_item_into_view(&el);
                }
            }
        }
    };
    view! {
        <div class=cls tabindex="-1" cmdk-root="" on:keydown=keydown>
            <label
                cmdk-label=""
                for=move || format!("{}-input", ctx.base_id.get())
                id=move || format!("{}-label", ctx.base_id.get())
                style=SR_ONLY
            ></label>
            // Scoped Provider — bare provide_context would let a later sibling
            // instance shadow this ctx for lazily-built children (see select.rs).
            <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider>
        </div>
    }
}

#[component]
pub fn CommandInput(#[prop(optional, into)] placeholder: Option<String>) -> impl IntoView {
    let ctx = use_context::<CommandCtx>().expect("invariant: CommandInput inside Command");
    let input_ref: NodeRef<leptos::html::Input> = NodeRef::new();
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    Effect::new(move |_| {
        // Hydration keeps the server-rendered id in the DOM; re-derive the
        // base from it so client-side ids and lookups stay consistent.
        if let Some(el) = input_ref.get() {
            let el: &web_sys::Element = el.as_ref();
            if let Some(base) = el.id().strip_suffix("-input") {
                if ctx.base_id.with_untracked(|b| b != base) {
                    ctx.base_id.set(base.to_owned());
                }
            }
        }
    });
    view! {
        <div class=CMD_INPUT_WRAP>
            <Icon d=RI_SEARCH_LINE class=CMD_INPUT_ICON />
            <input
                class=CMD_INPUT
                node_ref=input_ref
                placeholder=placeholder
                cmdk-input=""
                autocomplete="off"
                spellcheck="false"
                aria-autocomplete="list"
                role="combobox"
                aria-expanded="true"
                aria-controls=move || format!("{}-list", ctx.base_id.get())
                aria-labelledby=move || format!("{}-label", ctx.base_id.get())
                id=move || format!("{}-input", ctx.base_id.get())
                aria-activedescendant=move || ctx.active_id.get()
                type="text"
                value=move || ctx.search.get()
                prop:value=move || ctx.search.get()
                // cmdk's search path, in its exact order: sort mounted
                // items against the incoming search (pre-commit), commit
                // the filter, then re-select first post-commit.
                on:input:target=move |ev| {
                    let value = ev.target().value();
                    let changed = ctx.search.with_untracked(|s| s != &value);
                    #[cfg(any(feature = "csr", feature = "hydrate"))]
                    if changed {
                        sort_items(&ctx, &value);
                    }
                    ctx.search.set(value);
                    #[cfg(any(feature = "csr", feature = "hydrate"))]
                    if changed {
                        request_animation_frame(move || select_first(ctx));
                    }
                    let _ = changed;
                }
                {..leptos::tachys::html::attribute::custom::custom_attribute("autocorrect", "off")}
            />
        </div>
    }
}

#[component]
pub fn CommandList(children: Children) -> impl IntoView {
    let ctx = use_context::<CommandCtx>().expect("invariant: CommandList inside Command");
    view! {
        <div
            class=CMD_LIST
            cmdk-list=""
            role="listbox"
            tabindex="-1"
            aria-label="Suggestions"
            aria-activedescendant=move || ctx.active_id.get()
            id=move || format!("{}-list", ctx.base_id.get())
        >
            <div cmdk-list-sizer="">{children()}</div>
        </div>
    }
}

/// Rendered only while no item matches the search.
#[component]
pub fn CommandEmpty(children: ChildrenFn) -> impl IntoView {
    let ctx = use_context::<CommandCtx>().expect("invariant: CommandEmpty inside Command");
    let children = StoredValue::new(children);
    view! {
        <Show when=move || {
            !ctx.search.with(String::is_empty)
                && ctx.items.with(|items| {
                    !items.iter().any(|it| it.value.with(|v| ctx.matches(v)))
                })
        }>
            <div class=CMD_EMPTY cmdk-empty="" role="presentation">
                {children.with_value(|c| c())}
            </div>
        </Show>
    }
}

#[derive(Clone, Copy)]
struct GroupCtx {
    /// Group ordinal; the heading id derives from the reactive base.
    heading_id: StoredValue<u64>,
    /// Registry indices of this group's items.
    members: RwSignal<Vec<usize>>,
}

static GROUP_ID: AtomicU64 = AtomicU64::new(0);

#[component]
pub fn CommandGroup(
    #[prop(into)] heading: String,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<CommandCtx>().expect("invariant: CommandGroup inside Command");
    let g = GROUP_ID.fetch_add(1, Ordering::Relaxed);
    let group = GroupCtx {
        heading_id: StoredValue::new(g),
        members: RwSignal::new(Vec::new()),
    };
    let hidden = move || {
        !group.members.get().iter().any(|i| {
            ctx.items.with(|items| {
                items.get(*i).is_some_and(|it| it.value.with(|v| ctx.matches(v)))
            })
        })
    };
    view! {
        <div
            class=CMD_GROUP
            cmdk-group=""
            role="presentation"
            data-value=heading.clone()
            
            hidden=move || hidden().then_some("")
        >
            <div
                class=CMD_GROUP_HEADING
                cmdk-group-heading=""
                aria-hidden="true"
                id=move || format!("{}-heading-{}", ctx.base_id.get(), group.heading_id.get_value())
            >
                {heading.clone()}
            </div>
            <div
                cmdk-group-items=""
                role="group"
                aria-labelledby=move || format!("{}-heading-{}", ctx.base_id.get(), group.heading_id.get_value())
            >
                <leptos::context::Provider value=group>{children()}</leptos::context::Provider>
            </div>
        </div>
    }
}

#[component]
pub fn CommandItem(
    /// cmdk derives the value from rendered text when absent.
    #[prop(optional, into)]
    value: Option<String>,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] on_select: Option<Callback<String>>,
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let ctx = use_context::<CommandCtx>().expect("invariant: CommandItem inside Command");
    let group = use_context::<GroupCtx>();
    let value_sig = RwSignal::new(value.unwrap_or_default());
    let node_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let index = ctx.items.try_update(|items| {
        items.push(ItemReg { value: value_sig, disabled, on_select, node: node_ref });
        items.len() - 1
    });
    let index = index.unwrap_or(0);
    if let Some(group) = group {
        group.members.update(|m| m.push(index));
    }
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    Effect::new(move |_| {
        // cmdk reads `textContent` for items without an explicit value.
        if let Some(el) = node_ref.get() {
            if value_sig.with_untracked(String::is_empty) {
                let el: &web_sys::Element = el.as_ref();
                value_sig.set(el.text_content().unwrap_or_default());
            }
        }
    });

    #[cfg_attr(not(any(feature = "csr", feature = "hydrate")), allow(unused_variables))]
    let visible = move || value_sig.with(|v| v.is_empty() || ctx.matches(v));

    // Filtering is a manual detach/attach, not a `<Show>`: cmdk's sort pass
    // physically moves item nodes and the moved order must persist, while a
    // Show's markers would pin every remount back to its original slot. A
    // remounting item inserts before its nearest still-mounted successor in
    // registration order (React's placement anchor — verified against the
    // reference's post-clear interleaving), else appends to the wrap it was
    // detached from.
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        let members = group.map(|g| g.members);
        let wrap: StoredValue<Option<web_sys::Element>, LocalStorage> =
            StoredValue::new_local(None);
        Effect::new(move |_| {
            let vis = visible();
            let Some(el) = node_ref.get() else { return };
            let el: web_sys::Element = (*el).clone().into();
            if vis == el.is_connected() {
                return;
            }
            if !vis {
                wrap.set_value(el.parent_element());
                el.remove();
                return;
            }
            let order = members
                .map(|m| m.get_untracked())
                .unwrap_or_else(|| ctx.items.with_untracked(|it| (0..it.len()).collect()));
            let anchor: Option<web_sys::Element> = order
                .iter()
                .skip_while(|i| **i != index)
                .skip(1)
                .find_map(|i| {
                    ctx.items
                        .with_untracked(|items| items.get(*i).and_then(|it| it.node.get_untracked()))
                        .map(|n| -> web_sys::Element { (*n).clone().into() })
                        .filter(|n| n.is_connected())
                });
            match anchor {
                Some(anchor) => {
                    if let Some(parent) = anchor.parent_element() {
                        let _ = parent.insert_before(&el, Some(anchor.as_ref()));
                    }
                }
                None => {
                    if let Some(parent) = wrap.get_value() {
                        let _ = parent.append_child(&el);
                    }
                }
            }
        });
    }

    let is_selected = move || ctx.selected.get() == Some(index);
    let mut cls = CMD_ITEM.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let cls = StoredValue::new(cls);
    let children = StoredValue::new(children);
    view! {
        <div
            class=cls.get_value()
            node_ref=node_ref
            id=move || format!("{}-item-{index}", ctx.base_id.get())
            cmdk-item=""
            role="option"
            aria-disabled=if disabled { "true" } else { "false" }
            aria-selected=move || if is_selected() { "true" } else { "false" }
            data-disabled=if disabled { "true" } else { "false" }
            data-selected=move || if is_selected() { "true" } else { "false" }
            data-value=move || value_sig.get()
            on:pointermove=move |_| {
                if !disabled
                    && selection_value(&ctx, Some(index))
                        != selection_value(&ctx, ctx.selected.get_untracked())
                {
                    ctx.selected.set(Some(index));
                    // Event-context stamp: post-commit, the new selection.
                    ctx.active_id
                        .set(Some(format!("{}-item-{index}", ctx.base_id.get_untracked())));
                }
            }
            on:click=move |_| {
                if !disabled {
                    if let Some(cb) = on_select {
                        cb.run(value_sig.get_untracked());
                    }
                }
            }
        >
            {children.with_value(|c| c())}
        </div>
    }
}

/// cmdk separators hide while a search is active.
#[component]
pub fn CommandSeparator(#[prop(optional, into)] class: Option<String>) -> impl IntoView {
    let ctx = use_context::<CommandCtx>().expect("invariant: CommandSeparator inside Command");
    let mut cls = CMD_SEP.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <Show when=move || ctx.search.with(String::is_empty)>
            <div class=cls.clone() cmdk-separator="" role="separator"></div>
        </Show>
    }
}

/// The site's utility classNames translated; cmdk's own vocabulary attrs
/// (`cmdk-*`) carry no styles here (its stylesheet is not imported).
pub fn css() -> String {
    format!(
        ".{CMD}{{display:flex;height:100%;width:100%;flex-direction:column;\
overflow:hidden;border-radius:var(--radius-md);\
background-color:var(--color-surface);color:var(--color-text)}}\
.{CMD_INPUT_WRAP}{{display:flex;align-items:center;gap:.5rem;\
border-bottom-width:1px;border-color:var(--color-border);\
padding-left:.75rem;padding-right:.75rem}}\
.{CMD_INPUT_ICON}{{width:1rem;height:1rem;color:var(--color-text-dim)}}\
.{CMD_INPUT}{{display:flex;height:2.5rem;width:100%;\
background-color:transparent;font-size:.875rem;\
line-height:calc(1.25/.875);outline:none;border:0;padding:0;\
color:inherit;font-family:inherit}}\
.{CMD_INPUT}::placeholder{{color:var(--color-text-dim)}}\
.{CMD_LIST}{{max-height:18rem;overflow-y:auto;overflow-x:hidden}}\
.{CMD_EMPTY}{{padding-top:1.5rem;padding-bottom:1.5rem;text-align:center;\
font-size:.875rem;line-height:calc(1.25/.875);\
color:var(--color-text-muted)}}\
.{CMD_GROUP}{{overflow:hidden;padding:.25rem;color:var(--color-text)}}\
.{CMD_GROUP_HEADING}{{padding:.375rem .5rem;font-size:11.5px;\
font-weight:500;text-transform:uppercase;letter-spacing:.04em;\
color:var(--color-text-dim)}}\
.{CMD_ITEM}{{position:relative;display:flex;min-width:0;cursor:pointer;\
user-select:none;align-items:center;gap:.5rem;\
border-radius:var(--radius-sm);padding:.375rem .5rem;\
font-size:.875rem;line-height:calc(1.25/.875);outline:none}}\
.{CMD_ITEM}[data-selected=true]{{background-color:var(--color-surface-2);\
color:var(--color-text)}}\
.{CMD_ITEM}[data-disabled=true]{{pointer-events:none;opacity:.5}}\
.{CMD_SEP}{{margin-left:-.25rem;margin-right:-.25rem;height:1px;\
background-color:var(--color-border-soft)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_matches_command_score_semantics() {
        // Exact match is 1, prefix continues at 0.99 completeness penalty.
        assert_eq!(command_score("Tunnels", "tunnels"), PENALTY_CASE_MISMATCH);
        assert_eq!(command_score("tunnels", "tunnels"), 1.0);
        assert!(command_score("Tunnels", "tun") > 0.0);
        assert!(command_score("Create tunnel…", "tun") > 0.0);
        assert_eq!(command_score("Overview", "zz"), 0.0);
        // Word-jump beats character-jump.
        assert!(command_score("Claim IP from pool…", "ip") > command_score("Devices", "ic"));
        // The transposition path scores adjacent swaps at 0.1 — matching
        // command-score exactly, which does NOT require strict order.
        assert_eq!(command_score("abc", "cb"), SCORE_TRANSPOSITION);
    }

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            CMD,
            CMD_INPUT_WRAP,
            CMD_INPUT_ICON,
            CMD_INPUT,
            CMD_LIST,
            CMD_EMPTY,
            CMD_GROUP,
            CMD_GROUP_HEADING,
            CMD_ITEM,
            CMD_SEP,
        ] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
    }
}
