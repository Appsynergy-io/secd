//! LegalPage — port of `marketing/legal-page.tsx`: the shared `/legal/*`
//! template. Hero (badge, clamp title, effective date, intro) over a
//! two-column body: sticky table of contents left, numbered prose sections
//! right. One layout, six content variants — all content enters via props;
//! section bodies are consumer-supplied views exactly like the reference's
//! `ReactNode` bodies. TOC anchors are same-page fragments derived from
//! section numbers (deterministic ids, not site routes).

use crate::components::badge::{Badge, BadgeTone};
use leptos::prelude::*;

pub const LEGAL_HERO: &str = "asy-legal__hero";
pub const LEGAL_BADGE: &str = "asy-legal__badge";
pub const LEGAL_H1: &str = "asy-legal__h1";
pub const LEGAL_EFFECTIVE: &str = "asy-legal__effective";
pub const LEGAL_INTRO: &str = "asy-legal__intro";
pub const LEGAL_BODY: &str = "asy-legal__body";
pub const LEGAL_TOC: &str = "asy-legal__toc";
pub const LEGAL_TOC_TITLE: &str = "asy-legal__toc-title";
pub const LEGAL_TOC_LIST: &str = "asy-legal__toc-list";
pub const LEGAL_TOC_LINK: &str = "asy-legal__toc-link";
pub const LEGAL_TOC_NUM: &str = "asy-legal__toc-num";
pub const LEGAL_ARTICLE: &str = "asy-legal__article";
pub const LEGAL_SEC: &str = "asy-legal__sec";
pub const LEGAL_SEC_H2: &str = "asy-legal__sec-h2";
pub const LEGAL_SEC_NUM: &str = "asy-legal__sec-num";
pub const LEGAL_SEC_BODY: &str = "asy-legal__sec-body";

#[derive(Clone)]
pub struct LegalSection {
    pub n: String,
    pub title: String,
    /// Section prose — a consumer-supplied view, the port of the
    /// reference's `ReactNode` body.
    pub body: ViewFn,
}

#[component]
pub fn LegalPage(
    /// Document slug — e.g. "terms-of-service".
    #[prop(into)]
    slug: String,
    /// Display title, e.g. "Terms of Service".
    #[prop(into)]
    title: String,
    /// Effective date, RFC 3339 (YYYY-MM-DD).
    #[prop(into)]
    effective: String,
    /// Short paragraph immediately under the title.
    #[prop(into)]
    intro: String,
    /// Numbered sections, rendered with the sticky table of contents.
    sections: Vec<LegalSection>,
) -> impl IntoView {
    let toc = sections.clone();
    view! {
        <section class=LEGAL_HERO>
            <Badge tone=BadgeTone::Default class=LEGAL_BADGE>"Legal · " {slug}</Badge>
            <h1 class=LEGAL_H1>{title}</h1>
            <p class=LEGAL_EFFECTIVE>"Effective " <span class="mono">{effective}</span></p>
            <p class=LEGAL_INTRO>{intro}</p>
        </section>
        <section class=LEGAL_BODY>
            <nav class=LEGAL_TOC>
                <span class=LEGAL_TOC_TITLE>"Contents"</span>
                <ol class=LEGAL_TOC_LIST>
                    {toc
                        .into_iter()
                        .map(|s| {
                            view! {
                                <li>
                                    <a class=LEGAL_TOC_LINK href=format!("#sec-{}", s.n)>
                                        <span class=format!("mono {LEGAL_TOC_NUM}")>
                                            {s.n.clone()}
                                        </span>
                                        {s.title}
                                    </a>
                                </li>
                            }
                        })
                        .collect_view()}
                </ol>
            </nav>
            <article class=LEGAL_ARTICLE>
                {sections
                    .into_iter()
                    .map(|s| {
                        view! {
                            <section id=format!("sec-{}", s.n) class=LEGAL_SEC>
                                <h2 class=LEGAL_SEC_H2>
                                    <span class=format!("mono {LEGAL_SEC_NUM}")>
                                        {s.n.clone()}
                                    </span>
                                    {s.title}
                                </h2>
                                <div class=LEGAL_SEC_BODY>{s.body.run()}</div>
                            </section>
                        }
                    })
                    .collect_view()}
            </article>
        </section>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{hero}{{margin-inline:auto;max-width:1180px;padding-inline:1rem;",
            "padding-bottom:2rem;padding-top:72px}}",
            "@media (width >= 40rem){{.{hero}{{padding-inline:2rem}}}}",
            ".{badge}{{margin-bottom:1rem}}",
            ".{h1}{{max-width:760px;text-wrap:balance;",
            "font-size:clamp(34px,4vw,46px);font-weight:600;line-height:1.08;",
            "letter-spacing:-0.03em}}",
            ".{effective}{{margin-top:.75rem;font-size:12.5px;",
            "color:var(--color-text-muted)}}",
            ".{intro}{{margin-top:1rem;max-width:680px;font-size:15px;line-height:1.6;",
            "color:var(--color-text-muted)}}",
            ".{body}{{margin-inline:auto;display:grid;max-width:1180px;",
            "grid-template-columns:repeat(1,minmax(0,1fr));gap:3rem;",
            "padding-inline:1rem;padding-bottom:4rem}}",
            "@media (width >= 40rem){{.{body}{{padding-inline:2rem}}}}",
            "@media (width >= 48rem){{.{body}{{grid-template-columns:220px 1fr}}}}",
            ".{toc}{{position:sticky;top:88px;height:fit-content}}",
            ".{toc_title}{{display:block;padding-bottom:.5rem;font-size:11px;",
            "font-weight:600;text-transform:uppercase;letter-spacing:.05em;",
            "color:var(--color-text-dim)}}",
            ".{toc_list}{{display:flex;flex-direction:column;gap:.375rem}}",
            ".{toc_link}{{display:block;font-size:13px;color:var(--color-text-muted)}}",
            "@media(hover:hover){{.{toc_link}:hover{{color:var(--color-text)}}}}",
            ".{toc_num}{{margin-right:.5rem;color:var(--color-text-dim)}}",
            ".{article}{{display:flex;flex-direction:column;gap:2.5rem}}",
            ".{sec}{{scroll-margin-top:6rem}}",
            ".{sec_h2}{{margin-bottom:.5rem;font-size:18px;font-weight:600;",
            "letter-spacing:-0.01em}}",
            ".{sec_num}{{margin-right:.75rem;color:var(--color-text-dim)}}",
            ".{sec_body}{{display:flex;flex-direction:column;gap:.75rem;font-size:14px;",
            "line-height:1.65;color:var(--color-text-muted)}}",
        ),
        hero = LEGAL_HERO,
        badge = LEGAL_BADGE,
        h1 = LEGAL_H1,
        effective = LEGAL_EFFECTIVE,
        intro = LEGAL_INTRO,
        body = LEGAL_BODY,
        toc = LEGAL_TOC,
        toc_title = LEGAL_TOC_TITLE,
        toc_list = LEGAL_TOC_LIST,
        toc_link = LEGAL_TOC_LINK,
        toc_num = LEGAL_TOC_NUM,
        article = LEGAL_ARTICLE,
        sec = LEGAL_SEC,
        sec_h2 = LEGAL_SEC_H2,
        sec_num = LEGAL_SEC_NUM,
        sec_body = LEGAL_SEC_BODY,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            LEGAL_HERO, LEGAL_BADGE, LEGAL_H1, LEGAL_EFFECTIVE, LEGAL_INTRO, LEGAL_BODY,
            LEGAL_TOC, LEGAL_TOC_TITLE, LEGAL_TOC_LIST, LEGAL_TOC_LINK, LEGAL_TOC_NUM,
            LEGAL_ARTICLE, LEGAL_SEC, LEGAL_SEC_H2, LEGAL_SEC_NUM, LEGAL_SEC_BODY,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
