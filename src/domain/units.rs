//! The units engine: a thin wrapper over [`rink_core`].
//!
//! rink owns the unit database, dimensional analysis, conversion and arithmetic
//! with units; this module exposes just what calcli needs: [`eval`] to evaluate
//! a unit-bearing query into a `(value, unit)` pair, and [`is_unit`] to tell the
//! router and the syntax highlighter whether a symbol names a unit.
//!
//! The rink [`Context`] is expensive to build (tens of milliseconds) and is not
//! thread-safe, so it is kept lazily in thread-local storage and reused. Unit
//! lookups are memoized, since whether a symbol is a unit never changes.

use std::cell::RefCell;
use std::collections::HashMap;

use rink_core::output::QueryReply;
use rink_core::{Context, eval as rink_eval, simple_context};

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
/// Returns a one-line message when rink cannot parse the query, the units are
/// incompatible, or the result is not a plain number.
pub fn eval(query: &str) -> Result<(f64, Option<String>), String> {
    let reply = with_context(|ctx| rink_eval(ctx, query))
        .map_err(|error| first_line(&error.to_string()))?;
    extract(&reply).ok_or_else(|| "not a numeric result".to_string())
}

/// The SI base-unit magnitude of one `unit` (e.g. `1000` for `kN`/`kilonewton`,
/// `1` for `m/s`).
///
/// rink reports a result's [`eval`] value in SI base units but may name the unit
/// with a prefix; dividing the value by this scale re-expresses it in the named
/// unit. Memoized indirectly through rink's context.
///
/// # Errors
/// Returns a message when `unit` is not a unit rink can evaluate.
pub fn scale_of(unit: &str) -> Result<f64, String> {
    let (value, _) = eval(&format!("1 {unit}"))?;
    Ok(value)
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
    fn is_unit_recognizes_units_but_not_unknown_identifiers() {
        assert!(is_unit("m"));
        assert!(is_unit("kN"));
        assert!(is_unit("MPa"));
        assert!(is_unit("dm"));
        assert!(!is_unit("notaunit"));
        assert!(!is_unit(""));
    }
}
