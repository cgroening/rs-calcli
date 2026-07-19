//! The units engine: a thin wrapper over [`rink_core`].
//!
//! rink owns the unit database, dimensional analysis, conversion and arithmetic
//! with units; this module exposes just what calcli needs: [`eval`] to evaluate
//! a unit-bearing query into a `(value, unit)` pair, and [`is_unit`] to tell
//! the
//! router and the syntax highlighter whether a symbol names a unit.
//!
//! The rink [`Context`] is expensive to build (tens of milliseconds) and is not
//! thread-safe, so it is kept lazily in thread-local storage and reused. Unit
//! lookups are memoized, since whether a symbol is a unit never changes.

use std::cell::RefCell;
use std::collections::HashMap;

use rink_core::output::QueryReply;
use rink_core::{Context, eval as rink_eval, simple_context};

use crate::domain::errors::{AppError, Result};

thread_local! {
    /// The lazily-built rink context, reused across evaluations on this thread.
    static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
    /// Memoized unit lookups (`symbol -> is a unit`).
    static UNIT_CACHE: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());
}

/// Runs `query` through a thread-local rink context, returning the raw reply.
fn with_context<T>(f: impl FnOnce(&mut Context) -> T) -> T {
    CONTEXT.with(|cell| {
        let mut slot = cell.borrow_mut();
        let context = slot.get_or_insert_with(|| {
            simple_context().expect("the bundle-files feature is enabled")
        });
        f(context)
    })
}

/// Evaluates a unit-bearing `query` (a conversion, a quantity or unit
/// arithmetic), returning rink's numeric value and the unit name it chose
/// (`None` when the result is dimensionless).
///
/// Note the value is in SI *base* units, while the name may carry a prefix
/// (e.g. `(20300, "kilonewton")` for `20 kN + 300 N`); divide by [`scale_of`]
/// the name to express it in that unit. A conversion (`… -> X`) is the
/// exception: rink reports the value already in `X`.
///
/// # Errors
///
/// Returns [`AppError::Units`] when rink cannot parse the query, the units are
/// incompatible, or the result is not a plain number. rink's own one-line
/// wording is kept, since it names the offending unit.
///
/// # Examples
///
/// A conversion reports the value already in the target unit:
///
/// ```
/// use calcli::domain::units::eval;
///
/// let (value, _) = eval("3 m -> cm")?;
/// assert_eq!(value, 300.0);
/// # Ok::<(), calcli::domain::errors::AppError>(())
/// ```
///
/// Anything else comes back in SI base units, so `scale_of` the reported name
/// is what expresses it in that unit:
///
/// ```
/// use calcli::domain::units::{eval, scale_of};
///
/// let (base, unit) = eval("20 kN + 300 N")?;
/// let unit = unit.expect("a force carries a unit");
/// assert_eq!(base / scale_of(&unit)?, 20.3);
/// # Ok::<(), calcli::domain::errors::AppError>(())
/// ```
pub fn eval(query: &str) -> Result<(f64, Option<String>)> {
    let reply = with_context(|ctx| rink_eval(ctx, query))
        .map_err(|error| AppError::Units(first_line(&error.to_string())))?;
    extract(&reply)
        .ok_or_else(|| AppError::Units("not a numeric result".to_string()))
}

/// The SI base-unit magnitude of one `unit` (e.g. `1000` for `kN`/`kilonewton`,
/// `1` for `m/s`).
///
/// rink reports a result's [`eval`] value in SI base units but may name the
/// unit
/// with a prefix; dividing the value by this scale re-expresses it in the named
/// unit. Memoized indirectly through rink's context.
///
/// # Errors
///
/// Returns [`AppError::Units`] when `unit` is not a unit rink can evaluate.
///
/// # Examples
///
/// ```
/// use calcli::domain::units::scale_of;
///
/// assert_eq!(scale_of("kN")?, 1000.0);
/// assert_eq!(scale_of("m")?, 1.0);
/// # Ok::<(), calcli::domain::errors::AppError>(())
/// ```
pub fn scale_of(unit: &str) -> Result<f64> {
    let (value, _) = eval(&format!("1 {unit}"))?;
    Ok(value)
}

/// SI prefixes spelled out by rink, mapped to their symbol.
const PREFIX_SYMBOLS: &[(&str, &str)] = &[
    ("yotta", "Y"),
    ("zetta", "Z"),
    ("exa", "E"),
    ("peta", "P"),
    ("tera", "T"),
    ("giga", "G"),
    ("mega", "M"),
    ("kilo", "k"),
    ("hecto", "h"),
    ("deca", "da"),
    ("deci", "d"),
    ("centi", "c"),
    ("milli", "m"),
    ("micro", "\u{00b5}"),
    ("nano", "n"),
    ("pico", "p"),
    ("femto", "f"),
    ("atto", "a"),
];

/// Unit names spelled out by rink, mapped to their conventional symbol.
const BASE_SYMBOLS: &[(&str, &str)] = &[
    ("meter", "m"),
    ("gram", "g"),
    ("second", "s"),
    ("ampere", "A"),
    ("kelvin", "K"),
    ("mole", "mol"),
    ("candela", "cd"),
    ("newton", "N"),
    ("pascal", "Pa"),
    ("joule", "J"),
    ("watt", "W"),
    ("hertz", "Hz"),
    ("coulomb", "C"),
    ("volt", "V"),
    ("ohm", "\u{03a9}"),
    ("farad", "F"),
    ("siemens", "S"),
    ("weber", "Wb"),
    ("tesla", "T"),
    ("henry", "H"),
    ("lumen", "lm"),
    ("lux", "lx"),
    ("becquerel", "Bq"),
    ("gray", "Gy"),
    ("sievert", "Sv"),
    ("katal", "kat"),
    ("liter", "l"),
    ("tonne", "t"),
    ("bar", "bar"),
    ("radian", "rad"),
    ("steradian", "sr"),
    ("minute", "min"),
    ("hour", "h"),
    ("day", "d"),
];

/// Shortens a rink unit name to conventional symbols (`kilonewton` → `kN`,
/// `meter / second` → `m/s`, `kilonewton / meter^2` → `kN/m^2`).
///
/// The result stays parseable by rink (exponents keep `^n`, division uses `/`),
/// so a quantity carrying a shortened unit still round-trips through `ans` and
/// variables. Unrecognized names are left unchanged.
pub fn prettify_unit(name: &str) -> String {
    name.split(" / ")
        .map(prettify_factor)
        .collect::<Vec<_>>()
        .join("/")
}

/// Shortens one factor of a unit name (a space-separated product of words).
fn prettify_factor(factor: &str) -> String {
    factor
        .split_whitespace()
        .map(prettify_word)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Shortens a single unit word, preserving any `^n` exponent.
fn prettify_word(word: &str) -> String {
    let (stem, exponent) = match word.split_once('^') {
        Some((stem, power)) => (stem, format!("^{power}")),
        None => (word, String::new()),
    };
    let symbol = symbol_for(stem).unwrap_or_else(|| stem.to_string());
    format!("{symbol}{exponent}")
}

/// The symbol for a (possibly prefixed) spelled-out unit word, if known.
fn symbol_for(stem: &str) -> Option<String> {
    if let Some((_, symbol)) =
        BASE_SYMBOLS.iter().find(|(name, _)| *name == stem)
    {
        return Some((*symbol).to_string());
    }
    for (prefix, prefix_symbol) in PREFIX_SYMBOLS {
        let Some(base) = stem.strip_prefix(prefix) else {
            continue;
        };
        if let Some((_, symbol)) =
            BASE_SYMBOLS.iter().find(|(name, _)| *name == base)
        {
            return Some(format!("{prefix_symbol}{symbol}"));
        }
    }
    None
}

/// Whether `symbol` names a unit known to rink. Memoized.
///
/// Note that rink also resolves the constants `pi` and `e`; callers that treat
/// those as constants (the highlighter, the router) check for them first, so
/// this is only consulted for otherwise-unclassified identifiers.
pub fn is_unit(symbol: &str) -> bool {
    if symbol.is_empty() {
        return false;
    }
    if let Some(hit) = UNIT_CACHE.with(|c| c.borrow().get(symbol).copied()) {
        return hit;
    }
    let result = with_context(|ctx| rink_eval(ctx, symbol)).is_ok();
    UNIT_CACHE.with(|c| c.borrow_mut().insert(symbol.to_string(), result));
    result
}

/// Pulls the value and unit string out of a rink reply, for the two shapes that
/// carry a number (a bare evaluation and a conversion).
fn extract(reply: &QueryReply) -> Option<(f64, Option<String>)> {
    let parts = match reply {
        QueryReply::Number(parts) => parts,
        QueryReply::Conversion(conversion) => &conversion.value,
        _ => return None,
    };
    let value = parts.raw_value.as_ref()?.value.to_f64();
    Some((value, parts.unit.clone()))
}

/// The first line of a (possibly multi-line) rink message, dropping the
/// "Suggestions: ..." continuation rink appends to some errors.
fn first_line(message: &str) -> String {
    message.lines().next().unwrap_or(message).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_compatible_units() {
        let (value, unit) = eval("123 MPa -> bar").unwrap();
        assert!((value - 1230.0).abs() < 1e-6);
        assert_eq!(unit.as_deref(), Some("bar"));
    }

    #[test]
    fn exponent_and_compound_units_convert() {
        let (value, _) = eval("1 dm^3 -> cm^3").unwrap();
        assert!((value - 1000.0).abs() < 1e-6);
        let (value, _) = eval("N/mm^2 -> MPa").unwrap();
        assert!((value - 1.0).abs() < 1e-9);
    }

    #[test]
    fn arithmetic_with_units_produces_a_derived_unit() {
        let (value, unit) = eval("1 m * 2 m").unwrap();
        assert!((value - 2.0).abs() < 1e-9);
        // The result is an area, however rink chooses to spell it.
        assert!(unit.unwrap().contains("meter"));
    }

    #[test]
    fn addition_picks_a_single_unit() {
        let (value, _) = eval("1 m + 50 cm").unwrap();
        assert!((value - 1.5).abs() < 1e-9);
    }

    #[test]
    fn incompatible_units_error() {
        assert!(eval("5 N -> bar").is_err());
        assert!(eval("1 m + 1 s").is_err());
    }

    #[test]
    fn prettify_unit_shortens_rink_names_to_symbols() {
        assert_eq!(prettify_unit("kilonewton"), "kN");
        assert_eq!(prettify_unit("meter^2"), "m^2");
        assert_eq!(prettify_unit("meter / second"), "m/s");
        assert_eq!(prettify_unit("kilonewton / meter^2"), "kN/m^2");
        assert_eq!(prettify_unit("millimeter^3"), "mm^3");
        assert_eq!(prettify_unit("pascal"), "Pa");
        assert_eq!(prettify_unit("megajoule"), "MJ");
        assert_eq!(prettify_unit("kilogram meter / second^2"), "kg m/s^2");
        // An unknown name is left untouched.
        assert_eq!(prettify_unit("frobnicate"), "frobnicate");
    }

    #[test]
    fn prettified_symbols_are_still_parseable_by_rink() {
        // The shortened forms must round-trip, since quantities carry them.
        for symbol in ["kN", "m^2", "m/s", "kN/m^2", "mm^3"] {
            assert!(is_unit(symbol) || eval(&format!("1 {symbol}")).is_ok());
        }
    }

    #[test]
    fn is_unit_recognizes_units_but_not_unknown_identifiers() {
        assert!(is_unit("m"));
        assert!(is_unit("kN"));
        assert!(is_unit("MPa"));
        assert!(is_unit("dm"));
        assert!(!is_unit("notaunit"));
        assert!(!is_unit(""));
    }
}
