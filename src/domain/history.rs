//! The calculation history and its replay logic.
//!
//! Each entry keeps the raw input plus the last computed value (or error). The
//! `ans` of a line is the value of the line above it, so editing or deleting a
//! line means re-evaluating everything below: [`History::recompute_from`] walks
//! the tail, threading each value into the next line's `ans`. The actual
//! evaluation is supplied by the caller, keeping this module free of the engine
//! and variable store.

use crate::domain::quantity::Quantity;

/// One line of history: what was typed and what it evaluated to.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// The raw input as typed by the user.
    pub input: String,
    /// The computed value (a quantity), or `None` for a note or an error.
    pub value: Option<Quantity>,
    /// The error message, or `None` when the line succeeded.
    pub error: Option<String>,
}

impl HistoryEntry {
    /// A successfully evaluated entry.
    pub fn evaluated(input: String, value: Quantity) -> Self {
        HistoryEntry {
            input,
            value: Some(value),
            error: None,
        }
    }

    /// An entry that failed to evaluate.
    pub fn failed(input: String, error: String) -> Self {
        HistoryEntry {
            input,
            value: None,
            error: Some(error),
        }
    }
}

/// The outcome of evaluating one line: a value (quantity) or an error message.
pub type LineResult = (Option<Quantity>, Option<String>);

/// An ordered list of history entries, capped at `max_len` (oldest dropped).
#[derive(Debug, Clone, Default)]
pub struct History {
    entries: Vec<HistoryEntry>,
    max_len: usize,
}

impl History {
    /// Creates an empty history holding at most `max_len` entries (at least 1).
    pub fn new(max_len: usize) -> Self {
        History {
            entries: Vec::new(),
            max_len: max_len.max(1),
        }
    }

    /// Builds a history from loaded entries, trimming to `max_len`.
    pub fn from_entries(entries: Vec<HistoryEntry>, max_len: usize) -> Self {
        let mut history = History::new(max_len);
        history.entries = entries;
        history.trim_to_cap();
        history
    }

    /// The entries, oldest first.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The most recent computed value, i.e. the `ans` for the next input.
    ///
    /// Lines without a value (notes or errors) are skipped, so a comment-only
    /// note never breaks the `ans` chain.
    pub fn last_value(&self) -> Option<Quantity> {
        self.entries
            .iter()
            .rev()
            .find_map(|entry| entry.value.clone())
    }

    /// Appends `entry`, dropping the oldest entry when over capacity.
    pub fn push(&mut self, entry: HistoryEntry) {
        self.entries.push(entry);
        self.trim_to_cap();
    }

    /// Replaces the input of the entry at `index`; a no-op when out of range.
    pub fn set_input(&mut self, index: usize, input: String) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.input = input;
        }
    }

    /// Removes the entry at `index`; a no-op when out of range.
    pub fn remove(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    /// Swaps the entries at `a` and `b`; a no-op when either is out of range.
    pub fn swap(&mut self, a: usize, b: usize) {
        let len = self.entries.len();
        if a < len && b < len {
            self.entries.swap(a, b);
        }
    }

    /// Inserts `entry` at `index` (clamped to the end), dropping the oldest
    /// entry when this exceeds capacity.
    pub fn insert(&mut self, index: usize, entry: HistoryEntry) {
        let index = index.min(self.entries.len());
        self.entries.insert(index, entry);
        self.trim_to_cap();
    }

    /// Removes every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Re-evaluates entries from `start` to the end, threading the most recent
    /// computed value into the next line's `ans`.
    ///
    /// `evaluate` receives the line's input and its `ans` (the most recent value
    /// produced above it, or `None` when there is none) and returns the new
    /// value/error. Lines without a value (notes or errors) leave `ans`
    /// unchanged, so they never break the chain. It is called in order, so a
    /// caller mutating a shared variable store sees the effect of earlier
    /// assignments on later lines.
    pub fn recompute_from<F>(&mut self, start: usize, mut evaluate: F)
    where
        F: FnMut(&str, Option<Quantity>) -> LineResult,
    {
        let mut running = self.entries[..start]
            .iter()
            .rev()
            .find_map(|e| e.value.clone());
        for index in start..self.entries.len() {
            let input = self.entries[index].input.clone();
            let (value, error) = evaluate(&input, running.clone());
            if value.is_some() {
                running = value.clone();
            }
            let entry = &mut self.entries[index];
            entry.value = value;
            entry.error = error;
        }
    }

    /// Drops the oldest entries until the cap is met.
    fn trim_to_cap(&mut self) {
        if self.entries.len() > self.max_len {
            let excess = self.entries.len() - self.max_len;
            self.entries.drain(0..excess);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(value: f64) -> Quantity {
        Quantity::dimensionless(value)
    }

    /// The display values of the entries, for assertions.
    fn values(history: &History) -> Vec<Option<f64>> {
        history
            .entries()
            .iter()
            .map(|e| e.value.as_ref().map(Quantity::display_value))
            .collect()
    }

    /// An evaluator that ignores the input and returns `ans + 1`, so the value
    /// chain becomes 1, 2, 3, ... - enough to test `ans` threading.
    fn increment(_input: &str, ans: Option<Quantity>) -> LineResult {
        let previous = ans.map_or(0.0, |a| a.display_value());
        (Some(q(previous + 1.0)), None)
    }

    fn history_of(inputs: &[&str]) -> History {
        let entries = inputs
            .iter()
            .map(|input| HistoryEntry::evaluated(input.to_string(), q(0.0)))
            .collect();
        History::from_entries(entries, 100)
    }

    #[test]
    fn recompute_threads_ans_through_the_chain() {
        let mut history = history_of(&["a", "b", "c"]);
        history.recompute_from(0, increment);
        assert_eq!(values(&history), vec![Some(1.0), Some(2.0), Some(3.0)]);
        assert_eq!(history.last_value().map(|a| a.display_value()), Some(3.0));
    }

    #[test]
    fn recompute_skips_value_less_lines_when_threading_ans() {
        fn maybe_increment(input: &str, ans: Option<Quantity>) -> LineResult {
            if input == "note" {
                (None, None)
            } else {
                let previous = ans.map_or(0.0, |a| a.display_value());
                (Some(q(previous + 1.0)), None)
            }
        }
        let mut history = history_of(&["a", "note", "b"]);
        history.recompute_from(0, maybe_increment);
        // The note has no value and does not break the chain (`b` sees `a`).
        assert_eq!(values(&history), vec![Some(1.0), None, Some(2.0)]);
    }

    #[test]
    fn recompute_from_the_middle_uses_the_prior_value() {
        let mut history = history_of(&["a", "b", "c"]);
        history.set_input(0, "a".to_string());
        // Seed the first value, then recompute only the tail.
        history.recompute_from(0, increment);
        // Pretend the first entry was edited to evaluate to 10.
        history.entries[0].value = Some(q(10.0));
        history.recompute_from(1, increment);
        assert_eq!(values(&history), vec![Some(10.0), Some(11.0), Some(12.0)]);
    }

    #[test]
    fn push_drops_oldest_beyond_capacity() {
        let mut history = History::new(2);
        history.push(HistoryEntry::evaluated("1".to_string(), q(1.0)));
        history.push(HistoryEntry::evaluated("2".to_string(), q(2.0)));
        history.push(HistoryEntry::evaluated("3".to_string(), q(3.0)));
        let inputs: Vec<&str> =
            history.entries().iter().map(|e| e.input.as_str()).collect();
        assert_eq!(inputs, vec!["2", "3"]);
    }

    #[test]
    fn swap_and_insert_reorder_entries() {
        let mut history = history_of(&["a", "b", "c"]);
        history.swap(0, 2);
        let inputs: Vec<&str> =
            history.entries().iter().map(|e| e.input.as_str()).collect();
        assert_eq!(inputs, vec!["c", "b", "a"]);

        history.insert(1, HistoryEntry::evaluated("x".to_string(), q(0.0)));
        let inputs: Vec<&str> =
            history.entries().iter().map(|e| e.input.as_str()).collect();
        assert_eq!(inputs, vec!["c", "x", "b", "a"]);

        // Out-of-range swap is a no-op; insert past the end clamps.
        history.swap(0, 99);
        history.insert(99, HistoryEntry::evaluated("end".to_string(), q(0.0)));
        assert_eq!(history.entries().last().unwrap().input, "end");
    }

    #[test]
    fn remove_is_bounds_checked() {
        let mut history = history_of(&["a", "b"]);
        history.remove(5);
        assert_eq!(history.len(), 2);
        history.remove(0);
        assert_eq!(history.entries()[0].input, "b");
    }
}
