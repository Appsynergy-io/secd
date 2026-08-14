//! Dismiss — close an overlay on Escape or on pointer-down outside its
//! protected subtrees. Replaces `@radix-ui/react-dismissable-layer` for the
//! dismissal cases the site's overlays use.
//!
//! # Spec (state machine)
//!
//! States: `Armed` · `Disarmed`.
//!
//! | State    | Input                          | Next     | Fire dismiss? |
//! |----------|--------------------------------|----------|---------------|
//! | Armed    | `PointerDown { inside: false }`| Disarmed | yes           |
//! | Armed    | `Escape`                       | Disarmed | yes           |
//! | Armed    | `PointerDown { inside: true }` | Armed    | no            |
//! | Disarmed | any                            | Disarmed | no            |
//!
//! `rearm()` returns to `Armed` (overlay reopened). Invariant: at most one
//! fire per armed period, no matter how many qualifying events race in.
//!
//! # Keyboard map
//!
//! | Key    | Effect  |
//! |--------|---------|
//! | Escape | dismiss |
//!
//! "Inside" = the event target is contained in any registered protected node
//! (overlay content, and the trigger — so the opening click does not
//! immediately re-dismiss).

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

/// Pure decision core — the tested machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DismissState {
    armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissInput {
    PointerDown { inside: bool },
    Escape,
}

impl DismissState {
    pub fn armed() -> Self {
        Self { armed: true }
    }

    /// Returns `true` exactly when the dismiss callback must fire.
    pub fn decide(&mut self, input: DismissInput) -> bool {
        if !self.armed {
            return false;
        }
        match input {
            DismissInput::PointerDown { inside: true } => false,
            DismissInput::PointerDown { inside: false } | DismissInput::Escape => {
                self.armed = false;
                true
            }
        }
    }

    pub fn rearm(&mut self) {
        self.armed = true;
    }
}

/// DOM adapter: document-level `pointerdown` + `keydown` listeners feeding
/// the machine. Dropping the guard removes both listeners.
pub struct DismissGuard {
    document: web_sys::Document,
    pointer: Closure<dyn FnMut(web_sys::Event)>,
    key: Closure<dyn FnMut(web_sys::Event)>,
}

impl DismissGuard {
    /// `protected`: subtrees that never count as "outside". `on_dismiss` is
    /// called at most once per armed period.
    pub fn install(
        document: &web_sys::Document,
        protected: Vec<web_sys::Node>,
        on_dismiss: impl Fn() + 'static,
    ) -> Self {
        let state = Rc::new(RefCell::new(DismissState::armed()));
        let on_dismiss = Rc::new(on_dismiss);

        let pointer = {
            let state = Rc::clone(&state);
            let on_dismiss = Rc::clone(&on_dismiss);
            Closure::wrap(Box::new(move |event: web_sys::Event| {
                let inside = event
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                    .is_some_and(|target| protected.iter().any(|node| node.contains(Some(&target))));
                if state.borrow_mut().decide(DismissInput::PointerDown { inside }) {
                    on_dismiss();
                }
            }) as Box<dyn FnMut(web_sys::Event)>)
        };

        let key = {
            let state = Rc::clone(&state);
            let on_dismiss = Rc::clone(&on_dismiss);
            Closure::wrap(Box::new(move |event: web_sys::Event| {
                let is_escape = event
                    .dyn_ref::<web_sys::KeyboardEvent>()
                    .is_some_and(|k| k.key() == "Escape");
                if is_escape && state.borrow_mut().decide(DismissInput::Escape) {
                    on_dismiss();
                }
            }) as Box<dyn FnMut(web_sys::Event)>)
        };

        let _ = document
            .add_event_listener_with_callback("pointerdown", pointer.as_ref().unchecked_ref());
        let _ = document.add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref());

        Self { document: document.clone(), pointer, key }
    }
}

impl Drop for DismissGuard {
    fn drop(&mut self) {
        let _ = self
            .document
            .remove_event_listener_with_callback("pointerdown", self.pointer.as_ref().unchecked_ref());
        let _ = self
            .document
            .remove_event_listener_with_callback("keydown", self.key.as_ref().unchecked_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_fires_once_and_disarms() {
        let mut s = DismissState::armed();
        assert!(s.decide(DismissInput::Escape));
        assert!(!s.decide(DismissInput::Escape));
        assert!(!s.decide(DismissInput::PointerDown { inside: false }));
    }

    #[test]
    fn outside_pointer_fires_inside_pointer_does_not() {
        let mut s = DismissState::armed();
        assert!(!s.decide(DismissInput::PointerDown { inside: true }));
        assert!(s.decide(DismissInput::PointerDown { inside: false }));
        assert!(!s.decide(DismissInput::PointerDown { inside: false }));
    }

    #[test]
    fn rearm_allows_exactly_one_more_fire() {
        let mut s = DismissState::armed();
        assert!(s.decide(DismissInput::Escape));
        s.rearm();
        assert!(!s.decide(DismissInput::PointerDown { inside: true }));
        assert!(s.decide(DismissInput::PointerDown { inside: false }));
        assert!(!s.decide(DismissInput::Escape));
    }
}
