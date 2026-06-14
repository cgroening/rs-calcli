//! A physical quantity: a value together with an optional unit.
//!
//! The value is stored in the coherent SI unit of its dimension; the `display`
//! unit only affects how it is shown and what [`convert_to`](Quantity::convert_to)
//! produces. A quantity with no unit is dimensionless and behaves like a plain
//! number, so the rest of the calculator treats `f64` results as dimensionless
//! quantities.

use crate::domain::units::{self, Dimension, Unit};

/// A value in SI base units plus the unit it should be displayed in.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    /// The value in the coherent SI unit of `dim`.
    value: f64,
    /// The dimension of the value.
    dim: Dimension,
    /// The unit to display in, or `None` when dimensionless.
    display: Option<Unit>,
}

impl Quantity {
    /// A dimensionless quantity (a plain number).
    pub fn dimensionless(value: f64) -> Self {
        Quantity {
            value,
            dim: Dimension::ZERO,
            display: None,
        }
    }

    /// Reconstructs a quantity from a persisted display value and unit symbol.
    /// An unknown unit falls back to a dimensionless value.
    pub fn from_persisted(value: f64, unit: Option<&str>) -> Self {
        match unit.and_then(units::parse) {
            Some(unit) => Quantity::new(value, unit),
            None => Quantity::dimensionless(value),
        }
    }

    /// A quantity of `value` in `unit` (stored converted to SI base).
    pub fn new(value: f64, unit: Unit) -> Self {
        Quantity {
            value: value * unit.factor + unit.offset,
            dim: unit.dim,
            display: Some(unit),
        }
    }

    /// Whether this quantity carries no unit.
    pub fn is_dimensionless(&self) -> bool {
        self.display.is_none()
    }

    /// The value expressed in the display unit (the number shown to the user).
    pub fn display_value(&self) -> f64 {
        match &self.display {
            Some(unit) => (self.value - unit.offset) / unit.factor,
            None => self.value,
        }
    }

    /// The display unit's symbol, or `None` when dimensionless.
    pub fn unit_symbol(&self) -> Option<&str> {
        self.display.as_ref().map(|unit| unit.symbol.as_str())
    }

    /// Converts to `target`, keeping the same physical value.
    ///
    /// # Errors
    /// Returns a message when `target` has a different dimension.
    pub fn convert_to(&self, target: Unit) -> Result<Quantity, String> {
        if self.dim != target.dim {
            return Err(format!(
                "cannot convert {} to {}",
                self.unit_symbol().unwrap_or("a number"),
                target.symbol
            ));
        }
        Ok(Quantity {
            value: self.value,
            dim: self.dim,
            display: Some(target),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::units::parse;

    #[test]
    fn dimensionless_round_trips_a_plain_number() {
        let q = Quantity::dimensionless(42.0);
        assert!(q.is_dimensionless());
        assert_eq!(q.display_value(), 42.0);
        assert_eq!(q.unit_symbol(), None);
    }

    #[test]
    fn converts_between_compatible_units() {
        let q = Quantity::new(123.0, parse("MPa").unwrap());
        let bar = q.convert_to(parse("bar").unwrap()).unwrap();
        assert_eq!(bar.unit_symbol(), Some("bar"));
        assert!((bar.display_value() - 1230.0).abs() < 1e-6);
    }

    #[test]
    fn force_prefix_conversion() {
        let q = Quantity::new(50.0, parse("kN").unwrap());
        let newtons = q.convert_to(parse("N").unwrap()).unwrap();
        assert!((newtons.display_value() - 50_000.0).abs() < 1e-9);
    }

    #[test]
    fn temperature_offset_conversion() {
        let q = Quantity::new(100.0, parse("°C").unwrap());
        let kelvin = q.convert_to(parse("K").unwrap()).unwrap();
        assert!((kelvin.display_value() - 373.15).abs() < 1e-9);
        let fahrenheit = q.convert_to(parse("°F").unwrap()).unwrap();
        assert!((fahrenheit.display_value() - 212.0).abs() < 1e-6);
    }

    #[test]
    fn incompatible_dimensions_error() {
        let q = Quantity::new(1.0, parse("N").unwrap());
        assert!(q.convert_to(parse("bar").unwrap()).is_err());
    }
}
