//! The evaluation router: which engine a line goes to, and what it comes back
//! as.
//!
//! A line is classified (`=name`, `name = expr`, plain expression) and then
//! routed. Dimensionless arithmetic goes to meval, which keeps the functions
//! and the angle mode; anything touching units - a conversion, a unit literal,
//! or arithmetic on a unit-bearing value - goes to rink through
//! [`crate::domain::units`]. [`needs_units`] is the switch.
//!
//! Nothing here knows about [`CalcService`](super::CalcService): every function
//! takes the engine, the variables and the settings it needs, which is what
//! lets the router be tested on its own.

use crate::domain::errors::{AppError, Result};
use crate::domain::evaluator::Evaluator;
use crate::domain::expression::{self, Statement};
use crate::domain::format::FormatSettings;
use crate::domain::history::LineResult;
use crate::domain::quantity::Quantity;
use crate::domain::units;
use crate::domain::variables::VariableStore;

/// Names that may not be used as variables because they collide with the
/// previous-answer keyword or meval's built-in constants.
const RESERVED_NAMES: &[&str] = &["ans", "pi", "e"];

/// The operators that make an expression a continuation of the previous answer,
/// so `* 2` on its own line means `ans * 2`.
const CONTINUING_OPERATORS: [char; 5] = ['+', '-', '*', '/', '^'];

/// Whether `expr` continues from the previous answer rather than standing on
/// its own, which it does by opening with an operator.
fn continues_from_ans(expr: &str) -> bool {
    expr.trim_start().starts_with(CONTINUING_OPERATORS)
}

/// Rewrites every reference to a variable in `expr` into its literal, as
/// `render` spells it.
///
/// Shared by both engines: they disagree only on how a value is written down
/// (rink wants `(50 kN)`, meval a bare number) and on which variables qualify,
/// which is what `render` returning `None` expresses.
fn substitute_variables(
    expr: &str,
    variables: &VariableStore,
    render: impl Fn(&Quantity) -> Option<String>,
) -> String {
    let mut prepared = expr.to_string();
    for (name, value) in variables.iter() {
        if !expression::references(&prepared, name) {
            continue;
        }
        if let Some(literal) = render(value) {
            prepared = expression::substitute_identifier_with(
                &prepared, name, &literal,
            );
        }
    }
    prepared
}

/// Evaluates one line: a `=name` save, a `name = expr` assignment or a plain
/// expression. Variable assignments mutate `variables`; reserved and malformed
/// names are rejected with a message rather than a panic.
pub(super) fn evaluate_line(
    evaluator: &dyn Evaluator,
    variables: &mut VariableStore,
    settings: &FormatSettings,
    input: &str,
    ans: Option<Quantity>,
) -> LineResult {
    // Strip the inline comment; the full input is kept by the history.
    let code = expression::strip_comment(input);
    if code.trim().is_empty() {
        return (None, None);
    }
    match expression::classify(code) {
        Statement::SaveAns(name) => save_ans(variables, &name, ans),
        Statement::Assign { name, expr } => {
            assign(evaluator, variables, settings, &name, &expr, ans)
        }
        Statement::Expression(expr) => {
            match eval_expression(evaluator, variables, settings, &expr, ans) {
                Ok(value) => (Some(value), None),
                Err(error) => (None, Some(error.to_string())),
            }
        }
    }
}

/// Stores the previous answer in `name`, or reports why it cannot.
fn save_ans(
    variables: &mut VariableStore,
    name: &str,
    ans: Option<Quantity>,
) -> LineResult {
    if let Some(error) = reject_name(name) {
        return (None, Some(error.to_string()));
    }
    match ans {
        Some(value) => {
            variables.set(name, value.clone());
            (Some(value), None)
        }
        None => (None, Some(AppError::NoAnswerToSave.to_string())),
    }
}

/// Evaluates `expr` and stores the result in `name`, or reports the error.
fn assign(
    evaluator: &dyn Evaluator,
    variables: &mut VariableStore,
    settings: &FormatSettings,
    name: &str,
    expr: &str,
    ans: Option<Quantity>,
) -> LineResult {
    if let Some(error) = reject_name(name) {
        return (None, Some(error.to_string()));
    }
    match eval_expression(evaluator, variables, settings, expr, ans) {
        Ok(value) => {
            variables.set(name, value.clone());
            (Some(value), None)
        }
        Err(error) => (None, Some(error.to_string())),
    }
}

/// Evaluates `expr` into a [`Quantity`].
///
/// A sole `ans` or variable reference returns the stored quantity verbatim (so
/// its unit and full precision survive). Otherwise the line is routed: a
/// unit-free expression goes to meval (preserving functions and the angle
/// mode), while anything involving units - a conversion, a unit literal, or
/// arithmetic on unit-bearing values - goes to rink (see [`needs_units`]).
pub(super) fn eval_expression(
    evaluator: &dyn Evaluator,
    variables: &VariableStore,
    settings: &FormatSettings,
    expr: &str,
    ans: Option<Quantity>,
) -> Result<Quantity> {
    let trimmed = expr.trim();
    if trimmed == "ans" {
        return ans.ok_or(AppError::NoPreviousAnswer);
    }
    if let Some(quantity) = variables.get(trimmed) {
        return Ok(quantity.clone());
    }
    if needs_units(expr, variables, ans.as_ref()) {
        return eval_with_rink(variables, settings, expr, ans);
    }
    let value = eval_with_meval(evaluator, variables, settings, expr, ans)?;
    Ok(Quantity::dimensionless(value))
}

/// Whether `expr` must be evaluated by the units engine rather than meval.
///
/// True when it converts (`->`/` to `), continues from a unit-bearing `ans`,
/// references a unit-bearing variable, or contains a unit symbol. The constants
/// `pi`/`e` and `ans` are never units, and defined variables are handled by the
/// unit-bearing checks above, so they are excluded from the token scan.
fn needs_units(
    expr: &str,
    variables: &VariableStore,
    ans: Option<&Quantity>,
) -> bool {
    if split_conversion(expr).is_some() {
        return true;
    }
    if ans.is_some_and(|a| !a.is_dimensionless())
        && (continues_from_ans(expr) || expression::references(expr, "ans"))
    {
        return true;
    }
    for (name, value) in variables.iter() {
        if !value.is_dimensionless() && expression::references(expr, name) {
            return true;
        }
    }
    expr.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|token| !token.is_empty())
        .any(|token| {
            !matches!(token, "pi" | "e" | "ans")
                && variables.get(token).is_none()
                && units::is_unit(token)
        })
}

/// Splits a conversion `<source> -> <unit>` (or `<source> to <unit>`).
fn split_conversion(expr: &str) -> Option<(&str, &str)> {
    if let Some((source, target)) = expr.split_once("->") {
        return Some((source, target));
    }
    expr.split_once(" to ")
}

/// Evaluates a unit-bearing `expr` through rink, substituting `ans` and any
/// referenced variables as unit literals first.
///
/// The display unit is chosen so the value reads as the user expects:
/// - a conversion (`… -> X`) is shown in the typed target `X` (rink reports the
///   value already in `X`);
/// - a simple quantity literal (`50 kN`) keeps the unit the user wrote;
/// - otherwise rink's unit name is shortened to symbols (`meter^2` → `m^2`,
///   `kilonewton` → `kN`), re-expressing the SI base value via
///   [`units::scale_of`]. The user can pin any other unit with a `->`.
fn eval_with_rink(
    variables: &VariableStore,
    settings: &FormatSettings,
    expr: &str,
    ans: Option<Quantity>,
) -> Result<Quantity> {
    let prepared = substitute_for_units(variables, settings, expr, ans)?;

    // A conversion: rink validates the dimensions and reports the value in the
    // target, so the typed target symbol is pinned directly.
    if let Some((_, target)) = split_conversion(expr) {
        let target = target.trim();
        if !target.is_empty() {
            let (value, _) = units::eval(&prepared)?;
            return Ok(Quantity::new(value, target.to_string()));
        }
    }

    let (base_value, unit) = units::eval(&prepared)?;
    let Some(unit) = unit else {
        return Ok(Quantity::dimensionless(base_value));
    };
    // Choose the display unit: the user's own symbol for a plain
    // `<number> <unit>` literal, else rink's name shortened to symbols. The
    // value is in SI base units, so scale it into that display unit (the
    // shortened forms stay rink-parseable, so `scale_of` resolves them).
    let display_unit = simple_literal_unit(expr)
        .unwrap_or_else(|| units::prettify_unit(&unit));
    let value = base_value / units::scale_of(&display_unit)?;
    Ok(Quantity::new(value, display_unit))
}

/// Substitutes `ans` and referenced variables into `expr` as rink literals,
/// applying the light units preprocessing and `ans`-on-leading-operator rule.
fn substitute_for_units(
    variables: &VariableStore,
    settings: &FormatSettings,
    expr: &str,
    ans: Option<Quantity>,
) -> Result<String> {
    let mut prepared = prepare_units_expr(expr, settings.decimal_separator);
    if continues_from_ans(&prepared) && ans.is_some() {
        prepared = format!("ans {prepared}");
    }
    if expression::references(&prepared, "ans") {
        let value = ans.as_ref().ok_or(AppError::NoPreviousAnswer)?;
        prepared = expression::substitute_identifier_with(
            &prepared,
            "ans",
            &quantity_literal(value),
        );
    }
    // Every variable qualifies here: rink can read a unit-bearing literal.
    Ok(substitute_variables(&prepared, variables, |value| {
        Some(quantity_literal(value))
    }))
}

/// The unit symbol of a plain `<number> <unit>` literal (e.g. `"kN"` for
/// `50 kN`), or `None` when `expr` is anything more complex.
fn simple_literal_unit(expr: &str) -> Option<String> {
    let (head, last) = expr.trim().rsplit_once(char::is_whitespace)?;
    let head = head.trim();
    let number_chars = |c: char| {
        c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-')
    };
    if head.is_empty()
        || !head.chars().all(number_chars)
        || !head.chars().any(|c| c.is_ascii_digit())
        || !units::is_unit(last)
    {
        return None;
    }
    Some(last.to_string())
}

/// Renders a quantity as a rink-parseable literal (e.g. `(50 kN)`), for
/// substituting `ans`/variables into a unit expression.
fn quantity_literal(quantity: &Quantity) -> String {
    match quantity.unit_symbol() {
        Some(symbol) => format!("({} {})", quantity.display_value(), symbol),
        None => format!("({})", quantity.display_value()),
    }
}

/// Light preprocessing for the rink path: `**`→`^` and, in comma-decimal mode,
/// the decimal mark to `.`. Spaces are kept (rink needs them between a number
/// and its unit) and SI prefixes are left for rink to resolve.
fn prepare_units_expr(expr: &str, decimal_separator: char) -> String {
    let replaced = expr.replace("**", "^");
    if decimal_separator == ',' {
        replaced.replace(',', ".")
    } else {
        replaced
    }
}

/// Evaluates a dimensionless numeric expression with meval, substituting `ans`
/// and dimensionless variables (the router guarantees no units are involved).
fn eval_with_meval(
    evaluator: &dyn Evaluator,
    variables: &VariableStore,
    settings: &FormatSettings,
    expr: &str,
    ans: Option<Quantity>,
) -> Result<f64> {
    let prepared = expression::preprocess(expr, settings.decimal_separator);
    let ans_number = ans
        .as_ref()
        .filter(|a| a.is_dimensionless())
        .map(Quantity::display_value);

    let prepared = expression::prepend_ans(&prepared, ans_number);
    let prepared = expression::substitute_ans(&prepared, ans_number);
    // Only dimensionless variables qualify: meval has no notion of a unit, and
    // the router has already sent anything unit-bearing to rink.
    let prepared = substitute_variables(&prepared, variables, |value| {
        value
            .is_dimensionless()
            .then(|| value.display_value().to_string())
    });
    // The evaluator already speaks `AppError`; there is nothing to translate.
    evaluator.eval(&prepared, settings.angle_mode)
}

/// The reason `name` cannot be a variable, or `None` when it can.
pub(super) fn reject_name(name: &str) -> Option<AppError> {
    if !expression::is_valid_var_name(name) {
        return Some(AppError::InvalidVariableName(name.to_string()));
    }
    if RESERVED_NAMES.contains(&name) {
        return Some(AppError::ReservedName(name.to_string()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(pairs: &[(&str, Quantity)]) -> VariableStore {
        let mut variables = VariableStore::new();
        for (name, value) in pairs {
            variables.set(name, value.clone());
        }
        variables
    }

    fn metres(value: f64) -> Quantity {
        Quantity::new(value, "m".to_string())
    }

    #[test]
    fn a_leading_operator_marks_a_continuation_of_the_previous_answer() {
        for expr in ["* 2", "  + 1", "-3", "/2", "^2"] {
            assert!(continues_from_ans(expr), "{expr}");
        }
        for expr in ["2 * 3", "sin(1)", "", "  "] {
            assert!(!continues_from_ans(expr), "{expr}");
        }
    }

    #[test]
    fn substitution_rewrites_referenced_variables_only() {
        let variables = store(&[
            ("x", Quantity::dimensionless(2.0)),
            ("unused", Quantity::dimensionless(9.0)),
        ]);
        let out = substitute_variables("x + 1", &variables, |value| {
            Some(value.display_value().to_string())
        });
        assert_eq!(out, "2 + 1");
    }

    /// A `render` returning `None` is how the meval path skips a unit-bearing
    /// variable: the name must survive untouched rather than become garbage.
    #[test]
    fn a_declined_variable_is_left_in_place() {
        let variables = store(&[("d", metres(3.0))]);
        let out = substitute_variables("d + 1", &variables, |_| None);
        assert_eq!(out, "d + 1");
    }

    #[test]
    fn a_conversion_splits_on_both_spellings() {
        assert_eq!(split_conversion("3 m -> cm"), Some(("3 m ", " cm")));
        assert_eq!(split_conversion("3 m to cm"), Some(("3 m", "cm")));
        assert_eq!(split_conversion("3 + 4"), None);
    }

    #[test]
    fn a_plain_number_and_unit_keeps_the_typed_symbol() {
        assert_eq!(simple_literal_unit("50 kN"), Some("kN".to_string()));
        assert_eq!(simple_literal_unit("1.5e3 m"), Some("m".to_string()));
        assert_eq!(simple_literal_unit("50 kN + 1 kN"), None);
        assert_eq!(simple_literal_unit("kN"), None);
        assert_eq!(simple_literal_unit("50 notaunit"), None);
    }

    #[test]
    fn a_quantity_renders_as_a_parenthesised_rink_literal() {
        assert_eq!(quantity_literal(&metres(3.0)), "(3 m)");
        assert_eq!(quantity_literal(&Quantity::dimensionless(3.0)), "(3)",);
    }

    /// Rink needs the spaces between a number and its unit, so the units path
    /// preprocesses more gently than the meval one.
    #[test]
    fn the_units_preprocessor_keeps_spaces_and_maps_the_decimal_mark() {
        assert_eq!(prepare_units_expr("2 ** 3", '.'), "2 ^ 3");
        assert_eq!(prepare_units_expr("1,5 m", ','), "1.5 m");
        assert_eq!(prepare_units_expr("1,5 m", '.'), "1,5 m");
    }

    #[test]
    fn reserved_and_malformed_variable_names_are_rejected() {
        for name in ["ans", "pi", "e"] {
            assert!(
                matches!(reject_name(name), Some(AppError::ReservedName(_))),
                "{name}",
            );
        }
        assert!(matches!(
            reject_name("1abc"),
            Some(AppError::InvalidVariableName(_)),
        ));
        assert!(reject_name("width").is_none());
    }

    #[test]
    fn the_router_sends_a_conversion_to_the_units_engine() {
        let variables = VariableStore::new();
        assert!(needs_units("3 m -> cm", &variables, None));
    }

    #[test]
    fn the_router_keeps_bare_arithmetic_and_constants_on_meval() {
        let variables = VariableStore::new();
        for expr in ["2 + 3", "sin(pi)", "e^2", "ans * 2"] {
            assert!(!needs_units(expr, &variables, None), "{expr}");
        }
    }

    #[test]
    fn a_unit_bearing_variable_routes_its_expression_to_the_units_engine() {
        let variables = store(&[("d", metres(3.0))]);
        assert!(needs_units("d * 2", &variables, None));
    }

    /// A dimensionless variable must not drag the line onto the units engine,
    /// even when its name happens to read like a unit symbol.
    #[test]
    fn a_dimensionless_variable_named_like_a_unit_stays_on_meval() {
        let variables = store(&[("m", Quantity::dimensionless(4.0))]);
        assert!(!needs_units("m * 2", &variables, None));
    }

    #[test]
    fn continuing_from_a_unit_bearing_answer_routes_to_the_units_engine() {
        let variables = VariableStore::new();
        let ans = metres(3.0);
        assert!(needs_units("* 2", &variables, Some(&ans)));

        let plain = Quantity::dimensionless(3.0);
        assert!(!needs_units("* 2", &variables, Some(&plain)));
    }
}
