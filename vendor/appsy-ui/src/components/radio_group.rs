//! RadioGroup — port of `components/ui/radio-group.tsx` (Radix RadioGroup).
//! Machine (WAI-ARIA radio group pattern, Radix semantics):
//!
//! ```text
//! Tab      -> the checked item (or first, when none) is the only tabbable
//! Arrows   -> Down/Right select+focus next, Up/Left select+focus prev, wrap
//! Space    -> selects the focused item; Enter prevented
//! click    -> selects
//! ```
//!
//! Items navigate in DOM order (querying `[role=radio]` siblings, as Radix's
//! roving focus does). Indicator dot mounts only while checked.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

pub const RADIO_GROUP: &str = "asy-radio-group";
pub const RADIO: &str = "asy-radio";
pub const RADIO_CHECKED: &str = "asy-radio--checked";
pub const RADIO_INDICATOR: &str = "asy-radio__indicator";
pub const RADIO_DOT: &str = "asy-radio__dot";

#[derive(Clone, Copy)]
struct RadioGroupCtx {
    value: RwSignal<Option<String>>,
    /// Radix controlled mode: item activation only fires
    /// `on_value_change`; the shown value follows the consumer's prop.
    controlled: bool,
    on_value_change: Option<Callback<String>>,
    /// Registration order == DOM order (SSR render order).
    items: StoredValue<Vec<String>>,
    /// Roving tab stop: the item that last held focus (Radix
    /// `currentTabStopId`); `None` at rest — the root delegates.
    current_tab_stop: RwSignal<Option<usize>>,
}

#[component]
pub fn RadioGroup(
    #[prop(optional, into)] default_value: Option<String>,
    /// Controlled value (Radix's `value` prop).
    #[prop(optional, into)]
    value: Option<Signal<String>>,
    #[prop(optional)] on_value_change: Option<Callback<String>>,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = RadioGroupCtx {
        value: RwSignal::new(value.map(|v| v.get_untracked()).or(default_value)),
        controlled: value.is_some(),
        on_value_change,
        items: StoredValue::new(Vec::new()),
        current_tab_stop: RwSignal::new(None),
    };
    if let Some(value) = value {
        Effect::new(move |_| {
            let v = value.get();
            if ctx.value.with_untracked(|cur| cur.as_deref() != Some(v.as_str())) {
                ctx.value.set(Some(v));
            }
        });
    }
    let mut cls = RADIO_GROUP.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <div
            role="radiogroup"
            aria-required="false"
            dir="ltr"
            tabindex="0"
            class=cls
            // Focus landing on the group itself forwards to the checked
            // item (or the first), per Radix roving focus.
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
                    .with(|v| {
                        v.as_deref().and_then(|val| {
                            ctx.items.with_value(|items| items.iter().position(|i| i == val))
                        })
                    })
                    .unwrap_or(0);
                if let Some(el) = ev
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    .and_then(|group| {
                        group
                            .query_selector_all("[role=radio]")
                            .ok()
                            .and_then(|list| list.item(target_index as u32))
                    })
                    .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = el.focus();
                }
            }
        >
            // Scoped Provider — bare provide_context would let a later sibling
            // instance shadow this ctx for lazily-built children (see select.rs).
            <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider>
        </div>
    }
}

#[component]
pub fn RadioGroupItem(
    #[prop(into)] value: String,
    #[prop(optional, into)] id: Option<String>,
    #[prop(optional)] disabled: bool,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let ctx = use_context::<RadioGroupCtx>().expect("invariant: RadioGroupItem inside RadioGroup");
    let index = {
        let mut idx = 0;
        ctx.items.update_value(|items| {
            idx = items.len();
            items.push(value.clone());
        });
        idx
    };
    let is_checked = {
        let value = value.clone();
        move || ctx.value.with(|v| v.as_deref() == Some(value.as_str()))
    };
    // Items rest at -1 (the root is the tab entry); the item that last
    // held focus becomes the group's single tab stop.
    let tab_index = move || {
        if ctx.current_tab_stop.get() == Some(index) { "0" } else { "-1" }
    };
    let select = {
        let value = value.clone();
        move || {
            if !ctx.controlled {
                ctx.value.set(Some(value.clone()));
            }
            if let Some(cb) = ctx.on_value_change {
                cb.run(value.clone());
            }
        }
    };
    let on_click = {
        let select = select.clone();
        move |_| select()
    };
    let is_checked_kb = is_checked.clone();
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        let key = ev.key();
        match key.as_str() {
            "Enter" => ev.prevent_default(),
            " " => {
                ev.prevent_default();
                if !is_checked_kb() {
                    select();
                }
            }
            "ArrowDown" | "ArrowRight" | "ArrowUp" | "ArrowLeft" => {
                ev.prevent_default();
                let forward = matches!(key.as_str(), "ArrowDown" | "ArrowRight");
                let count = ctx.items.with_value(Vec::len);
                if count == 0 {
                    return;
                }
                let next_index = if forward { (index + 1) % count } else { (index + count - 1) % count };
                let next_value = ctx.items.with_value(|items| items[next_index].clone());
                // Focus the sibling in DOM order, then select it (Radix
                // radio roving focus selects on arrow navigation).
                if let Some(target) = ev
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    .and_then(|el| el.closest("[role=radiogroup]").ok().flatten())
                    .and_then(|group| {
                        group
                            .query_selector_all("[role=radio]")
                            .ok()
                            .and_then(|list| list.item(next_index as u32))
                    })
                    .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = target.focus();
                }
                if !ctx.controlled {
                    ctx.value.set(Some(next_value.clone()));
                }
                if let Some(cb) = ctx.on_value_change {
                    cb.run(next_value);
                }
            }
            _ => {}
        }
    };
    let is_checked_ind = is_checked.clone();
    let is_checked_state = is_checked.clone();
    view! {
        <button
            type="button"
            role="radio"
            aria-checked=move || if is_checked() { "true" } else { "false" }
            data-state=move || if is_checked_state() { "checked" } else { "unchecked" }
            value=value.clone()
            disabled=disabled
            id=id
            tabindex=tab_index
            class=move || {
                let mut cls = RADIO.to_owned();
                if is_checked_ind() {
                    cls.push(' ');
                    cls.push_str(RADIO_CHECKED);
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
            {
                let is_checked = is_checked_ind.clone();
                move || {
                    is_checked()
                        .then(|| {
                            view! {
                                <span class=RADIO_INDICATOR data-state="checked">
                                    <span class=RADIO_DOT></span>
                                </span>
                            }
                        })
                }
            }
        </button>
    }
}

/// Root `grid gap-1.5`; item `aspect-square size-4 rounded-full border
/// border-border bg-surface-2` with focus-visible accent ring, disabled
/// dim, accent border when checked; indicator `flex items-center
/// justify-center` around a `size-2` accent dot.
pub fn css() -> String {
    format!(
        ".{RADIO_GROUP}{{display:grid;gap:.375rem;outline:none}}\
.{RADIO}{{aspect-ratio:1/1;width:1.25rem;height:1.25rem;\
border-radius:calc(infinity * 1px);border:1px solid var(--color-border);\
background-color:var(--color-surface-2)}}\
.{RADIO}:focus-visible{{outline-style:none;\
box-shadow:0 0 0 2px var(--color-accent-line)}}\
.{RADIO}:disabled{{cursor:not-allowed;opacity:.5}}\
.{RADIO_CHECKED}{{border-color:var(--color-accent)}}\
.{RADIO_INDICATOR}{{display:flex;align-items:center;justify-content:center}}\
.{RADIO_DOT}{{display:block;width:.5rem;height:.5rem;\
border-radius:calc(infinity * 1px);background-color:var(--color-accent)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [RADIO_GROUP, RADIO, RADIO_CHECKED, RADIO_INDICATOR, RADIO_DOT] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
        assert!(css.contains(&format!(".{RADIO}:focus-visible")));
    }
}
