//! DashShell + PageHead — port of `dashboard/dash-shell.tsx` (T1).
//!
//! Props/callbacks split: `useMe`/`useOrg` → the identity props
//! (`user_name`/`user_email`/`is_platform_admin`) and `org_name` (which the
//! reference already lets a caller override; the consumer resolves it);
//! `useLocation().pathname` → the `path` prop, which re-keys the content
//! region and dismisses the mobile drawer on navigation exactly like the
//! reference's effects. The ⌘K/Ctrl+K document listener and the drawer
//! state are presentation behavior and port here. The palette's nav
//! catalog arrives as `palette_groups` (nav-as-props; the reference's
//! palette derives it from the hardcoded catalogs) and navigation leaves
//! via `on_navigate`.

use crate::components::command_palette::{CommandPalette, PaletteGroup};
use crate::components::sidebar::{SideNavItem, Sidebar};
use crate::components::topbar::{OrgMembership, Topbar};
use leptos::prelude::*;

pub const SHELL: &str = "asy-shell";
pub const SHELL_DESKTOP_SIDE: &str = "asy-shell__desktop-side";
pub const SHELL_DRAWER: &str = "asy-shell__drawer";
pub const SHELL_SCRIM: &str = "asy-shell__scrim";
pub const SHELL_DRAWER_PANEL: &str = "asy-shell__drawer-panel";
pub const SHELL_MAIN_COL: &str = "asy-shell__main-col";
pub const SHELL_MAIN: &str = "asy-shell__main";
pub const PAGEHEAD: &str = "asy-pagehead";
pub const PAGEHEAD_COL: &str = "asy-pagehead__col";
pub const PAGEHEAD_ROW: &str = "asy-pagehead__row";
pub const PAGEHEAD_H1: &str = "asy-pagehead__h1";
pub const PAGEHEAD_SUB: &str = "asy-pagehead__sub";
pub const PAGEHEAD_ACTIONS: &str = "asy-pagehead__actions";

/// The app-stable half of [`DashShell`]'s props: identity, nav catalogs,
/// hrefs, org memberships, and callbacks — everything a consumer resolves
/// once per session rather than per page. Group them here, keep the
/// page-varying props (`path`, `platform_mode`, `breadcrumb`,
/// `content_pad`, children) on the component call. Additive convenience
/// (2026-08-07): [`DashShell`]'s prop-by-prop form is unchanged; a
/// consumer app wrapper around either form remains the recommended shape.
#[derive(Clone)]
pub struct DashShellConfig {
    pub show_platform: bool,
    pub org_name: Option<String>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub is_platform_admin: bool,
    pub customer_items: Vec<SideNavItem>,
    pub platform_items: Vec<SideNavItem>,
    pub home_href: String,
    pub account_href: String,
    pub platform_href: Option<String>,
    pub notifications_href: String,
    pub help_href: String,
    /// Topbar search caption; the VPN product copy when `None` (additive
    /// 2026-08-14).
    pub search_text: Option<String>,
    pub orgs: Vec<OrgMembership>,
    pub on_switch_org: Callback<String>,
    pub switching: Signal<bool>,
    pub unread: u32,
    pub on_sign_out: Callback<()>,
    pub signing_out: Signal<bool>,
    pub palette_groups: Vec<PaletteGroup>,
    pub on_navigate: Callback<String>,
}

/// [`DashShell`] taking its app-stable props as one [`DashShellConfig`].
/// Renders exactly what the prop-by-prop form renders (test-enforced).
#[component]
pub fn ConfiguredDashShell(
    config: DashShellConfig,
    #[prop(optional)] platform_mode: bool,
    #[prop(optional)] breadcrumb: Vec<ViewFn>,
    /// Inner padding for the main content region.
    #[prop(optional, into, default = "16px 16px".into())]
    content_pad: String,
    /// Current route path — see [`DashShell`]'s `path`.
    #[prop(into)]
    path: Signal<String>,
    children: ChildrenFn,
) -> impl IntoView {
    view! {
        <DashShell
            show_platform=config.show_platform
            platform_mode=platform_mode
            breadcrumb=breadcrumb
            org_name=config.org_name
            content_pad=content_pad
            user_name=config.user_name
            user_email=config.user_email
            is_platform_admin=config.is_platform_admin
            path=path
            customer_items=config.customer_items
            platform_items=config.platform_items
            home_href=config.home_href
            account_href=config.account_href
            platform_href=config.platform_href
            notifications_href=config.notifications_href
            help_href=config.help_href
            search_text=config
                .search_text
                .unwrap_or_else(|| "Search devices, tunnels, IPs…".to_owned())
            orgs=config.orgs
            on_switch_org=config.on_switch_org
            switching=config.switching
            unread=config.unread
            on_sign_out=config.on_sign_out
            signing_out=config.signing_out
            palette_groups=config.palette_groups
            on_navigate=config.on_navigate
            children=children
        />
    }
}

/// Two-column dashboard shell — sidebar left, topbar + scrollable main
/// right; fixed slide-over drawer below `md`.
#[component]
pub fn DashShell(
    #[prop(optional)] show_platform: bool,
    #[prop(optional)] platform_mode: bool,
    #[prop(optional)] breadcrumb: Vec<ViewFn>,
    org_name: Option<String>,
    /// Inner padding for the main content region.
    #[prop(optional, into, default = "16px 16px".into())]
    content_pad: String,
    user_name: Option<String>,
    user_email: Option<String>,
    #[prop(optional)] is_platform_admin: bool,
    /// Current route path — re-keys the content region (mount entrance)
    /// and dismisses the drawer on navigation.
    #[prop(into)]
    path: Signal<String>,
    customer_items: Vec<SideNavItem>,
    #[prop(optional)] platform_items: Vec<SideNavItem>,
    #[prop(into)] home_href: String,
    #[prop(into)] account_href: String,
    platform_href: Option<String>,
    #[prop(into)] notifications_href: String,
    #[prop(into)] help_href: String,
    /// Topbar search caption — see [`Topbar`]'s `search_text`.
    #[prop(optional, into, default = "Search devices, tunnels, IPs…".into())]
    search_text: String,
    #[prop(optional)] orgs: Vec<OrgMembership>,
    #[prop(into)] on_switch_org: Callback<String>,
    #[prop(optional, into)] switching: Signal<bool>,
    #[prop(optional)] unread: u32,
    #[prop(into)] on_sign_out: Callback<()>,
    #[prop(optional, into)] signing_out: Signal<bool>,
    #[prop(optional)] palette_groups: Vec<PaletteGroup>,
    #[prop(into)] on_navigate: Callback<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let search_open = RwSignal::new(false);
    let nav_open = RwSignal::new(false);

    // Global ⌘K / Ctrl+K toggle — the reference's document listener.
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        struct KeyGuard(Closure<dyn FnMut(web_sys::KeyboardEvent)>);
        impl Drop for KeyGuard {
            fn drop(&mut self) {
                let _ = leptos::tachys::dom::document().remove_event_listener_with_callback(
                    "keydown",
                    self.0.as_ref().unchecked_ref(),
                );
            }
        }

        let handler = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |e: web_sys::KeyboardEvent| {
                if (e.meta_key() || e.ctrl_key()) && e.key().to_lowercase() == "k" {
                    e.prevent_default();
                    search_open.update(|o| *o = !*o);
                }
            },
        );
        let _ = leptos::tachys::dom::document()
            .add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        let guard = send_wrapper::SendWrapper::new(KeyGuard(handler));
        on_cleanup(move || drop(guard));
    }

    // Drawer open: body scroll lock (B5/D14), mirrored from dialog's
    // scroll_lock::apply/restore + data-scroll-locked.
    #[cfg(any(feature = "csr", feature = "hydrate"))]
    {
        use crate::behavior::scroll_lock::{self, SavedBodyStyle};

        struct DrawerScrollLock(SavedBodyStyle);
        impl Drop for DrawerScrollLock {
            fn drop(&mut self) {
                let document = leptos::tachys::dom::document();
                if let Some(body) = document.body() {
                    let _ = body.remove_attribute("data-scroll-locked");
                }
                scroll_lock::restore(&document, &self.0);
            }
        }

        let lock: StoredValue<Option<send_wrapper::SendWrapper<DrawerScrollLock>>> =
            StoredValue::new(None);
        Effect::new(move |_| {
            if nav_open.get() {
                if lock.with_value(Option::is_some) {
                    return;
                }
                let document = leptos::tachys::dom::document();
                let window = leptos::tachys::dom::window();
                let Some(body) = document.body() else {
                    return;
                };
                let saved = scroll_lock::apply(&document, &window);
                let _ = body.set_attribute("data-scroll-locked", "1");
                lock.set_value(Some(send_wrapper::SendWrapper::new(DrawerScrollLock(saved))));
            } else {
                lock.set_value(None);
            }
        });
        on_cleanup(move || lock.set_value(None));
    }

    // Navigating dismisses the drawer — the reference's `[pathname]` effect.
    Effect::new(move |prev: Option<String>| {
        let p = path.get();
        if prev.is_some_and(|old| old != p) {
            nav_open.set(false);
        }
        p
    });

    let sidebar_data = StoredValue::new((
        customer_items,
        platform_items,
        home_href.into(),
        account_href.into(),
        platform_href,
        user_name,
        user_email,
    ));
    let sidebar = move || {
        sidebar_data.with_value(|(ci, pi, hh, ah, ph, un, ue): &(
            Vec<SideNavItem>,
            Vec<SideNavItem>,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )| {
            view! {
                <Sidebar
                    show_platform=show_platform
                    platform_mode=platform_mode
                    user_name=un.clone()
                    user_email=ue.clone()
                    is_platform_admin=is_platform_admin
                    path=path
                    home_href=hh.clone()
                    customer_items=ci.clone()
                    platform_items=pi.clone()
                    account_href=ah.clone()
                    platform_href=ph.clone()
                    on_sign_out=on_sign_out
                    signing_out=signing_out
                />
            }
        })
    };

    let children = StoredValue::new(children);
    view! {
        <div class=SHELL>
            <div class=SHELL_DESKTOP_SIDE>{sidebar()}</div>
            {move || {
                nav_open
                    .get()
                    .then(|| {
                        view! {
                            <div class=SHELL_DRAWER>
                                <button
                                    type="button"
                                    aria-label="Close navigation"
                                    class=SHELL_SCRIM
                                    on:click=move |_| nav_open.set(false)
                                ></button>
                                <div class=SHELL_DRAWER_PANEL>{sidebar()}</div>
                            </div>
                        }
                    })
            }}
            <div class=SHELL_MAIN_COL>
                <Topbar
                    platform_mode=platform_mode
                    breadcrumb=breadcrumb
                    org_name=org_name
                    orgs=orgs
                    on_switch_org=on_switch_org
                    switching=switching
                    unread=unread
                    notifications_href=notifications_href
                    help_href=help_href
                    search_text=search_text
                    on_open_search=Callback::new(move |_| search_open.set(true))
                    on_open_nav=Callback::new(move |_| nav_open.set(true))
                />
                <main class=SHELL_MAIN style:padding=content_pad>
                    // Re-keyed by path: each navigation replays the mount
                    // entrance (inert under prefers-reduced-motion).
                    {move || {
                        let _ = path.get();
                        view! {
                            <div class="fade-slide-in">
                                {children.with_value(|c| c())}
                            </div>
                        }
                    }}
                </main>
            </div>
            <CommandPalette
                open=search_open
                on_open_change=Callback::new(move |o| search_open.set(o))
                groups=palette_groups
                on_navigate=on_navigate
            />
        </div>
    }
}

/// Page heading block: title + chips row, optional subtitle, actions.
#[component]
pub fn PageHead(
    #[prop(into)] title: ViewFn,
    #[prop(optional)] subtitle: Option<ViewFn>,
    #[prop(optional)] chips: Option<ViewFn>,
    #[prop(optional)] actions: Option<ViewFn>,
) -> impl IntoView {
    view! {
        <div class=PAGEHEAD>
            <div class=PAGEHEAD_COL>
                <div class=PAGEHEAD_ROW>
                    <h1 class=PAGEHEAD_H1>{title.run()}</h1>
                    {chips.map(|c| c.run())}
                </div>
                {subtitle.map(|s| view! { <p class=PAGEHEAD_SUB>{s.run()}</p> })}
            </div>
            {actions.map(|a| view! { <div class=PAGEHEAD_ACTIONS>{a.run()}</div> })}
        </div>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{shell}{{display:flex;height:100svh;width:100%;overflow:hidden;",
            "background-color:var(--color-bg);",
            "padding-top:env(safe-area-inset-top);",
            "padding-right:env(safe-area-inset-right);",
            "padding-bottom:env(safe-area-inset-bottom);",
            "padding-left:env(safe-area-inset-left)}}",
            ".{desktop_side}{{display:none}}",
            "@media (width >= 48rem){{.{desktop_side}{{display:contents}}}}",
            ".{drawer}{{position:fixed;inset:0;z-index:40;display:flex}}",
            "@media (width >= 48rem){{.{drawer}{{display:none}}}}",
            // bg-black/50 via Tailwind v4's color-mix computes to oklab.
            ".{scrim}{{position:absolute;inset:0;",
            "background-color:oklab(0 0 0 / 0.5)}}",
            ".{drawer_panel}{{position:relative;z-index:10;height:100%}}",
            ".{main_col}{{display:flex;min-width:0;flex:1;flex-direction:column}}",
            ".{main}{{flex:1;overflow:auto}}",
            ".{ph}{{margin-bottom:1.25rem;display:flex;flex-wrap:wrap;",
            "align-items:flex-start;justify-content:space-between;gap:1rem}}",
            ".{ph_col}{{display:flex;min-width:0;flex:1 1 12rem;",
            "flex-direction:column;gap:.25rem}}",
            ".{ph_row}{{display:flex;align-items:center;gap:.625rem}}",
            ".{ph_h1}{{font-size:22px;font-weight:600;letter-spacing:-0.02em}}",
            ".{ph_sub}{{font-size:13px;color:var(--color-text-muted)}}",
            ".{ph_actions}{{display:flex;align-items:center;gap:.5rem}}",
        ),
        shell = SHELL,
        desktop_side = SHELL_DESKTOP_SIDE,
        drawer = SHELL_DRAWER,
        scrim = SHELL_SCRIM,
        drawer_panel = SHELL_DRAWER_PANEL,
        main_col = SHELL_MAIN_COL,
        main = SHELL_MAIN,
        ph = PAGEHEAD,
        ph_col = PAGEHEAD_COL,
        ph_row = PAGEHEAD_ROW,
        ph_h1 = PAGEHEAD_H1,
        ph_sub = PAGEHEAD_SUB,
        ph_actions = PAGEHEAD_ACTIONS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            SHELL, SHELL_DESKTOP_SIDE, SHELL_DRAWER, SHELL_SCRIM, SHELL_DRAWER_PANEL,
            SHELL_MAIN_COL, SHELL_MAIN, PAGEHEAD, PAGEHEAD_COL, PAGEHEAD_ROW, PAGEHEAD_H1,
            PAGEHEAD_SUB, PAGEHEAD_ACTIONS,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
