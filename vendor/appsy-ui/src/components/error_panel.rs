//! ErrorPanel — port of `components/ui/error-panel.tsx`: danger-tinted
//! data-load failure surface on the card class — warning icon chip, title
//! ("Couldn't load {label}" / "Something went wrong"), optional detail line,
//! optional Retry button. The reference derives the detail from a thrown
//! `unknown`; extracting a message from an error value is consumer logic, so
//! the port takes the detail string directly via `error`.

use crate::components::button::{Button, ButtonSize};
use crate::components::card::CARD;
use crate::icons::{Icon, RI_ERROR_WARNING_LINE, RI_REFRESH_LINE};
use leptos::prelude::*;

pub const ERROR_PANEL: &str = "asy-error";
pub const ERROR_CHIP: &str = "asy-error__chip";
pub const ERROR_GLYPH: &str = "asy-error__glyph";
pub const ERROR_BODY: &str = "asy-error__body";
pub const ERROR_TITLE: &str = "asy-error__title";
pub const ERROR_DETAIL: &str = "asy-error__detail";
pub const ERROR_RETRY: &str = "asy-error__retry";
pub const ERROR_RETRY_GLYPH: &str = "asy-error__retry-glyph";

#[component]
pub fn ErrorPanel(
    /// What failed to load, e.g. "tunnels" — renders as "Couldn't load tunnels".
    #[prop(optional, into)] label: Option<String>,
    /// Error detail line (the reference's `error.message`).
    #[prop(optional, into)] error: Option<String>,
    /// When provided, renders a Retry button wired to this callback.
    #[prop(optional)] on_retry: Option<Callback<()>>,
) -> impl IntoView {
    let title = match label {
        Some(label) => format!("Couldn't load {label}"),
        None => "Something went wrong".to_owned(),
    };
    view! {
        <div role="alert" class=format!("{CARD} {ERROR_PANEL}")>
            <span class=ERROR_CHIP>
                <Icon d=RI_ERROR_WARNING_LINE class=ERROR_GLYPH />
            </span>
            <div class=ERROR_BODY>
                <p class=ERROR_TITLE>{title}</p>
                {error.map(|detail| view! { <p class=ERROR_DETAIL>{detail}</p> })}
                {on_retry.map(|cb| {
                    view! {
                        <Button size=ButtonSize::Sm class=ERROR_RETRY on:click=move |_| cb.run(())>
                            <Icon d=RI_REFRESH_LINE class=ERROR_RETRY_GLYPH />
                            "Retry"
                        </Button>
                    }
                })}
            </div>
        </div>
    }
}

/// Surface `flex items-start gap-3 border-[oklch(64%_0.18_25_/_0.35)]
/// bg-danger-soft p-5` (Tailwind compiles the arbitrary oklch to
/// `#e5555159` — TT-2, hence the rgba); chip `mt-0.5 inline-flex size-8
/// shrink-0 items-center justify-center rounded-md border` same tint with a
/// `size-4` glyph; body `flex flex-col gap-1`; title 13px/600; detail
/// 12.5px muted; retry `mt-1.5 self-start` with a `size-3.5` glyph.
pub fn css() -> String {
    format!(
        ".{ERROR_PANEL}{{display:flex;align-items:flex-start;gap:.75rem;\
border-color:rgba(229,85,81,.35);background-color:var(--color-danger-soft);\
padding:1.25rem}}\
.{ERROR_CHIP}{{margin-top:.125rem;display:inline-flex;width:2rem;height:2rem;\
flex-shrink:0;align-items:center;justify-content:center;\
border-radius:var(--radius-md);border-width:1px;border-style:solid;\
border-color:rgba(229,85,81,.35);background-color:var(--color-danger-soft);\
color:var(--color-danger)}}\
.{ERROR_GLYPH}{{width:1rem;height:1rem}}\
.{ERROR_BODY}{{display:flex;flex-direction:column;gap:.25rem}}\
.{ERROR_TITLE}{{font-size:13px;font-weight:600;color:var(--color-text)}}\
.{ERROR_DETAIL}{{font-size:12.5px;color:var(--color-text-muted)}}\
.{ERROR_RETRY}{{margin-top:.375rem;align-self:flex-start}}\
.{ERROR_RETRY_GLYPH}{{width:.875rem;height:.875rem}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            ERROR_PANEL,
            ERROR_CHIP,
            ERROR_GLYPH,
            ERROR_BODY,
            ERROR_TITLE,
            ERROR_DETAIL,
            ERROR_RETRY,
            ERROR_RETRY_GLYPH,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
