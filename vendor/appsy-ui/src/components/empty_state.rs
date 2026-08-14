//! EmptyState — port of `components/ui/empty-state.tsx`: centered
//! empty/no-results placeholder on a card surface — icon chip above a bold
//! title, optional body copy, optional action slot. Reuses the card surface
//! class directly (the reference renders `Card` with internal utilities).

use crate::components::card::CARD;
use crate::icons::{Icon, RI_INBOX_LINE};
use leptos::prelude::*;

pub const EMPTY: &str = "asy-empty";
pub const EMPTY_CHIP: &str = "asy-empty__chip";
pub const EMPTY_GLYPH: &str = "asy-empty__glyph";
pub const EMPTY_TITLE: &str = "asy-empty__title";
pub const EMPTY_BODY: &str = "asy-empty__body";
pub const EMPTY_ACTION: &str = "asy-empty__action";

#[component]
pub fn EmptyState(
    /// Icon rendered above the title (an `icons::*` path constant).
    /// Defaults to the inbox glyph.
    #[prop(optional, default = RI_INBOX_LINE)] icon: &'static str,
    #[prop(into)] title: String,
    #[prop(optional, into)] body: Option<ViewFnOnce>,
    /// Optional action area (typically a Button or link).
    #[prop(optional, into)] action: Option<ViewFnOnce>,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let mut cls = format!("{CARD} {EMPTY}");
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <div class=cls>
            <span class=EMPTY_CHIP>
                <Icon d=icon class=EMPTY_GLYPH />
            </span>
            <span class=EMPTY_TITLE>{title}</span>
            {body.map(|b| view! { <p class=EMPTY_BODY>{b.run()}</p> })}
            {action.map(|a| view! { <div class=EMPTY_ACTION>{a.run()}</div> })}
        </div>
    }
}

/// Surface `flex flex-col items-center gap-3 px-6 py-12 text-center`; chip
/// `flex size-12 items-center justify-center rounded-full bg-surface-2
/// text-dim` with a `size-5` glyph; title 14px/600; body
/// `max-w-[340px] text-[12.5px] leading-[1.55] text-muted`; action `mt-1.5`.
pub fn css() -> String {
    format!(
        ".{EMPTY}{{display:flex;flex-direction:column;align-items:center;gap:.75rem;\
padding-left:1.5rem;padding-right:1.5rem;padding-top:3rem;padding-bottom:3rem;\
text-align:center}}\
.{EMPTY_CHIP}{{display:flex;width:3rem;height:3rem;align-items:center;\
justify-content:center;border-radius:calc(infinity * 1px);\
background-color:var(--color-surface-2);color:var(--color-text-dim)}}\
.{EMPTY_GLYPH}{{width:1.25rem;height:1.25rem}}\
.{EMPTY_TITLE}{{font-size:14px;font-weight:600}}\
.{EMPTY_BODY}{{max-width:340px;font-size:12.5px;line-height:1.55;\
color:var(--color-text-muted)}}\
.{EMPTY_ACTION}{{margin-top:.375rem}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [EMPTY, EMPTY_CHIP, EMPTY_GLYPH, EMPTY_TITLE, EMPTY_BODY, EMPTY_ACTION] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
