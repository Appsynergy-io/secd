//! DeviceCodeChallenge — port of `onboarding/device-code-challenge.tsx`:
//! W-42 step 3, the device-link code with a live countdown to its 5-minute
//! expiry. The parent owns the linking lifecycle (mint, poll, connected);
//! this component is pure presentation plus the expiry timer. States:
//!
//! ```text
//! minting || !code  -> loading spinner
//! seconds_left == 0 -> expired (message + re-mint button), on_expired fired once
//! otherwise         -> code + "Single-use · expires in m:ss"
//! ```

use crate::components::button::{Button, ButtonSize};
use crate::icons::{Icon, RI_LOADER_4_LINE, RI_REFRESH_LINE};
use leptos::either::EitherOf3;
use leptos::prelude::*;

pub const DEVICE_CODE: &str = "asy-device-code";
pub const DEVICE_CODE_LOADING: &str = "asy-device-code__loading";
pub const DEVICE_CODE_SPINNER: &str = "asy-device-code__spinner";
pub const DEVICE_CODE_EXPIRED: &str = "asy-device-code__expired";
pub const DEVICE_CODE_EXPIRED_MSG: &str = "asy-device-code__expired-msg";
pub const DEVICE_CODE_REMINT_GLYPH: &str = "asy-device-code__remint-glyph";
pub const DEVICE_CODE_CODE: &str = "asy-device-code__code";
pub const DEVICE_CODE_HINT: &str = "asy-device-code__hint";

#[component]
pub fn DeviceCodeChallenge(
    /// The `XXXX-XXXX` code from the device-link start call.
    #[prop(optional, into)] code: Option<String>,
    /// Seconds of validity at mint time (`expires_in`).
    expires_in: u32,
    /// True while the mint request is in flight.
    #[prop(optional)] minting: bool,
    /// Fired once when the countdown reaches zero.
    on_expired: Callback<()>,
    /// User-initiated re-mint after expiry.
    on_remint: Callback<()>,
) -> impl IntoView {
    let seconds_left = RwSignal::new(expires_in);
    let has_code = code.is_some();

    // Tick every second while a code is live (client only, like the
    // reference's interval effect); fire on_expired exactly once at zero.
    if has_code {
        let expired_fired = StoredValue::new(false);
        Effect::new(move |_| {
            if seconds_left.get() == 0 && !expired_fired.get_value() {
                expired_fired.set_value(true);
                on_expired.run(());
            }
        });
        Effect::new(move |_| {
            let handle = set_interval_with_handle(
                move || seconds_left.update(|s| *s = s.saturating_sub(1)),
                std::time::Duration::from_secs(1),
            )
            .expect("invariant: interval registration on a live window");
            on_cleanup(move || handle.clear());
        });
    }

    view! {
        <div class=DEVICE_CODE>
            {move || {
                if minting || !has_code {
                    EitherOf3::A(
                        view! {
                            <div class=DEVICE_CODE_LOADING>
                                <Icon d=RI_LOADER_4_LINE class=DEVICE_CODE_SPINNER />
                                "Generating your link code\u{2026}"
                            </div>
                        },
                    )
                } else if seconds_left.get() == 0 {
                    EitherOf3::B(
                        view! {
                            <div class=DEVICE_CODE_EXPIRED>
                                <span class=DEVICE_CODE_EXPIRED_MSG>
                                    "That code expired before it was used."
                                </span>
                                <Button
                                    size=ButtonSize::Sm
                                    attr:r#type="button"
                                    on:click=move |_| on_remint.run(())
                                >
                                    <Icon d=RI_REFRESH_LINE class=DEVICE_CODE_REMINT_GLYPH />
                                    "Generate a new code"
                                </Button>
                            </div>
                        },
                    )
                } else {
                    let code = code.clone().unwrap_or_default();
                    let mm = (seconds_left.get() / 60).to_string();
                    let ss = format!("{:02}", seconds_left.get() % 60);
                    EitherOf3::C(
                        view! {
                            <span class=format!("mono {DEVICE_CODE_CODE}") aria-label="device link code">
                                {code}
                            </span>
                            <span class=DEVICE_CODE_HINT>
                                "Single-use \u{b7} expires in " {mm} ":" {ss}
                            </span>
                        },
                    )
                }
            }}
        </div>
    }
}

/// Column `flex flex-col items-center gap-3 py-2`; loading/expired rows are
/// 64px tall; code `mono select-all rounded-sm border accent-line
/// bg-accent-soft px-5 py-3 text-[28px] font-semibold tracking-[0.14em]
/// text-accent`; hint `text-[11.5px] text-dim`. Spinner reuses the global
/// `asy-spin` keyframes (defined with install-command).
pub fn css() -> String {
    format!(
        ".{DEVICE_CODE}{{display:flex;flex-direction:column;align-items:center;gap:.75rem;\
padding-top:.5rem;padding-bottom:.5rem}}\
.{DEVICE_CODE_LOADING}{{display:flex;height:64px;align-items:center;gap:.5rem;\
font-size:12.5px;color:var(--color-text-muted)}}\
.{DEVICE_CODE_SPINNER}{{width:1rem;height:1rem;animation:asy-spin 1s linear infinite}}\
.{DEVICE_CODE_EXPIRED}{{display:flex;height:64px;flex-direction:column;align-items:center;\
justify-content:center;gap:.5rem}}\
.{DEVICE_CODE_EXPIRED_MSG}{{font-size:12.5px;color:var(--color-text-muted)}}\
.{DEVICE_CODE_REMINT_GLYPH}{{width:.875rem;height:.875rem}}\
.{DEVICE_CODE_CODE}{{user-select:all;border-radius:var(--radius-sm);\
border:1px solid var(--color-accent-line);background-color:var(--color-accent-soft);\
padding-left:1.25rem;padding-right:1.25rem;padding-top:.75rem;padding-bottom:.75rem;\
font-size:28px;font-weight:600;letter-spacing:0.14em;color:var(--color-accent);\
overflow-wrap:anywhere;max-width:100%}}\
.{DEVICE_CODE_HINT}{{font-size:11.5px;color:var(--color-text-dim)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            DEVICE_CODE,
            DEVICE_CODE_LOADING,
            DEVICE_CODE_SPINNER,
            DEVICE_CODE_EXPIRED,
            DEVICE_CODE_EXPIRED_MSG,
            DEVICE_CODE_REMINT_GLYPH,
            DEVICE_CODE_CODE,
            DEVICE_CODE_HINT,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
