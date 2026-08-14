//! Banner — port of `components/ui/banner.tsx`: inline tinted callout with
//! icon chip, bold title, optional detail line, optional trailing action.
//! Tone drives border/background tint, icon color, and the default icon.
//! The reference applies tone border/background as inline styles (bypassing
//! Tailwind), so success/warning/danger borders are the raw oklch literals.

use crate::icons::{
    self, Icon, RI_ALERT_LINE, RI_CHECKBOX_CIRCLE_LINE, RI_ERROR_WARNING_LINE,
    RI_INFORMATION_LINE,
};
use leptos::prelude::*;

pub const BANNER: &str = "asy-banner";
pub const BANNER_INFO: &str = "asy-banner--info";
pub const BANNER_SUCCESS: &str = "asy-banner--success";
pub const BANNER_WARNING: &str = "asy-banner--warning";
pub const BANNER_DANGER: &str = "asy-banner--danger";
pub const BANNER_CHIP: &str = "asy-banner__chip";
pub const BANNER_GLYPH: &str = "asy-banner__glyph";
pub const BANNER_BODY: &str = "asy-banner__body";
pub const BANNER_TITLE: &str = "asy-banner__title";
pub const BANNER_DETAIL: &str = "asy-banner__detail";
pub const BANNER_ACTION: &str = "asy-banner__action";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BannerTone {
    #[default]
    Info,
    Success,
    Warning,
    Danger,
}

impl BannerTone {
    fn class(self) -> &'static str {
        match self {
            BannerTone::Info => BANNER_INFO,
            BannerTone::Success => BANNER_SUCCESS,
            BannerTone::Warning => BANNER_WARNING,
            BannerTone::Danger => BANNER_DANGER,
        }
    }

    fn default_icon(self) -> &'static str {
        match self {
            BannerTone::Info => RI_INFORMATION_LINE,
            BannerTone::Success => RI_CHECKBOX_CIRCLE_LINE,
            BannerTone::Warning => RI_ALERT_LINE,
            BannerTone::Danger => RI_ERROR_WARNING_LINE,
        }
    }
}

#[component]
pub fn Banner(
    #[prop(optional)] tone: BannerTone,
    /// Override the default per-tone icon (an `icons::*` path constant).
    #[prop(optional)] icon: Option<&'static str>,
    /// Bold lead line.
    #[prop(into)] title: ViewFnOnce,
    /// Optional trailing slot (a button / link), right-aligned.
    #[prop(optional, into)] action: Option<ViewFnOnce>,
    #[prop(optional, into)] class: Option<String>,
    /// Optional secondary detail line.
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let mut cls = format!("{BANNER} {}", tone.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let d = icon.unwrap_or_else(|| tone.default_icon());
    let _ = icons::RI_INFORMATION_LINE;
    view! {
        <div role="status" class=cls>
            <span class=BANNER_CHIP>
                <Icon d=d class=BANNER_GLYPH />
            </span>
            <div class=BANNER_BODY>
                <p class=BANNER_TITLE>{title.run()}</p>
                {children.map(|detail| view! { <p class=BANNER_DETAIL>{detail()}</p> })}
            </div>
            {action.map(|a| view! { <div class=BANNER_ACTION>{a.run()}</div> })}
        </div>
    }
}

/// Root `flex items-start gap-3 rounded-[var(--radius-md)] border p-3.5`;
/// chip `mt-0.5 inline-flex size-7 shrink-0 items-center justify-center
/// rounded-[var(--radius-sm)] border` with icon `size-4`; body
/// `flex min-w-0 flex-1 flex-col gap-0.5`; title 13px/600, detail 12.5px
/// muted; action `shrink-0 self-center`. Tone border/bg/fg mirror
/// `toneStyle()` exactly — oklch literals where the reference inlines them.
pub fn css() -> String {
    format!(
        ".{BANNER}{{display:flex;align-items:flex-start;gap:.75rem;\
border-radius:var(--radius-md);border-width:1px;border-style:solid;padding:.875rem}}\
@media (width < 40rem){{.{BANNER}{{flex-wrap:wrap}}}}\
.{BANNER_INFO}{{border-color:var(--color-accent-line);background:var(--color-accent-soft)}}\
.{BANNER_INFO} .{BANNER_CHIP}{{border-color:var(--color-accent-line);color:var(--color-accent)}}\
.{BANNER_SUCCESS}{{border-color:oklch(70% 0.15 145 / 0.4);background:var(--color-success-soft)}}\
.{BANNER_SUCCESS} .{BANNER_CHIP}{{border-color:oklch(70% 0.15 145 / 0.4);color:var(--color-success)}}\
.{BANNER_WARNING}{{border-color:oklch(78% 0.13 80 / 0.4);background:var(--color-warning-soft)}}\
.{BANNER_WARNING} .{BANNER_CHIP}{{border-color:oklch(78% 0.13 80 / 0.4);color:var(--color-warning)}}\
.{BANNER_DANGER}{{border-color:oklch(64% 0.18 25 / 0.4);background:var(--color-danger-soft)}}\
.{BANNER_DANGER} .{BANNER_CHIP}{{border-color:oklch(64% 0.18 25 / 0.4);color:var(--color-danger)}}\
.{BANNER_CHIP}{{margin-top:.125rem;display:inline-flex;width:1.75rem;height:1.75rem;\
flex-shrink:0;align-items:center;justify-content:center;\
border-radius:var(--radius-sm);border-width:1px;border-style:solid}}\
.{BANNER_GLYPH}{{width:1rem;height:1rem}}\
.{BANNER_BODY}{{display:flex;min-width:0;flex:1 1 0%;flex-direction:column;gap:.125rem}}\
.{BANNER_TITLE}{{font-size:13px;font-weight:600;color:var(--color-text)}}\
.{BANNER_DETAIL}{{font-size:12.5px;color:var(--color-text-muted)}}\
.{BANNER_ACTION}{{flex-shrink:0;align-self:center}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            BANNER,
            BANNER_INFO,
            BANNER_SUCCESS,
            BANNER_WARNING,
            BANNER_DANGER,
            BANNER_CHIP,
            BANNER_BODY,
            BANNER_TITLE,
            BANNER_DETAIL,
            BANNER_ACTION,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
        assert!(css.contains(&format!(".{BANNER_GLYPH}{{")));
    }

    #[test]
    fn tone_default_icons_match_reference() {
        assert_eq!(BannerTone::Info.default_icon(), RI_INFORMATION_LINE);
        assert_eq!(BannerTone::Success.default_icon(), RI_CHECKBOX_CIRCLE_LINE);
        assert_eq!(BannerTone::Warning.default_icon(), RI_ALERT_LINE);
        assert_eq!(BannerTone::Danger.default_icon(), RI_ERROR_WARNING_LINE);
    }
}
