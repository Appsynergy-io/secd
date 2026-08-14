//! Mark — port of `components/brand/mark.tsx`: the hub-and-spoke brand
//! mark. Pure SVG, accent-coloured by default, `currentColor`-friendly for
//! monochrome surfaces.

use leptos::prelude::*;

pub const MARK: &str = "asy-mark";

#[component]
pub fn Mark(
    /// Pixel size of the mark.
    #[prop(optional, default = 24.0)] size: f64,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let mut cls = MARK.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <svg width=size height=size viewBox="0 0 24 24" role="img" aria-label="appsynergy mark" class=cls>
            <circle cx="12" cy="12" r="3" fill="currentColor" />
            <g stroke="currentColor" stroke-width="1.4" stroke-linecap="round">
                <line x1="12" y1="3" x2="12" y2="7" />
                <line x1="12" y1="17" x2="12" y2="21" />
                <line x1="3" y1="12" x2="7" y2="12" />
                <line x1="17" y1="12" x2="21" y2="12" />
                <line x1="6.2" y1="6.2" x2="8.6" y2="8.6" />
                <line x1="15.4" y1="15.4" x2="17.8" y2="17.8" />
                <line x1="17.8" y1="6.2" x2="15.4" y2="8.6" />
                <line x1="8.6" y1="15.4" x2="6.2" y2="17.8" />
            </g>
            <g fill="currentColor" opacity="0.55">
                <circle cx="12" cy="3" r="1.1" />
                <circle cx="12" cy="21" r="1.1" />
                <circle cx="3" cy="12" r="1.1" />
                <circle cx="21" cy="12" r="1.1" />
                <circle cx="6" cy="6" r="0.9" />
                <circle cx="18" cy="18" r="0.9" />
                <circle cx="18" cy="6" r="0.9" />
                <circle cx="6" cy="18" r="0.9" />
            </g>
        </svg>
    }
}

/// `text-[var(--color-accent)]` on the svg.
pub fn css() -> String {
    format!(".{MARK}{{color:var(--color-accent)}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        assert!(css().contains(&format!(".{MARK}{{")));
    }
}
