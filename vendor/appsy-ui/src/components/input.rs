//! Input — port of `components/ui/input.tsx`: a styled `<input>`, default
//! `type="text"`, focus ring via `:focus`, disabled dimming.

use leptos::prelude::*;

pub const INPUT: &str = "asy-input";

#[component]
pub fn Input(
    /// `type` attribute; the reference defaults to `text`.
    #[prop(optional, into, default = "text".into())]
    r#type: String,
    #[prop(optional, into)] placeholder: Option<String>,
    /// Initial value, or a reactive binding: pass a signal and later writes
    /// reset the field via the DOM *property* — a plain string cannot once
    /// the user has typed (approved API change, 2026-08-07; frozen again).
    #[prop(optional, into)]
    value: MaybeProp<String>,
    #[prop(optional, into)] id: Option<String>,
    #[prop(optional)] disabled: bool,
    #[prop(optional, into)] class: Option<String>,
    /// Handle to the underlying `<input>` for focus management.
    #[prop(optional)]
    node_ref: NodeRef<leptos::html::Input>,
) -> impl IntoView {
    let mut cls = INPUT.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <input
            node_ref=node_ref
            class=cls
            type=r#type
            placeholder=placeholder
            value=move || value.get()
            prop:value=move || value.get().unwrap_or_default()
            id=id
            disabled=disabled
        />
    }
}

/// `h-8 w-full rounded-[var(--radius-sm)] px-[10px] text-[13px] outline-none`
/// + surface/border/text tokens + placeholder color + focus border/ring +
/// disabled cursor/opacity. Focus ring = `ring-[3px]` with
/// `ring-[var(--color-accent-soft)]` (harness normalizes Tailwind's no-op
/// shadow layers away, so the single real layer is the contract).
pub fn css() -> String {
    format!(
        ".{INPUT}{{height:2rem;width:100%;border-radius:var(--radius-sm);\
padding-left:10px;padding-right:10px;font-size:13px;outline:none;\
background-color:var(--color-surface-2);border:1px solid var(--color-border);\
color:var(--color-text)}}\
.{INPUT}::placeholder{{color:var(--color-text-dim)}}\
.{INPUT}:focus{{border-color:var(--color-accent-line);\
box-shadow:0 0 0 3px var(--color-accent-soft)}}\
.{INPUT}:disabled{{cursor:not-allowed;opacity:.5}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        assert!(css().contains(&format!(".{INPUT}{{")));
    }

    #[test]
    fn states_have_rules() {
        let css = css();
        for state in ["::placeholder", ":focus", ":disabled"] {
            assert!(css.contains(&format!(".{INPUT}{state}")), "missing {state}");
        }
    }
}
