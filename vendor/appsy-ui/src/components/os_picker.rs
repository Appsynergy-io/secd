//! OsPicker — port of `components/onboarding/os-picker.tsx`: W-42 step 1,
//! a radiogroup of OS chips. The option set is the reference's fixed enum
//! domain (ALLOW-HARDCODE upstream: mirrors the backend allowlist).
//! Controlled: the parent owns the selected slug.

use crate::icons::{Icon, RI_ANDROID_FILL, RI_APPLE_FILL, RI_MICROSOFT_FILL, RI_UBUNTU_FILL};
use leptos::prelude::*;

pub const OS_PICKER: &str = "asy-os-picker";
pub const OS_PICKER_BTN: &str = "asy-os-picker__btn";
pub const OS_PICKER_BTN_ON: &str = "asy-os-picker__btn--on";
pub const OS_PICKER_BTN_OFF: &str = "asy-os-picker__btn--off";
pub const OS_PICKER_GLYPH: &str = "asy-os-picker__glyph";

/// Platform slug accepted by the device-link verify endpoint.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OsSlug {
    Macos,
    Windows,
    Linux,
    Ios,
    Android,
}

impl OsSlug {
    pub fn slug(self) -> &'static str {
        match self {
            OsSlug::Macos => "macos",
            OsSlug::Windows => "windows",
            OsSlug::Linux => "linux",
            OsSlug::Ios => "ios",
            OsSlug::Android => "android",
        }
    }
}

const OS_OPTIONS: [(&str, &str, OsSlug); 5] = [
    ("macOS", RI_APPLE_FILL, OsSlug::Macos),
    ("Windows", RI_MICROSOFT_FILL, OsSlug::Windows),
    ("Linux", RI_UBUNTU_FILL, OsSlug::Linux),
    ("iOS", RI_APPLE_FILL, OsSlug::Ios),
    ("Android", RI_ANDROID_FILL, OsSlug::Android),
];

#[component]
pub fn OsPicker(#[prop(into)] value: Signal<OsSlug>, on_change: Callback<OsSlug>) -> impl IntoView {
    view! {
        <div class=OS_PICKER role="radiogroup" aria-label="Operating system">
            {OS_OPTIONS
                .into_iter()
                .map(|(label, icon, slug)| {
                    view! {
                        <button
                            type="button"
                            role="radio"
                            aria-checked=move || if value.get() == slug { "true" } else { "false" }
                            class=move || {
                                let state = if value.get() == slug {
                                    OS_PICKER_BTN_ON
                                } else {
                                    OS_PICKER_BTN_OFF
                                };
                                format!("{OS_PICKER_BTN} {state}")
                            }
                            on:click=move |_| on_change.run(slug)
                        >
                            <Icon d=icon class=OS_PICKER_GLYPH />
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// Group `flex flex-wrap items-center gap-2`; chips `inline-flex h-8
/// items-center gap-2 rounded-sm border px-3 text-[12.5px]` with accent
/// tint when checked, surface tint otherwise, and `size-3.5` glyphs.
pub fn css() -> String {
    format!(
        ".{OS_PICKER}{{display:flex;flex-wrap:wrap;align-items:center;gap:.5rem}}\
.{OS_PICKER_BTN}{{display:inline-flex;min-height:2.75rem;align-items:center;gap:.5rem;\
border-radius:var(--radius-sm);border-width:1px;border-style:solid;\
padding-left:.75rem;padding-right:.75rem;font-size:12.5px}}\
.{OS_PICKER_BTN_ON}{{border-color:var(--color-accent-line);\
background-color:var(--color-accent-soft);color:var(--color-accent)}}\
.{OS_PICKER_BTN_OFF}{{border-color:var(--color-border);\
background-color:var(--color-surface-2);color:var(--color-text)}}\
.{OS_PICKER_GLYPH}{{width:.875rem;height:.875rem}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [OS_PICKER, OS_PICKER_BTN, OS_PICKER_BTN_ON, OS_PICKER_BTN_OFF, OS_PICKER_GLYPH]
        {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn slugs_match_the_verify_endpoint_domain() {
        let slugs: Vec<&str> = OS_OPTIONS.iter().map(|(_, _, s)| s.slug()).collect();
        assert_eq!(slugs, ["macos", "windows", "linux", "ios", "android"]);
    }
}
