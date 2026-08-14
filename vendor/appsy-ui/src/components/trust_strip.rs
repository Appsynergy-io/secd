//! TrustStrip — port of `marketing/trust-strip.tsx`: the bordered strip of
//! five icon+label trust items under the marketing hero. No props; the item
//! catalog is the reference's hardcoded marketing copy (ALLOW-HARDCODE).

use crate::icons::{
    Icon, RI_CLOSE_CIRCLE_LINE, RI_LOCK_2_LINE, RI_ROUTE_LINE, RI_SHIELD_CHECK_LINE, RI_TIME_LINE,
};
use leptos::prelude::*;

pub const TRUST_STRIP: &str = "asy-trust-strip";
pub const TRUST_STRIP_ROW: &str = "asy-trust-strip__row";
pub const TRUST_STRIP_ITEM: &str = "asy-trust-strip__item";
pub const TRUST_STRIP_ICON: &str = "asy-trust-strip__icon";

const ITEMS: [(&str, &str); 5] = [
    (RI_LOCK_2_LINE, "Stripe + BTC/ETH"),
    (RI_TIME_LINE, "7-day trial"),
    (RI_CLOSE_CIRCLE_LINE, "Cancel any day"),
    (RI_SHIELD_CHECK_LINE, "WireGuard + QUIC (beta)"),
    (RI_ROUTE_LINE, "Path-tier routing"),
];

#[component]
pub fn TrustStrip() -> impl IntoView {
    view! {
        <section class=TRUST_STRIP>
            <div class=TRUST_STRIP_ROW>
                {ITEMS
                    .iter()
                    .map(|(d, label)| {
                        view! {
                            <span class=TRUST_STRIP_ITEM>
                                <Icon d=*d class=TRUST_STRIP_ICON />
                                {*label}
                            </span>
                        }
                    })
                    .collect_view()}
            </div>
        </section>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{strip}{{border-top-width:1px;border-bottom-width:1px;",
            "border-color:var(--color-border)}}",
            ".{row}{{margin-inline:auto;display:flex;max-width:1180px;flex-wrap:wrap;",
            "align-items:center;justify-content:space-between;gap:1rem;",
            "padding-inline:1rem;padding-block:1rem;font-size:12.5px;",
            "color:var(--color-text-muted)}}",
            "@media (width >= 40rem){{.{row}{{padding-inline:2rem}}}}",
            ".{item}{{display:inline-flex;align-items:center;gap:.5rem}}",
            ".{icon}{{width:.875rem;height:.875rem;color:var(--color-text-dim)}}",
        ),
        strip = TRUST_STRIP,
        row = TRUST_STRIP_ROW,
        item = TRUST_STRIP_ITEM,
        icon = TRUST_STRIP_ICON,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [TRUST_STRIP, TRUST_STRIP_ROW, TRUST_STRIP_ITEM, TRUST_STRIP_ICON] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn item_catalog_mirrors_reference() {
        assert_eq!(ITEMS.len(), 5);
        assert_eq!(ITEMS[3].1, "WireGuard + QUIC (beta)");
    }
}
