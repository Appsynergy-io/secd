//! Topbar — port of `dashboard/topbar.tsx` (T3).
//!
//! Props/callbacks split: `useMyOrgs`/`useSetActiveOrg` → the `orgs`
//! membership list plus `on_switch_org`/`switching` (the single-membership
//! rule — a static label, no false dropdown affordance — derives from the
//! list length exactly as upstream); `useNotifications`+`unreadCount` →
//! the `unread` count. The notifications/docs link targets are props per
//! navigation-is-props; `on_open_search`/`on_open_nav` were already
//! callbacks in the reference (wired by the shell).

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dropdown_menu::{
    DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator,
    DropdownMenuTrigger,
};
use crate::icons::{
    Icon, RI_ARROW_DOWN_S_LINE, RI_ARROW_RIGHT_S_LINE, RI_BUILDING_4_LINE, RI_CHECK_LINE,
    RI_EYE_LINE, RI_MENU_LINE, RI_NOTIFICATION_3_LINE, RI_QUESTION_LINE, RI_SEARCH_LINE,
};
use leptos::prelude::*;

pub const TOP: &str = "asy-top";
pub const TOP_BURGER: &str = "asy-top__burger";
pub const TOP_BURGER_ICON: &str = "asy-top__burger-icon";
pub const TOP_CLUSTER: &str = "asy-top__cluster";
pub const TOP_WS: &str = "asy-top__ws";
pub const TOP_WS_ICON: &str = "asy-top__ws-icon";
pub const TOP_WS_BTN: &str = "asy-top__ws-btn";
pub const TOP_PILL: &str = "asy-top__pill";
pub const TOP_PILL_ICON: &str = "asy-top__pill-icon";
pub const TOP_CRUMBS: &str = "asy-top__crumbs";
pub const TOP_CRUMB: &str = "asy-top__crumb";
pub const TOP_CRUMB_LAST: &str = "asy-top__crumb--last";
pub const TOP_CRUMB_ARROW: &str = "asy-top__crumb-arrow";
pub const TOP_SPACER: &str = "asy-top__spacer";
pub const TOP_SEARCH: &str = "asy-top__search";
pub const TOP_SEARCH_ICON: &str = "asy-top__search-icon";
pub const TOP_SEARCH_TEXT: &str = "asy-top__search-text";
pub const TOP_KBD: &str = "asy-top__kbd";
pub const TOP_CMD: &str = "asy-top__cmd";
pub const TOP_CMD_ICON: &str = "asy-top__cmd-icon";
pub const TOP_ICONBTN: &str = "asy-top__iconbtn";
pub const TOP_ICONBTN_REL: &str = "asy-top__iconbtn--rel";
pub const TOP_BTN_ICON: &str = "asy-top__btn-icon";
pub const TOP_DOT: &str = "asy-top__dot";
pub const TOP_MENU: &str = "asy-top__menu";
pub const TOP_ORG_ICON: &str = "asy-top__org-icon";
pub const TOP_ORG_NAME: &str = "asy-top__org-name";
pub const TOP_ORG_CHECK: &str = "asy-top__org-check";

/// One row of `GET /auth/me/orgs` the switcher renders.
#[derive(Clone, PartialEq, Debug)]
pub struct OrgMembership {
    pub org_id: String,
    pub name: String,
    pub is_active: bool,
}

#[component]
pub fn Topbar(
    /// Breadcrumb segments (the reference's `ReactNode[]`).
    #[prop(optional)]
    breadcrumb: Vec<ViewFn>,
    #[prop(optional)] platform_mode: bool,
    /// Shell-resolved org label used until the membership list is loaded.
    org_name: Option<String>,
    /// The caller's org memberships (`useMyOrgs` in the reference).
    #[prop(optional)]
    orgs: Vec<OrgMembership>,
    /// The consumer's `useSetActiveOrg.mutate`.
    #[prop(into)]
    on_switch_org: Callback<String>,
    /// The switch mutation's `isPending`.
    #[prop(optional, into)]
    switching: Signal<bool>,
    /// Unread notification count (`unreadCount(useNotifications().data)`).
    #[prop(optional)]
    unread: u32,
    /// Bell / help targets (`/app/notifications`, `/docs` on the site).
    #[prop(into)]
    notifications_href: String,
    #[prop(into)] help_href: String,
    /// Opens the global ⌘K command palette (wired by the shell).
    #[prop(optional)]
    on_open_search: Option<Callback<()>>,
    /// Opens the mobile nav drawer (below md; wired by the shell).
    #[prop(optional)]
    on_open_nav: Option<Callback<()>>,
) -> impl IntoView {
    let bell_label = if unread > 0 {
        format!("Notifications ({unread} unread)")
    } else {
        "Notifications".to_owned()
    };
    view! {
        <header class=TOP>
            <button
                type="button"
                on:click=move |_| {
                    if let Some(cb) = on_open_nav {
                        cb.run(());
                    }
                }
                aria-label="Open navigation"
                class=TOP_BURGER
            >
                <Icon d=RI_MENU_LINE class=TOP_BURGER_ICON />
            </button>
            <div class=TOP_CLUSTER>
                <WorkspaceSwitcher
                    org_name=org_name
                    orgs=orgs
                    on_switch_org=on_switch_org
                    switching=switching
                />
                {platform_mode
                    .then(|| {
                        view! {
                            <span class=TOP_PILL>
                                <Icon d=RI_EYE_LINE class=TOP_PILL_ICON />
                                "Platform · all orgs"
                            </span>
                        }
                    })}
            </div>
            {(!breadcrumb.is_empty())
                .then(|| {
                    let last = breadcrumb.len() - 1;
                    view! {
                        <nav class=TOP_CRUMBS>
                            <Icon d=RI_ARROW_RIGHT_S_LINE class=TOP_CRUMB_ARROW />
                            {breadcrumb
                                .into_iter()
                                .enumerate()
                                .map(|(i, b)| {
                                    let cls = if i == last {
                                        format!("{TOP_CRUMB} {TOP_CRUMB_LAST}")
                                    } else {
                                        TOP_CRUMB.to_owned()
                                    };
                                    view! {
                                        <span class=cls>{b.run()}</span>
                                        {(i < last)
                                            .then(|| {
                                                view! {
                                                    <Icon
                                                        d=RI_ARROW_RIGHT_S_LINE
                                                        class=TOP_CRUMB_ARROW
                                                    />
                                                }
                                            })}
                                    }
                                })
                                .collect_view()}
                        </nav>
                    }
                })}
            <div class=TOP_SPACER></div>
            <button
                type="button"
                on:click=move |_| {
                    if let Some(cb) = on_open_search {
                        cb.run(());
                    }
                }
                aria-label="Search (Command-K)"
                class=TOP_SEARCH
            >
                <Icon d=RI_SEARCH_LINE class=TOP_SEARCH_ICON />
                <span class=TOP_SEARCH_TEXT>"Search devices, tunnels, IPs…"</span>
                <span class=format!("mono {TOP_KBD}")>"⌘K"</span>
            </button>
            <button
                type="button"
                on:click=move |_| {
                    if let Some(cb) = on_open_search {
                        cb.run(());
                    }
                }
                aria-label="Open command palette"
                class=TOP_CMD
            >
                <Icon d=RI_SEARCH_LINE class=TOP_CMD_ICON />
            </button>
            <Button
                variant=ButtonVariant::Ghost
                size=ButtonSize::Sm
                class=format!("{TOP_ICONBTN} {TOP_ICONBTN_REL}")
                href=notifications_href
                attr:aria-label=bell_label
            >
                <Icon d=RI_NOTIFICATION_3_LINE class=TOP_BTN_ICON />
                {(unread > 0).then(|| view! { <span class=TOP_DOT></span> })}
            </Button>
            <Button
                variant=ButtonVariant::Ghost
                size=ButtonSize::Sm
                class=TOP_ICONBTN
                href=help_href
                attr:aria-label="Help"
            >
                <Icon d=RI_QUESTION_LINE class=TOP_BTN_ICON />
            </Button>
        </header>
    }
}

/// Workspace control: static label for a single membership, a real
/// switcher for more.
#[component]
fn WorkspaceSwitcher(
    org_name: Option<String>,
    orgs: Vec<OrgMembership>,
    on_switch_org: Callback<String>,
    switching: Signal<bool>,
) -> impl IntoView {
    use leptos::either::Either;
    let label = orgs
        .iter()
        .find(|o| o.is_active)
        .map(|o| o.name.clone())
        .or(org_name)
        .unwrap_or_else(|| "Workspace".to_owned());
    if orgs.len() <= 1 {
        return Either::Left(view! {
            <span class=TOP_WS>
                <Icon d=RI_BUILDING_4_LINE class=TOP_WS_ICON />
                {label}
            </span>
        });
    }
    let rows = StoredValue::new(orgs);
    Either::Right(view! {
        <DropdownMenu>
            <DropdownMenuTrigger
                variant=ButtonVariant::Ghost
                size=ButtonSize::Sm
                class=TOP_WS_BTN
                attr:disabled=move || switching.get().then_some("")
            >
                <Icon d=RI_BUILDING_4_LINE class=TOP_WS_ICON />
                {label}
                <Icon d=RI_ARROW_DOWN_S_LINE class=TOP_WS_ICON />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" class=TOP_MENU>
                <DropdownMenuLabel>"Switch workspace"</DropdownMenuLabel>
                <DropdownMenuSeparator />
                {rows
                    .get_value()
                    .into_iter()
                    .map(|o| {
                        let id = o.org_id.clone();
                        let is_active = o.is_active;
                        view! {
                            <DropdownMenuItem
                                disabled=is_active || switching.get_untracked()
                                on_select=Callback::new(move |_| {
                                    if !is_active {
                                        on_switch_org.run(id.clone());
                                    }
                                })
                            >
                                <Icon d=RI_BUILDING_4_LINE class=TOP_ORG_ICON />
                                <span class=TOP_ORG_NAME>{o.name}</span>
                                {is_active
                                    .then(|| {
                                        view! { <Icon d=RI_CHECK_LINE class=TOP_ORG_CHECK /> }
                                    })}
                            </DropdownMenuItem>
                        }
                    })
                    .collect_view()}
            </DropdownMenuContent>
        </DropdownMenu>
    })
}

pub fn css() -> String {
    format!(
        concat!(
            ".{top}{{display:flex;height:3.5rem;flex-shrink:0;align-items:center;",
            "gap:1rem;border:0 solid var(--color-border);border-bottom-width:1px;",
            "background-color:var(--color-bg);padding-inline:1.25rem}}",
            ".{burger}{{margin-left:-.25rem;display:flex;width:2.25rem;",
            "height:2.25rem;flex-shrink:0;align-items:center;",
            "justify-content:center;border-radius:var(--radius-sm);",
            "color:var(--color-text-muted)}}",
            "@media (hover:hover){{.{burger}:hover{{",
            "background-color:var(--color-surface-2);color:var(--color-text)}}}}",
            "@media (width >= 48rem){{.{burger}{{display:none}}}}",
            ".{burger_icon}{{width:1.25rem;height:1.25rem}}",
            ".{cluster}{{display:flex;align-items:center;gap:.5rem}}",
            ".{ws}{{display:inline-flex;height:1.875rem;align-items:center;",
            "gap:.375rem;min-width:0;overflow:hidden;padding-inline:.5rem;",
            "font-size:13px;font-weight:500;",
            "text-overflow:ellipsis;white-space:nowrap}}",
            ".{ws_icon}{{width:.875rem;height:.875rem;flex-shrink:0;",
            "color:var(--color-text-dim)}}",
            ".{ws_btn}{{height:1.875rem;min-width:0;overflow:hidden;",
            "padding-inline:.5rem;font-weight:500;",
            "text-overflow:ellipsis;white-space:nowrap}}",
            // warning pill: arbitrary oklch border compiles to rgba (TT-2).
            ".{pill}{{display:inline-flex;align-items:center;gap:.25rem;",
            "border-radius:.25rem;border:1px solid rgba(227,173,75,.3);",
            "padding-inline:.5rem;padding-block:.125rem;font-size:11px;",
            "color:var(--color-warning);background:var(--color-warning-soft)}}",
            ".{pill_icon}{{width:.75rem;height:.75rem}}",
            ".{crumbs}{{display:flex;min-width:0;overflow:hidden;",
            "align-items:center;gap:.25rem;",
            "font-size:13px;color:var(--color-text-muted)}}",
            ".{crumb}{{overflow:hidden;text-overflow:ellipsis;",
            "white-space:nowrap;color:var(--color-text-muted)}}",
            ".{crumb_last}{{color:var(--color-text)}}",
            ".{crumb_arrow}{{width:.875rem;height:.875rem;flex-shrink:0;opacity:.5}}",
            ".{spacer}{{flex:1 1 0%}}",
            ".{search}{{position:relative;display:none;height:1.875rem;",
            "width:240px;align-items:center;border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface);padding-left:2rem;",
            "padding-right:.5rem;text-align:left;font-size:12.5px;",
            "color:var(--color-text-dim);",
            "transition-property:color,background-color,border-color,",
            "text-decoration-color,fill,stroke;",
            "transition-timing-function:cubic-bezier(.4,0,.2,1);",
            "transition-duration:.15s}}",
            "@media (hover:hover){{.{search}:hover{{",
            "border-color:var(--color-accent-line);",
            "color:var(--color-text-muted)}}}}",
            "@media (width >= 48rem){{.{search}{{display:flex}}}}",
            ".{search_icon}{{pointer-events:none;position:absolute;",
            "left:.625rem;top:50%;width:.875rem;height:.875rem;",
            "translate:0 -50%;color:var(--color-text-dim)}}",
            ".{search_text}{{flex:1 1 0%;overflow:hidden;",
            "text-overflow:ellipsis;white-space:nowrap}}",
            ".{kbd}{{margin-left:.5rem;border-radius:.25rem;",
            "border:1px solid var(--color-border);padding-inline:5px;",
            "padding-block:1px;font-size:10.5px;line-height:1}}",
            ".{cmd}{{display:inline-flex;width:2.25rem;height:2.25rem;",
            "flex-shrink:0;align-items:center;justify-content:center;",
            "border-radius:var(--radius-sm);color:var(--color-text-muted)}}",
            "@media (hover:hover){{.{cmd}:hover{{",
            "background-color:var(--color-surface-2);color:var(--color-text)}}}}",
            "@media (width >= 48rem){{.{cmd}{{display:none}}}}",
            ".{cmd_icon}{{width:1rem;height:1rem}}",
            ".{iconbtn}{{padding-inline:.5rem}}",
            ".{iconbtn_rel}{{position:relative}}",
            ".{btn_icon}{{width:1rem;height:1rem}}",
            ".{dot}{{position:absolute;right:.375rem;top:.25rem;width:.375rem;",
            "height:.375rem;border-radius:calc(infinity * 1px);",
            "background-color:var(--color-accent)}}",
            ".{menu}{{min-width:220px}}",
            ".{org_icon}{{width:.875rem;height:.875rem;",
            "color:var(--color-text-dim)}}",
            ".{org_name}{{flex:1 1 0%;overflow:hidden;text-overflow:ellipsis;",
            "white-space:nowrap}}",
            ".{org_check}{{width:.875rem;height:.875rem;",
            "color:var(--color-accent)}}",
        ),
        top = TOP,
        burger = TOP_BURGER,
        burger_icon = TOP_BURGER_ICON,
        cluster = TOP_CLUSTER,
        ws = TOP_WS,
        ws_icon = TOP_WS_ICON,
        ws_btn = TOP_WS_BTN,
        pill = TOP_PILL,
        pill_icon = TOP_PILL_ICON,
        crumbs = TOP_CRUMBS,
        crumb = TOP_CRUMB,
        crumb_last = TOP_CRUMB_LAST,
        crumb_arrow = TOP_CRUMB_ARROW,
        spacer = TOP_SPACER,
        search = TOP_SEARCH,
        search_icon = TOP_SEARCH_ICON,
        search_text = TOP_SEARCH_TEXT,
        kbd = TOP_KBD,
        cmd = TOP_CMD,
        cmd_icon = TOP_CMD_ICON,
        iconbtn = TOP_ICONBTN,
        iconbtn_rel = TOP_ICONBTN_REL,
        btn_icon = TOP_BTN_ICON,
        dot = TOP_DOT,
        menu = TOP_MENU,
        org_icon = TOP_ORG_ICON,
        org_name = TOP_ORG_NAME,
        org_check = TOP_ORG_CHECK,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            TOP, TOP_BURGER, TOP_BURGER_ICON, TOP_CLUSTER, TOP_WS, TOP_WS_ICON, TOP_WS_BTN,
            TOP_PILL, TOP_PILL_ICON, TOP_CRUMBS, TOP_CRUMB, TOP_CRUMB_LAST, TOP_CRUMB_ARROW,
            TOP_SPACER, TOP_SEARCH, TOP_SEARCH_ICON, TOP_SEARCH_TEXT, TOP_KBD, TOP_CMD,
            TOP_CMD_ICON, TOP_ICONBTN, TOP_ICONBTN_REL, TOP_BTN_ICON, TOP_DOT, TOP_MENU,
            TOP_ORG_ICON, TOP_ORG_NAME, TOP_ORG_CHECK,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
