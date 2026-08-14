//! ScreenshotSection — port of `marketing/screenshot-section.tsx`: eyebrow +
//! headline over the faux MiniDashboard (sidebar nav, KPI grid, bandwidth
//! AreaChart, ACL-hit rows) with three numbered callouts floated over it.
//! No props; all content is the reference's hardcoded marketing mock
//! (ALLOW-HARDCODE). Charts-gated: the mock renders AreaChart.
//!
//! The 30 bandwidth points are the reference's
//! `round(20 + sin(i/3)*8 + i/2)` sequence precomputed — all integers, so
//! no cross-runtime float-formatting risk.

use crate::components::area_chart::AreaChart;
use crate::components::badge::{Badge, BadgeTone};
use crate::components::logo::{Logo, LogoSize};
use crate::icons::{
    Icon, RI_COMPUTER_LINE, RI_DASHBOARD_LINE, RI_GLOBAL_LINE, RI_ROUTER_LINE, RI_ROUTE_LINE,
    RI_SHIELD_FLASH_LINE, RI_SHIELD_KEYHOLE_LINE,
};
use leptos::prelude::*;

pub const SHOT: &str = "asy-shot";
pub const SHOT_HEAD: &str = "asy-shot__head";
pub const SHOT_EYEBROW: &str = "asy-shot__eyebrow";
pub const SHOT_H2: &str = "asy-shot__h2";
pub const SHOT_P: &str = "asy-shot__p";
pub const SHOT_FRAME: &str = "asy-shot__frame";
pub const SHOT_DASH: &str = "asy-shot__dash";
pub const SHOT_SIDE: &str = "asy-shot__side";
pub const SHOT_SIDE_GAP: &str = "asy-shot__side-gap";
pub const SHOT_NAVROW: &str = "asy-shot__navrow";
pub const SHOT_NAVROW_ON: &str = "asy-shot__navrow--on";
pub const SHOT_NAVICON: &str = "asy-shot__navicon";
pub const SHOT_NAVICON_ON: &str = "asy-shot__navicon--on";
pub const SHOT_MAIN: &str = "asy-shot__main";
pub const SHOT_TOP: &str = "asy-shot__top";
pub const SHOT_TITLE_COL: &str = "asy-shot__title-col";
pub const SHOT_TITLE: &str = "asy-shot__title";
pub const SHOT_SUB: &str = "asy-shot__sub";
pub const SHOT_ORG_BADGE: &str = "asy-shot__org-badge";
pub const SHOT_KPIS: &str = "asy-shot__kpis";
pub const SHOT_KPI: &str = "asy-shot__kpi";
pub const SHOT_KPI_LABEL: &str = "asy-shot__kpi-label";
pub const SHOT_KPI_VALUE: &str = "asy-shot__kpi-value";
pub const SHOT_CHART: &str = "asy-shot__chart";
pub const SHOT_CHART_HEAD: &str = "asy-shot__chart-head";
pub const SHOT_CHART_TITLE: &str = "asy-shot__chart-title";
pub const SHOT_CHART_SUB: &str = "asy-shot__chart-sub";
pub const SHOT_HITS: &str = "asy-shot__hits";
pub const SHOT_HITS_TITLE: &str = "asy-shot__hits-title";
pub const SHOT_HITS_COL: &str = "asy-shot__hits-col";
pub const SHOT_HIT: &str = "asy-shot__hit";
pub const SHOT_HIT_LEFT: &str = "asy-shot__hit-left";
pub const SHOT_HIT_BADGE: &str = "asy-shot__hit-badge";
pub const SHOT_HIT_WHO: &str = "asy-shot__hit-who";
pub const SHOT_HIT_ARROW: &str = "asy-shot__hit-arrow";
pub const SHOT_HIT_WHERE: &str = "asy-shot__hit-where";
pub const SHOT_HIT_WHEN: &str = "asy-shot__hit-when";
pub const SHOT_CALLOUT: &str = "asy-shot__callout";
pub const SHOT_CALLOUT_DOT: &str = "asy-shot__callout-dot";
pub const SHOT_CALLOUT_TEXT: &str = "asy-shot__callout-text";

const NAV: [(&str, &str); 7] = [
    ("Overview", RI_DASHBOARD_LINE),
    ("Tunnels", RI_SHIELD_FLASH_LINE),
    ("Dedicated IPs", RI_ROUTER_LINE),
    ("Devices", RI_COMPUTER_LINE),
    ("ACLs", RI_SHIELD_KEYHOLE_LINE),
    ("DNS", RI_GLOBAL_LINE),
    ("Path tiers", RI_ROUTE_LINE),
];

const KPIS: [(&str, &str, &str); 4] = [
    ("Tunnels", "18", "+2 today"),
    ("Devices", "42", "2 offline"),
    ("Dedicated IPs", "2/2", "in use"),
    ("Bandwidth", "187 GB", "of 500"),
];

const ACL_HITS: [(&str, &str, &str, &str); 3] = [
    ("allow", "laptop-lena", "staging.internal:443", "2m ago"),
    ("allow", "ci-runner-3", "registry.internal:5000", "9m ago"),
    ("deny", "contractor-6", "prod.db:5432", "12m ago"),
];

/// `Math.max(0, Math.round(20 + Math.sin(i/3)*8 + i/2))` for i in 0..30.
const POINTS: [f64; 30] = [
    20.0, 23.0, 26.0, 28.0, 30.0, 30.0, 30.0, 29.0, 28.0, 26.0, 23.0, 21.0, 20.0, 19.0, 19.0,
    20.0, 21.0, 24.0, 27.0, 30.0, 33.0, 36.0, 38.0, 39.0, 40.0, 40.0, 39.0, 37.0, 35.0, 33.0,
];

fn callout(x: &'static str, y: &'static str, label: &'static str, text: &'static str) -> impl IntoView {
    view! {
        <div class=SHOT_CALLOUT style=format!("left: {x}; top: {y};")>
            <span
                class=SHOT_CALLOUT_DOT
                style="background: var(--color-accent); box-shadow: 0 0 0 4px color-mix(in oklch, var(--color-accent) 30%, transparent);"
            >
                {label}
            </span>
            <span class=SHOT_CALLOUT_TEXT>{text}</span>
        </div>
    }
}

fn mini_dashboard() -> impl IntoView {
    view! {
        <div class=SHOT_DASH>
            <div class=SHOT_SIDE>
                <Logo size=LogoSize::Sm />
                <div class=SHOT_SIDE_GAP></div>
                {NAV
                    .iter()
                    .enumerate()
                    .map(|(i, (label, d))| {
                        let row = if i == 0 {
                            format!("{SHOT_NAVROW} {SHOT_NAVROW_ON}")
                        } else {
                            SHOT_NAVROW.to_owned()
                        };
                        let ic = if i == 0 {
                            format!("{SHOT_NAVICON} {SHOT_NAVICON_ON}")
                        } else {
                            SHOT_NAVICON.to_owned()
                        };
                        view! {
                            <div class=row>
                                <Icon d=*d class=ic />
                                {*label}
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
            <div class=SHOT_MAIN>
                <div class=SHOT_TOP>
                    <div class=SHOT_TITLE_COL>
                        <span class=SHOT_TITLE>"Overview"</span>
                        <span class=SHOT_SUB>"Acme Robotics · Two Dedicated IP · day 12 / 30"</span>
                    </div>
                    <Badge tone=BadgeTone::Warn class=SHOT_ORG_BADGE>"Platform · all orgs"</Badge>
                </div>
                <div class=SHOT_KPIS>
                    {KPIS
                        .iter()
                        .map(|(label, value, sub)| {
                            view! {
                                <div class=SHOT_KPI>
                                    <div class=SHOT_KPI_LABEL>{*label}</div>
                                    <div class=SHOT_KPI_VALUE>{*value}</div>
                                    <div class=SHOT_KPI_LABEL>{*sub}</div>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>
                <div class=SHOT_CHART>
                    <div class=SHOT_CHART_HEAD>
                        <span class=SHOT_CHART_TITLE>"Bandwidth · 30 days"</span>
                        <span class=SHOT_CHART_SUB>"187 / 500 GB"</span>
                    </div>
                    <AreaChart data=POINTS.to_vec() height=140.0 />
                </div>
                <div class=SHOT_HITS>
                    <div class=SHOT_HITS_TITLE>"ACL hits — last 24h"</div>
                    <div class=SHOT_HITS_COL>
                        {ACL_HITS
                            .iter()
                            .map(|(eff, who, place, when)| {
                                let tone = if *eff == "allow" { BadgeTone::Ok } else { BadgeTone::Bad };
                                view! {
                                    <div class=SHOT_HIT>
                                        <span class=SHOT_HIT_LEFT>
                                            <Badge tone=tone class=SHOT_HIT_BADGE>{*eff}</Badge>
                                            <span class=SHOT_HIT_WHO>{*who}</span>
                                            <span class=SHOT_HIT_ARROW>"→"</span>
                                            <span class=SHOT_HIT_WHERE>{*place}</span>
                                        </span>
                                        <span class=SHOT_HIT_WHEN>{*when}</span>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn ScreenshotSection() -> impl IntoView {
    view! {
        <section class=SHOT>
            <div class=SHOT_HEAD>
                <span class=SHOT_EYEBROW>"One dashboard"</span>
                <h2 class=SHOT_H2>"Everyone — customer, support, admin — lands here."</h2>
                <p class=SHOT_P>
                    "Role decides which nav items render. Same chrome, same density, same accent. No \u{201c}admin app\u{201d} in a different colour."
                </p>
            </div>
            <div class=SHOT_FRAME>
                {mini_dashboard()}
                {callout("6.5%", "14%", "01", "Cross-org chip flags platform mode")}
                {callout("48%", "42%", "02", "Bandwidth ring + period-reset countdown")}
                {callout("76%", "73%", "03", "ACL hits surface in-line, never in JSON")}
            </div>
        </section>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{shot}{{margin-inline:auto;max-width:1180px;padding-inline:1rem;",
            "padding-bottom:2rem;padding-top:4rem}}",
            "@media (width >= 40rem){{.{shot}{{padding-inline:2rem}}}}",
            ".{head}{{margin-bottom:1.75rem;display:flex;max-width:640px;",
            "flex-direction:column;gap:.5rem}}",
            ".{eyebrow}{{font-size:11px;font-weight:600;text-transform:uppercase;",
            "letter-spacing:.08em;color:var(--color-accent)}}",
            ".{h2}{{font-size:28px;font-weight:600;letter-spacing:-0.02em}}",
            ".{p}{{font-size:14.5px;color:var(--color-text-muted)}}",
            ".{frame}{{position:relative;overflow:hidden;border-radius:var(--radius-lg);",
            "border-width:1px;border-color:var(--color-border);",
            "background-color:var(--color-surface)}}",
            ".{dash}{{display:flex;height:420px;font-size:11px}}",
            ".{side}{{display:flex;width:140px;flex-direction:column;gap:.25rem;",
            "border-right-width:1px;border-color:var(--color-border);",
            "background-color:var(--color-surface-2);padding:.75rem}}",
            ".{side_gap}{{height:.5rem}}",
            ".{navrow}{{display:flex;align-items:center;gap:.375rem;",
            "border-radius:.25rem;padding:.25rem;font-size:11px;",
            "color:var(--color-text-muted)}}",
            ".{navrow_on}{{background-color:var(--color-surface);color:var(--color-text)}}",
            ".{navicon}{{width:.75rem;height:.75rem;color:var(--color-text-dim)}}",
            ".{navicon_on}{{color:var(--color-accent)}}",
            ".{main}{{display:flex;flex:1;flex-direction:column;gap:.75rem;padding:1rem}}",
            ".{top}{{display:flex;align-items:center;justify-content:space-between}}",
            ".{title_col}{{display:flex;flex-direction:column}}",
            ".{title}{{font-size:15px;font-weight:600}}",
            ".{sub}{{font-size:10.5px;color:var(--color-text-muted)}}",
            ".{org_badge}{{height:18px}}",
            ".{kpis}{{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:.5rem}}",
            ".{kpi}{{border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:.625rem}}",
            ".{kpi_label}{{font-size:9.5px;color:var(--color-text-muted)}}",
            ".{kpi_value}{{font-size:17px;font-weight:600;letter-spacing:-0.02em;",
            "font-variant-numeric:tabular-nums}}",
            ".{chart}{{flex:1;border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:.75rem}}",
            ".{chart_head}{{margin-bottom:.375rem;display:flex;align-items:center;",
            "justify-content:space-between}}",
            ".{chart_title}{{font-size:11.5px;font-weight:500}}",
            ".{chart_sub}{{font-size:10px;color:var(--color-text-muted)}}",
            ".{hits}{{border-radius:var(--radius-md);border-width:1px;",
            "border-color:var(--color-border);background-color:var(--color-surface);",
            "padding:.75rem}}",
            ".{hits_title}{{margin-bottom:.5rem;font-size:11.5px;font-weight:500}}",
            ".{hits_col}{{display:flex;flex-direction:column;gap:.25rem}}",
            ".{hit}{{display:flex;align-items:center;justify-content:space-between;",
            "font-size:10.5px}}",
            ".{hit_left}{{display:flex;align-items:center;gap:.5rem}}",
            // tailwind-merge: the reference's text-[9.5px] override strips the
            // Badge base's leading-none (font-size conflicts with leading in
            // cn()), so the badge inherits the page's 1.55.
            ".{hit_badge}{{height:14px;padding-inline:.375rem;font-size:9.5px;",
            "line-height:1.55}}",
            ".{hit_who}{{font-family:var(--font-mono);font-feature-settings:\"ss01\";",
            "color:var(--color-text-muted)}}",
            ".{hit_arrow}{{color:var(--color-text-dim)}}",
            ".{hit_where}{{font-family:var(--font-mono);font-feature-settings:\"ss01\"}}",
            ".{hit_when}{{color:var(--color-text-muted)}}",
            ".{callout}{{pointer-events:none;position:absolute;display:flex;",
            "align-items:center;gap:.5rem}}",
            ".{callout_dot}{{display:flex;width:22px;height:22px;align-items:center;",
            "justify-content:center;border-radius:calc(infinity * 1px);",
            "font-size:10.5px;font-weight:600;color:#fff}}",
            ".{callout_text}{{white-space:nowrap;border-radius:var(--radius-md);",
            "border-width:1px;border-color:var(--color-border);",
            "background-color:var(--color-bg);padding-inline:.5rem;",
            "padding-block:.25rem;font-size:11.5px}}",
            // Mobile phone treatment (M3): hide chrome that only works at desktop
            // mock width; restack KPIs; let the dash grow with content.
            "@media (width < 48rem){{",
            ".{side}{{display:none}}",
            ".{callout}{{display:none}}",
            ".{kpis}{{grid-template-columns:repeat(2,minmax(0,1fr))}}",
            ".{dash}{{height:auto}}",
            "}}",
            "@media (width < 40rem){{",
            ".{kpis}{{grid-template-columns:minmax(0,1fr)}}",
            "}}",
        ),
        shot = SHOT,
        head = SHOT_HEAD,
        eyebrow = SHOT_EYEBROW,
        h2 = SHOT_H2,
        p = SHOT_P,
        frame = SHOT_FRAME,
        dash = SHOT_DASH,
        side = SHOT_SIDE,
        side_gap = SHOT_SIDE_GAP,
        navrow = SHOT_NAVROW,
        navrow_on = SHOT_NAVROW_ON,
        navicon = SHOT_NAVICON,
        navicon_on = SHOT_NAVICON_ON,
        main = SHOT_MAIN,
        top = SHOT_TOP,
        title_col = SHOT_TITLE_COL,
        title = SHOT_TITLE,
        sub = SHOT_SUB,
        org_badge = SHOT_ORG_BADGE,
        kpis = SHOT_KPIS,
        kpi = SHOT_KPI,
        kpi_label = SHOT_KPI_LABEL,
        kpi_value = SHOT_KPI_VALUE,
        chart = SHOT_CHART,
        chart_head = SHOT_CHART_HEAD,
        chart_title = SHOT_CHART_TITLE,
        chart_sub = SHOT_CHART_SUB,
        hits = SHOT_HITS,
        hits_title = SHOT_HITS_TITLE,
        hits_col = SHOT_HITS_COL,
        hit = SHOT_HIT,
        hit_left = SHOT_HIT_LEFT,
        hit_badge = SHOT_HIT_BADGE,
        hit_who = SHOT_HIT_WHO,
        hit_arrow = SHOT_HIT_ARROW,
        hit_where = SHOT_HIT_WHERE,
        hit_when = SHOT_HIT_WHEN,
        callout = SHOT_CALLOUT,
        callout_dot = SHOT_CALLOUT_DOT,
        callout_text = SHOT_CALLOUT_TEXT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            SHOT, SHOT_HEAD, SHOT_EYEBROW, SHOT_H2, SHOT_P, SHOT_FRAME, SHOT_DASH, SHOT_SIDE,
            SHOT_SIDE_GAP, SHOT_NAVROW, SHOT_NAVROW_ON, SHOT_NAVICON, SHOT_NAVICON_ON, SHOT_MAIN,
            SHOT_TOP, SHOT_TITLE_COL, SHOT_TITLE, SHOT_SUB, SHOT_ORG_BADGE, SHOT_KPIS, SHOT_KPI,
            SHOT_KPI_LABEL, SHOT_KPI_VALUE, SHOT_CHART, SHOT_CHART_HEAD, SHOT_CHART_TITLE,
            SHOT_CHART_SUB, SHOT_HITS, SHOT_HITS_TITLE, SHOT_HITS_COL, SHOT_HIT, SHOT_HIT_LEFT,
            SHOT_HIT_BADGE, SHOT_HIT_WHO, SHOT_HIT_ARROW, SHOT_HIT_WHERE, SHOT_HIT_WHEN,
            SHOT_CALLOUT, SHOT_CALLOUT_DOT, SHOT_CALLOUT_TEXT,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn points_are_the_reference_sequence() {
        assert_eq!(POINTS.len(), 30);
        for (i, p) in POINTS.iter().enumerate() {
            let expected = (20.0 + (i as f64 / 3.0).sin() * 8.0 + i as f64 / 2.0).round().max(0.0);
            assert_eq!(*p, expected, "point {i}");
        }
    }
}
