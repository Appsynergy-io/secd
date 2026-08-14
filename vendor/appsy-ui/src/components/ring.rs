//! Ring — port of `components/charts/ring.tsx`: single-stat progress ring
//! (the Overview bandwidth card). SVG-only. Starts empty and transitions
//! to the target offset on mount (`.ring-anim`, app.css motion layer:
//! stroke-dashoffset transition, dropped under reduced motion — the value
//! still lands, so the settled DOM matches). Arc numbers print with f64
//! shortest-roundtrip formatting, byte-identical to JS `toString`.

use leptos::prelude::*;

pub const RING: &str = "asy-ring";
pub const RING_ANIM: &str = "asy-ring-anim";
pub const RING_COL: &str = "asy-ring__col";
pub const RING_LABEL: &str = "asy-ring__label";
pub const RING_VALUE: &str = "asy-ring__value";
pub const RING_SUB: &str = "asy-ring__sub";

#[component]
pub fn Ring(
    /// Fraction in [0, 1]. Out-of-range values clamp.
    pct: f64,
    #[prop(into)] label: String,
    #[prop(into)] value: String,
    #[prop(optional, into)] sub: Option<String>,
    /// Pixel diameter of the ring.
    #[prop(optional, default = 120.0)] size: f64,
    /// Sweep the arc in from empty on mount; inert under reduced motion.
    #[prop(optional, default = true)] animate: bool,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let clamped = pct.clamp(0.0, 1.0);
    let stroke = 8.0;
    let radius = (size - stroke) / 2.0;
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let offset = circumference * (1.0 - clamped);
    let dash_offset = RwSignal::new(if animate { circumference } else { offset });
    Effect::new(move |_| dash_offset.set(offset));
    let mut cls = RING.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <div class=cls>
            <svg
                width=size
                height=size
                viewBox=format!("0 0 {size} {size}")
                role="img"
                aria-label=format!("{label}: {value}")
            >
                <circle
                    cx=size / 2.0
                    cy=size / 2.0
                    r=radius
                    fill="none"
                    stroke="var(--color-surface-2)"
                    stroke-width=stroke
                />
                <circle
                    cx=size / 2.0
                    cy=size / 2.0
                    r=radius
                    fill="none"
                    stroke="var(--color-accent)"
                    stroke-width=stroke
                    stroke-dasharray=circumference
                    stroke-dashoffset=move || dash_offset.get()
                    stroke-linecap="round"
                    transform=format!("rotate(-90 {} {})", size / 2.0, size / 2.0)
                    class=animate.then_some(RING_ANIM)
                />
            </svg>
            <div class=RING_COL>
                <span class=RING_LABEL>{label.clone()}</span>
                <span class=RING_VALUE>{value.clone()}</span>
                {sub.map(|s| view! { <span class=RING_SUB>{s}</span> })}
            </div>
        </div>
    }
}

/// Wrapper `flex items-center gap-4`; `.ring-anim` transition verbatim with
/// its reduced-motion drop; column `flex flex-col gap-0.5` with `text-xs`
/// muted label, 22px/600 tracking-tight value, `text-xs` dim sub.
pub fn css() -> String {
    format!(
        ".{RING}{{display:flex;flex-wrap:wrap;align-items:center;gap:1rem;max-width:100%}}\
.{RING_ANIM}{{transition:stroke-dashoffset 0.7s ease-out}}\
@media (prefers-reduced-motion: reduce){{.{RING_ANIM}{{transition:none}}}}\
.{RING_COL}{{display:flex;flex-direction:column;gap:.125rem}}\
.{RING_LABEL}{{font-size:.75rem;line-height:calc(1/.75);color:var(--color-text-muted)}}\
.{RING_VALUE}{{font-size:22px;font-weight:600;letter-spacing:-.025em}}\
.{RING_SUB}{{font-size:.75rem;line-height:calc(1/.75);color:var(--color-text-dim)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [RING, RING_ANIM, RING_COL, RING_LABEL, RING_VALUE, RING_SUB] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn arc_numbers_match_js_shortest_roundtrip() {
        let radius: f64 = (120.0 - 8.0) / 2.0;
        let c = 2.0 * std::f64::consts::PI * radius;
        assert_eq!(c.to_string(), "351.85837720205683");
        assert_eq!((c * (1.0 - 0.374)).to_string(), "220.26334412848757");
    }
}
