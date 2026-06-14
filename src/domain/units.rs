//! Physical dimensions and a curated registry of engineering units.
//!
//! A [`Dimension`] is a vector of base-dimension exponents; a [`Unit`] maps a
//! symbol to its factor (and offset, for temperature) relative to the coherent
//! SI unit of its dimension. [`parse`] resolves a symbol, accepting an SI prefix
//! in front of a base unit (`MPa` = mega·Pa). Conversion lives in
//! [`crate::domain::quantity`].

use std::f64::consts::PI;

/// Base dimensions: length, mass, time, temperature, current, angle.
const DIMENSION_COUNT: usize = 6;

/// A physical dimension as exponents over the base dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimension {
    exponents: [i8; DIMENSION_COUNT],
}

impl Dimension {
    /// Builds a dimension from the six base exponents.
    pub const fn new(
        length: i8,
        mass: i8,
        time: i8,
        temperature: i8,
        current: i8,
        angle: i8,
    ) -> Self {
        Dimension {
            exponents: [length, mass, time, temperature, current, angle],
        }
    }

    /// The dimensionless dimension (all exponents zero).
    pub const ZERO: Dimension = Dimension::new(0, 0, 0, 0, 0, 0);

    /// Whether this is the dimensionless dimension.
    pub fn is_zero(&self) -> bool {
        self.exponents == [0; DIMENSION_COUNT]
    }
}

const LENGTH: Dimension = Dimension::new(1, 0, 0, 0, 0, 0);
const MASS: Dimension = Dimension::new(0, 1, 0, 0, 0, 0);
const TIME: Dimension = Dimension::new(0, 0, 1, 0, 0, 0);
const TEMPERATURE: Dimension = Dimension::new(0, 0, 0, 1, 0, 0);
const ANGLE: Dimension = Dimension::new(0, 0, 0, 0, 0, 1);
const FORCE: Dimension = Dimension::new(1, 1, -2, 0, 0, 0);
const PRESSURE: Dimension = Dimension::new(-1, 1, -2, 0, 0, 0);
const ENERGY: Dimension = Dimension::new(2, 1, -2, 0, 0, 0);
const POWER: Dimension = Dimension::new(2, 1, -3, 0, 0, 0);
const FREQUENCY: Dimension = Dimension::new(0, 0, -1, 0, 0, 0);
const AREA: Dimension = Dimension::new(2, 0, 0, 0, 0, 0);
const VOLUME: Dimension = Dimension::new(3, 0, 0, 0, 0, 0);

/// A resolved unit: its display symbol, its factor and offset relative to the
/// coherent SI unit of its dimension, and that dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// The symbol as written by the user (e.g. `MPa`).
    pub symbol: String,
    /// SI value = `unit_value * factor + offset`.
    pub factor: f64,
    /// Additive offset in the SI unit (non-zero only for `°C`/`°F`).
    pub offset: f64,
    /// The unit's dimension.
    pub dim: Dimension,
}

/// A base unit entry in the registry.
struct BaseUnit {
    symbol: &'static str,
    factor: f64,
    offset: f64,
    dim: Dimension,
}

/// The curated base units (the coherent SI unit of each dimension has factor 1).
const BASE_UNITS: &[BaseUnit] = &[
    base("m", 1.0, LENGTH),
    base("in", 0.0254, LENGTH),
    base("ft", 0.3048, LENGTH),
    base("yd", 0.9144, LENGTH),
    base("mi", 1609.344, LENGTH),
    base("g", 1e-3, MASS),
    base("t", 1000.0, MASS),
    base("lb", 0.45359237, MASS),
    base("oz", 0.028349523125, MASS),
    base("s", 1.0, TIME),
    base("min", 60.0, TIME),
    base("h", 3600.0, TIME),
    base("d", 86400.0, TIME),
    base("N", 1.0, FORCE),
    base("Pa", 1.0, PRESSURE),
    base("bar", 1e5, PRESSURE),
    base("psi", 6894.757293168, PRESSURE),
    base("atm", 101325.0, PRESSURE),
    base("J", 1.0, ENERGY),
    base("Wh", 3600.0, ENERGY),
    base("cal", 4.184, ENERGY),
    base("W", 1.0, POWER),
    base("hp", 745.699871582, POWER),
    base("K", 1.0, TEMPERATURE),
    offset_base("°C", 1.0, 273.15, TEMPERATURE),
    offset_base("degC", 1.0, 273.15, TEMPERATURE),
    offset_base("°F", 5.0 / 9.0, 255.372222222222, TEMPERATURE),
    offset_base("degF", 5.0 / 9.0, 255.372222222222, TEMPERATURE),
    base("Hz", 1.0, FREQUENCY),
    base("rad", 1.0, ANGLE),
    base("deg", PI / 180.0, ANGLE),
    base("°", PI / 180.0, ANGLE),
    base("gon", PI / 200.0, ANGLE),
    base("m2", 1.0, AREA),
    base("ha", 1e4, AREA),
    base("m3", 1.0, VOLUME),
    base("l", 1e-3, VOLUME),
];

/// Builds a base unit with no offset.
const fn base(symbol: &'static str, factor: f64, dim: Dimension) -> BaseUnit {
    BaseUnit {
        symbol,
        factor,
        offset: 0.0,
        dim,
    }
}

/// Builds a base unit with an additive offset (temperature).
const fn offset_base(
    symbol: &'static str,
    factor: f64,
    offset: f64,
    dim: Dimension,
) -> BaseUnit {
    BaseUnit {
        symbol,
        factor,
        offset,
        dim,
    }
}

/// SI prefixes accepted in front of a (non-offset) base unit.
const UNIT_PREFIXES: &[(&str, f64)] = &[
    ("T", 1e12),
    ("G", 1e9),
    ("M", 1e6),
    ("k", 1e3),
    ("h", 1e2),
    ("d", 1e-1),
    ("c", 1e-2),
    ("m", 1e-3),
    ("µ", 1e-6),
    ("u", 1e-6),
    ("n", 1e-9),
    ("p", 1e-12),
];

/// Resolves a unit symbol, accepting an SI prefix before a base unit. Whole
/// symbols win over prefix splits (`min` is a minute, not milli-inch).
pub fn parse(symbol: &str) -> Option<Unit> {
    if let Some(unit) = lookup_base(symbol) {
        return Some(unit);
    }
    for (prefix, factor) in UNIT_PREFIXES {
        let Some(rest) = symbol.strip_prefix(prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        if let Some(base) = lookup_base(rest)
            && base.offset == 0.0
        {
            return Some(Unit {
                symbol: symbol.to_string(),
                factor: factor * base.factor,
                offset: 0.0,
                dim: base.dim,
            });
        }
    }
    None
}

/// Whether `symbol` resolves to a known unit.
pub fn is_unit(symbol: &str) -> bool {
    parse(symbol).is_some()
}

/// Looks up an exact base symbol.
fn lookup_base(symbol: &str) -> Option<Unit> {
    BASE_UNITS
        .iter()
        .find(|u| u.symbol == symbol)
        .map(|u| Unit {
            symbol: symbol.to_string(),
            factor: u.factor,
            offset: u.offset,
            dim: u.dim,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_whole_symbols_and_prefixes() {
        assert_eq!(parse("Pa").unwrap().factor, 1.0);
        assert_eq!(parse("MPa").unwrap().factor, 1e6);
        assert_eq!(parse("kN").unwrap().factor, 1e3);
        assert_eq!(parse("kg").unwrap().factor, 1.0); // k * g(1e-3)
        // Whole symbols win over a prefix split.
        assert_eq!(parse("min").unwrap().factor, 60.0);
        assert_eq!(parse("m").unwrap().dim, LENGTH);
    }

    #[test]
    fn dimensions_distinguish_quantities() {
        assert_eq!(parse("bar").unwrap().dim, PRESSURE);
        assert_eq!(parse("MPa").unwrap().dim, PRESSURE);
        assert_ne!(parse("N").unwrap().dim, parse("Pa").unwrap().dim);
    }

    #[test]
    fn temperature_units_carry_an_offset_and_reject_prefixes() {
        let celsius = parse("°C").unwrap();
        assert_eq!(celsius.offset, 273.15);
        // A prefix on an offset unit is not accepted.
        assert!(parse("k°C").is_none());
    }

    #[test]
    fn unknown_symbols_do_not_parse() {
        assert!(parse("xyz").is_none());
        assert!(!is_unit("notaunit"));
    }
}
