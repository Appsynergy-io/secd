//! Roving tabindex — one tab stop for a whole widget; arrow keys move the
//! active item. Replaces `@radix-ui/react-roving-focus` (menus, tabs, radio
//! groups, toolbars). APG pattern: "Keyboard Navigation Inside Components
//! Using a Roving tabindex".
//!
//! # Spec
//!
//! State: the active index `i ∈ [0, len)`. The active item carries
//! `tabindex="0"`, every other item `tabindex="-1"`; moving focuses the new
//! active item.
//!
//! # Keyboard map
//!
//! | Key        | Orientation        | Effect                          |
//! |------------|--------------------|---------------------------------|
//! | ArrowRight | Horizontal / Both  | next item                       |
//! | ArrowLeft  | Horizontal / Both  | previous item                   |
//! | ArrowDown  | Vertical / Both    | next item                       |
//! | ArrowUp    | Vertical / Both    | previous item                   |
//! | Home       | any                | first item                      |
//! | End        | any                | last item                       |
//!
//! Arrow keys off-axis for the orientation are ignored. `looped` wraps
//! next/prev at the edges; unlooped clamps.

use wasm_bindgen::JsCast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    Next,
    Prev,
    First,
    Last,
}

/// Map a DOM `KeyboardEvent::key` to a navigation action for `orientation`.
pub fn nav_key(key: &str, orientation: Orientation) -> Option<NavKey> {
    use Orientation::*;
    match (key, orientation) {
        ("ArrowRight", Horizontal | Both) => Some(NavKey::Next),
        ("ArrowLeft", Horizontal | Both) => Some(NavKey::Prev),
        ("ArrowDown", Vertical | Both) => Some(NavKey::Next),
        ("ArrowUp", Vertical | Both) => Some(NavKey::Prev),
        ("Home", _) => Some(NavKey::First),
        ("End", _) => Some(NavKey::Last),
        _ => None,
    }
}

/// Pure index movement — the tested machine. `len` must be > 0.
pub fn move_index(current: usize, len: usize, key: NavKey, looped: bool) -> usize {
    debug_assert!(len > 0, "invariant: move_index needs a non-empty list");
    let last = len - 1;
    match key {
        NavKey::First => 0,
        NavKey::Last => last,
        NavKey::Next => {
            if current >= last {
                if looped { 0 } else { last }
            } else {
                current + 1
            }
        }
        NavKey::Prev => {
            if current == 0 {
                if looped { last } else { 0 }
            } else {
                current - 1
            }
        }
    }
}

/// DOM adapter: stamp `tabindex` over `items` for `active` and focus it.
pub fn apply(items: &[web_sys::Element], active: usize) {
    for (i, item) in items.iter().enumerate() {
        let _ = item.set_attribute("tabindex", if i == active { "0" } else { "-1" });
    }
    if let Some(el) = items.get(active).and_then(|e| e.dyn_ref::<web_sys::HtmlElement>()) {
        let _ = el.focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_respect_orientation() {
        assert_eq!(nav_key("ArrowRight", Orientation::Horizontal), Some(NavKey::Next));
        assert_eq!(nav_key("ArrowRight", Orientation::Vertical), None);
        assert_eq!(nav_key("ArrowDown", Orientation::Vertical), Some(NavKey::Next));
        assert_eq!(nav_key("ArrowDown", Orientation::Horizontal), None);
        assert_eq!(nav_key("ArrowUp", Orientation::Both), Some(NavKey::Prev));
        assert_eq!(nav_key("Enter", Orientation::Both), None);
    }

    #[test]
    fn home_end_ignore_orientation() {
        for o in [Orientation::Horizontal, Orientation::Vertical, Orientation::Both] {
            assert_eq!(nav_key("Home", o), Some(NavKey::First));
            assert_eq!(nav_key("End", o), Some(NavKey::Last));
        }
    }

    #[test]
    fn looped_wraps_at_both_edges() {
        assert_eq!(move_index(2, 3, NavKey::Next, true), 0);
        assert_eq!(move_index(0, 3, NavKey::Prev, true), 2);
    }

    #[test]
    fn unlooped_clamps_at_both_edges() {
        assert_eq!(move_index(2, 3, NavKey::Next, false), 2);
        assert_eq!(move_index(0, 3, NavKey::Prev, false), 0);
    }

    #[test]
    fn interior_moves_and_jumps() {
        assert_eq!(move_index(1, 5, NavKey::Next, false), 2);
        assert_eq!(move_index(3, 5, NavKey::Prev, false), 2);
        assert_eq!(move_index(3, 5, NavKey::First, false), 0);
        assert_eq!(move_index(1, 5, NavKey::Last, true), 4);
    }
}
