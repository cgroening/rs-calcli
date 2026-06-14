//! The calculation history and its replay logic.
//!
//! Each entry keeps the raw input plus the last computed value (or error). The
//! `ans` of a line is the value of the line above it, so editing or deleting a
//! line means re-evaluating everything below: [`History::recompute_from`] walks
//! the tail, threading each value into the next line's `ans`. The actual
//! evaluation is supplied by the caller, keeping this module free of the engine
//! and variable store.

/// One line of history: what was typed and what it evaluated to.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// The raw input as typed by the user.
    pub input: String,
    /// The computed value, or `None` when the line errored.
    pub value: Option<f64>,
    /// The error message, or `None` when the line succeeded.
    pub error: Option<String>,
}

impl HistoryEntry {
    /// A successfully evaluated entry.
    pub fn evaluated(input: String, value: f64) -> Self {
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

/// The outcome of evaluating one line: a value or an error message.
pub type LineResult = (Option<f64>, Option<String>);

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

    /// The value of the most recent entry, i.e. the `ans` for the next input.
    pub fn last_value(&self) -> Option<f64> {
        self.entries.last().and_then(|entry| entry.value)
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

    /// Removes every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Re-evaluates entries from `start` to the end, threading each value into
    /// the next line's `ans`.
    ///
    /// `evaluate` receives the line's input and its `ans` (the previous line's
    /// value, or `None` for the first line) and returns the new value/error. It
    /// is called in order, so a caller mutating a shared variable store sees the
    /// effect of earlier assignments on later lines.
    pub fn recompute_from<F>(&mut self, start: usize, mut evaluate: F)
    where
        F: FnMut(&str, Option<f64>) -> LineResult,
    {
        for index in start..self.entries.len() {
            let ans = if index == 0 {
                None
            } else {
                self.entries[index - 1].value
            };
            let input = self.entries[index].input.clone();
            let (value, error) = evaluate(&input, ans);
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

    /// An evaluator that ignores the input and returns `ans + 1`, so the value
    /// chain becomes 1, 2, 3, ... — enough to test `ans` threading.
    fn increment(_input: &str, ans: Option<f64>) -> LineResult {
        (Some(ans.unwrap_or(0.0) + 1.0), None)
    }

    fn history_of(inputs: &[&str]) -> History {
        let entries = inputs
            .iter()
            .map(|input| HistoryEntry::evaluated(input.to_string(), 0.0))
            .collect();
        History::from_entries(entries, 100)
    }

    #[test]
    fn recompute_threads_ans_through_the_chain() {
        let mut history = history_of(&["a", "b", "c"]);
        history.recompute_from(0, increment);
        let values: Vec<Option<f64>> =
            history.entries().iter().map(|e| e.value).collect();
        assert_eq!(values, vec![Some(1.0), Some(2.0), Some(3.0)]);
        assert_eq!(history.last_value(), Some(3.0));
    }

    #[test]
    fn recompute_from_the_middle_uses_the_prior_value() {
        let mut history = history_of(&["a", "b", "c"]);
        history.set_input(0, "a".to_string());
        // Seed the first value, then recompute only the tail.
        history.recompute_from(0, increment);
        // Pretend the first entry was edited to evaluate to 10.
        history.entries[0].value = Some(10.0);
        history.recompute_from(1, increment);
        let values: Vec<Option<f64>> =
            history.entries().iter().map(|e| e.value).collect();
        assert_eq!(values, vec![Some(10.0), Some(11.0), Some(12.0)]);
    }

    #[test]
    fn push_drops_oldest_beyond_capacity() {
        let mut history = History::new(2);
        history.push(HistoryEntry::evaluated("1".to_string(), 1.0));
        history.push(HistoryEntry::evaluated("2".to_string(), 2.0));
        history.push(HistoryEntry::evaluated("3".to_string(), 3.0));
        let inputs: Vec<&str> =
            history.entries().iter().map(|e| e.input.as_str()).collect();
        assert_eq!(inputs, vec!["2", "3"]);
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
