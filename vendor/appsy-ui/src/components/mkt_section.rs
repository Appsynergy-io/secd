//! MktSection — port of `marketing/section.tsx`: the marketing-page section
//! wrapper. Kicker (small accent caps), optional title and subtitle, and a
//! max-width inner container. Site usage passes strings for all three.
//!
//! `pad` replaces the default padding classes with an inline padding string
//! and `max` drives the inner container's inline max-width, exactly like
//! the reference's style props.

use leptos::prelude::*;

pub const MKTS: &str = "asy-mkts";
pub const MKTS_INNER: &str = "asy-mkts__inner";
pub const MKTS_HEAD: &str = "asy-mkts__head";
pub const MKTS_KICKER: &str = "asy-mkts__kicker";
pub const MKTS_TITLE: &str = "asy-mkts__title";
pub const MKTS_SUB: &str = "asy-mkts__sub";

#[component]
pub fn MktSection(
    #[prop(optional, into)] kicker: Option<String>,
    #[prop(optional, into)] title: Option<String>,
    /// Muted prose-width subtitle under the title.
    #[prop(optional, into)]
    sub: Option<String>,
    /// Inner max width in px. 1180 default; 900 for prose-heavy pages.
    #[prop(optional, default = 1180)]
    max: u32,
    /// CSS padding string replacing the default `px-8 py-12`.
    #[prop(optional, into)]
    pad: Option<String>,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = String::new();
    if pad.is_none() {
        cls.push_str(MKTS);
    }
    if let Some(extra) = class {
        if !cls.is_empty() {
            cls.push(' ');
        }
        cls.push_str(&extra);
    }
    let style = pad.map(|p| format!("padding: {p};"));
    let has_head = kicker.is_some() || title.is_some() || sub.is_some();
    view! {
        <section class=(!cls.is_empty()).then_some(cls) style=style>
            <div class=MKTS_INNER style=format!("max-width: {max}px;")>
                {has_head
                    .then(|| {
                        view! {
                            <div class=MKTS_HEAD>
                                {kicker.map(|k| view! { <span class=MKTS_KICKER>{k}</span> })}
                                {title.map(|t| view! { <h2 class=MKTS_TITLE>{t}</h2> })}
                                {sub.map(|s| view! { <p class=MKTS_SUB>{s}</p> })}
                            </div>
                        }
                    })}
                {children()}
            </div>
        </section>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{mkts}{{padding-inline:1rem;padding-block:3rem}}",
            "@media (width >= 40rem){{.{mkts}{{padding-inline:2rem}}}}",
            ".{inner}{{margin-inline:auto;display:flex;flex-direction:column;gap:1.75rem}}",
            ".{head}{{display:flex;max-width:680px;flex-direction:column;gap:.5rem}}",
            ".{kicker}{{font-size:11px;font-weight:600;text-transform:uppercase;",
            "letter-spacing:.08em;color:var(--color-accent)}}",
            ".{title}{{text-wrap:balance;font-size:28px;font-weight:600;",
            "letter-spacing:-0.02em}}",
            ".{sub}{{max-width:640px;font-size:15px;line-height:1.55;",
            "color:var(--color-text-muted)}}",
        ),
        mkts = MKTS,
        inner = MKTS_INNER,
        head = MKTS_HEAD,
        kicker = MKTS_KICKER,
        title = MKTS_TITLE,
        sub = MKTS_SUB,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [MKTS, MKTS_INNER, MKTS_HEAD, MKTS_KICKER, MKTS_TITLE, MKTS_SUB] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
