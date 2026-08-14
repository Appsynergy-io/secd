//! Design tokens — the sole definition of every token value. Values are
//! verbatim from the reference `@theme` block (`frontend/app/app.css`);
//! zero-drift forbids touching them here. `css()` emits the custom
//! properties: dark values on `:root` (dark is the default theme),
//! light overrides scoped under `.theme-light`.

/// A color token: dark value always, light override only where the
/// reference's `.theme-light` block redefines it.
pub struct Color {
    pub name: &'static str,
    pub dark: &'static str,
    pub light: Option<&'static str>,
}

pub const COLORS: &[Color] = &[
    Color { name: "--color-bg", dark: "oklch(15% 0 0)", light: Some("oklch(99% 0 0)") },
    Color { name: "--color-surface", dark: "oklch(18% 0 0)", light: Some("oklch(98% 0 0)") },
    Color { name: "--color-surface-2", dark: "oklch(21% 0 0)", light: Some("oklch(96% 0 0)") },
    Color { name: "--color-border", dark: "oklch(28% 0 0)", light: Some("oklch(88% 0 0)") },
    Color { name: "--color-border-soft", dark: "oklch(24% 0 0)", light: Some("oklch(92% 0 0)") },
    Color { name: "--color-text", dark: "oklch(96% 0 0)", light: Some("oklch(15% 0 0)") },
    Color { name: "--color-text-muted", dark: "oklch(70% 0 0)", light: Some("oklch(45% 0 0)") },
    Color { name: "--color-text-dim", dark: "oklch(55% 0 0)", light: Some("oklch(60% 0 0)") },
    Color { name: "--color-accent", dark: "oklch(62% 0.12 220)", light: None },
    Color {
        name: "--color-accent-soft",
        dark: "oklch(62% 0.12 220 / 0.18)",
        light: Some("oklch(62% 0.12 220 / 0.12)"),
    },
    Color { name: "--color-accent-line", dark: "oklch(62% 0.12 220 / 0.4)", light: None },
    Color { name: "--color-success", dark: "oklch(70% 0.15 145)", light: None },
    Color { name: "--color-success-soft", dark: "oklch(70% 0.15 145 / 0.18)", light: None },
    Color { name: "--color-warning", dark: "oklch(78% 0.13 80)", light: None },
    Color { name: "--color-warning-soft", dark: "oklch(78% 0.13 80 / 0.18)", light: None },
    Color { name: "--color-danger", dark: "oklch(64% 0.18 25)", light: None },
    Color { name: "--color-danger-soft", dark: "oklch(64% 0.18 25 / 0.18)", light: None },
];

pub struct Radius {
    pub name: &'static str,
    pub value: &'static str,
}

pub const RADII: &[Radius] = &[
    Radius { name: "--radius-sm", value: "0.375rem" },
    Radius { name: "--radius-md", value: "0.5rem" },
    Radius { name: "--radius-lg", value: "0.75rem" },
];

/// Font stacks. Family names here are the names `fonts.rs` registers — the
/// approved fix for the upstream bug where tokens requested `"Geist"` but the
/// faces were registered as `"Geist Variable"`. Fallback chains verbatim.
pub const FONT_SANS: &str =
    r#""Geist", -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif"#;
pub const FONT_MONO: &str = r#""Geist Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace"#;

/// Family names that must have a registered `@font-face` (test-enforced
/// against `fonts.rs` in both directions).
pub const FONT_FAMILIES: &[&str] = &["Geist", "Geist Mono"];

/// The token layer: `:root` dark defaults + `.theme-light` overrides.
pub fn css() -> String {
    let mut out = String::from(":root{");
    for c in COLORS {
        out.push_str(&format!("{}:{};", c.name, c.dark));
    }
    for r in RADII {
        out.push_str(&format!("{}:{};", r.name, r.value));
    }
    out.push_str(&format!("--font-sans:{FONT_SANS};--font-mono:{FONT_MONO};}}"));
    out.push_str(".theme-light{");
    for c in COLORS {
        if let Some(light) = c.light {
            out.push_str(&format!("{}:{};", c.name, light));
        }
    }
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_emits_every_token_once() {
        let css = css();
        for c in COLORS {
            assert_eq!(
                css.matches(&format!("{}:", c.name)).count(),
                1 + usize::from(c.light.is_some()),
                "{} emitted wrong number of times",
                c.name
            );
        }
        for r in RADII {
            assert!(css.contains(&format!("{}:{}", r.name, r.value)));
        }
        assert!(css.contains("--font-sans:"));
        assert!(css.contains("--font-mono:"));
    }

    #[test]
    fn dark_is_root_and_light_is_scoped() {
        let css = css();
        let root = css.find(":root{").expect("root scope");
        let light = css.find(".theme-light{").expect("light scope");
        assert!(root < light, "dark default must come first");
        assert!(css[light..].contains("--color-bg:oklch(99% 0 0)"));
    }

    #[test]
    fn token_font_families_match_stacks() {
        assert!(FONT_SANS.contains("\"Geist\""));
        assert!(FONT_MONO.contains("\"Geist Mono\""));
        // The buggy upstream names must never appear.
        assert!(!FONT_SANS.contains("Variable"));
        assert!(!FONT_MONO.contains("Variable"));
    }
}
