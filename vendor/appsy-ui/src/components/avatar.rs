//! Avatar — port of `components/ui/avatar.tsx` (Radix Avatar). Root is a
//! styled inline span; `AvatarImage` mounts its `<img>` only once the source
//! has actually loaded (Radix preloads via an off-DOM `Image` and tracks a
//! loading status); `AvatarFallback` renders until then. Status machine
//! (Radix `useImageLoadingStatus`):
//!
//! ```text
//! Idle --(src set, client)--> Loading --onload--> Loaded
//!                                     --onerror-> Error
//! ```
//!
//! Image shows iff `Loaded`; fallback shows iff not `Loaded` (the reference
//! passes no `delayMs`). On the server the status never leaves `Idle`, so
//! SSR emits the fallback only — exactly Radix's SSR shape.

use leptos::prelude::*;

pub const AVATAR: &str = "asy-avatar";
pub const AVATAR_IMAGE: &str = "asy-avatar__image";
pub const AVATAR_FALLBACK: &str = "asy-avatar__fallback";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ImageLoadingStatus {
    #[default]
    Idle,
    Loading,
    Loaded,
    Error,
}

#[component]
pub fn Avatar(#[prop(optional, into)] class: Option<String>, children: Children) -> impl IntoView {
    let status = RwSignal::new(ImageLoadingStatus::Idle);
    let mut cls = AVATAR.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <span class=cls><leptos::context::Provider value=status>{children()}</leptos::context::Provider></span> }
}

#[component]
pub fn AvatarImage(
    #[prop(into)] src: String,
    #[prop(optional, into)] alt: Option<String>,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let status =
        use_context::<RwSignal<ImageLoadingStatus>>().unwrap_or_else(|| RwSignal::new(ImageLoadingStatus::Idle));
    let mut cls = AVATAR_IMAGE.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    let preload_src = src.clone();
    Effect::new(move |_| track_load(status, &preload_src));
    move || {
        (status.get() == ImageLoadingStatus::Loaded).then({
            let src = src.clone();
            let alt = alt.clone();
            let cls = cls.clone();
            move || view! { <img class=cls src=src alt=alt /> }
        })
    }
}

#[component]
pub fn AvatarFallback(
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let status =
        use_context::<RwSignal<ImageLoadingStatus>>().unwrap_or_else(|| RwSignal::new(ImageLoadingStatus::Idle));
    let mut cls = AVATAR_FALLBACK.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    move || {
        (status.get() != ImageLoadingStatus::Loaded).then({
            let cls = cls.clone();
            let children = children.clone();
            move || view! { <span class=cls>{children()}</span> }
        })
    }
}

/// Preload `src` off-DOM and drive the status signal. Client-only: effects
/// never run during SSR, so the server stays at `Idle`.
fn track_load(status: RwSignal<ImageLoadingStatus>, src: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        status.set(ImageLoadingStatus::Loading);
        let img = web_sys::HtmlImageElement::new().expect("invariant: Image() constructor exists");
        let on_load = Closure::<dyn FnMut()>::new(move || status.set(ImageLoadingStatus::Loaded));
        let on_error = Closure::<dyn FnMut()>::new(move || status.set(ImageLoadingStatus::Error));
        img.set_onload(Some(on_load.as_ref().unchecked_ref()));
        img.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        img.set_src(src);
        // The closures outlive this scope for as long as the page does —
        // the load fires exactly once per preload, so this leak is bounded
        // by the number of AvatarImage mounts.
        on_load.forget();
        on_error.forget();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (status, src);
}

/// Root `relative inline-flex size-7 shrink-0 overflow-hidden rounded-full
/// bg-surface-2`; image `aspect-square h-full w-full`; fallback `flex h-full
/// w-full items-center justify-center text-xs font-medium text-muted`.
pub fn css() -> String {
    format!(
        ".{AVATAR}{{position:relative;display:inline-flex;width:1.75rem;height:1.75rem;\
flex-shrink:0;overflow:hidden;border-radius:calc(infinity * 1px);\
background-color:var(--color-surface-2)}}\
.{AVATAR_IMAGE}{{aspect-ratio:1/1;height:100%;width:100%}}\
.{AVATAR_FALLBACK}{{display:flex;height:100%;width:100%;align-items:center;\
justify-content:center;font-size:.75rem;line-height:calc(1/.75);\
font-weight:500;color:var(--color-text-muted)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [AVATAR, AVATAR_IMAGE, AVATAR_FALLBACK] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn status_machine_default_is_idle() {
        assert_eq!(ImageLoadingStatus::default(), ImageLoadingStatus::Idle);
    }
}
