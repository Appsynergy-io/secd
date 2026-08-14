//! Card — port of `components/ui/card.tsx`: surface + header / title /
//! description / content / footer sub-elements, all plain styled divs.

use leptos::prelude::*;

pub const CARD: &str = "asy-card";
pub const CARD_HEADER: &str = "asy-card__header";
pub const CARD_TITLE: &str = "asy-card__title";
pub const CARD_DESCRIPTION: &str = "asy-card__description";
pub const CARD_CONTENT: &str = "asy-card__content";
pub const CARD_FOOTER: &str = "asy-card__footer";

fn merged(base: &str, class: Option<String>) -> String {
    match class {
        Some(extra) => format!("{base} {extra}"),
        None => base.to_owned(),
    }
}

#[component]
pub fn Card(#[prop(optional, into)] class: Option<String>, children: Children) -> impl IntoView {
    view! { <div class=merged(CARD, class)>{children()}</div> }
}

#[component]
pub fn CardHeader(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    view! { <div class=merged(CARD_HEADER, class)>{children()}</div> }
}

#[component]
pub fn CardTitle(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    view! { <div class=merged(CARD_TITLE, class)>{children()}</div> }
}

#[component]
pub fn CardDescription(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    view! { <div class=merged(CARD_DESCRIPTION, class)>{children()}</div> }
}

#[component]
pub fn CardContent(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    view! { <div class=merged(CARD_CONTENT, class)>{children()}</div> }
}

#[component]
pub fn CardFooter(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    view! { <div class=merged(CARD_FOOTER, class)>{children()}</div> }
}

pub fn css() -> String {
    format!(
        ".{CARD}{{border-radius:var(--radius-md);border:1px solid var(--color-border);\
background-color:var(--color-surface)}}\
.{CARD_HEADER}{{display:flex;align-items:center;justify-content:space-between;gap:.75rem;\
padding-left:1rem;padding-right:1rem;padding-top:.875rem;padding-bottom:.875rem;\
border-color:var(--color-border);border-bottom-width:1px}}\
.{CARD_TITLE}{{font-size:.875rem;line-height:1.25rem;font-weight:500}}\
.{CARD_DESCRIPTION}{{font-size:.75rem;line-height:1rem;color:var(--color-text-muted)}}\
.{CARD_CONTENT}{{padding:1rem}}\
.{CARD_FOOTER}{{display:flex;align-items:center;gap:.5rem;\
padding-left:1rem;padding-right:1rem;padding-top:.75rem;padding-bottom:.75rem;\
border-color:var(--color-border-soft);border-top-width:1px}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [CARD, CARD_HEADER, CARD_TITLE, CARD_DESCRIPTION, CARD_CONTENT, CARD_FOOTER] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
