//! Sidebar — port of `dashboard/sidebar.tsx` (T2).
//!
//! Props/callbacks split: `useLocation().pathname` → the `path` prop (the
//! active-state derivation ports as given logic over it); `useLogout` +
//! `useNavigate` → `on_sign_out` + `signing_out` (the consumer runs the
//! mutation and navigates on settle). The hardcoded `NAV_CUSTOMER` /
//! `NAV_PLATFORM` catalogs become the `customer_items`/`platform_items`
//! props per navigation-is-props, as do the logo/account/platform hrefs —
//! fixtures mirror `nav-items.ts` the way `marketing-nav--default` does.

use crate::components::dropdown_menu::{
    DropdownMenu, DropdownMenuContent, DropdownMenuLabel, DropdownMenuLinkItem, DropdownMenuItem,
    DropdownMenuSeparator, DropdownMenuTriggerBare,
};
use crate::components::logo::{Logo, LogoSize};
use crate::icons::{Icon, RI_ACCOUNT_CIRCLE_LINE, RI_LOGOUT_BOX_R_LINE, RI_MORE_2_FILL, RI_SHIELD_USER_LINE};
use leptos::prelude::*;

pub const SIDE: &str = "asy-side";
pub const SIDE_HEAD: &str = "asy-side__head";
pub const SIDE_SCROLL: &str = "asy-side__scroll";
pub const SIDE_GROUP: &str = "asy-side__group";
pub const SIDE_GROUP_DIM: &str = "asy-side__group--dim";
pub const SIDE_LINK: &str = "asy-side__link";
pub const SIDE_LINK_ACTIVE: &str = "asy-side__link--active";
pub const SIDE_BAR: &str = "asy-side__bar";
pub const SIDE_ICON: &str = "asy-side__icon";
pub const SIDE_ICON_ACTIVE: &str = "asy-side__icon--active";
pub const SIDE_LABEL: &str = "asy-side__label";
pub const SIDE_RULE: &str = "asy-side__rule";
pub const SIDE_PLAT_ROW: &str = "asy-side__plat-row";
pub const SIDE_PLAT_LABEL: &str = "asy-side__plat-label";
pub const SIDE_PILL: &str = "asy-side__pill";
pub const SIDE_FOOT: &str = "asy-side__foot";
pub const SIDE_AVATAR: &str = "asy-side__avatar";
pub const SIDE_ID_COL: &str = "asy-side__id-col";
pub const SIDE_NAME: &str = "asy-side__name";
pub const SIDE_SECONDARY: &str = "asy-side__secondary";
pub const SIDE_MORE: &str = "asy-side__more";
pub const SIDE_MENU: &str = "asy-side__menu";
pub const SIDE_MENU_LINK: &str = "asy-side__menu-link";
pub const SIDE_MENU_ICON: &str = "asy-side__menu-icon";
pub const SIDE_MENU_TRUNC: &str = "asy-side__menu-trunc";

/// One sidebar nav entry — label/icon/target, supplied by the consumer.
#[derive(Clone, PartialEq, Debug)]
pub struct SideNavItem {
    pub label: String,
    pub icon: &'static str,
    pub href: String,
}

/// The reference's `initialsOf`: up-to-two uppercase initials from a name
/// or email; `"··"` when both are absent.
pub fn initials_of(name: Option<&str>, email: Option<&str>) -> String {
    let name = name.unwrap_or("").trim();
    let email = email.unwrap_or("").trim();
    let source = if !name.is_empty() { name } else { email };
    if source.is_empty() {
        return "··".to_owned();
    }
    let parts: Vec<&str> = source
        .split(|c: char| c.is_whitespace() || matches!(c, '@' | '.' | '_' | '-'))
        .filter(|p| !p.is_empty())
        .collect();
    let letters: String = if parts.len() >= 2 {
        parts[0].chars().take(1).chain(parts[1].chars().take(1)).collect()
    } else {
        source.chars().take(2).collect()
    };
    letters.to_uppercase()
}

#[component]
pub fn Sidebar(
    /// Render the Platform nav group below the customer group.
    #[prop(optional)]
    show_platform: bool,
    /// The Platform group is opaque (admin actively in platform mode).
    #[prop(optional)]
    platform_mode: bool,
    user_name: Option<String>,
    user_email: Option<String>,
    #[prop(optional)] is_platform_admin: bool,
    /// The current route path (`useLocation().pathname` in the reference)
    /// — drives the active-link derivation.
    #[prop(into)]
    path: Signal<String>,
    /// Logo target (`/app` on the site).
    #[prop(into)]
    home_href: String,
    customer_items: Vec<SideNavItem>,
    #[prop(optional)] platform_items: Vec<SideNavItem>,
    /// Account-menu targets (`/app/account`, `/platform` on the site).
    #[prop(into)]
    account_href: String,
    platform_href: Option<String>,
    /// The consumer's logout mutation; it navigates on settle.
    #[prop(into)]
    on_sign_out: Callback<()>,
    /// The logout mutation's `isPending`.
    #[prop(optional, into)]
    signing_out: Signal<bool>,
) -> impl IntoView {
    let display_name = {
        let n = user_name.clone().unwrap_or_default().trim().to_owned();
        let e = user_email.clone().unwrap_or_default().trim().to_owned();
        if !n.is_empty() {
            n
        } else if !e.is_empty() {
            e
        } else {
            "Account".to_owned()
        }
    };
    let secondary = user_email.clone().unwrap_or_else(|| {
        if is_platform_admin { "Platform admin".to_owned() } else { "Member".to_owned() }
    });
    let initials = initials_of(user_name.as_deref(), user_email.as_deref());
    let menu_data =
        StoredValue::new((display_name.clone(), account_href.into(), platform_href));

    view! {
        <aside class=SIDE>
            <div class=SIDE_HEAD>
                <a href=home_href.clone() aria-label="Overview">
                    <Logo size=LogoSize::Sm />
                </a>
            </div>
            <div class=SIDE_SCROLL>
                <NavGroup items=customer_items path=path dim=false home=home_href.clone() />
                {show_platform
                    .then(|| {
                        view! {
                            <div class=SIDE_RULE></div>
                            <div class=SIDE_PLAT_ROW>
                                <span class=SIDE_PLAT_LABEL>"Platform"</span>
                                <span class=SIDE_PILL>"all orgs"</span>
                            </div>
                            <NavGroup items=platform_items path=path dim=!platform_mode home=home_href.clone() />
                        }
                    })}
            </div>
            <DropdownMenu>
                <DropdownMenuTriggerBare class=SIDE_FOOT>
                    <div class=SIDE_AVATAR>{initials}</div>
                    <div class=SIDE_ID_COL>
                        <span class=SIDE_NAME>{display_name.clone()}</span>
                        <span class=SIDE_SECONDARY>{secondary}</span>
                    </div>
                    <Icon d=RI_MORE_2_FILL class=SIDE_MORE />
                </DropdownMenuTriggerBare>
                <DropdownMenuContent align="end" side="top" class=SIDE_MENU>
                    <DropdownMenuLabel class=SIDE_MENU_TRUNC>
                        {menu_data.with_value(|(dn, _, _): &(String, String, Option<String>)| dn.clone())}
                    </DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    <DropdownMenuLinkItem
                        href=menu_data.with_value(|(_, ah, _)| ah.clone())
                        class=SIDE_MENU_LINK
                    >
                        <Icon d=RI_ACCOUNT_CIRCLE_LINE class=SIDE_MENU_ICON />
                        "Account"
                    </DropdownMenuLinkItem>
                    {(is_platform_admin
                        && menu_data.with_value(|(_, _, ph)| ph.is_some()))
                        .then(|| {
                            view! {
                                <DropdownMenuLinkItem
                                    href=menu_data
                                        .with_value(|(_, _, ph)| ph.clone().unwrap_or_default())
                                    class=SIDE_MENU_LINK
                                >
                                    <Icon d=RI_SHIELD_USER_LINE class=SIDE_MENU_ICON />
                                    "Platform admin"
                                </DropdownMenuLinkItem>
                            }
                        })}
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                        disabled=signing_out.get_untracked()
                        on_select=Callback::new(move |_| on_sign_out.run(()))
                    >
                        <Icon d=RI_LOGOUT_BOX_R_LINE class=SIDE_MENU_ICON />
                        "Sign out"
                    </DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
        </aside>
    }
}

/// Whether `path` is active for `href` given the group's `hrefs` and `home`.
///
/// Home matches only exact path. Other items match exact or a boundary-aware
/// prefix (`/platform` matches `/platform/settings`, not `/platform-extra`).
/// Among matches in the same group, only the longest href is active.
pub(crate) fn nav_item_active(path: &str, href: &str, home: &str, group_hrefs: &[String]) -> bool {
    fn path_matches(path: &str, href: &str, home: &str) -> bool {
        if href == home {
            path == home
        } else {
            path == href || path.starts_with(&format!("{href}/"))
        }
    }
    if !path_matches(path, href, home) {
        return false;
    }
    !group_hrefs.iter().any(|other| {
        other.len() > href.len() && path_matches(path, other, home)
    })
}

#[component]
fn NavGroup(
    items: Vec<SideNavItem>,
    #[prop(into)] path: Signal<String>,
    dim: bool,
    /// The root item's target: exact path match only (the reference's `/app`
    /// special case, generalized per navigation-is-props). Non-home items use
    /// longest-prefix among matching hrefs in the same group.
    home: String,
) -> impl IntoView {
    let group_cls = if dim { format!("{SIDE_GROUP} {SIDE_GROUP_DIM}") } else { SIDE_GROUP.to_owned() };
    let group_hrefs: Vec<String> = items.iter().map(|i| i.href.clone()).collect();
    view! {
        <div class=group_cls>
            {items
                .into_iter()
                .map(|it| {
                    let href = it.href.clone();
                    let home = home.clone();
                    let group_hrefs = group_hrefs.clone();
                    let active = Signal::derive(move || {
                        nav_item_active(&path.get(), &href, &home, &group_hrefs)
                    });
                    view! {
                        <a
                            href=it.href.clone()
                            class=move || {
                                if active.get() {
                                    format!("{SIDE_LINK} {SIDE_LINK_ACTIVE}")
                                } else {
                                    SIDE_LINK.to_owned()
                                }
                            }
                        >
                            {move || active.get().then(|| view! { <span class=SIDE_BAR></span> })}
                            {
                                let d = it.icon;
                                move || {
                                    let cls = if active.get() {
                                        format!("{SIDE_ICON} {SIDE_ICON_ACTIVE}")
                                    } else {
                                        SIDE_ICON.to_owned()
                                    };
                                    view! { <Icon d=d class=cls /> }
                                }
                            }
                            <span class=SIDE_LABEL>{it.label}</span>
                        </a>
                    }
                })
                .collect_view()}
        </div>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{side}{{display:flex;height:100%;width:232px;flex-shrink:0;",
            "flex-direction:column;border:0 solid var(--color-border);",
            "border-right-width:1px;",
            "background-color:var(--color-surface)}}",
            ".{head}{{display:flex;align-items:center;",
            "border:0 solid var(--color-border);border-bottom-width:1px;",
            "padding-inline:1rem;",
            "padding-block:.875rem}}",
            ".{scroll}{{flex:1;overflow:auto;padding-inline:.5rem;",
            "padding-block:.75rem}}",
            ".{group}{{margin-bottom:.25rem}}",
            ".{group_dim}{{opacity:.6}}",
            ".{link}{{position:relative;margin-block:.125rem;display:flex;",
            "align-items:center;gap:.625rem;border-radius:var(--radius-sm);",
            "padding-inline:.625rem;padding-block:.375rem;font-size:13px;",
            "transition-property:color,background-color,border-color,",
            "text-decoration-color,fill,stroke;",
            "transition-timing-function:cubic-bezier(.4,0,.2,1);",
            "transition-duration:.15s;color:var(--color-text-muted)}}",
            "@media (hover:hover){{.{link}:hover{{",
            "background-color:var(--color-surface-2);color:var(--color-text)}}}}",
            ".{link_active}{{background-color:var(--color-surface-2);",
            "font-weight:500;color:var(--color-text)}}",
            ".{bar}{{position:absolute;left:-.5rem;bottom:.375rem;top:.375rem;",
            "width:.125rem;border-radius:.25rem;",
            "background-color:var(--color-accent)}}",
            ".{icon}{{width:1rem;height:1rem;flex-shrink:0;",
            "color:var(--color-text-dim)}}",
            ".{icon_active}{{color:var(--color-accent)}}",
            ".{label}{{flex:1 1 0%;min-width:0;overflow:hidden;",
            "text-overflow:ellipsis;white-space:nowrap}}",
            ".{rule}{{margin-block:.75rem;",
            "border:0 solid var(--color-border);border-top-width:1px}}",
            ".{plat_row}{{display:flex;align-items:center;",
            "justify-content:space-between;padding-inline:.625rem;",
            "padding-block:.375rem}}",
            ".{plat_label}{{font-size:10.5px;font-weight:600;",
            "text-transform:uppercase;letter-spacing:.08em;",
            "color:var(--color-text-dim)}}",
            // all-orgs pill: arbitrary oklch border compiles to rgba (TT-2).
            ".{pill}{{border-radius:.25rem;border:1px solid rgba(227,173,75,.3);",
            "padding-inline:.375rem;font-size:9.5px;color:var(--color-warning);",
            "background:var(--color-warning-soft)}}",
            ".{foot}{{display:flex;width:100%;align-items:center;gap:.5rem;",
            "border:0 solid var(--color-border);border-top-width:1px;",
            "padding:.625rem;text-align:left}}",
            "@media (hover:hover){{.{foot}:hover{{",
            "background-color:var(--color-surface-2)}}}}",
            ".{foot}:focus{{outline:2px solid transparent;outline-offset:2px}}",
            ".{foot}:focus-visible{{outline:none;",
            "box-shadow:0 0 0 2px var(--color-accent-soft)}}",
            ".{avatar}{{display:flex;width:1.75rem;height:1.75rem;",
            "align-items:center;justify-content:center;",
            "border-radius:calc(infinity * 1px);font-size:12px;font-weight:600;",
            "background:var(--color-accent-soft);color:var(--color-accent)}}",
            ".{id_col}{{display:flex;min-width:0;flex:1;flex-direction:column}}",
            ".{name}{{overflow:hidden;text-overflow:ellipsis;",
            "white-space:nowrap;font-size:12.5px;font-weight:500}}",
            ".{secondary}{{overflow:hidden;text-overflow:ellipsis;",
            "white-space:nowrap;font-size:11px;color:var(--color-text-muted)}}",
            ".{more}{{width:.875rem;height:.875rem;color:var(--color-text-dim)}}",
            ".{menu}{{width:200px}}",
            ".asy-side__menu-trunc{{overflow:hidden;text-overflow:ellipsis;",
            "white-space:nowrap}}",
            ".{menu_link}{{display:flex;align-items:center;gap:.5rem}}",
            ".{menu_icon}{{width:1rem;height:1rem}}",
        ),
        side = SIDE,
        head = SIDE_HEAD,
        scroll = SIDE_SCROLL,
        group = SIDE_GROUP,
        group_dim = SIDE_GROUP_DIM,
        link = SIDE_LINK,
        link_active = SIDE_LINK_ACTIVE,
        bar = SIDE_BAR,
        icon = SIDE_ICON,
        icon_active = SIDE_ICON_ACTIVE,
        rule = SIDE_RULE,
        plat_row = SIDE_PLAT_ROW,
        plat_label = SIDE_PLAT_LABEL,
        pill = SIDE_PILL,
        foot = SIDE_FOOT,
        avatar = SIDE_AVATAR,
        id_col = SIDE_ID_COL,
        name = SIDE_NAME,
        secondary = SIDE_SECONDARY,
        more = SIDE_MORE,
        menu = SIDE_MENU,
        menu_link = SIDE_MENU_LINK,
        menu_icon = SIDE_MENU_ICON,
        label = SIDE_LABEL,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            SIDE, SIDE_HEAD, SIDE_SCROLL, SIDE_GROUP, SIDE_GROUP_DIM, SIDE_LINK,
            SIDE_LINK_ACTIVE, SIDE_BAR, SIDE_ICON, SIDE_ICON_ACTIVE, SIDE_LABEL, SIDE_RULE,
            SIDE_PLAT_ROW, SIDE_PLAT_LABEL, SIDE_PILL, SIDE_FOOT, SIDE_AVATAR, SIDE_ID_COL,
            SIDE_NAME, SIDE_SECONDARY, SIDE_MORE, SIDE_MENU, SIDE_MENU_LINK, SIDE_MENU_ICON,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    /// Case-for-case mirror of the JS `initialsOf`.
    #[test]
    fn initials_mirror_reference() {
        assert_eq!(initials_of(Some("Lena Fischer"), None), "LF");
        assert_eq!(initials_of(None, Some("lena@example.com")), "LE");
        assert_eq!(initials_of(Some("lena.fischer"), None), "LF");
        assert_eq!(initials_of(Some("mono"), None), "MO");
        assert_eq!(initials_of(None, None), "··");
        assert_eq!(initials_of(Some("  "), Some("")), "··");
    }

    // Synthetic paths only — logic fence forbids site route literals in src.
    fn s(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn nav_active_nested_settings_wins() {
        let hrefs = s(&["/plat", "/plat/cfg", "/plat/dns"]);
        let home = "/dash";
        assert!(!nav_item_active("/plat/cfg", "/plat", home, &hrefs));
        assert!(nav_item_active("/plat/cfg", "/plat/cfg", home, &hrefs));
        assert!(!nav_item_active("/plat/cfg", "/plat/dns", home, &hrefs));
    }

    #[test]
    fn nav_active_platform_overview_only() {
        let hrefs = s(&["/plat", "/plat/cfg", "/plat/dns"]);
        let home = "/dash";
        assert!(nav_item_active("/plat", "/plat", home, &hrefs));
        assert!(!nav_item_active("/plat", "/plat/cfg", home, &hrefs));
        assert!(!nav_item_active("/plat", "/plat/dns", home, &hrefs));
    }

    #[test]
    fn nav_active_home_exact_only() {
        let hrefs = s(&["/dash", "/dash/devices"]);
        let home = "/dash";
        assert!(nav_item_active("/dash", "/dash", home, &hrefs));
        assert!(!nav_item_active("/dash", "/dash/devices", home, &hrefs));
    }

    #[test]
    fn nav_active_devices_not_home() {
        let hrefs = s(&["/dash", "/dash/devices"]);
        let home = "/dash";
        assert!(!nav_item_active("/dash/devices", "/dash", home, &hrefs));
        assert!(nav_item_active("/dash/devices", "/dash/devices", home, &hrefs));
    }

    #[test]
    fn nav_active_boundary_rejects_bare_prefix() {
        let hrefs = s(&["/plat", "/plat/cfg"]);
        let home = "/dash";
        assert!(!nav_item_active("/plat-extra", "/plat", home, &hrefs));
        assert!(!nav_item_active("/plat-extra", "/plat/cfg", home, &hrefs));
    }
}
