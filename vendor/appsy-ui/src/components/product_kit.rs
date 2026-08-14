//! Product-kit — port of `marketing/product-kit.tsx`: the within-page
//! building blocks of the `/product/*` marketing pages. Hero, feature
//! block, two-column, pro/con card, price tile, tier preview, DNS filter
//! row, mock terminal, and the accent beta chip.
//!
//! The reference exports its chip as `BetaPill`, colliding with the
//! dashboard's gray `BetaPill` (`ip-chip.tsx`) in the crate's flat
//! prelude — the marketing one is `MktBetaPill` here (same DOM, distinct
//! name; the only rename in the kit).
//!
//! Content slots (`ReactNode` props in the reference) are `ViewFn`s;
//! plain-string usage stays ergonomic through `ViewFn: From<closure>`.

use crate::icons::Icon;
use leptos::prelude::*;

pub const MKT_BETA: &str = "asy-mkt-beta";
pub const MKT_DOT: &str = "asy-mkt-dot";
pub const KIT_HERO: &str = "asy-kit-hero";
pub const KIT_HERO_GRID: &str = "asy-kit-hero__grid";
pub const KIT_HERO_GRID_VISUAL: &str = "asy-kit-hero__grid--visual";
pub const KIT_HERO_COL: &str = "asy-kit-hero__col";
pub const KIT_HERO_KICKER: &str = "asy-kit-hero__kicker";
pub const KIT_HERO_H1: &str = "asy-kit-hero__h1";
pub const KIT_HERO_SUB: &str = "asy-kit-hero__sub";
pub const KIT_HERO_CTAS: &str = "asy-kit-hero__ctas";
pub const KIT_FEAT: &str = "asy-kit-feat";
pub const KIT_FEAT_ICON: &str = "asy-kit-feat__icon";
pub const KIT_FEAT_H3: &str = "asy-kit-feat__h3";
pub const KIT_FEAT_P: &str = "asy-kit-feat__p";
pub const KIT_TWOCOL: &str = "asy-kit-twocol";
pub const KIT_PROS: &str = "asy-kit-pros";
pub const KIT_PROS_GLYPH: &str = "asy-kit-pros__glyph";
pub const KIT_PROS_H: &str = "asy-kit-pros__h";
pub const KIT_PROS_P: &str = "asy-kit-pros__p";
pub const KIT_PRICE: &str = "asy-kit-price";
pub const KIT_PRICE_POP: &str = "asy-kit-price__pop";
pub const KIT_PRICE_NAME: &str = "asy-kit-price__name";
pub const KIT_PRICE_ROW: &str = "asy-kit-price__row";
pub const KIT_PRICE_VALUE: &str = "asy-kit-price__value";
pub const KIT_PRICE_PER: &str = "asy-kit-price__per";
pub const KIT_PRICE_SUB: &str = "asy-kit-price__sub";
pub const KIT_TIER: &str = "asy-kit-tier";
pub const KIT_TIER_ICON: &str = "asy-kit-tier__icon";
pub const KIT_TIER_NAME: &str = "asy-kit-tier__name";
pub const KIT_TIER_LINE: &str = "asy-kit-tier__line";
pub const KIT_TIER_DIAGRAM: &str = "asy-kit-tier__diagram";
pub const KIT_TIER_COST: &str = "asy-kit-tier__cost";
pub const KIT_FILTER: &str = "asy-kit-filter";
pub const KIT_FILTER_HEAD: &str = "asy-kit-filter__head";
pub const KIT_FILTER_SLUG: &str = "asy-kit-filter__slug";
pub const KIT_FILTER_COUNT: &str = "asy-kit-filter__count";
pub const KIT_FILTER_DESC: &str = "asy-kit-filter__desc";
pub const KIT_TERM: &str = "asy-kit-term";
pub const KIT_TERM_BAR: &str = "asy-kit-term__bar";
pub const KIT_TERM_DOT: &str = "asy-kit-term__dot";
pub const KIT_TERM_TITLE: &str = "asy-kit-term__title";
pub const KIT_TERM_PRE: &str = "asy-kit-term__pre";
pub const KIT_TERM_PROMPT: &str = "asy-kit-term__prompt";
pub const KIT_TERM_PAD: &str = "asy-kit-term__pad";
pub const KIT_TERM_CMD: &str = "asy-kit-term__cmd";
pub const KIT_TERM_OUT: &str = "asy-kit-term__out";

/// The marketing "beta" chip (accent pill; the dashboard's gray pill is
/// `ip_chip::BetaPill`).
#[component]
pub fn MktBetaPill() -> impl IntoView {
    view! { <span class=MKT_BETA>"beta"</span> }
}

/// Product-page hero: kicker chip, big headline, sub, CTAs, optional visual.
#[component]
pub fn MktHero(
    #[prop(optional)] kicker: Option<ViewFn>,
    h1: ViewFn,
    sub: ViewFn,
    #[prop(optional)] ctas: Option<ViewFn>,
    #[prop(optional)] visual: Option<ViewFn>,
) -> impl IntoView {
    let grid_class = if visual.is_some() {
        format!("{KIT_HERO_GRID} {KIT_HERO_GRID_VISUAL}")
    } else {
        KIT_HERO_GRID.to_string()
    };
    view! {
        <section class=KIT_HERO>
            <div class=grid_class>
                <div class=KIT_HERO_COL>
                    {kicker
                        .map(|k| {
                            view! {
                                <span class=KIT_HERO_KICKER>
                                    <span class=MKT_DOT aria-hidden="true"></span>
                                    " "
                                    {k.run()}
                                </span>
                            }
                        })}
                    <h1 class=KIT_HERO_H1>{h1.run()}</h1>
                    <p class=KIT_HERO_SUB>{sub.run()}</p>
                    {ctas.map(|c| view! { <div class=KIT_HERO_CTAS>{c.run()}</div> })}
                </div>
                {visual.map(|v| view! { <div>{v.run()}</div> })}
            </div>
        </section>
    }
}

/// Icon + heading + paragraph, the workhorse of the product feature grids.
#[component]
pub fn MktFeatureBlock(icon: &'static str, h: ViewFn, p: ViewFn) -> impl IntoView {
    view! {
        <div class=KIT_FEAT>
            <Icon d=icon class=KIT_FEAT_ICON />
            <h3 class=KIT_FEAT_H3>{h.run()}</h3>
            <p class=KIT_FEAT_P>{p.run()}</p>
        </div>
    }
}

/// Two equal columns, vertically centered — prose beside visual.
#[component]
pub fn MktTwoCol(left: ViewFn, right: ViewFn) -> impl IntoView {
    view! {
        <div class=KIT_TWOCOL>
            <div>{left.run()}</div>
            <div>{right.run()}</div>
        </div>
    }
}

/// Pro/con card with a success or warning border + glyph (QUIC tradeoffs).
#[component]
pub fn ProsCard(#[prop(optional)] pro: bool, h: ViewFn, p: ViewFn) -> impl IntoView {
    let border = if pro {
        "border-color: oklch(70% 0.15 145 / 0.30);"
    } else {
        "border-color: oklch(78% 0.13 80 / 0.30);"
    };
    let glyph_color = if pro {
        "color: var(--color-success);"
    } else {
        "color: var(--color-warning);"
    };
    view! {
        <div class=KIT_PROS style=border>
            <span class=KIT_PROS_GLYPH style=glyph_color aria-hidden="true">
                {if pro { "✓" } else { "!" }}
            </span>
            <span class=KIT_PROS_H>{h.run()}</span>
            <p class=KIT_PROS_P>{p.run()}</p>
        </div>
    }
}

/// Compact price tile; `pop` lights the accent border + "most popular" chip.
#[component]
pub fn PriceCard(
    #[prop(into)] name: String,
    #[prop(into)] price: String,
    #[prop(into)] sub: String,
    #[prop(optional)] pop: bool,
) -> impl IntoView {
    let style = if pop {
        "border-color: var(--color-accent-line); background: linear-gradient(180deg, var(--color-accent-soft), transparent), var(--color-surface);"
    } else {
        "border-color: var(--color-border); background: var(--color-surface);"
    };
    view! {
        <div class=KIT_PRICE style=style>
            {pop.then(|| view! { <span class=KIT_PRICE_POP>"most popular"</span> })}
            <span class=KIT_PRICE_NAME>{name}</span>
            <div class=KIT_PRICE_ROW>
                <span class=KIT_PRICE_VALUE>{price}</span>
                <span class=KIT_PRICE_PER>"/ mo"</span>
            </div>
            <span class=KIT_PRICE_SUB>{sub}</span>
        </div>
    }
}

/// One of the five path-tier preview cards (icon, blurb, mini-path, cost).
#[component]
pub fn TierPreviewCard(
    icon: &'static str,
    #[prop(into)] name: String,
    #[prop(into)] line: String,
    #[prop(into)] cost: String,
    diagram: ViewFn,
) -> impl IntoView {
    view! {
        <div class=KIT_TIER>
            <Icon d=icon class=KIT_TIER_ICON />
            <span class=KIT_TIER_NAME>{name}</span>
            <p class=KIT_TIER_LINE>{line}</p>
            <div class=KIT_TIER_DIAGRAM>{diagram.run()}</div>
            <span class=KIT_TIER_COST>{cost}</span>
        </div>
    }
}

/// DNS filter-SKU row: mono slug + entry count + plain-English description.
#[component]
pub fn FilterRow(
    #[prop(into)] slug: String,
    #[prop(into)] count: String,
    #[prop(into)] desc: String,
) -> impl IntoView {
    view! {
        <div class=KIT_FILTER>
            <div class=KIT_FILTER_HEAD>
                <span class=KIT_FILTER_SLUG>{slug}</span>
                <span class=KIT_FILTER_COUNT>{count} " entries"</span>
            </div>
            <p class=KIT_FILTER_DESC>{desc}</p>
        </div>
    }
}

/// Terminal mockup. Each line is `(prompt, text)`; prompt rows accent the
/// prompt and brighten the text, output rows indent and stay muted.
#[component]
pub fn MockTerminal(
    #[prop(into)] title: String,
    lines: Vec<(Option<String>, String)>,
) -> impl IntoView {
    view! {
        <div class=KIT_TERM>
            <div class=KIT_TERM_BAR>
                <span class=KIT_TERM_DOT></span>
                <span class=KIT_TERM_DOT></span>
                <span class=KIT_TERM_DOT></span>
                <span class=KIT_TERM_TITLE>{title}</span>
            </div>
            <pre class=KIT_TERM_PRE>
                {lines
                    .into_iter()
                    .map(|(prompt, text)| {
                        let text_cls = if prompt.is_some() { KIT_TERM_CMD } else { KIT_TERM_OUT };
                        view! {
                            {match prompt {
                                Some(p) => view! {
                                    <span class=KIT_TERM_PROMPT>{p} " "</span>
                                }
                                .into_any(),
                                None => view! { <span class=KIT_TERM_PAD></span> }.into_any(),
                            }}
                            <span class=text_cls>{text}</span>
                            "\n"
                        }
                    })
                    .collect_view()}
            </pre>
        </div>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{beta}{{display:inline-flex;height:18px;align-items:center;",
            "border-radius:calc(infinity * 1px);border-width:1px;",
            "border-color:var(--color-accent-line);background-color:transparent;",
            "padding-inline:.5rem;font-size:10px;font-weight:500;",
            "color:var(--color-accent)}}",
            ".{dot}{{width:.375rem;height:.375rem;",
            "border-radius:calc(infinity * 1px);",
            "background-color:var(--color-accent)}}",
            ".{hero}{{margin-inline:auto;max-width:1180px;padding-inline:1rem;",
            "padding-bottom:3.5rem;padding-top:72px}}",
            "@media (width >= 40rem){{.{hero}{{padding-inline:2rem}}}}",
            ".{hero_grid}{{display:grid;align-items:center;gap:3rem}}",
            "@media (width >= 48rem){{.{hero_grid_visual}{{grid-template-columns:1.1fr 1fr}}}}",
            ".{hero_col}{{display:flex;flex-direction:column;gap:18px}}",
            ".{hero_kicker}{{display:inline-flex;width:fit-content;align-items:center;",
            "gap:.5rem;border-radius:calc(infinity * 1px);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding-inline:.625rem;padding-block:.25rem;font-size:12px;",
            "color:var(--color-text-muted)}}",
            ".{hero_h1}{{text-wrap:pretty;font-size:clamp(36px,4.6vw,56px);",
            "font-weight:600;line-height:1.05;letter-spacing:-0.03em}}",
            ".{hero_sub}{{max-width:540px;font-size:17px;line-height:1.55;",
            "color:var(--color-text-muted)}}",
            ".{hero_ctas}{{margin-top:.375rem;display:flex;flex-wrap:wrap;",
            "align-items:center;gap:.625rem}}",
            ".{feat}{{display:flex;flex-direction:column;gap:.625rem}}",
            ".{feat_icon}{{width:22px;height:22px;color:var(--color-accent)}}",
            ".{feat_h3}{{font-size:17px;font-weight:600;letter-spacing:-0.01em}}",
            ".{feat_p}{{font-size:13.5px;line-height:1.55;color:var(--color-text-muted)}}",
            ".{twocol}{{display:grid;align-items:center;gap:3rem}}",
            "@media (width >= 48rem){{.{twocol}{{",
            "grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            ".{pros}{{display:flex;flex-direction:column;gap:.5rem;",
            "border-radius:var(--radius-md);border-width:1px;",
            "background-color:var(--color-surface);padding:1.25rem}}",
            ".{pros_glyph}{{font-size:20px}}",
            ".{pros_h}{{font-size:14.5px;font-weight:600}}",
            ".{pros_p}{{font-size:13px;line-height:1.55;color:var(--color-text-muted)}}",
            ".{price}{{position:relative;border-radius:var(--radius-md);",
            "border-width:1px;padding:18px}}",
            ".{price_pop}{{position:absolute;top:-.625rem;right:.75rem;",
            "display:inline-flex;height:18px;align-items:center;",
            "border-radius:calc(infinity * 1px);border-width:1px;",
            "border-color:var(--color-accent-line);",
            "background-color:var(--color-accent-soft);padding-inline:.5rem;",
            "font-size:10px;font-weight:500;color:var(--color-accent)}}",
            ".{price_name}{{font-size:11.5px;color:var(--color-text-muted)}}",
            ".{price_row}{{margin-top:.25rem;display:flex;align-items:baseline;gap:3px}}",
            ".{price_value}{{font-size:30px;font-weight:600}}",
            ".{price_per}{{font-size:12px;color:var(--color-text-muted)}}",
            ".{price_sub}{{font-size:12px;color:var(--color-text-muted)}}",
            ".{tier}{{display:flex;flex-direction:column;gap:.625rem;",
            "border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:18px}}",
            ".{tier_icon}{{width:22px;height:22px;color:var(--color-accent)}}",
            ".{tier_name}{{font-size:15px;font-weight:600}}",
            ".{tier_line}{{min-height:56px;font-size:12.5px;line-height:1.5;",
            "color:var(--color-text-muted)}}",
            ".{tier_diagram}{{height:2.25rem}}",
            ".{tier_cost}{{font-family:var(--font-mono);font-size:11.5px;",
            "color:var(--color-text-muted)}}",
            ".{filter}{{display:flex;flex-direction:column;gap:.375rem;",
            "border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:1rem}}",
            ".{filter_head}{{display:flex;align-items:center;justify-content:space-between}}",
            ".{filter_slug}{{font-family:var(--font-mono);font-size:13.5px;",
            "font-weight:600;color:var(--color-accent)}}",
            ".{filter_count}{{font-family:var(--font-mono);font-size:12px;",
            "color:var(--color-text-muted)}}",
            ".{filter_desc}{{font-size:13px;line-height:1.55;",
            "color:var(--color-text-muted)}}",
            ".{term}{{overflow:hidden;border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface)}}",
            ".{term_bar}{{display:flex;align-items:center;gap:.375rem;",
            "border-bottom-width:1px;border-color:var(--color-border);",
            "padding-inline:.75rem;padding-block:.5rem}}",
            ".{term_dot}{{width:.625rem;height:.625rem;",
            "border-radius:calc(infinity * 1px);",
            "background-color:var(--color-surface-2)}}",
            ".{term_title}{{margin-left:.5rem;font-family:var(--font-mono);",
            "font-size:11px;color:var(--color-text-muted)}}",
            ".{term_pre}{{margin:0;overflow:auto;padding:1rem;",
            "font-family:var(--font-mono);font-size:12.5px;line-height:1.7;",
            "color:var(--color-text-muted)}}",
            ".{term_prompt}{{color:var(--color-accent)}}",
            ".{term_pad}{{padding-left:.875rem}}",
            ".{term_cmd}{{color:var(--color-text)}}",
            ".{term_out}{{color:var(--color-text-muted)}}",
        ),
        beta = MKT_BETA,
        dot = MKT_DOT,
        hero = KIT_HERO,
        hero_grid = KIT_HERO_GRID,
        hero_grid_visual = KIT_HERO_GRID_VISUAL,
        hero_col = KIT_HERO_COL,
        hero_kicker = KIT_HERO_KICKER,
        hero_h1 = KIT_HERO_H1,
        hero_sub = KIT_HERO_SUB,
        hero_ctas = KIT_HERO_CTAS,
        feat = KIT_FEAT,
        feat_icon = KIT_FEAT_ICON,
        feat_h3 = KIT_FEAT_H3,
        feat_p = KIT_FEAT_P,
        twocol = KIT_TWOCOL,
        pros = KIT_PROS,
        pros_glyph = KIT_PROS_GLYPH,
        pros_h = KIT_PROS_H,
        pros_p = KIT_PROS_P,
        price = KIT_PRICE,
        price_pop = KIT_PRICE_POP,
        price_name = KIT_PRICE_NAME,
        price_row = KIT_PRICE_ROW,
        price_value = KIT_PRICE_VALUE,
        price_per = KIT_PRICE_PER,
        price_sub = KIT_PRICE_SUB,
        tier = KIT_TIER,
        tier_icon = KIT_TIER_ICON,
        tier_name = KIT_TIER_NAME,
        tier_line = KIT_TIER_LINE,
        tier_diagram = KIT_TIER_DIAGRAM,
        tier_cost = KIT_TIER_COST,
        filter = KIT_FILTER,
        filter_head = KIT_FILTER_HEAD,
        filter_slug = KIT_FILTER_SLUG,
        filter_count = KIT_FILTER_COUNT,
        filter_desc = KIT_FILTER_DESC,
        term = KIT_TERM,
        term_bar = KIT_TERM_BAR,
        term_dot = KIT_TERM_DOT,
        term_title = KIT_TERM_TITLE,
        term_pre = KIT_TERM_PRE,
        term_prompt = KIT_TERM_PROMPT,
        term_pad = KIT_TERM_PAD,
        term_cmd = KIT_TERM_CMD,
        term_out = KIT_TERM_OUT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            MKT_BETA, MKT_DOT, KIT_HERO, KIT_HERO_GRID, KIT_HERO_GRID_VISUAL, KIT_HERO_COL, KIT_HERO_KICKER,
            KIT_HERO_H1, KIT_HERO_SUB, KIT_HERO_CTAS, KIT_FEAT, KIT_FEAT_ICON, KIT_FEAT_H3,
            KIT_FEAT_P, KIT_TWOCOL, KIT_PROS, KIT_PROS_GLYPH, KIT_PROS_H, KIT_PROS_P, KIT_PRICE,
            KIT_PRICE_POP, KIT_PRICE_NAME, KIT_PRICE_ROW, KIT_PRICE_VALUE, KIT_PRICE_PER,
            KIT_PRICE_SUB, KIT_TIER, KIT_TIER_ICON, KIT_TIER_NAME, KIT_TIER_LINE,
            KIT_TIER_DIAGRAM, KIT_TIER_COST, KIT_FILTER, KIT_FILTER_HEAD, KIT_FILTER_SLUG,
            KIT_FILTER_COUNT, KIT_FILTER_DESC, KIT_TERM, KIT_TERM_BAR, KIT_TERM_DOT,
            KIT_TERM_TITLE, KIT_TERM_PRE, KIT_TERM_PROMPT, KIT_TERM_PAD, KIT_TERM_CMD,
            KIT_TERM_OUT,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
