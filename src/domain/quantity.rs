//! A physical quantity: a value together with an optional unit symbol.
//!
//! The value is the number shown to the user, paired with the unit it is
//! expressed in (e.g. `1230` and `"bar"`). A quantity with no unit is
//! dimensionless and behaves like a plain number, so the rest of the calculator
//! treats `f64` results as dimensionless quantities. All dimensional reasoning -
//! conversion, arithmetic, derived units - is done by [`crate::domain::units`]
//! (rink) when the expression is evaluated; this type just carries the result.

/// A value paired with the unit it is expressed in.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    /// The value, in the unit named by `unit`.
    value: f64,
    /// The unit symbol, or `None` when dimensionless.
    unit: Option<String>,
}

impl Quantity {
    /// A dimensionless quantity (a plain number).
    pub fn dimensionless(value: f64) -> Self {
        Quantity { value, unit: None }
    }

    /// A quantity of `value` expressed in `unit`.
    pub fn new(value: f64, unit: impl Into<String>) -> Self {
        Quantity {
            value,
            unit: Some(unit.into()),
        }
    }

    /// Reconstructs a quantity from a persisted value and unit symbol.
    pub fn from_persisted(value: f64, unit: Option<&str>) -> Self {
        match unit {
            Some(symbol) => Quantity::new(value, symbol),
            None => Quantity::dimensionless(value),
        }
    }

    /// Whether this quantity carries no unit.
    pub fn is_dimensionless(&self) -> bool {
        self.unit.is_none()
    }

    /// The value shown to the user (in the display unit).
    pub fn display_value(&self) -> f64 {
        self.value
    }

    /// The unit symbol, or `None` when dimensionless.
    pub fn unit_symbol(&self) -> Option<&str> {
        self.unit.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensionless_round_trips_a_plain_number() {
        let q = Quantity::dimensionless(42.0);
        assert!(q.is_dimensionless());
        assert_eq!(q.display_value(), 42.0);
        assert_eq!(q.unit_symbol(), None);
    }

    #[test]
    fn a_unit_quantity_keeps_its_value_and_symbol() {
        let q = Quantity::new(1230.0, "bar");
        assert!(!q.is_dimensionless());
        assert_eq!(q.display_value(), 1230.0);
        assert_eq!(q.unit_symbol(), Some("bar"));
    }

    #[test]
    fn from_persisted_reconstructs_with_and_without_a_unit() {
        let with = Quantity::from_persisted(50.0, Some("kN"));
        assert_eq!(with.unit_symbol(), Some("kN"));
        assert_eq!(with.display_value(), 50.0);
        let without = Quantity::from_persisted(7.0, None);
        assert!(without.is_dimensionless());
    }
}
