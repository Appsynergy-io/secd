use ratatui::style::Color;

pub const ACCENT: &str = "#A78BFA";
pub const HIGHLIGHT: &str = "#F48FCD";
pub const OK: &str = "#6ED6BA";
pub const WARN: &str = "#F0BE6E";
pub const DANGER: &str = "#F87185";
pub const DIM: &str = "#7A7A94";
pub const ON_ACCENT: &str = "#181825";

/// Parse a locked `#RRGGBB` token into a terminal color.
pub fn color(hex: &str) -> Color {
    let h = hex.strip_prefix('#').unwrap_or(hex);
    if h.len() != 6 {
        return Color::Reset;
    }
    let ok = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    match (ok(0), ok(2), ok(4)) {
        (Some(r), Some(g), Some(b)) => Color::Rgb(r, g, b),
        _ => Color::Reset,
    }
}
