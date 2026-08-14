//! Focus trap — keep Tab focus cycling inside a container while active;
//! restore focus to the previously focused element on deactivation.
//! Replaces `@radix-ui/react-focus-scope` for modal overlays.
//! APG pattern: Dialog (Modal) keyboard behavior.
//!
//! # Spec (state machine)
//!
//! States: `Inactive` · `Active` (holds the element focused before
//! activation).
//!
//! | State    | Event        | Next     | Effect                      |
//! |----------|--------------|----------|-----------------------------|
//! | Inactive | `Activate`   | Active   | `SaveFocus` + `FocusFirst`  |
//! | Active   | `Deactivate` | Inactive | `RestoreFocus`              |
//! | Active   | `Activate`   | Active   | none                        |
//! | Inactive | `Deactivate` | Inactive | none                        |
//!
//! # Keyboard map (while Active)
//!
//! | Key         | Effect                                              |
//! |-------------|-----------------------------------------------------|
//! | Tab         | focus next tabbable in the container; wraps to first|
//! | Shift+Tab   | focus previous tabbable; wraps to last              |
//!
//! Wrap logic (`next_focus`): with no tabbables focus stays on the
//! container; with focus outside the list, Tab goes to the first and
//! Shift+Tab to the last tabbable.

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapKey {
    Tab,
    ShiftTab,
}

/// Pure wrap logic — the tested machine. `current` = index of the currently
/// focused element in the tabbable list (`None` if focus is elsewhere).
pub fn next_focus(current: Option<usize>, len: usize, key: TrapKey) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match (key, current) {
        (TrapKey::Tab, None) => 0,
        (TrapKey::ShiftTab, None) => len - 1,
        (TrapKey::Tab, Some(i)) => (i + 1) % len,
        (TrapKey::ShiftTab, Some(i)) => (i + len - 1) % len,
    })
}

/// Tabbable elements inside `container`, in DOM order. The selector is the
/// standard interactive set; the fidelity keyboard trace against the
/// reference is the final arbiter per overlay.
pub const TABBABLE_SELECTOR: &str = "a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])";

pub fn tabbables(container: &web_sys::Element) -> Vec<web_sys::HtmlElement> {
    let Ok(list) = container.query_selector_all(TABBABLE_SELECTOR) else {
        return Vec::new();
    };
    (0..list.length())
        .filter_map(|i| list.get(i))
        .filter_map(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        .collect()
}

/// DOM adapter. On install: saves the currently focused element and focuses
/// the first tabbable (or the container). While alive: intercepts Tab /
/// Shift+Tab on the container and cycles per `next_focus`. On drop: removes
/// the listener and restores focus to the saved element.
pub struct FocusTrapGuard {
    container: web_sys::Element,
    key: Closure<dyn FnMut(web_sys::Event)>,
    previous: Option<web_sys::HtmlElement>,
}

impl FocusTrapGuard {
    pub fn install(document: &web_sys::Document, container: &web_sys::Element) -> Self {
        let previous = document
            .active_element()
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());

        match tabbables(container).first() {
            Some(first) => {
                let _ = first.focus();
            }
            None => {
                if let Ok(el) = container.clone().dyn_into::<web_sys::HtmlElement>() {
                    let _ = el.focus();
                }
            }
        }

        let key = {
            let container = container.clone();
            let document = document.clone();
            Closure::wrap(Box::new(move |event: web_sys::Event| {
                let Some(k) = event.dyn_ref::<web_sys::KeyboardEvent>() else { return };
                if k.key() != "Tab" {
                    return;
                }
                let key = if k.shift_key() { TrapKey::ShiftTab } else { TrapKey::Tab };
                let items = tabbables(&container);
                let active = document.active_element();
                let current = active.as_ref().and_then(|a| {
                    items.iter().position(|el| {
                        let el: &web_sys::Element = el.as_ref();
                        el == a
                    })
                });
                event.prevent_default();
                if let Some(next) = next_focus(current, items.len(), key) {
                    let _ = items[next].focus();
                }
            }) as Box<dyn FnMut(web_sys::Event)>)
        };
        let _ = container.add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref());

        Self { container: container.clone(), key, previous }
    }
}

impl Drop for FocusTrapGuard {
    fn drop(&mut self) {
        let _ = self
            .container
            .remove_event_listener_with_callback("keydown", self.key.as_ref().unchecked_ref());
        if let Some(prev) = self.previous.take() {
            let _ = prev.focus();
        }
    }
}

/// Style a focus-guard span via CSSOM. Same computed inline style as Radix's
/// literal `style` attribute, but property writes are exempt from a
/// `style-src` policy that blocks attributes (no `'unsafe-hashes'`).
pub(crate) fn style_guard(span: &web_sys::Element) {
    use wasm_bindgen::JsCast;
    if let Some(el) = span.dyn_ref::<web_sys::HtmlElement>() {
        let style = el.style();
        let _ = style.set_property("outline", "none");
        let _ = style.set_property("opacity", "0");
        let _ = style.set_property("position", "fixed");
        let _ = style.set_property("pointer-events", "none");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_wraps_forward_at_the_end() {
        assert_eq!(next_focus(Some(0), 3, TrapKey::Tab), Some(1));
        assert_eq!(next_focus(Some(2), 3, TrapKey::Tab), Some(0));
    }

    #[test]
    fn shift_tab_wraps_backward_at_the_start() {
        assert_eq!(next_focus(Some(2), 3, TrapKey::ShiftTab), Some(1));
        assert_eq!(next_focus(Some(0), 3, TrapKey::ShiftTab), Some(2));
    }

    #[test]
    fn focus_outside_the_list_enters_at_the_edges() {
        assert_eq!(next_focus(None, 3, TrapKey::Tab), Some(0));
        assert_eq!(next_focus(None, 3, TrapKey::ShiftTab), Some(2));
    }

    #[test]
    fn empty_list_keeps_focus_on_container() {
        assert_eq!(next_focus(None, 0, TrapKey::Tab), None);
        assert_eq!(next_focus(Some(0), 0, TrapKey::ShiftTab), None);
    }

    #[test]
    fn single_item_always_focuses_itself() {
        assert_eq!(next_focus(Some(0), 1, TrapKey::Tab), Some(0));
        assert_eq!(next_focus(Some(0), 1, TrapKey::ShiftTab), Some(0));
    }
}
