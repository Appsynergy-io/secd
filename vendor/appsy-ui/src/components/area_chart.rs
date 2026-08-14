//! AreaChart — port of `components/charts/area-chart.tsx`: filled-area trend
//! chart (the dashboard bandwidth panel), pure SVG like the sister Sparkline
//! and Ring. Path coordinates print as `toFixed(2)` ↔ `{:.2}` on the same
//! f64 math; bare numbers (viewBox, fill-path closers) use shortest-roundtrip
//! formatting — both byte-identical to the JS template output. The gradient
//! id derives from the paint props (FNV-1a), not a runtime counter — the
//! reference's `useId` stamp is excluded-id vocabulary, and identical props
//! colliding on one page reference identical gradient defs.

use crate::components::sparkline::CHART_DRAW;
use leptos::either::Either;
use leptos::prelude::*;

pub const AREA_CHART: &str = "asy-area";

/// FNV-1a 64 — the crate's deterministic-id scheme (also the topology
/// position seed, per the bar in CLAUDE.md).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[component]
pub fn AreaChart(
    data: Vec<f64>,
    #[prop(optional, default = 640.0)] width: f64,
    #[prop(optional, default = 200.0)] height: f64,
    /// Stroke color via CSS var; defaults to `--color-accent`.
    #[prop(optional, into, default = "var(--color-accent)".into())] stroke: String,
    /// Fill gradient stop; defaults to a translucent accent.
    #[prop(optional, into, default = "var(--color-accent-soft)".into())] fill: String,
    /// Wipe the area+line in from the left on mount. Default true; inert
    /// under reduced motion.
    #[prop(optional, default = true)] animate: bool,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let mut cls = AREA_CHART.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    if data.is_empty() {
        return Either::Left(view! { <svg width=width height=height class=cls></svg> });
    }
    let grad_id = format!(
        "asy-area-{:016x}",
        fnv1a(format!("{stroke}\u{1f}{fill}\u{1f}{width}\u{1f}{height}").as_bytes())
    );
    let (line_path, fill_path) = paths(&data, width, height);
    let g_class = animate.then_some(CHART_DRAW);
    Either::Right(view! {
        <svg
            width=width
            height=height
            viewBox=format!("0 0 {width} {height}")
            preserveAspectRatio="none"
            role="img"
            aria-label="area chart"
            class=cls
        >
            <defs>
                <linearGradient id=grad_id.clone() x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stop-color=fill.clone() stop-opacity="0.6" />
                    <stop offset="100%" stop-color=fill stop-opacity="0.05" />
                </linearGradient>
            </defs>
            <g class=g_class>
                <path d=fill_path fill=format!("url(#{grad_id})") />
                <path
                    d=line_path
                    fill="none"
                    stroke=stroke
                    stroke-width="1.6"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                />
            </g>
        </svg>
    })
}

/// The reference's path math verbatim: `pad = 2` vertical inset so the round
/// line caps never clip, flat series centered at 0.5, `M/L x.xx y.yy` line
/// path, fill path closing along the bottom edge with bare-number closers.
fn paths(data: &[f64], width: f64, height: f64) -> (String, String) {
    let min = data.iter().copied().fold(f64::INFINITY, f64::min);
    let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let flat = max == min;
    let span = max - min;
    let step = width / (data.len() as f64 - 1.0).max(1.0);
    let pad = 2.0;
    let plot_height = height - pad * 2.0;
    let mut line = String::new();
    let mut last_x = 0.0f64;
    for (i, value) in data.iter().enumerate() {
        let x = i as f64 * step;
        let frac = if flat { 0.5 } else { (value - min) / span };
        let y = height - pad - frac * plot_height;
        if i > 0 {
            line.push(' ');
        }
        line.push_str(if i == 0 { "M" } else { "L" });
        line.push_str(&format!(" {x:.2} {y:.2}"));
        last_x = x;
    }
    let fill = format!("{line} L {last_x:.2} {height} L 0 {height} Z");
    (line, fill)
}

/// `block w-full` on the svg; the `chart-draw` wipe-in is shared with
/// Sparkline (same motion-layer class, one rule).
pub fn css() -> String {
    format!(".{AREA_CHART}{{display:block;width:100%;max-width:100%}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        assert!(css().contains(&format!(".{AREA_CHART}{{")));
    }

    #[test]
    fn paths_match_reference_dom() {
        // The Ladle default story's data through the reference formula —
        // expected strings lifted from the reference's rendered DOM.
        let data: Vec<f64> = [
            20, 23, 26, 28, 30, 30, 30, 29, 28, 26, 23, 21, 20, 19, 19, 20, 21, 24, 27, 30, 33,
            36, 38, 39, 40, 40, 39, 37, 35, 33,
        ]
        .into_iter()
        .map(f64::from)
        .collect();
        let (line, fill) = paths(&data, 640.0, 210.0);
        assert!(line.starts_with("M 0.00 198.19 L 22.07 168.76 L 44.14 139.33"), "{line}");
        assert!(line.ends_with("L 617.93 51.05 L 640.00 70.67"), "{line}");
        assert!(fill.ends_with("L 640.00 70.67 L 640.00 210 L 0 210 Z"), "{fill}");
    }

    #[test]
    fn flat_series_centers_and_single_point_avoids_div_by_zero() {
        let (line, _) = paths(&[7.0, 7.0], 640.0, 210.0);
        assert_eq!(line, "M 0.00 105.00 L 640.00 105.00");
        let (single, fill) = paths(&[5.0], 640.0, 210.0);
        assert_eq!(single, "M 0.00 105.00");
        assert_eq!(fill, "M 0.00 105.00 L 0.00 210 L 0 210 Z");
    }

    #[test]
    fn gradient_id_is_deterministic_over_props() {
        let a = fnv1a(b"var(--color-accent)\x1fvar(--color-accent-soft)\x1f640\x1f210");
        let b = fnv1a(b"var(--color-accent)\x1fvar(--color-accent-soft)\x1f640\x1f210");
        assert_eq!(a, b);
    }
}
