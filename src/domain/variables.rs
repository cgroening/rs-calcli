//! The user's named variables.
//!
//! A thin wrapper over a sorted map so the variables view lists names in a
//! stable order and persistence round-trips deterministically. Values are full
//! `f64`s, like everything the calculator stores.

use std::collections::BTreeMap;

/// A set of named variables and their values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VariableStore {
    values: BTreeMap<String, f64>,
}

impl VariableStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        VariableStore::default()
    }

    /// Builds a store from existing name/value pairs (e.g. loaded state).
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, f64)>) -> Self {
        VariableStore {
            values: pairs.into_iter().collect(),
        }
    }

    /// Sets `name` to `value`, overwriting any previous value.
    pub fn set(&mut self, name: &str, value: f64) {
        self.values.insert(name.to_string(), value);
    }

    /// The value of `name`, if defined.
    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }

    /// Removes `name`, returning its previous value if it existed.
    pub fn remove(&mut self, name: &str) -> Option<f64> {
        self.values.remove(name)
    }

    /// Removes every variable.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Whether no variables are defined.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The number of defined variables.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Iterates over the variables in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &f64)> {
        self.values.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_and_overwrite() {
        let mut store = VariableStore::new();
        store.set("x", 5.0);
        assert_eq!(store.get("x"), Some(5.0));
        store.set("x", 7.0);
        assert_eq!(store.get("x"), Some(7.0));
        assert_eq!(store.get("missing"), None);
    }

    #[test]
    fn remove_and_clear() {
        let mut store = VariableStore::from_pairs([
            ("a".to_string(), 1.0),
            ("b".to_string(), 2.0),
        ]);
        assert_eq!(store.remove("a"), Some(1.0));
        assert_eq!(store.remove("a"), None);
        assert_eq!(store.len(), 1);
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn iteration_is_sorted_by_name() {
        let store = VariableStore::from_pairs([
            ("gamma".to_string(), 3.0),
            ("alpha".to_string(), 1.0),
            ("beta".to_string(), 2.0),
        ]);
        let names: Vec<&String> = store.iter().map(|(name, _)| name).collect();
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);
    }
}
