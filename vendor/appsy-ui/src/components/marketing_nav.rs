//! MarketingNav — port of `marketing/marketing-nav.tsx`: the sticky
//! backdrop-blurred marketing bar. Logo, hover/focus-revealed Product
//! dropdown (pure CSS: group-hover + group-focus-within, hover legs
//! `@media(hover:hover)`-gated exactly like Tailwind's variants), five nav
//! links, auth actions, and below `md` a hamburger + stacked sheet driven
//! by internal open state exactly like the reference's `useState`.
//!
//! All navigation data is props (navigation is always props): items,
//! product entries, and the home/sign-in/sign-up targets. Active-link
//! state is data too — the reference derives `NavLink` activeness from the
//! router, which stays in the consumer.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::logo::{Logo, LogoSize};
use crate::icons::{Icon, RI_ARROW_DOWN_S_LINE, RI_CLOSE_LINE, RI_MENU_LINE};
use leptos::prelude::*;

pub const MNAV: &str = "asy-mnav";
pub const MNAV_ROW: &str = "asy-mnav__row";
pub const MNAV_DESKTOP: &str = "asy-mnav__desktop";
pub const MNAV_GROUP: &str = "asy-mnav__group";
pub const MNAV_PRODUCT_LINK: &str = "asy-mnav__product-link";
pub const MNAV_CARET: &str = "asy-mnav__caret";
pub const MNAV_DROPWRAP: &str = "asy-mnav__dropwrap";
pub const MNAV_DROPPANEL: &str = "asy-mnav__droppanel";
pub const MNAV_DROPITEM: &str = "asy-mnav__dropitem";
pub const MNAV_DROPLABEL: &str = "asy-mnav__droplabel";
pub const MNAV_DROPDESC: &str = "asy-mnav__dropdesc";
pub const MNAV_LINK: &str = "asy-mnav__link";
pub const MNAV_LINK_ACTIVE: &str = "asy-mnav__link--active";
pub const MNAV_SPACER: &str = "asy-mnav__spacer";
pub const MNAV_ACTIONS: &str = "asy-mnav__actions";
pub const MNAV_BURGER: &str = "asy-mnav__burger";
pub const MNAV_BURGER_ICON: &str = "asy-mnav__burger-icon";
pub const MNAV_SHEET: &str = "asy-mnav__sheet";
pub const MNAV_SHEET_TITLE: &str = "asy-mnav__sheet-title";
pub const MNAV_SHEET_GRID: &str = "asy-mnav__sheet-grid";
pub const MNAV_SHEET_LINK: &str = "asy-mnav__sheet-link";
pub const MNAV_SHEET_NAV: &str = "asy-mnav__sheet-nav";
pub const MNAV_SHEET_NAVLINK: &str = "asy-mnav__sheet-navlink";
pub const MNAV_SHEET_ACTIONS: &str = "asy-mnav__sheet-actions";

#[derive(Clone, PartialEq)]
pub struct NavItem {
    pub label: String,
    pub href: String,
    /// The reference's router-derived `NavLink` active state, as data.
    pub active: bool,
}

#[derive(Clone, PartialEq)]
pub struct ProductItem {
    pub label: String,
    pub href: String,
    pub desc: String,
}

#[component]
pub fn MarketingNav(
    /// The five top-level links.
    items: Vec<NavItem>,
    /// The Product dropdown entries.
    products: Vec<ProductItem>,
    #[prop(into)] home_href: String,
    /// Target of the "Product" trigger link itself.
    #[prop(into)] product_href: String,
    /// Router-active state of the Product trigger.
    #[prop(optional)] product_active: bool,
    #[prop(into)] sign_in_href: String,
    #[prop(into)] sign_up_href: String,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let sheet_products = products.clone();
    let sheet_items = items.clone();
    let sheet_sign_in = sign_in_href.clone();
    let sheet_sign_up = sign_up_href.clone();
    let product_cls = if product_active {
        format!("{MNAV_PRODUCT_LINK} {MNAV_LINK_ACTIVE}")
    } else {
        MNAV_PRODUCT_LINK.to_owned()
    };
    view! {
        <nav
            class=MNAV
            style="background: color-mix(in oklch, var(--color-bg) 88%, transparent);"
        >
            <div class=MNAV_ROW>
                <a href=home_href aria-label="appsynergy home" on:click=move |_| open.set(false)>
                    <Logo size=LogoSize::Sm />
                </a>
                <div class=MNAV_DESKTOP>
                    <div class=MNAV_GROUP>
                        <a class=product_cls href=product_href>
                            "Product"
                            <Icon d=RI_ARROW_DOWN_S_LINE class=MNAV_CARET />
                        </a>
                        <div class=MNAV_DROPWRAP>
                            <div class=MNAV_DROPPANEL>
                                {products
                                    .into_iter()
                                    .map(|p| {
                                        view! {
                                            <a class=MNAV_DROPITEM href=p.href>
                                                <span class=MNAV_DROPLABEL>{p.label}</span>
                                                <span class=MNAV_DROPDESC>{p.desc}</span>
                                            </a>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                    </div>
                    {items
                        .into_iter()
                        .map(|item| {
                            let cls = if item.active {
                                format!("{MNAV_LINK} {MNAV_LINK_ACTIVE}")
                            } else {
                                MNAV_LINK.to_owned()
                            };
                            view! { <a class=cls href=item.href>{item.label}</a> }
                        })
                        .collect_view()}
                </div>
                <div class=MNAV_SPACER></div>
                <div class=MNAV_ACTIONS>
                    <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm href=sign_in_href>
                        "Sign in"
                    </Button>
                    <Button variant=ButtonVariant::Primary size=ButtonSize::Sm href=sign_up_href>
                        "Start 7-day trial"
                    </Button>
                </div>
                <button
                    type="button"
                    aria-label=move || if open.get() { "Close menu" } else { "Open menu" }
                    aria-expanded=move || if open.get() { "true" } else { "false" }
                    class=MNAV_BURGER
                    on:click=move |_| open.update(|v| *v = !*v)
                >
                    {move || {
                        if open.get() {
                            view! { <Icon d=RI_CLOSE_LINE class=MNAV_BURGER_ICON /> }
                        } else {
                            view! { <Icon d=RI_MENU_LINE class=MNAV_BURGER_ICON /> }
                        }
                    }}
                </button>
            </div>
            {move || {
                open.get()
                    .then(|| {
                        let products = sheet_products.clone();
                        let items = sheet_items.clone();
                        let sign_in = sheet_sign_in.clone();
                        let sign_up = sheet_sign_up.clone();
                        view! {
                            <div class=MNAV_SHEET>
                                <div class=MNAV_SHEET_TITLE>"Product"</div>
                                <div class=MNAV_SHEET_GRID>
                                    {products
                                        .into_iter()
                                        .map(|p| {
                                            view! {
                                                <a
                                                    class=MNAV_SHEET_LINK
                                                    href=p.href
                                                    on:click=move |_| open.set(false)
                                                >
                                                    {p.label}
                                                </a>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                                <div class=MNAV_SHEET_NAV>
                                    {items
                                        .into_iter()
                                        .map(|item| {
                                            let cls = if item.active {
                                                format!("{MNAV_SHEET_NAVLINK} {MNAV_LINK_ACTIVE}")
                                            } else {
                                                MNAV_SHEET_NAVLINK.to_owned()
                                            };
                                            view! {
                                                <a
                                                    class=cls
                                                    href=item.href
                                                    on:click=move |_| open.set(false)
                                                >
                                                    {item.label}
                                                </a>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                                <div class=MNAV_SHEET_ACTIONS>
                                    <Button
                                        variant=ButtonVariant::Ghost
                                        size=ButtonSize::Sm
                                        href=sign_in
                                    >
                                        "Sign in"
                                    </Button>
                                    <Button
                                        variant=ButtonVariant::Primary
                                        size=ButtonSize::Sm
                                        href=sign_up
                                    >
                                        "Start 7-day trial"
                                    </Button>
                                </div>
                            </div>
                        }
                    })
            }}
        </nav>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{nav}{{position:sticky;top:0;z-index:10;border-bottom-width:1px;",
            "border-color:var(--color-border);backdrop-filter:blur(8px)}}",
            ".{row}{{margin-inline:auto;display:flex;max-width:1180px;align-items:center;",
            "gap:1.75rem;padding-inline:1rem;padding-block:1rem}}",
            "@media (width >= 40rem){{.{row}{{padding-inline:2rem}}}}",
            ".{desktop}{{display:none;align-items:center;gap:22px;font-size:13px;",
            "color:var(--color-text-muted)}}",
            "@media (width >= 48rem){{.{desktop}{{display:flex}}}}",
            ".{group}{{position:relative}}",
            ".{plink}{{display:flex;cursor:pointer;align-items:center;gap:.25rem;",
            "transition-property:color,background-color,border-color,",
            "text-decoration-color,fill,stroke;",
            "transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}",
            "@media(hover:hover){{.{plink}:hover{{color:var(--color-text)}}}}",
            ".{group}:focus-within .{plink}{{color:var(--color-text)}}",
            ".{caret}{{width:.75rem;height:.75rem;opacity:.6}}",
            ".{dropwrap}{{visibility:hidden;position:absolute;left:0;top:100%;z-index:20;",
            "padding-top:.5rem;opacity:0;transition-property:opacity;",
            "transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}",
            "@media(hover:hover){{.{group}:hover .{dropwrap}{{visibility:visible;opacity:1}}}}",
            ".{group}:focus-within .{dropwrap}{{visibility:visible;opacity:1}}",
            ".{droppanel}{{width:280px;border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:.375rem;box-shadow:0 10px 15px -3px #0000001a,0 4px 6px -4px #0000001a}}",
            ".{dropitem}{{display:block;border-radius:var(--radius-sm);",
            "padding-inline:.75rem;padding-block:.5rem}}",
            "@media(hover:hover){{.{dropitem}:hover{{background-color:var(--color-surface-2)}}}}",
            ".{droplabel}{{display:block;font-size:13px;font-weight:500;",
            "color:var(--color-text)}}",
            ".{dropdesc}{{display:block;font-size:11.5px;color:var(--color-text-muted)}}",
            ".{link}{{display:flex;cursor:pointer;align-items:center;gap:.25rem;",
            "transition-property:color,background-color,border-color,",
            "text-decoration-color,fill,stroke;",
            "transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}}",
            "@media(hover:hover){{.{link}:hover{{color:var(--color-text)}}}}",
            ".{active}{{color:var(--color-text)}}",
            ".{spacer}{{flex:1}}",
            ".{actions}{{display:none;align-items:center;gap:.5rem}}",
            "@media (width >= 48rem){{.{actions}{{display:flex}}}}",
            ".{burger}{{display:flex;width:2.25rem;height:2.25rem;align-items:center;",
            "justify-content:center;border-radius:var(--radius-sm);",
            "color:var(--color-text-muted)}}",
            "@media(hover:hover){{.{burger}:hover{{background-color:var(--color-surface-2);",
            "color:var(--color-text)}}}}",
            "@media (width >= 48rem){{.{burger}{{display:none}}}}",
            ".{burger_icon}{{width:1.25rem;height:1.25rem}}",
            ".{sheet}{{border-top-width:1px;border-color:var(--color-border);",
            "background-color:var(--color-surface);padding-inline:1rem;",
            "padding-block:.75rem;",
            "max-height:min(70dvh,calc(100dvh - 3.5rem));overflow-y:auto}}",
            "@media (width >= 48rem){{.{sheet}{{display:none}}}}",
            ".{sheet_title}{{margin-bottom:.5rem;font-size:11px;font-weight:500;",
            "letter-spacing:.025em;color:var(--color-text-muted);text-transform:uppercase}}",
            ".{sheet_grid}{{margin-bottom:.75rem;display:grid;gap:.25rem}}",
            ".{sheet_link}{{border-radius:var(--radius-sm);padding-inline:.75rem;",
            "padding-block:.5rem;font-size:14px;color:var(--color-text)}}",
            "@media(hover:hover){{.{sheet_link}:hover{{",
            "background-color:var(--color-surface-2)}}}}",
            ".{sheet_nav}{{display:grid;gap:.25rem;border-top-width:1px;",
            "border-color:var(--color-border);padding-top:.75rem}}",
            ".{sheet_navlink}{{border-radius:var(--radius-sm);padding-inline:.75rem;",
            "padding-block:.5rem;font-size:14px;color:var(--color-text-muted)}}",
            "@media(hover:hover){{.{sheet_navlink}:hover{{",
            "background-color:var(--color-surface-2);color:var(--color-text)}}}}",
            ".{sheet_actions}{{margin-top:.75rem;display:grid;gap:.5rem;",
            "border-top-width:1px;border-color:var(--color-border);padding-top:.75rem}}",
        ),
        nav = MNAV,
        row = MNAV_ROW,
        desktop = MNAV_DESKTOP,
        group = MNAV_GROUP,
        plink = MNAV_PRODUCT_LINK,
        caret = MNAV_CARET,
        dropwrap = MNAV_DROPWRAP,
        droppanel = MNAV_DROPPANEL,
        dropitem = MNAV_DROPITEM,
        droplabel = MNAV_DROPLABEL,
        dropdesc = MNAV_DROPDESC,
        link = MNAV_LINK,
        active = MNAV_LINK_ACTIVE,
        spacer = MNAV_SPACER,
        actions = MNAV_ACTIONS,
        burger = MNAV_BURGER,
        burger_icon = MNAV_BURGER_ICON,
        sheet = MNAV_SHEET,
        sheet_title = MNAV_SHEET_TITLE,
        sheet_grid = MNAV_SHEET_GRID,
        sheet_link = MNAV_SHEET_LINK,
        sheet_nav = MNAV_SHEET_NAV,
        sheet_navlink = MNAV_SHEET_NAVLINK,
        sheet_actions = MNAV_SHEET_ACTIONS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            MNAV, MNAV_ROW, MNAV_DESKTOP, MNAV_GROUP, MNAV_PRODUCT_LINK, MNAV_CARET,
            MNAV_DROPWRAP, MNAV_DROPPANEL, MNAV_DROPITEM, MNAV_DROPLABEL, MNAV_DROPDESC,
            MNAV_LINK, MNAV_LINK_ACTIVE, MNAV_SPACER, MNAV_ACTIONS, MNAV_BURGER,
            MNAV_BURGER_ICON, MNAV_SHEET, MNAV_SHEET_TITLE, MNAV_SHEET_GRID, MNAV_SHEET_LINK,
            MNAV_SHEET_NAV, MNAV_SHEET_NAVLINK, MNAV_SHEET_ACTIONS,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn hover_reveals_are_hover_gated_and_focus_within_is_not() {
        let css = css();
        assert!(css.contains(&format!(
            "@media(hover:hover){{.{MNAV_GROUP}:hover .{MNAV_DROPWRAP}"
        )));
        assert!(css.contains(&format!(
            ".{MNAV_GROUP}:focus-within .{MNAV_DROPWRAP}{{visibility:visible"
        )));
    }
}
