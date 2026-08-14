//! Interaction primitives — the layer that replaces Radix. Each module lands
//! with a written state machine + keyboard map in its doc comment (the
//! matching WAI-ARIA APG pattern is the spec) and unit tests against that
//! machine. Pure decision logic is separated from the thin `web-sys` DOM
//! adapters so the machines test natively, without a browser.

pub mod dismiss;
pub mod floating;
pub mod focus_trap;
pub mod portal;
pub mod roving_tabindex;
pub mod scroll_lock;
pub mod typeahead;
