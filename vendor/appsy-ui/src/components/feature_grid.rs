//! FeatureGrid — port of `marketing/feature-grid.tsx`: six feature cells in
//! a hairline grid (1px gaps over a border-color backdrop) that steps
//! 1 → 2 → 3 columns at the sm/md breakpoints. No props; hardcoded
//! marketing copy per ALLOW-HARDCODE.

use crate::icons::{
    Icon, RI_FILE_LIST_3_LINE, RI_FLASHLIGHT_LINE, RI_GLOBAL_LINE, RI_ROUTER_LINE, RI_ROUTE_LINE,
    RI_SHIELD_FLASH_LINE,
};
use leptos::prelude::*;

pub const FGRID: &str = "asy-fgrid";
pub const FGRID_GRID: &str = "asy-fgrid__grid";
pub const FGRID_CELL: &str = "asy-fgrid__cell";
pub const FGRID_ICON: &str = "asy-fgrid__icon";
pub const FGRID_H3: &str = "asy-fgrid__h3";
pub const FGRID_P: &str = "asy-fgrid__p";

const CELLS: [(&str, &str, &str); 6] = [
    (
        RI_ROUTER_LINE,
        "Dedicated IPs",
        "A public IP attached to your tunnel. Survives reconnects. Bind it to one device or a whole org.",
    ),
    (
        RI_SHIELD_FLASH_LINE,
        "Always-on agent",
        "Connects on boot, before the user logs in. Reconnects on a network change in under 800ms.",
    ),
    (
        RI_FLASHLIGHT_LINE,
        "Kill-switch",
        "If the tunnel drops, traffic stops. Trusted networks can pass; everything else fails closed.",
    ),
    (
        RI_GLOBAL_LINE,
        "Magic DNS",
        "Resolve other tunneled devices by name. In-house recursive resolver, four filter SKUs.",
    ),
    (
        RI_ROUTE_LINE,
        "Path-tier routing",
        "Five presets — Simple, Privacy, P2P, Static IP, Streaming. We classify each flow, you don't write rules.",
    ),
    (
        RI_FILE_LIST_3_LINE,
        "Audit log",
        "Every connect, ACL hit, policy change. Append-only, signed, exportable. SAML/SSO on enterprise.",
    ),
];

#[component]
pub fn FeatureGrid() -> impl IntoView {
    view! {
        <section class=FGRID>
            <div class=FGRID_GRID style="background: var(--color-border);">
                {CELLS
                    .iter()
                    .map(|(d, h, p)| {
                        view! {
                            <div class=FGRID_CELL style="background: var(--color-bg);">
                                <Icon d=*d class=FGRID_ICON />
                                <h3 class=FGRID_H3>{*h}</h3>
                                <p class=FGRID_P>{*p}</p>
                            </div>
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
            ".{fgrid}{{margin-inline:auto;max-width:1180px;padding-inline:1rem;",
            "padding-bottom:3rem;padding-top:.75rem}}",
            "@media (width >= 40rem){{.{fgrid}{{padding-inline:2rem}}}}",
            ".{grid}{{display:grid;grid-template-columns:repeat(1,minmax(0,1fr));gap:1px;",
            "overflow:hidden;border-radius:var(--radius-lg);border-width:1px;",
            "border-color:var(--color-border)}}",
            "@media (width >= 40rem){{.{grid}{{grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            "@media (width >= 48rem){{.{grid}{{grid-template-columns:repeat(3,minmax(0,1fr))}}}}",
            ".{cell}{{display:flex;min-height:168px;flex-direction:column;gap:.625rem;",
            "padding:1.5rem}}",
            ".{icon}{{width:1.25rem;height:1.25rem;color:var(--color-accent)}}",
            ".{h3}{{font-size:15px;font-weight:600}}",
            ".{p}{{font-size:13px;line-height:1.55;color:var(--color-text-muted)}}",
        ),
        fgrid = FGRID,
        grid = FGRID_GRID,
        cell = FGRID_CELL,
        icon = FGRID_ICON,
        h3 = FGRID_H3,
        p = FGRID_P,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [FGRID, FGRID_GRID, FGRID_CELL, FGRID_ICON, FGRID_H3, FGRID_P] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn cells_mirror_reference() {
        assert_eq!(CELLS.len(), 6);
        assert_eq!(CELLS[4].1, "Path-tier routing");
    }
}
