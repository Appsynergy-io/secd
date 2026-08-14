//! MarketingFooter — port of `marketing/marketing-footer.tsx`: brand column
//! (logo, tagline, social glyphs) beside four link groups, over the
//! copyright/build bottom bar.
//!
//! Two prop-driven deviations from the reference's hardcoded content, both
//! mandated by crate rules: the link groups are props (navigation is always
//! props), and `year` is a prop (the reference calls
//! `new Date().getFullYear()`; the crate's determinism invariant forbids
//! reading the clock). The tagline, social glyphs, and build-string shape
//! mirror the reference verbatim.

use crate::components::logo::{Logo, LogoSize};
use crate::icons::{Icon, RI_GITHUB_FILL, RI_MASTODON_FILL, RI_TWITTER_X_FILL};
use leptos::prelude::*;

pub const MFOOT: &str = "asy-mfoot";
pub const MFOOT_GRID: &str = "asy-mfoot__grid";
pub const MFOOT_BRAND: &str = "asy-mfoot__brand";
pub const MFOOT_TAGLINE: &str = "asy-mfoot__tagline";
pub const MFOOT_SOCIAL: &str = "asy-mfoot__social";
pub const MFOOT_SOCIAL_ICON: &str = "asy-mfoot__social-icon";
pub const MFOOT_GROUP: &str = "asy-mfoot__group";
pub const MFOOT_GROUP_TITLE: &str = "asy-mfoot__group-title";
pub const MFOOT_LINK: &str = "asy-mfoot__link";
pub const MFOOT_BOTTOM: &str = "asy-mfoot__bottom";
pub const MFOOT_BUILD: &str = "asy-mfoot__build";

#[derive(Clone, PartialEq)]
pub struct FooterLink {
    pub label: String,
    pub href: String,
}

#[derive(Clone, PartialEq)]
pub struct FooterGroup {
    pub title: String,
    pub links: Vec<FooterLink>,
}

#[component]
pub fn MarketingFooter(
    /// The four link groups; navigation is props, never crate content.
    groups: Vec<FooterGroup>,
    /// Copyright/build year. A prop because the crate may not read the
    /// clock (determinism invariant); the consumer computes it.
    year: u16,
) -> impl IntoView {
    view! {
        <footer class=MFOOT>
            <div class=MFOOT_GRID>
                <div class=MFOOT_BRAND>
                    <Logo size=LogoSize::Sm />
                    <p class=MFOOT_TAGLINE>
                        "Remote-access tunnels and endpoint protection. Built quietly, deployed widely."
                    </p>
                    <div class=MFOOT_SOCIAL>
                        <Icon d=RI_GITHUB_FILL class=MFOOT_SOCIAL_ICON />
                        <Icon d=RI_TWITTER_X_FILL class=MFOOT_SOCIAL_ICON />
                        <Icon d=RI_MASTODON_FILL class=MFOOT_SOCIAL_ICON />
                    </div>
                </div>
                {groups
                    .into_iter()
                    .map(|group| {
                        view! {
                            <div class=MFOOT_GROUP>
                                <span class=MFOOT_GROUP_TITLE>{group.title}</span>
                                {group
                                    .links
                                    .into_iter()
                                    .map(|link| {
                                        view! {
                                            <a class=MFOOT_LINK href=link.href>{link.label}</a>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
            <div class=MFOOT_BOTTOM>
                // Three text nodes per span, mirroring the JSX children
                // ("© "/{year}/" AppSynergy") — the walker joins own-text
                // per node and subpixel advances round per text node, so a
                // single merged node measurably differs.
                <span>"© " {year.to_string()} " AppSynergy"</span>
                <span class=MFOOT_BUILD>"build " {year.to_string()} ".05.22 · fra-1"</span>
            </div>
        </footer>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{foot}{{margin-inline:auto;margin-top:2rem;max-width:1180px;",
            "border-top-width:1px;border-color:var(--color-border);",
            "padding-inline:1rem;padding-block:2rem}}",
            "@media (width >= 40rem){{.{foot}{{padding-inline:2rem}}}}",
            ".{grid}{{display:grid;grid-template-columns:1fr;gap:2rem}}",
            "@media (width >= 40rem){{.{grid}{{grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            "@media (width >= 48rem){{.{grid}{{grid-template-columns:1.4fr repeat(4,1fr)}}}}",
            ".{brand}{{display:flex;flex-direction:column;gap:.625rem}}",
            ".{tagline}{{max-width:220px;font-size:.75rem;line-height:calc(1/.75);",
            "color:var(--color-text-muted)}}",
            ".{social}{{margin-top:.25rem;display:flex;align-items:center;gap:.625rem;",
            "color:var(--color-text-dim)}}",
            ".{social_icon}{{width:1rem;height:1rem}}",
            ".{group}{{display:flex;flex-direction:column;gap:.5rem}}",
            ".{group_title}{{font-size:11.5px;font-weight:500;letter-spacing:.02em}}",
            ".{link}{{cursor:pointer;font-size:.75rem;line-height:calc(1/.75);",
            "color:var(--color-text-muted)}}",
            "@media(hover:hover){{.{link}:hover{{color:var(--color-text)}}}}",
            ".{bottom}{{margin-top:1.75rem;display:flex;flex-wrap:wrap;gap:.5rem;",
            "align-items:center;justify-content:space-between;border-top-width:1px;",
            "border-color:var(--color-border);padding-top:1rem;font-size:11.5px;",
            "color:var(--color-text-dim)}}",
            ".{build}{{font-family:var(--font-mono);font-feature-settings:\"ss01\"}}",
        ),
        foot = MFOOT,
        grid = MFOOT_GRID,
        brand = MFOOT_BRAND,
        tagline = MFOOT_TAGLINE,
        social = MFOOT_SOCIAL,
        social_icon = MFOOT_SOCIAL_ICON,
        group = MFOOT_GROUP,
        group_title = MFOOT_GROUP_TITLE,
        link = MFOOT_LINK,
        bottom = MFOOT_BOTTOM,
        build = MFOOT_BUILD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            MFOOT, MFOOT_GRID, MFOOT_BRAND, MFOOT_TAGLINE, MFOOT_SOCIAL, MFOOT_SOCIAL_ICON,
            MFOOT_GROUP, MFOOT_GROUP_TITLE, MFOOT_LINK, MFOOT_BOTTOM, MFOOT_BUILD,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn link_hover_is_hover_gated() {
        assert!(css().contains("@media(hover:hover)"));
    }
}
