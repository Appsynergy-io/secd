//! Skeleton — port of `components/ui/skeleton.tsx`: loading placeholders.
//! `Skeleton` defaults to the shimmer sweep (`.skel-shimmer` in app.css — a
//! translating gradient on `::after`, gated behind `prefers-reduced-motion`);
//! `pulse` falls back to Tailwind's `animate-pulse` opacity blink (which the
//! reference does *not* gate behind reduced motion — mirrored exactly).
//! `SkeletonTable` and `SkeletonCards` are the table- and card-grid-shaped
//! composites; both reuse the card surface class rather than the `Card`
//! component so they can stamp `aria-busy`/`aria-label` like the reference.

use crate::components::card::CARD;
use leptos::prelude::*;

pub const SKEL: &str = "asy-skel";
pub const SKEL_SHIMMER: &str = "asy-skel--shimmer";
pub const SKEL_PULSE: &str = "asy-skel--pulse";
pub const SKEL_TABLE: &str = "asy-skel-table";
pub const SKEL_TABLE_HEAD: &str = "asy-skel-table__head";
pub const SKEL_TABLE_HCELL: &str = "asy-skel-table__hcell";
pub const SKEL_TABLE_BODY: &str = "asy-skel-table__body";
pub const SKEL_TABLE_ROW: &str = "asy-skel-table__row";
pub const SKEL_TABLE_CELL: &str = "asy-skel-table__cell";
pub const SKEL_CARDS: &str = "asy-skel-cards";
pub const SKEL_CARDS_CARD: &str = "asy-skel-cards__card";
pub const SKEL_CARDS_LABEL: &str = "asy-skel-cards__label";
pub const SKEL_CARDS_VALUE: &str = "asy-skel-cards__value";
pub const SKEL_CARDS_BAR: &str = "asy-skel-cards__bar";

#[component]
pub fn Skeleton(
    /// Opacity-blink fallback instead of the default shimmer sweep.
    #[prop(optional)] pulse: bool,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let variant = if pulse { SKEL_PULSE } else { SKEL_SHIMMER };
    let mut cls = format!("{SKEL} {variant}");
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! { <div class=cls></div> }
}

#[component]
pub fn SkeletonTable(
    #[prop(optional, default = 6)] rows: usize,
    #[prop(optional, default = 5)] cols: usize,
) -> impl IntoView {
    view! {
        <div class=format!("{CARD} {SKEL_TABLE}") aria-busy="true" aria-label="Loading">
            <div class=SKEL_TABLE_HEAD>
                {(0..cols)
                    .map(|_| view! { <Skeleton class=SKEL_TABLE_HCELL /> })
                    .collect_view()}
            </div>
            <div class=SKEL_TABLE_BODY>
                {(0..rows)
                    .map(|_| {
                        view! {
                            <div class=SKEL_TABLE_ROW>
                                {(0..cols)
                                    .map(|_| view! { <Skeleton class=SKEL_TABLE_CELL /> })
                                    .collect_view()}
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[component]
pub fn SkeletonCards(#[prop(optional, default = 4)] count: usize) -> impl IntoView {
    view! {
        <div class=SKEL_CARDS aria-busy="true" aria-label="Loading">
            {(0..count)
                .map(|_| {
                    view! {
                        <div class=format!("{CARD} {SKEL_CARDS_CARD}")>
                            <Skeleton class=SKEL_CARDS_LABEL />
                            <Skeleton class=SKEL_CARDS_VALUE />
                            <Skeleton class=SKEL_CARDS_BAR />
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// Shimmer = `.skel-shimmer` from app.css verbatim (surface-2 base, `::after`
/// gradient sweep, reduced-motion gate); pulse = Tailwind `animate-pulse`
/// (`pulse 2s cubic-bezier(.4,0,.6,1) infinite`, 50% opacity .5), ungated.
/// Table: card surface `overflow-hidden p-0`; head `flex gap-4 border-b
/// border-[--color-border] px-4 py-2.5` with `h-3 flex-1` cells; body
/// `divide-y divide-[--color-border-soft]` (border-bottom on all but the
/// last child); rows `flex items-center gap-4 px-4 py-3` with `h-3.5 flex-1`
/// cells. Cards: `grid gap-3 sm:grid-cols-2 lg:grid-cols-4`; each card
/// `flex flex-col gap-3 p-4` with `h-3 w-20` / `h-6 w-28` / `h-2.5 w-full`.
pub fn css() -> String {
    format!(
        ".{SKEL}{{border-radius:var(--radius-sm)}}\
.{SKEL_SHIMMER}{{position:relative;overflow:hidden;background:var(--color-surface-2)}}\
.{SKEL_SHIMMER}::after{{content:\"\";position:absolute;inset:0;transform:translateX(-100%);\
background:linear-gradient(90deg,transparent,oklch(100% 0 0 / 0.06),transparent);\
animation:asy-skel-shimmer 1.4s ease-in-out infinite}}\
@keyframes asy-skel-shimmer{{100%{{transform:translateX(100%)}}}}\
@media (prefers-reduced-motion: reduce){{.{SKEL_SHIMMER}::after{{animation:none}}}}\
.{SKEL_PULSE}{{background-color:var(--color-surface-2);\
animation:asy-pulse 2s cubic-bezier(.4,0,.6,1) infinite}}\
@keyframes asy-pulse{{50%{{opacity:.5}}}}\
.{SKEL_TABLE}{{overflow:hidden;padding:0}}\
.{SKEL_TABLE_HEAD}{{display:flex;gap:1rem;border-color:var(--color-border);\
border-bottom-width:1px;padding-left:1rem;padding-right:1rem;\
padding-top:.625rem;padding-bottom:.625rem}}\
.{SKEL_TABLE_HCELL}{{height:.75rem;flex:1 1 0%}}\
.{SKEL_TABLE_BODY}>:not(:last-child){{border-color:var(--color-border-soft);\
border-bottom-width:1px}}\
.{SKEL_TABLE_ROW}{{display:flex;align-items:center;gap:1rem;\
padding-left:1rem;padding-right:1rem;padding-top:.75rem;padding-bottom:.75rem}}\
.{SKEL_TABLE_CELL}{{height:.875rem;flex:1 1 0%}}\
.{SKEL_CARDS}{{display:grid;gap:.75rem}}\
@media (min-width:640px){{.{SKEL_CARDS}{{grid-template-columns:repeat(2,minmax(0,1fr))}}}}\
@media (min-width:1024px){{.{SKEL_CARDS}{{grid-template-columns:repeat(4,minmax(0,1fr))}}}}\
.{SKEL_CARDS_CARD}{{display:flex;flex-direction:column;gap:.75rem;padding:1rem}}\
.{SKEL_CARDS_LABEL}{{height:.75rem;width:5rem}}\
.{SKEL_CARDS_VALUE}{{height:1.5rem;width:7rem}}\
.{SKEL_CARDS_BAR}{{height:.625rem;width:100%}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            SKEL,
            SKEL_SHIMMER,
            SKEL_PULSE,
            SKEL_TABLE,
            SKEL_TABLE_HEAD,
            SKEL_TABLE_HCELL,
            SKEL_TABLE_BODY,
            SKEL_TABLE_ROW,
            SKEL_TABLE_CELL,
            SKEL_CARDS,
            SKEL_CARDS_CARD,
            SKEL_CARDS_LABEL,
            SKEL_CARDS_VALUE,
            SKEL_CARDS_BAR,
        ] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
    }

    #[test]
    fn shimmer_gated_behind_reduced_motion_pulse_not() {
        let css = css();
        assert!(
            css.contains(&format!(
                "@media (prefers-reduced-motion: reduce){{.{SKEL_SHIMMER}::after{{animation:none}}}}"
            )),
            "shimmer sweep must be gated behind reduced motion, and nothing else with it"
        );
    }
}
