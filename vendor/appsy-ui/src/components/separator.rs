//! Separator — port of `components/ui/separator.tsx` (Radix Separator).
//! Decorative (the reference's default and only use) renders `role="none"`;
//! non-decorative renders `role="separator"` with `aria-orientation` on
//! vertical, exactly as Radix does.

use leptos::prelude::*;

pub const SEPARATOR: &str = "asy-separator";
pub const SEPARATOR_H: &str = "asy-separator--horizontal";
pub const SEPARATOR_V: &str = "asy-separator--vertical";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SeparatorOrientation {
    #[default]
    Horizontal,
    Vertical,
}

#[component]
pub fn Separator(
    #[prop(optional)] orientation: SeparatorOrientation,
    /// Purely visual separator (`role="none"`); the reference defaults true.
    #[prop(optional, default = true)] decorative: bool,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let modifier = match orientation {
        SeparatorOrientation::Horizontal => SEPARATOR_H,
        SeparatorOrientation::Vertical => SEPARATOR_V,
    };
    let mut cls = format!("{SEPARATOR} {modifier}");
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let role = if decorative { "none" } else { "separator" };
    let aria_orientation = (!decorative
        && orientation == SeparatorOrientation::Vertical)
        .then_some("vertical");
    view! {
        <div class=cls role=role aria-orientation=aria_orientation></div>
    }
}

/// `shrink-0 bg-[var(--color-border)]` + per-orientation `h-px w-full` /
/// `h-full w-px`.
pub fn css() -> String {
    format!(
        ".{SEPARATOR}{{flex-shrink:0;background-color:var(--color-border)}}\
.{SEPARATOR_H}{{height:1px;width:100%}}\
.{SEPARATOR_V}{{height:100%;width:1px}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [SEPARATOR, SEPARATOR_H, SEPARATOR_V] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
