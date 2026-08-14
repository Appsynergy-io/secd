//! CommandPalette — port of `components/dashboard/command-palette.tsx`
//! (Radix Dialog primitives wrapping the ui/command cmdk root).
//!
//! The reference is controlled (`open`/`onOpenChange`; the shell owns the
//! ⌘K shortcut) and jumps to nav destinations via react-router. Per the
//! navigation-is-props boundary, the nav catalogs (`NAV_CUSTOMER` /
//! `NAV_PLATFORM`) stay in the consumer: groups arrive as [`PaletteGroup`]
//! props and a chosen destination leaves through `on_navigate` — selecting
//! an item fires `on_open_change(false)` then `on_navigate(href)`, exactly
//! the reference's `go()`. Copy (title, placeholder, empty text, headings
//! from the consumer) mirrors the reference.
//!
//! Open-state DOM mirrors the Ladle story: overlay + content as direct body
//! children (the shared modal machinery in `dialog.rs`), content
//! `role="dialog"` with `aria-label="Command palette"`, `aria-labelledby`
//! → the sr-only `<h2>` title, `aria-describedby` stamped although no
//! description exists (Radix stamps the id unconditionally). The
//! `animate-in` classes compile to nothing upstream (tw-animate-css never
//! imported): no entry animation.

use crate::components::command::{
    Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList,
};
use crate::components::dialog::modal_open_effects;
use crate::behavior::portal::Portal;
use crate::icons::Icon;
use leptos::prelude::*;

pub const CMDP: &str = "asy-cmdp";
pub const CMDP_TITLE: &str = "asy-cmdp__title";
pub const CMDP_ICON: &str = "asy-cmdp__icon";
pub const CMDP_LABEL: &str = "asy-cmdp__label";
pub const CMDP_HINT: &str = "asy-cmdp__hint";

/// One nav destination: rendered as icon + label + mono href hint; the
/// cmdk filter value is `"{label} {href}"` (the reference's
/// `value={`${it.label} ${it.href}`}`).
#[derive(Clone, PartialEq)]
pub struct PaletteItem {
    pub label: String,
    pub href: String,
    /// Path data from [`crate::icons`], matching the catalog's remixicon.
    pub icon: &'static str,
}

/// One `CommandGroup` of destinations ("Go to", "Platform").
#[derive(Clone, PartialEq)]
pub struct PaletteGroup {
    pub heading: String,
    pub items: Vec<PaletteItem>,
}

/// Render-order id counter — same sequence on the server and in the
/// hydrating client (the Dialog precedent).
static CMDP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[component]
pub fn CommandPalette(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    groups: Vec<PaletteGroup>,
    #[prop(into)] on_navigate: Callback<String>,
) -> impl IntoView {
    let n = CMDP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ids = StoredValue::new((
        format!("asy-cmdp-{n}"),
        format!("asy-cmdp-{n}-title"),
        format!("asy-cmdp-{n}-desc"),
    ));
    let cls = StoredValue::new(CMDP.to_owned());
    let groups = StoredValue::new(groups);

    // Dismiss relay: the shared modal machinery closes by flipping a signal;
    // a controlled component instead reports the request outward and stays
    // put until the owner drops `open` (the Radix controlled contract).
    let relay = RwSignal::new(true);
    Effect::new(move |_| {
        if !relay.get() {
            relay.set(true);
            on_open_change.run(false);
        }
    });

    view! {
        <Show when=move || open.get()>
            // Empty portal: its host is claimed as the overlay element by
            // the open effect (Radix portals add no wrapper level).
            <Portal>{()}</Portal>
            <Portal>
                {
                    // The sr-only title anchors the ref; the host IS the
                    // dialog element.
                    let title_ref: NodeRef<leptos::html::H2> = NodeRef::new();
                    let trigger: NodeRef<leptos::html::Button> = NodeRef::new();
                    modal_open_effects(
                        title_ref,
                        trigger,
                        relay,
                        ids,
                        cls,
                        "dialog",
                        true,
                        Some("Command palette"),
                    );
                    view! {
                        <h2
                            id=ids.with_value(|(_, t, _)| t.clone())
                            class=CMDP_TITLE
                            node_ref=title_ref
                        >
                            "Command palette"
                        </h2>
                        <Command>
                            <CommandInput placeholder="Jump to…" />
                            <CommandList>
                                <CommandEmpty>"No matches."</CommandEmpty>
                                {groups
                                    .get_value()
                                    .into_iter()
                                    .map(|g| palette_group(g, on_open_change, on_navigate))
                                    .collect_view()}
                            </CommandList>
                        </Command>
                    }
                }
            </Portal>
        </Show>
    }
}

fn palette_group(
    group: PaletteGroup,
    on_open_change: Callback<bool>,
    on_navigate: Callback<String>,
) -> impl IntoView {
    view! {
        <CommandGroup heading=group.heading>
            {group
                .items
                .into_iter()
                .map(|it| {
                    let value = format!("{} {}", it.label, it.href);
                    let href = it.href.clone();
                    let on_select = Callback::new(move |_: String| {
                        on_open_change.run(false);
                        on_navigate.run(href.clone());
                    });
                    let icon = it.icon;
                    let label = it.label;
                    let hint = it.href;
                    view! {
                        <CommandItem value=value on_select=on_select>
                            <Icon d=icon class=CMDP_ICON />
                            <span class=CMDP_LABEL>{label.clone()}</span>
                            <span class=CMDP_HINT>{hint.clone()}</span>
                        </CommandItem>
                    }
                })
                .collect_view()}
        </CommandGroup>
    }
}

/// Content `fixed left-1/2 top-[18%] z-50 w-full max-w-lg -translate-x-1/2
/// overflow-hidden rounded-lg border bg-surface` with the reference's
/// literal shadow; title Tailwind v4 `sr-only`; item internals `size-4
/// text-dim` icon, `flex-1` label, `mono text-[11px] text-dim` hint.
pub fn css() -> String {
    format!(
        ".{CMDP}{{position:fixed;left:50%;top:max(1rem,10%);z-index:50;\
width:calc(100% - 2rem);max-width:32rem;\
max-height:min(28rem,calc(100dvh - 20%));translate:-50%;overflow:hidden;\
display:flex;flex-direction:column;\
border-radius:var(--radius-lg);border:1px solid var(--color-border);\
background-color:var(--color-surface);\
box-shadow:0 12px 40px oklch(0% 0 0 / 0.35)}}\
.{CMDP_TITLE}{{position:absolute;width:1px;height:1px;padding:0;\
margin:-1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;\
border-width:0}}\
.{CMDP_ICON}{{width:1rem;height:1rem;flex-shrink:0;color:var(--color-text-dim)}}\
.{CMDP_LABEL}{{flex:1;min-width:0;overflow:hidden;\
text-overflow:ellipsis;white-space:nowrap}}\
.{CMDP_HINT}{{min-width:0;overflow:hidden;\
text-overflow:ellipsis;white-space:nowrap;\
font-family:var(--font-mono);\
font-feature-settings:\"ss01\";font-size:11px;\
color:var(--color-text-dim)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [CMDP, CMDP_TITLE, CMDP_ICON, CMDP_LABEL, CMDP_HINT] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }
}
