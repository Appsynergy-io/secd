//! BarList — port of `components/charts/bar-list.tsx`: horizontal stacked
//! bar list (the Overview "Path tiers · this period" panel). Labels left,
//! scaled fill, value right. Fill widths print with f64 shortest-roundtrip
//! formatting, byte-identical to JS template interpolation. The `bar-grow`
//! scale-in (app.css motion layer) ports here with its reduced-motion gate.

use leptos::prelude::*;

pub const BAR_LIST: &str = "asy-bar-list";
pub const BAR_LIST_ROW: &str = "asy-bar-list__row";
pub const BAR_LIST_LABEL: &str = "asy-bar-list__label";
pub const BAR_LIST_TRACK: &str = "asy-bar-list__track";
pub const BAR_LIST_FILL: &str = "asy-bar-list__fill";
pub const BAR_GROW: &str = "asy-bar-grow";

#[derive(Clone, Debug, PartialEq)]
pub struct BarListItem {
    pub label: String,
    pub value: f64,
    /// Pre-formatted display string (e.g. "92 GB", "—"); defaults to the
    /// value's JS-identical string form.
    pub display: Option<String>,
    /// Optional bar colour override; defaults to `var(--color-accent)`.
    pub color: Option<String>,
}

impl BarListItem {
    pub fn new(label: impl Into<String>, value: f64, display: impl Into<String>) -> Self {
        Self { label: label.into(), value, display: Some(display.into()), color: None }
    }
}

#[component]
pub fn BarList(
    items: Vec<BarListItem>,
    /// Reference value the bars are scaled against.
    max: f64,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let mut cls = BAR_LIST.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <div class=cls>
            {items
                .into_iter()
                .map(|item| {
                    let ratio = if max > 0.0 { (item.value / max).min(1.0) } else { 0.0 };
                    let display = item.display.unwrap_or_else(|| item.value.to_string());
                    let color = item
                        .color
                        .unwrap_or_else(|| "var(--color-accent)".to_owned());
                    view! {
                        <div class=BAR_LIST_ROW>
                            <span class=BAR_LIST_LABEL>{item.label}</span>
                            <div class=BAR_LIST_TRACK>
                                <span
                                    class=format!("{BAR_GROW} {BAR_LIST_FILL}")
                                    style=format!("width:{}%;background:{color}", ratio * 100.0)
                                ></span>
                            </div>
                            <span class=BAR_LIST_VALUE>{display}</span>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

pub const BAR_LIST_VALUE: &str = "asy-bar-list__value";

/// Root `flex flex-col gap-2`; row `flex items-center gap-3 text-xs`;
/// `w-20` muted label; `h-2 grow rounded-full` surface-2 track clipping an
/// absolute `rounded-full opacity-80` fill; `w-12` right-aligned
/// tabular-nums value. `bar-grow` = app.css motion layer verbatim
/// (scaleX ease-out, reduced-motion inert).
pub fn css() -> String {
    format!(
        ".{BAR_LIST}{{display:flex;flex-direction:column;gap:.5rem}}\
.{BAR_LIST_ROW}{{display:flex;align-items:center;gap:.75rem;\
font-size:.75rem;line-height:calc(1/.75)}}\
.{BAR_LIST_LABEL}{{width:5rem;flex-shrink:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap;color:var(--color-text-muted)}}\
.{BAR_LIST_TRACK}{{position:relative;height:.5rem;flex-grow:1;\
overflow:hidden;border-radius:calc(infinity * 1px);\
background-color:var(--color-surface-2)}}\
.{BAR_LIST_FILL}{{position:absolute;top:0;left:0;height:100%;\
border-radius:calc(infinity * 1px);opacity:.8}}\
.{BAR_GROW}{{transform-origin:left;animation:asy-bar-grow 0.6s ease-out both}}\
@keyframes asy-bar-grow{{from{{transform:scaleX(0)}}}}\
@media (prefers-reduced-motion: reduce){{.{BAR_GROW}{{animation:none}}}}\
.{BAR_LIST_VALUE}{{width:3rem;flex-shrink:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap;text-align:right;\
color:var(--color-text);font-variant-numeric:tabular-nums}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            BAR_LIST,
            BAR_LIST_ROW,
            BAR_LIST_LABEL,
            BAR_LIST_TRACK,
            BAR_LIST_FILL,
            BAR_GROW,
            BAR_LIST_VALUE,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn width_percent_matches_js_shortest_roundtrip() {
        // `${ratio * 100}%` on the story's values — integers print bare.
        for (value, expect) in [(92.0, "92"), (64.0, "64"), (21.0, "21"), (10.0, "10"), (0.0, "0")]
        {
            let ratio: f64 = (value / 100.0f64).min(1.0);
            assert_eq!((ratio * 100.0).to_string(), expect);
        }
        // A non-terminating case keeps the JS float tail.
        let ratio: f64 = (1.0f64 / 3.0).min(1.0);
        assert_eq!((ratio * 100.0).to_string(), "33.33333333333333");
    }
}
