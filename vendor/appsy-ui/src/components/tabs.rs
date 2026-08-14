//! Tabs — port of `components/ui/tabs.tsx` (Radix Tabs, horizontal,
//! automatic activation). Machine (WAI-ARIA tabs pattern, Radix roving
//! focus — same delegation scheme as RadioGroup):
//!
//! ```text
//! Tab        -> the tablist is the single tab stop; focus forwards to the
//!               active trigger (triggers rest at tabindex -1)
//! Left/Right -> focus + activate prev/next trigger, wrapping
//! Home/End   -> focus + activate first/last trigger
//! click      -> activate
//! Tab (again)-> the active panel (tabindex 0)
//! ```
//!
//! Inactive panels stay in the DOM as empty `hidden` tabpanels — Radix only
//! mounts the active panel's children. Ids follow the Radix shape
//! (`{base}-trigger-{value}` / `{base}-content-{value}`) deterministically.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

pub const TABS_LIST: &str = "asy-tabs__list";
pub const TABS_TRIGGER: &str = "asy-tabs__trigger";
pub const TABS_TRIGGER_ACTIVE: &str = "asy-tabs__trigger--active";
pub const TABS_CONTENT: &str = "asy-tabs__content";

#[derive(Clone, Copy)]
struct TabsCtx {
    value: RwSignal<String>,
    on_value_change: Option<Callback<String>>,
    base_id: StoredValue<String>,
    /// Registration order == DOM order (SSR render order).
    items: StoredValue<Vec<String>>,
    /// Roving tab stop: the trigger that last held focus; `None` at rest —
    /// the tablist delegates.
    current_tab_stop: RwSignal<Option<usize>>,
}

static TABS_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[component]
pub fn Tabs(
    #[prop(into)] default_value: String,
    #[prop(optional)] on_value_change: Option<Callback<String>>,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let n = TABS_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ctx = TabsCtx {
        value: RwSignal::new(default_value),
        on_value_change,
        base_id: StoredValue::new(format!("asy-tabs-{n}")),
        items: StoredValue::new(Vec::new()),
        current_tab_stop: RwSignal::new(None),
    };
    view! {
        <div dir="ltr" data-orientation="horizontal" class=class>
            // Scoped Provider — bare provide_context would let a later sibling
            // instance shadow this ctx for lazily-built children (see select.rs).
            <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider>
        </div>
    }
}

#[component]
pub fn TabsList(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<TabsCtx>().expect("invariant: TabsList inside Tabs");
    let mut cls = TABS_LIST.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <div
            role="tablist"
            aria-orientation="horizontal"
            data-orientation="horizontal"
            tabindex="0"
            class=cls
            // Focus landing on the list itself forwards to the active
            // trigger, per Radix roving focus.
            on:focus=move |ev| {
                let same = ev
                    .target()
                    .zip(ev.current_target())
                    .is_some_and(|(t, c)| t == c);
                if !same {
                    return;
                }
                let target_index = ctx
                    .value
                    .with(|v| ctx.items.with_value(|items| items.iter().position(|i| i == v)))
                    .unwrap_or(0);
                if let Some(el) = ev
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    .and_then(|list| {
                        list.query_selector_all("[role=tab]")
                            .ok()
                            .and_then(|l| l.item(target_index as u32))
                    })
                    .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = el.focus();
                }
            }
        >
            {children()}
        </div>
    }
}

#[component]
pub fn TabsTrigger(
    #[prop(into)] value: String,
    #[prop(optional)] disabled: bool,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<TabsCtx>().expect("invariant: TabsTrigger inside Tabs");
    let index = {
        let mut idx = 0;
        ctx.items.update_value(|items| {
            idx = items.len();
            items.push(value.clone());
        });
        idx
    };
    let is_active = {
        let value = value.clone();
        move || ctx.value.with(|v| v == &value)
    };
    let tab_index = move || {
        if ctx.current_tab_stop.get() == Some(index) { "0" } else { "-1" }
    };
    let select = {
        let value = value.clone();
        move || {
            ctx.value.set(value.clone());
            if let Some(cb) = ctx.on_value_change {
                cb.run(value.clone());
            }
        }
    };
    let on_click = {
        let select = select.clone();
        move |_| select()
    };
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        let key = ev.key();
        let count = ctx.items.with_value(Vec::len);
        if count == 0 {
            return;
        }
        let next_index = match key.as_str() {
            "ArrowRight" => (index + 1) % count,
            "ArrowLeft" => (index + count - 1) % count,
            "Home" => 0,
            "End" => count - 1,
            _ => return,
        };
        ev.prevent_default();
        let next_value = ctx.items.with_value(|items| items[next_index].clone());
        // Focus the sibling in DOM order, then activate it (automatic
        // activation: arrow navigation selects).
        if let Some(target) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest("[role=tablist]").ok().flatten())
            .and_then(|list| {
                list.query_selector_all("[role=tab]")
                    .ok()
                    .and_then(|l| l.item(next_index as u32))
            })
            .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = target.focus();
        }
        ctx.value.set(next_value.clone());
        if let Some(cb) = ctx.on_value_change {
            cb.run(next_value);
        }
    };
    let is_active_state = is_active.clone();
    let is_active_cls = is_active.clone();
    let base = ctx.base_id.get_value();
    view! {
        <button
            type="button"
            role="tab"
            aria-selected=move || if is_active() { "true" } else { "false" }
            aria-controls=format!("{base}-content-{value}")
            data-state=move || if is_active_state() { "active" } else { "inactive" }
            data-orientation="horizontal"
            id=format!("{base}-trigger-{value}")
            tabindex=tab_index
            disabled=disabled
            class=move || {
                let mut cls = TABS_TRIGGER.to_owned();
                if is_active_cls() {
                    cls.push(' ');
                    cls.push_str(TABS_TRIGGER_ACTIVE);
                }
                if let Some(extra) = &class {
                    cls.push(' ');
                    cls.push_str(extra);
                }
                cls
            }
            on:click=on_click
            on:keydown=on_keydown
            on:focus=move |_| ctx.current_tab_stop.set(Some(index))
        >
            {children()}
        </button>
    }
}

#[component]
pub fn TabsContent(
    #[prop(into)] value: String,
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let ctx = use_context::<TabsCtx>().expect("invariant: TabsContent inside Tabs");
    let is_active = {
        let value = value.clone();
        move || ctx.value.with(|v| v == &value)
    };
    let is_active_state = is_active.clone();
    let is_active_children = is_active.clone();
    let mut cls = TABS_CONTENT.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let base = ctx.base_id.get_value();
    let children = StoredValue::new(children);
    view! {
        <div
            role="tabpanel"
            aria-labelledby=format!("{base}-trigger-{value}")
            data-state=move || if is_active_state() { "active" } else { "inactive" }
            data-orientation="horizontal"
            id=format!("{base}-content-{value}")
            tabindex="0"
            hidden=move || !is_active()
            class=cls
        >
            // Only the active panel mounts its children (Radix default).
            {move || is_active_children().then(|| children.with_value(|c| c()))}
        </div>
    }
}

/// List `inline-flex items-center gap-1 border-b` with delegated focus
/// (`outline:none`); trigger `relative -mb-px inline-flex h-8 cursor-pointer
/// items-center border-b-2 border-transparent px-3 text-[13px] font-medium`
/// muted, hover text, active accent underline, disabled inert; content
/// `mt-3` with no focus outline.
pub fn css() -> String {
    format!(
        ".{TABS_LIST}{{display:inline-flex;align-items:center;gap:.25rem;\
overflow-x:auto;max-width:100%;\
border-bottom-width:1px;border-color:var(--color-border);outline:none}}\
.{TABS_TRIGGER}{{position:relative;margin-bottom:-1px;display:inline-flex;\
height:2rem;cursor:pointer;align-items:center;\
border-bottom-width:2px;border-color:transparent;padding-left:.75rem;\
padding-right:.75rem;font-size:13px;font-weight:500;\
color:var(--color-text-muted);transition-property:color,background-color,\
border-color,outline-color,text-decoration-color,fill,stroke;\
transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}\
@media(hover:hover){{.{TABS_TRIGGER}:hover{{color:var(--color-text)}}}}\
.{TABS_TRIGGER}:focus-visible{{outline-style:none}}\
.{TABS_TRIGGER}:disabled{{pointer-events:none;opacity:.5}}\
.{TABS_TRIGGER_ACTIVE}{{border-color:var(--color-accent);\
color:var(--color-text)}}\
.{TABS_CONTENT}{{margin-top:.75rem}}\
.{TABS_CONTENT}:focus-visible{{outline-style:none}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [TABS_LIST, TABS_TRIGGER, TABS_TRIGGER_ACTIVE, TABS_CONTENT] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
        assert!(css.contains(&format!(".{TABS_TRIGGER}:focus-visible")));
    }
}
