//! Typeahead — jump to items by typing their label. Replaces Radix's
//! `useTypeaheadSearch` (Select, DropdownMenu). APG pattern: Listbox /
//! Menu "Type-ahead" behavior.
//!
//! # Spec
//!
//! State: the search buffer + timestamp of the last keystroke. Time is
//! injected (`now_ms`) so the machine is deterministic and testable.
//!
//! Rules (Radix semantics, reproduced exactly):
//! - A keystroke ≥ 1000 ms after the previous one clears the buffer first.
//! - Candidates are the labels wrapped to start at the current item, current
//!   first — so a growing prefix keeps the current item selected while it
//!   still matches.
//! - If the buffer is one character repeated (e.g. `"aaa"`), search with the
//!   single character and exclude the current item — repeated presses of the
//!   same letter cycle through items starting with it.
//! - Matching is case-insensitive prefix match; first hit wins.
//!
//! # Keyboard map
//!
//! | Input                | Effect                                    |
//! |----------------------|-------------------------------------------|
//! | printable character  | append to buffer, jump to first match     |
//! | (1 s of silence)     | buffer resets                             |

pub const TIMEOUT_MS: f64 = 1000.0;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Typeahead {
    buffer: String,
    last_ms: Option<f64>,
}

impl Typeahead {
    /// Feed one printable character at time `now_ms`; returns the index in
    /// `labels` to move to, if any.
    pub fn on_char(
        &mut self,
        ch: char,
        now_ms: f64,
        labels: &[&str],
        current: Option<usize>,
    ) -> Option<usize> {
        if self.last_ms.is_some_and(|last| now_ms - last >= TIMEOUT_MS) {
            self.buffer.clear();
        }
        self.buffer.push(ch);
        self.last_ms = Some(now_ms);

        let mut chars = self.buffer.chars();
        let first = chars.next().expect("invariant: buffer just received a char");
        let repeated = self.buffer.chars().count() > 1 && self.buffer.chars().all(|c| c == first);
        let search: String = if repeated {
            first.to_lowercase().collect()
        } else {
            self.buffer.to_lowercase()
        };

        let len = labels.len();
        if len == 0 {
            return None;
        }
        let start = current.unwrap_or(0);
        // Wrapped order beginning at the current item (current first).
        let order = (0..len).map(|k| (start + k) % len);
        for i in order {
            if repeated && Some(i) == current {
                continue;
            }
            if labels[i].to_lowercase().starts_with(&search) {
                return Some(i);
            }
        }
        None
    }

    /// Clear the buffer (e.g. when the widget closes).
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.last_ms = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LABELS: &[&str] = &["Alpha", "Amber", "Bravo", "Beta", "Gamma"];

    #[test]
    fn prefix_match_is_case_insensitive_and_first_hit_wins() {
        let mut t = Typeahead::default();
        assert_eq!(t.on_char('b', 0.0, LABELS, None), Some(2)); // Bravo
    }

    #[test]
    fn growing_prefix_keeps_current_while_it_matches() {
        let mut t = Typeahead::default();
        assert_eq!(t.on_char('a', 0.0, LABELS, None), Some(0)); // Alpha
        assert_eq!(t.on_char('m', 100.0, LABELS, Some(0)), Some(1)); // "am" → Amber
        assert_eq!(t.on_char('b', 200.0, LABELS, Some(1)), Some(1)); // "amb" still Amber
    }

    #[test]
    fn repeated_char_cycles_past_the_current_item() {
        let mut t = Typeahead::default();
        assert_eq!(t.on_char('a', 0.0, LABELS, None), Some(0)); // Alpha
        assert_eq!(t.on_char('a', 100.0, LABELS, Some(0)), Some(1)); // Amber
        assert_eq!(t.on_char('a', 200.0, LABELS, Some(1)), Some(0)); // wraps to Alpha
    }

    #[test]
    fn silence_resets_the_buffer() {
        let mut t = Typeahead::default();
        assert_eq!(t.on_char('b', 0.0, LABELS, None), Some(2)); // Bravo
        assert_eq!(t.on_char('e', 100.0, LABELS, Some(2)), Some(3)); // "be" → Beta
        // ≥ 1s later: fresh buffer, "g" not "beg"
        assert_eq!(t.on_char('g', 1200.0, LABELS, Some(3)), Some(4)); // Gamma
    }

    #[test]
    fn wrapped_search_starts_at_current() {
        let mut t = Typeahead::default();
        // From Gamma, "b" wraps and finds Bravo (index 2) before Beta.
        assert_eq!(t.on_char('b', 0.0, LABELS, Some(4)), Some(2));
    }

    #[test]
    fn no_match_and_empty_lists_return_none() {
        let mut t = Typeahead::default();
        assert_eq!(t.on_char('z', 0.0, LABELS, None), None);
        let mut t2 = Typeahead::default();
        assert_eq!(t2.on_char('a', 0.0, &[], None), None);
    }
}
