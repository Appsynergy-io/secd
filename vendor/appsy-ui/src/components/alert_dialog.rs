//! AlertDialog — port of `components/ui/alert-dialog.tsx` (Radix
//! AlertDialog). Same modal machinery as Dialog (shared
//! `modal_open_effects`), with the alert differences: `role="alertdialog"`,
//! no built-in close X, and **no outside-pointerdown dismiss** — Escape and
//! the Cancel/Action buttons are the only ways out. The Cancel button is
//! first in DOM order, so the focus trap's first-tabbable initial focus is
//! Radix's focus-the-cancel behavior. Content styling differs from Dialog:
//! `rounded-md` + `shadow-lg` instead of `rounded-lg` + the custom shadow.

use crate::behavior::portal::Portal;
use crate::components::button::{ButtonSize, ButtonVariant, BTN};
use crate::components::dialog::modal_open_effects;
use leptos::prelude::*;

pub const ALERT_DIALOG: &str = "asy-alert-dialog";
pub const ALERT_DIALOG_HEADER: &str = "asy-alert-dialog__header";
pub const ALERT_DIALOG_FOOTER: &str = "asy-alert-dialog__footer";
pub const ALERT_DIALOG_TITLE: &str = "asy-alert-dialog__title";
pub const ALERT_DIALOG_DESCRIPTION: &str = "asy-alert-dialog__description";

#[derive(Clone, Copy)]
struct AlertDialogCtx {
    open: RwSignal<bool>,
    ids: StoredValue<(String, String, String)>,
    trigger: NodeRef<leptos::html::Button>,
    /// The Cancel button anchors host resolution and takes initial focus.
    cancel: NodeRef<leptos::html::Button>,
}

static ALERT_DIALOG_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[component]
pub fn AlertDialog(children: Children) -> impl IntoView {
    let n = ALERT_DIALOG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ctx = AlertDialogCtx {
        open: RwSignal::new(false),
        ids: StoredValue::new((
            format!("asy-alert-{n}"),
            format!("asy-alert-{n}-title"),
            format!("asy-alert-{n}-desc"),
        )),
        trigger: NodeRef::new(),
        cancel: NodeRef::new(),
    };
    // Scoped Provider — bare provide_context would let a later sibling
    // instance shadow this ctx for lazily-built children (see select.rs).
    view! { <leptos::context::Provider value=ctx>{children()}</leptos::context::Provider> }
}

/// `Trigger asChild` on a Button, per the Button precedent. Radix's alert
/// trigger stamps `aria-haspopup="dialog"` (not "alertdialog").
#[component]
pub fn AlertDialogTrigger(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx =
        use_context::<AlertDialogCtx>().expect("invariant: AlertDialogTrigger inside AlertDialog");
    let mut cls = format!("{BTN} {} {}", variant.class(), size.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <button
            class=cls
            node_ref=ctx.trigger
            type="button"
            aria-haspopup="dialog"
            aria-expanded=move || if ctx.open.get() { "true" } else { "false" }
            aria-controls=ctx.ids.with_value(|(c, _, _)| c.clone())
            data-state=move || if ctx.open.get() { "open" } else { "closed" }
            on:click=move |_| ctx.open.set(true)
        >
            {children()}
        </button>
    }
}

#[component]
pub fn AlertDialogContent(
    #[prop(optional, into)] class: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let ctx =
        use_context::<AlertDialogCtx>().expect("invariant: AlertDialogContent inside AlertDialog");
    let open = ctx.open;
    let trigger = ctx.trigger;
    let cancel = ctx.cancel;
    let ids = ctx.ids;
    let cls = StoredValue::new({
        let mut cls = ALERT_DIALOG.to_owned();
        if let Some(extra) = class {
            cls.push(' ');
            cls.push_str(&extra);
        }
        cls
    });
    let children = StoredValue::new(children);
    view! {
        <Show when=move || open.get()>
            <Portal>{()}</Portal>
            <Portal>
                {
                    modal_open_effects(cancel, trigger, open, ids, cls, "alertdialog", false, None);
                    children.with_value(|c| c())
                }
            </Portal>
        </Show>
    }
}

/// `Cancel asChild` on a Button — closes; first in DOM, takes initial focus.
#[component]
pub fn AlertDialogCancel(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx =
        use_context::<AlertDialogCtx>().expect("invariant: AlertDialogCancel inside AlertDialog");
    let mut cls = format!("{BTN} {} {}", variant.class(), size.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <button
            class=cls
            type="button"
            node_ref=ctx.cancel
            on:click=move |_| ctx.open.set(false)
        >
            {children()}
        </button>
    }
}

/// `Action asChild` on a Button — the confirming action; closes on click,
/// the consumer's handler arrives via `on_click`.
#[component]
pub fn AlertDialogAction(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional)] on_click: Option<Callback<()>>,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx =
        use_context::<AlertDialogCtx>().expect("invariant: AlertDialogAction inside AlertDialog");
    let mut cls = format!("{BTN} {} {}", variant.class(), size.class());
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <button
            class=cls
            type="button"
            on:click=move |_| {
                if let Some(cb) = on_click {
                    cb.run(());
                }
                ctx.open.set(false);
            }
        >
            {children()}
        </button>
    }
}

#[component]
pub fn AlertDialogHeader(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = ALERT_DIALOG_HEADER.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <div class=cls>{children()}</div> }
}

#[component]
pub fn AlertDialogFooter(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = ALERT_DIALOG_FOOTER.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <div class=cls>{children()}</div> }
}

#[component]
pub fn AlertDialogTitle(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx =
        use_context::<AlertDialogCtx>().expect("invariant: AlertDialogTitle inside AlertDialog");
    let mut cls = ALERT_DIALOG_TITLE.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <h2 id=ctx.ids.with_value(|(_, t, _)| t.clone()) class=cls>{children()}</h2> }
}

#[component]
pub fn AlertDialogDescription(
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let ctx = use_context::<AlertDialogCtx>()
        .expect("invariant: AlertDialogDescription inside AlertDialog");
    let mut cls = ALERT_DIALOG_DESCRIPTION.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <p id=ctx.ids.with_value(|(_, _, d)| d.clone()) class=cls>{children()}</p> }
}

/// Content `fixed left-1/2 top-1/2 z-50 grid w-full max-w-md
/// -translate-x-1/2 -translate-y-1/2 gap-4 rounded-md border bg-surface p-5
/// shadow-lg` (`#0000001a` layers, as v4 compiles); header/footer/title/
/// description mirror Dialog's. The overlay reuses `asy-dialog-overlay`.
pub fn css() -> String {
    format!(
        ".{ALERT_DIALOG}{{position:fixed;left:50%;top:50%;z-index:50;\
display:grid;width:calc(100% - 2rem);max-width:28rem;\
max-height:calc(100dvh - 2rem);overflow-y:auto;translate:-50% -50%;gap:1rem;\
border-radius:var(--radius-md);border:1px solid var(--color-border);\
background-color:var(--color-surface);padding:1.25rem;\
box-shadow:0 10px 15px -3px #0000001a,0 4px 6px -4px #0000001a}}\
.{ALERT_DIALOG_HEADER}{{display:flex;flex-direction:column;gap:.375rem}}\
.{ALERT_DIALOG_FOOTER}{{display:flex;flex-direction:row;\
justify-content:flex-end;gap:.5rem}}\
.{ALERT_DIALOG_TITLE}{{font-size:1rem;line-height:1.5rem;font-weight:600;\
letter-spacing:-.025em}}\
.{ALERT_DIALOG_DESCRIPTION}{{font-size:.875rem;line-height:1.25rem;\
color:var(--color-text-muted)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            ALERT_DIALOG,
            ALERT_DIALOG_HEADER,
            ALERT_DIALOG_FOOTER,
            ALERT_DIALOG_TITLE,
            ALERT_DIALOG_DESCRIPTION,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
