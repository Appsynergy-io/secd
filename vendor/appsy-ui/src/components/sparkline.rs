//! Sparkline — port of `components/charts/sparkline.tsx`: tiny inline SVG
//! trend line for KPI tiles, no chart-lib runtime. Points are computed
//! exactly as the reference does (`toFixed(2)` ↔ `{:.2}` on the same f64
//! math), so the emitted `points` attribute is byte-identical. The
//! `chart-draw` wipe-in (app.css motion layer) ports here with its
//! reduced-motion gate — its first consumer in the crate.

use leptos::either::Either;
use leptos::prelude::*;

pub const SPARKLINE: &str = "asy-sparkline";
pub const CHART_DRAW: &str = "asy-chart-draw";

#[component]
pub fn Sparkline(
    data: Vec<f64>,
    #[prop(optional, default = 64.0)] width: f64,
    #[prop(optional, default = 22.0)] height: f64,
    /// Stroke color via CSS var; defaults to `--color-accent`.
    #[prop(optional, into, default = "var(--color-accent)".into())] stroke: String,
    /// Wipe the line in from the left on mount. Default true; inert under
    /// reduced motion.
    #[prop(optional, default = true)] animate: bool,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let mut cls = SPARKLINE.to_owned();
    if let Some(extra) = &class {
        cls.push(' ');
        cls.push_str(extra);
    }
    if data.is_empty() {
        return Either::Left(view! { <svg width=width height=height class=class></svg> });
    }
    let points = points(&data, width, height);
    let g_class = animate.then_some(CHART_DRAW);
    Either::Right(view! {
        <svg
            width=width
            height=height
            viewBox=format!("0 0 {width} {height}")
            role="img"
            aria-label="trend sparkline"
            class=cls
        >
            <g class=g_class>
                <polyline
                    fill="none"
                    stroke=stroke
                    stroke-width="1.4"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    points=points
                />
            </g>
        </svg>
    })
}

/// The reference's point math verbatim: min/max normalize, `span || 1`,
/// `width / max(len - 1, 1)` step, two-decimal fixed formatting.
fn points(data: &[f64], width: f64, height: f64) -> String {
    let min = data.iter().copied().fold(f64::INFINITY, f64::min);
    let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = if max - min == 0.0 { 1.0 } else { max - min };
    let step = width / (data.len() as f64 - 1.0).max(1.0);
    data.iter()
        .enumerate()
        .map(|(i, value)| {
            let x = i as f64 * step;
            let y = height - ((value - min) / span) * height;
            format!("{x:.2},{y:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Svg `overflow-visible`; `chart-draw` = app.css motion layer verbatim
/// (clip-path wipe, reduced-motion inert: no animation, no clip).
pub fn css() -> String {
    format!(
        ".{SPARKLINE}{{overflow:visible}}\
.{CHART_DRAW}{{animation:asy-chart-draw 0.6s ease-out both;clip-path:inset(0 100% 0 0)}}\
@keyframes asy-chart-draw{{to{{clip-path:inset(0 0 0 0)}}}}\
@media (prefers-reduced-motion: reduce){{.{CHART_DRAW}{{animation:none;clip-path:none}}}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [SPARKLINE, CHART_DRAW] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn points_match_reference_fixed_two() {
        // The trend story's data through the reference formula.
        let p = points(
            &[3.0, 4.0, 4.0, 6.0, 7.0, 9.0, 11.0, 12.0, 14.0, 15.0, 17.0, 18.0],
            64.0,
            22.0,
        );
        assert!(p.starts_with("0.00,22.00 5.82,20.53"), "{p}");
        assert!(p.ends_with("64.00,0.00"), "{p}");
    }

    #[test]
    fn flat_series_uses_span_one_and_single_point_avoids_div_by_zero() {
        let p = points(&[12.0, 12.0], 64.0, 22.0);
        assert_eq!(p, "0.00,22.00 64.00,22.00");
        let single = points(&[5.0], 64.0, 22.0);
        assert_eq!(single, "0.00,22.00");
    }
}
