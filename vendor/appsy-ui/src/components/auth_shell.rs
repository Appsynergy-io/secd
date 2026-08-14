//! AuthShell + AuthHead — port of `auth/auth-shell.tsx`: the two-column
//! auth surface (form column left, AuthSide decoration right, one-column
//! when `full_width`) and the title/subtitle head every auth page uses.
//!
//! Navigation is props: the home link and the three footer links are
//! hardcoded in the reference and supplied by the consumer here. The
//! "© 2026 AppSynergy" line is a source literal in the reference (not a
//! clock read), so it mirrors as content.

use crate::components::auth_side::AuthSide;
use crate::components::logo::{Logo, LogoSize};
use crate::components::marketing_footer::FooterLink;
use leptos::prelude::*;

pub const AUTH: &str = "asy-auth";
pub const AUTH_COL: &str = "asy-auth__col";
pub const AUTH_CENTER: &str = "asy-auth__center";
pub const AUTH_SLOT: &str = "asy-auth__slot";
pub const AUTH_FOOT: &str = "asy-auth__foot";
pub const AUTH_FOOT_LINKS: &str = "asy-auth__foot-links";
pub const AUTH_FOOT_LINK: &str = "asy-auth__foot-link";
pub const AUTH_HEAD: &str = "asy-auth__head";
pub const AUTH_H1: &str = "asy-auth__h1";
pub const AUTH_SUB: &str = "asy-auth__sub";

#[component]
pub fn AuthShell(
    /// Home target of the logo link.
    #[prop(into)]
    home_href: String,
    /// The footer links (Status / Docs / Contact on the site).
    links: Vec<FooterLink>,
    /// Omit the AuthSide decoration column (one-column layout).
    #[prop(optional)]
    full_width: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=AUTH>
            <div class=AUTH_COL>
                <a href=home_href aria-label="appsynergy home">
                    <Logo size=LogoSize::Sm />
                </a>
                <div class=AUTH_CENTER>
                    <div class=format!("fade-slide-in {AUTH_SLOT}")>{children()}</div>
                </div>
                <div class=AUTH_FOOT>
                    <span>"© 2026 AppSynergy"</span>
                    <div class=AUTH_FOOT_LINKS>
                        {links
                            .into_iter()
                            .map(|l| {
                                view! {
                                    <a class=AUTH_FOOT_LINK href=l.href>{l.label}</a>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
            </div>
            {(!full_width).then(AuthSide)}
        </div>
    }
}

/// Title + subtitle pair used by every auth page. `sub` is a
/// consumer-supplied view (the reference's `ReactNode`).
#[component]
pub fn AuthHead(
    #[prop(into)] title: String,
    #[prop(optional)] sub: Option<ViewFn>,
) -> impl IntoView {
    view! {
        <div class=AUTH_HEAD>
            <h1 class=AUTH_H1>{title}</h1>
            {sub.map(|s| view! { <p class=AUTH_SUB>{s.run()}</p> })}
        </div>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{auth}{{display:flex;min-height:100svh;width:100%;",
            "background-color:var(--color-bg)}}",
            ".{col}{{display:flex;flex:1;flex-direction:column;padding-inline:1.5rem;",
            "padding-block:2rem}}",
            "@media (width >= 40rem){{.{col}{{padding-inline:3rem}}}}",
            ".{center}{{display:flex;flex:1;align-items:center;justify-content:center}}",
            ".{slot}{{width:100%;max-width:380px}}",
            ".{foot}{{display:flex;align-items:center;justify-content:space-between;",
            "font-size:11.5px;color:var(--color-text-dim)}}",
            ".{foot_links}{{display:flex;align-items:center;gap:1rem}}",
            ".{foot_link}{{cursor:pointer}}",
            ".{head}{{margin-bottom:1.5rem;display:flex;flex-direction:column;",
            "gap:.375rem}}",
            ".{h1}{{font-size:26px;font-weight:600;letter-spacing:-0.02em}}",
            ".{sub}{{font-size:13.5px;line-height:1.55;color:var(--color-text-muted)}}",
        ),
        auth = AUTH,
        col = AUTH_COL,
        center = AUTH_CENTER,
        slot = AUTH_SLOT,
        foot = AUTH_FOOT,
        foot_links = AUTH_FOOT_LINKS,
        foot_link = AUTH_FOOT_LINK,
        head = AUTH_HEAD,
        h1 = AUTH_H1,
        sub = AUTH_SUB,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            AUTH, AUTH_COL, AUTH_CENTER, AUTH_SLOT, AUTH_FOOT, AUTH_FOOT_LINKS, AUTH_FOOT_LINK,
            AUTH_HEAD, AUTH_H1, AUTH_SUB,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
