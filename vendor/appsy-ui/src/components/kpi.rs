//! KPI — port of `components/dashboard/kpi.tsx`: dashboard stat tile on the
//! card surface — muted label row with optional accent tag, large `num`
//! value with optional sub, optional sparkline slot. Carries the
//! `fade-slide-in` mount entrance from the app.css motion layer (its first
//! consumer in the crate), reduced-motion inert like the reference.

use crate::components::card::CARD;
use leptos::prelude::*;

pub const KPI: &str = "asy-kpi";
pub const KPI_HEAD: &str = "asy-kpi__head";
pub const KPI_ACCENT: &str = "asy-kpi__accent";
pub const KPI_ROW: &str = "asy-kpi__row";
pub const KPI_VALUE: &str = "asy-kpi__value";
pub const KPI_SUB: &str = "asy-kpi__sub";
pub const KPI_SPARK: &str = "asy-kpi__spark";
/// Mount entrance shared by dashboard tiles (`.fade-slide-in` upstream).
pub const FADE_SLIDE_IN: &str = "asy-fade-slide-in";

#[component]
pub fn Kpi(
    #[prop(into)] label: String,
    #[prop(into)] value: ViewFnOnce,
    #[prop(optional, into)] sub: Option<ViewFnOnce>,
    /// Small accent label rendered on the right side of the header row.
    #[prop(optional, into)] accent: Option<ViewFnOnce>,
    #[prop(optional, into)] sparkline: Option<ViewFnOnce>,
) -> impl IntoView {
    view! {
        <div class=format!("{CARD} {FADE_SLIDE_IN} {KPI}")>
            <div class=KPI_HEAD>
                <span>{label}</span>
                {accent.map(|a| view! { <span class=KPI_ACCENT>{a.run()}</span> })}
            </div>
            <div class=KPI_ROW>
                <span class=format!("num {KPI_VALUE}")>{value.run()}</span>
                {sub.map(|s| view! { <span class=KPI_SUB>{s.run()}</span> })}
            </div>
            {sparkline.map(|s| view! { <div class=KPI_SPARK>{s.run()}</div> })}
        </div>
    }
}

/// Tile `fade-slide-in flex min-h-[96px] flex-col gap-1.5 p-4`; head row
/// `flex items-center justify-between text-[12px] text-muted` with an
/// `text-[11px] text-accent` tag; value row `flex items-baseline gap-1.5`
/// with `num text-[28px] font-semibold leading-none tracking-[-0.02em]`
/// value and `text-[12px] text-muted` sub; spark slot `mt-1.5`.
pub fn css() -> String {
    format!(
        ".{KPI}{{display:flex;min-height:96px;flex-direction:column;gap:.375rem;padding:1rem}}\
.{KPI_HEAD}{{display:flex;align-items:center;justify-content:space-between;\
font-size:12px;color:var(--color-text-muted)}}\
.{KPI_ACCENT}{{font-size:11px;color:var(--color-accent)}}\
.{KPI_ROW}{{display:flex;align-items:baseline;gap:.375rem}}\
.{KPI_VALUE}{{font-size:28px;font-weight:600;line-height:1;letter-spacing:-0.02em}}\
.{KPI_SUB}{{font-size:12px;color:var(--color-text-muted)}}\
.{KPI_SPARK}{{margin-top:.375rem}}\
.{FADE_SLIDE_IN}{{animation:asy-fade-slide-in 0.22s ease-out both}}\
@keyframes asy-fade-slide-in{{from{{opacity:0;transform:translateY(6px)}}\
to{{opacity:1;transform:translateY(0)}}}}\
@media (prefers-reduced-motion: reduce){{.{FADE_SLIDE_IN}{{animation:none}}}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [KPI, KPI_HEAD, KPI_ACCENT, KPI_ROW, KPI_VALUE, KPI_SUB, KPI_SPARK, FADE_SLIDE_IN]
        {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn fade_slide_in_is_reduced_motion_inert() {
        assert!(css().contains(&format!(
            "@media (prefers-reduced-motion: reduce){{.{FADE_SLIDE_IN}{{animation:none}}}}"
        )));
    }
}
