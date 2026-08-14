//! InstallCommand — port of `components/onboarding/install-command.tsx`:
//! install snippet for the picked OS with a copy button when the snippet is
//! a real command. The channel row arrives as a prop (upstream it is
//! operator-managed data the parent fetches — HR-16); the copy interaction
//! (clipboard write + 2s "copied" swap) is the reference's own in-component
//! behavior, ported via web-sys.

use crate::components::button::{Button, ButtonSize};
use crate::icons::{Icon, RI_FILE_COPY_LINE, RI_LOADER_4_LINE};
use leptos::either::Either;
use leptos::prelude::*;

pub const INSTALL_CMD: &str = "asy-install-cmd";
pub const INSTALL_CMD_LABEL: &str = "asy-install-cmd__label";
pub const INSTALL_CMD_PRE: &str = "asy-install-cmd__pre";
pub const INSTALL_CMD_COPY: &str = "asy-install-cmd__copy";
pub const INSTALL_CMD_COPY_GLYPH: &str = "asy-install-cmd__copy-glyph";
pub const INSTALL_CMD_LOADING: &str = "asy-install-cmd__loading";
pub const INSTALL_CMD_SPINNER: &str = "asy-install-cmd__spinner";

/// One install channel row (`InstallChannel` upstream).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InstallChannel {
    pub platform: String,
    pub channel_label: String,
    pub command: String,
    pub copyable: bool,
}

#[component]
pub fn InstallCommand(
    /// The channel row for the selected OS, once loaded.
    #[prop(optional)] channel: Option<InstallChannel>,
    /// True while the channel list request is in flight.
    #[prop(optional)] loading: bool,
) -> impl IntoView {
    let Some(channel) = channel.filter(|_| !loading) else {
        return Either::Left(view! {
            <div class=INSTALL_CMD_LOADING>
                <Icon d=RI_LOADER_4_LINE class=INSTALL_CMD_SPINNER />
                "Loading install instructions\u{2026}"
            </div>
        });
    };
    let copied = RwSignal::new(false);
    let command = channel.command.clone();
    Either::Right(view! {
        <div class=INSTALL_CMD>
            <span class=INSTALL_CMD_LABEL>{channel.channel_label}</span>
            <pre class=format!("mono {INSTALL_CMD_PRE}")>
                {channel.command}
                {channel
                    .copyable
                    .then(|| {
                        view! {
                            <Button
                                size=ButtonSize::Sm
                                class=INSTALL_CMD_COPY
                                attr:aria-label="copy install command"
                                on:click=move |_| copy_to_clipboard(command.clone(), copied)
                            >
                                <Icon d=RI_FILE_COPY_LINE class=INSTALL_CMD_COPY_GLYPH />
                                {move || if copied.get() { "copied" } else { "copy" }}
                            </Button>
                        }
                    })}
            </pre>
        </div>
    })
}

/// Clipboard write, then flip `copied` for 2s — the reference's `copy()`.
/// Client-only; a no-op on the server.
fn copy_to_clipboard(text: String, copied: RwSignal<bool>) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::*;
        let Some(window) = web_sys::window() else { return };
        let promise = window.navigator().clipboard().write_text(&text);
        let on_ok = Closure::<dyn FnMut(JsValue)>::new(move |_| {
            copied.set(true);
            set_timeout(move || copied.set(false), std::time::Duration::from_millis(2000));
        });
        let on_err = Closure::<dyn FnMut(JsValue)>::new(move |_| { /* ignore denial */ });
        let _ = promise.then(&on_ok).catch(&on_err);
        on_ok.forget();
        on_err.forget();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (text, copied);
}

/// Wrapper `flex flex-col gap-1.5`; label `text-[11px] font-semibold
/// uppercase tracking-[0.05em] text-dim`; snippet `mono relative
/// overflow-auto whitespace-pre-wrap rounded-sm border border-border bg-bg
/// p-2.5 pr-16 text-[11.5px] leading-[1.55] text-muted`; copy button
/// `absolute right-1.5 top-1.5 h-[22px] px-1.5 text-[10.5px]` with a
/// `size-3` glyph; loading row `flex h-[64px] items-center gap-2
/// text-[12.5px] text-muted` with a `size-4 animate-spin` loader.
pub fn css() -> String {
    format!(
        ".{INSTALL_CMD}{{display:flex;flex-direction:column;gap:.375rem}}\
.{INSTALL_CMD_LABEL}{{font-size:11px;font-weight:600;text-transform:uppercase;\
letter-spacing:0.05em;color:var(--color-text-dim)}}\
.{INSTALL_CMD_PRE}{{position:relative;overflow:auto;white-space:pre-wrap;\
border-radius:var(--radius-sm);border:1px solid var(--color-border);\
background-color:var(--color-bg);padding:.625rem;padding-right:4rem;\
font-size:11.5px;line-height:1.55;color:var(--color-text-muted)}}\
.{INSTALL_CMD_COPY}{{position:absolute;right:.375rem;top:.375rem;min-height:2.75rem;height:auto;\
padding-left:.375rem;padding-right:.375rem;font-size:10.5px;line-height:inherit}}\
.{INSTALL_CMD_COPY_GLYPH}{{width:.75rem;height:.75rem}}\
.{INSTALL_CMD_LOADING}{{display:flex;height:64px;align-items:center;gap:.5rem;\
font-size:12.5px;color:var(--color-text-muted)}}\
.{INSTALL_CMD_SPINNER}{{width:1rem;height:1rem;\
animation:asy-spin 1s linear infinite}}\
@keyframes asy-spin{{to{{transform:rotate(360deg)}}}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            INSTALL_CMD,
            INSTALL_CMD_LABEL,
            INSTALL_CMD_PRE,
            INSTALL_CMD_COPY,
            INSTALL_CMD_COPY_GLYPH,
            INSTALL_CMD_LOADING,
            INSTALL_CMD_SPINNER,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn spinner_is_not_reduced_motion_gated() {
        // Tailwind's animate-spin has no reduced-motion gate upstream; the
        // harness freezes animation clocks instead.
        assert!(!css().contains("asy-spin\"}}"));
        assert!(css().contains("animation:asy-spin 1s linear infinite"));
    }
}
