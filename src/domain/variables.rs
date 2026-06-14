//! The user's named variables.
//!
//! A thin wrapper over a sorted map so the variables view lists names in a
//! stable order and persistence round-trips deterministically. Values are
//! [`Quantity`]s, so a variable can carry a unit (`x = 50 kN`).

use std::collections::BTreeMap;

use crate::domain::quantity::Quantity;

/// A set of named variables and their values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VariableStore {
    values: BTreeMap<String, Quantity>,
}

impl VariableStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        VariableStore::default()
    }

    /// Builds a store from existing name/value pairs (e.g. loaded state).
    pub fn from_pairs(
        pairs: impl IntoIterator<Item = (String, Quantity)>,
    ) -> Self {
        VariableStore {
            values: pairs.into_iter().collect(),
        }
    }

    /// Sets `name` to `value`, overwriting any previous value.
    pub fn set(&mut self, name: &str, value: Quantity) {
        self.values.insert(name.to_string(), value);
    }

    /// The value of `name`, if defined.
    pub fn get(&self, name: &str) -> Option<&Quantity> {
        self.values.get(name)
    }

    /// Removes `name`, returning its previous value if it existed.
    pub fn remove(&mut self, name: &str) -> Option<Quantity> {
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
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Quantity)> {
        self.values.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(value: f64) -> Quantity {
        Quantity::dimensionless(value)
    }

    #[test]
    fn set_get_and_overwrite() {
        let mut store = VariableStore::new();
        store.set("x", q(5.0));
        assert_eq!(store.get("x").map(Quantity::display_value), Some(5.0));
        store.set("x", q(7.0));
        assert_eq!(store.get("x").map(Quantity::display_value), Some(7.0));
        assert!(store.get("missing").is_none());
    }

    #[test]
    fn remove_and_clear() {
        let mut store = VariableStore::from_pairs([
            ("a".to_string(), q(1.0)),
            ("b".to_string(), q(2.0)),
        ]);
        assert!(store.remove("a").is_some());
        assert!(store.remove("a").is_none());
        assert_eq!(store.len(), 1);
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn iteration_is_sorted_by_name() {
        let store = VariableStore::from_pairs([
            ("gamma".to_string(), q(3.0)),
            ("alpha".to_string(), q(1.0)),
            ("beta".to_string(), q(2.0)),
        ]);
        let names: Vec<&String> = store.iter().map(|(name, _)| name).collect();
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);
    }
}
