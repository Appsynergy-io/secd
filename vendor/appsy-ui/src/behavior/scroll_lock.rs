//! Scroll lock — freeze body scrolling while an overlay is open, without
//! layout shift. Replaces the `react-remove-scroll` behavior Radix dialogs
//! use: body gets `overflow: hidden` plus right padding equal to the
//! scrollbar width it just lost.
//!
//! # Spec (state machine)
//!
//! State: a lock depth `n ≥ 0` (overlays nest — a dialog can open a select).
//!
//! | State | Event     | Next  | Effect    |
//! |-------|-----------|-------|-----------|
//! | n = 0 | `Acquire` | n = 1 | `Apply`   |
//! | n > 0 | `Acquire` | n + 1 | none      |
//! | n = 1 | `Release` | n = 0 | `Restore` |
//! | n > 1 | `Release` | n − 1 | none      |
//! | n = 0 | `Release` | n = 0 | none      |
//!
//! Invariants: `Apply` and `Restore` fire exactly once per outermost
//! lock/unlock pair; `Release` below zero is a no-op (never restores twice);
//! restore puts back the exact prior inline `overflow`/`padding-right`
//! values (which may be empty strings).
//!
//! Keyboard map: none (no APG pattern; pure environment control).

/// Pure depth counter — the tested machine.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScrollLock {
    depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockEffect {
    Apply,
    Restore,
    None,
}

impl ScrollLock {
    pub fn acquire(&mut self) -> LockEffect {
        self.depth += 1;
        if self.depth == 1 { LockEffect::Apply } else { LockEffect::None }
    }

    pub fn release(&mut self) -> LockEffect {
        match self.depth {
            0 => LockEffect::None,
            1 => {
                self.depth = 0;
                LockEffect::Restore
            }
            _ => {
                self.depth -= 1;
                LockEffect::None
            }
        }
    }

    pub fn depth(self) -> usize {
        self.depth
    }
}

/// Saved inline styles for an exact restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedBodyStyle {
    pub overflow: String,
    pub padding_right: String,
}

/// DOM adapter: apply the lock to `document.body`. Returns what to hand back
/// to [`restore`]. Scrollbar compensation = `innerWidth − clientWidth`
/// measured before locking.
pub fn apply(document: &web_sys::Document, window: &web_sys::Window) -> SavedBodyStyle {
    let body = document.body().expect("invariant: document has body");
    let style = body.style();
    let saved = SavedBodyStyle {
        overflow: style.get_property_value("overflow").unwrap_or_default(),
        padding_right: style.get_property_value("padding-right").unwrap_or_default(),
    };
    let inner = window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let client = document
        .document_element()
        .map(|e| f64::from(e.client_width()))
        .unwrap_or(0.0);
    let scrollbar = (inner - client).max(0.0);
    if scrollbar > 0.0 {
        let existing = window
            .get_computed_style(&body)
            .ok()
            .flatten()
            .and_then(|cs| cs.get_property_value("padding-right").ok())
            .and_then(|v| v.trim_end_matches("px").parse::<f64>().ok())
            .unwrap_or(0.0);
        let _ = style.set_property("padding-right", &format!("{}px", existing + scrollbar));
    }
    let _ = style.set_property("overflow", "hidden");
    saved
}

/// DOM adapter: undo [`apply`] with the exact saved values (empty string ⇒
/// remove the inline property).
pub fn restore(document: &web_sys::Document, saved: &SavedBodyStyle) {
    let body = document.body().expect("invariant: document has body");
    let style = body.style();
    for (name, value) in
        [("overflow", &saved.overflow), ("padding-right", &saved.padding_right)]
    {
        if value.is_empty() {
            let _ = style.remove_property(name);
        } else {
            let _ = style.set_property(name, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outermost_pair_applies_and_restores_once() {
        let mut lock = ScrollLock::default();
        assert_eq!(lock.acquire(), LockEffect::Apply);
        assert_eq!(lock.acquire(), LockEffect::None);
        assert_eq!(lock.release(), LockEffect::None);
        assert_eq!(lock.release(), LockEffect::Restore);
        assert_eq!(lock.depth(), 0);
    }

    #[test]
    fn release_below_zero_is_a_noop() {
        let mut lock = ScrollLock::default();
        assert_eq!(lock.release(), LockEffect::None);
        assert_eq!(lock.release(), LockEffect::None);
        assert_eq!(lock.depth(), 0);
        // A later legitimate cycle still works.
        assert_eq!(lock.acquire(), LockEffect::Apply);
        assert_eq!(lock.release(), LockEffect::Restore);
    }

    #[test]
    fn three_deep_nesting_effects_only_at_the_edges() {
        let mut lock = ScrollLock::default();
        let effects: Vec<LockEffect> =
            vec![lock.acquire(), lock.acquire(), lock.acquire(), lock.release(), lock.release(), lock.release()];
        assert_eq!(
            effects,
            vec![
                LockEffect::Apply,
                LockEffect::None,
                LockEffect::None,
                LockEffect::None,
                LockEffect::None,
                LockEffect::Restore
            ]
        );
    }
}
