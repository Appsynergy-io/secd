//! QuoteOrPricing — port of `marketing/quote-or-pricing.tsx`: the pricing
//! mini-grid (three plans, middle one "most popular") beside the customer
//! quote card. No props; all copy is the reference's hardcoded marketing
//! content (ALLOW-HARDCODE).

use crate::icons::{Icon, RI_DOUBLE_QUOTES_L};
use leptos::prelude::*;

pub const QOP: &str = "asy-qop";
pub const QOP_ROW: &str = "asy-qop__row";
pub const QOP_PRICING: &str = "asy-qop__pricing";
pub const QOP_EYEBROW: &str = "asy-qop__eyebrow";
pub const QOP_GRID: &str = "asy-qop__grid";
pub const QOP_PLAN: &str = "asy-qop__plan";
pub const QOP_POP: &str = "asy-qop__pop";
pub const QOP_PLAN_NAME: &str = "asy-qop__plan-name";
pub const QOP_PRICE_ROW: &str = "asy-qop__price-row";
pub const QOP_PRICE: &str = "asy-qop__price";
pub const QOP_PER: &str = "asy-qop__per";
pub const QOP_PLAN_SUB: &str = "asy-qop__plan-sub";
pub const QOP_FINE: &str = "asy-qop__fine";
pub const QOP_QUOTE: &str = "asy-qop__quote";
pub const QOP_QUOTE_ICON: &str = "asy-qop__quote-icon";
pub const QOP_QUOTE_P: &str = "asy-qop__quote-p";
pub const QOP_ATTEST: &str = "asy-qop__attest";
pub const QOP_AVATAR: &str = "asy-qop__avatar";
pub const QOP_WHO: &str = "asy-qop__who";
pub const QOP_NAME: &str = "asy-qop__name";
pub const QOP_ROLE: &str = "asy-qop__role";

/// (name, price, per, sub, most-popular)
const PLANS: [(&str, &str, &str, &str, bool); 3] = [
    ("General Use VPN", "$5", "/ mo", "2 tunnels · shared IP", false),
    ("One Dedicated IP", "$14", "/ mo", "2 tunnels · 1 dedicated IP", true),
    ("Two Dedicated IP", "$21", "/ mo", "2 tunnels · 2 dedicated IPs", false),
];

fn mini_plan(
    name: &'static str,
    price: &'static str,
    per: &'static str,
    sub: &'static str,
    pop: bool,
) -> impl IntoView {
    let card_style = if pop {
        "border-color: var(--color-accent-line); background: var(--color-accent-soft);"
    } else {
        "border-color: var(--color-border); background: var(--color-surface-2);"
    };
    view! {
        <div class=QOP_PLAN style=card_style>
            {pop
                .then(|| {
                    view! {
                        <span
                            class=QOP_POP
                            style="border-color: var(--color-accent-line); background: var(--color-accent-soft); color: var(--color-accent);"
                        >
                            "most popular"
                        </span>
                    }
                })}
            <span class=QOP_PLAN_NAME>{name}</span>
            <div class=QOP_PRICE_ROW>
                <span class=QOP_PRICE>{price}</span>
                <span class=QOP_PER>{per}</span>
            </div>
            <span class=QOP_PLAN_SUB>{sub}</span>
        </div>
    }
}

#[component]
pub fn QuoteOrPricing() -> impl IntoView {
    view! {
        <section class=QOP>
            <div class=QOP_ROW>
                <div class=QOP_PRICING>
                    <span class=QOP_EYEBROW>"Pricing — three plans"</span>
                    <div class=QOP_GRID>
                        {PLANS
                            .iter()
                            .map(|(name, price, per, sub, pop)| mini_plan(name, price, per, sub, *pop))
                            .collect_view()}
                    </div>
                    <span class=QOP_FINE>
                        "Annual discount available. Path-tier overages billed on top, usage-based."
                    </span>
                </div>
                <div class=QOP_QUOTE>
                    <Icon d=RI_DOUBLE_QUOTES_L class=QOP_QUOTE_ICON />
                    <p class=QOP_QUOTE_P>
                        "We replaced an OpenVPN + jump-box setup that nobody wanted to maintain. The ACL builder shipped to forty engineers in a Friday afternoon. Nobody filed a ticket."
                    </p>
                    <div class=QOP_ATTEST>
                        <div class=QOP_AVATAR>"PR"</div>
                        <div class=QOP_WHO>
                            <span class=QOP_NAME>"Priya R."</span>
                            <span class=QOP_ROLE>
                                "Security engineer, 300-person SaaS · with permission"
                            </span>
                        </div>
                    </div>
                </div>
            </div>
        </section>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{qop}{{margin-inline:auto;max-width:1180px;padding-inline:1rem;",
            "padding-block:3rem}}",
            "@media (width >= 40rem){{.{qop}{{padding-inline:2rem}}}}",
            ".{row}{{display:flex;flex-direction:column;align-items:stretch;gap:1rem}}",
            "@media (width >= 64rem){{.{row}{{flex-direction:row}}}}",
            ".{pricing}{{display:flex;flex:1.4;flex-direction:column;gap:.875rem;",
            "border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:1.75rem}}",
            ".{eyebrow}{{font-size:11px;font-weight:600;text-transform:uppercase;",
            "letter-spacing:.08em;color:var(--color-accent)}}",
            ".{grid}{{display:grid;grid-template-columns:repeat(1,minmax(0,1fr));gap:.75rem}}",
            "@media (width >= 40rem){{.{grid}{{grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            "@media (width >= 48rem){{.{grid}{{grid-template-columns:repeat(3,minmax(0,1fr))}}}}",
            ".{plan}{{position:relative;display:flex;flex-direction:column;gap:.25rem;",
            "border-radius:var(--radius-md);border-width:1px;padding:.875rem}}",
            ".{pop}{{position:absolute;top:-.625rem;right:.75rem;display:inline-flex;",
            "height:18px;align-items:center;border-radius:calc(infinity * 1px);",
            "border-width:1px;padding-inline:.5rem;font-size:10px}}",
            ".{plan_name}{{font-size:11px;color:var(--color-text-muted)}}",
            ".{price_row}{{display:flex;align-items:baseline;gap:.25rem}}",
            ".{price}{{font-size:26px;font-weight:600;letter-spacing:-0.02em}}",
            ".{per}{{font-size:.75rem;line-height:calc(1/.75);color:var(--color-text-muted)}}",
            ".{plan_sub}{{font-size:11.5px;color:var(--color-text-muted)}}",
            ".{fine}{{font-size:.75rem;line-height:calc(1/.75);color:var(--color-text-muted)}}",
            ".{quote}{{display:flex;flex:1;flex-direction:column;justify-content:center;",
            "gap:.75rem;border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:1.75rem}}",
            ".{quote_icon}{{width:1.25rem;height:1.25rem;color:var(--color-accent)}}",
            ".{quote_p}{{text-wrap:balance;font-size:15.5px;line-height:1.55}}",
            ".{attest}{{margin-top:.25rem;display:flex;align-items:center;gap:.625rem}}",
            ".{avatar}{{display:flex;width:2rem;height:2rem;align-items:center;",
            "justify-content:center;border-radius:calc(infinity * 1px);",
            "background-color:var(--color-surface-2);font-size:.75rem;",
            "line-height:calc(1/.75);font-weight:600;color:var(--color-text-muted)}}",
            ".{who}{{display:flex;flex-direction:column}}",
            ".{name}{{font-size:12.5px;font-weight:500}}",
            ".{role}{{font-size:11.5px;color:var(--color-text-muted)}}",
        ),
        qop = QOP,
        row = QOP_ROW,
        pricing = QOP_PRICING,
        eyebrow = QOP_EYEBROW,
        grid = QOP_GRID,
        plan = QOP_PLAN,
        pop = QOP_POP,
        plan_name = QOP_PLAN_NAME,
        price_row = QOP_PRICE_ROW,
        price = QOP_PRICE,
        per = QOP_PER,
        plan_sub = QOP_PLAN_SUB,
        fine = QOP_FINE,
        quote = QOP_QUOTE,
        quote_icon = QOP_QUOTE_ICON,
        quote_p = QOP_QUOTE_P,
        attest = QOP_ATTEST,
        avatar = QOP_AVATAR,
        who = QOP_WHO,
        name = QOP_NAME,
        role = QOP_ROLE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            QOP, QOP_ROW, QOP_PRICING, QOP_EYEBROW, QOP_GRID, QOP_PLAN, QOP_POP, QOP_PLAN_NAME,
            QOP_PRICE_ROW, QOP_PRICE, QOP_PER, QOP_PLAN_SUB, QOP_FINE, QOP_QUOTE, QOP_QUOTE_ICON,
            QOP_QUOTE_P, QOP_ATTEST, QOP_AVATAR, QOP_WHO, QOP_NAME, QOP_ROLE,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn plans_mirror_reference() {
        assert_eq!(PLANS.len(), 3);
        assert!(PLANS[1].4, "middle plan is most-popular");
        assert_eq!(PLANS[1].1, "$14");
    }
}
