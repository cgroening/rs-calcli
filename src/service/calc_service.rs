//! The calculator service: the one place that drives evaluation, history and
//! variables, and owns the display settings.
//!
//! It threads the previous answer through the history, re-evaluates the tail
//! when a line is edited or deleted, and applies settings changes (recomputing
//! when a change alters results, such as the angle mode or decimal separator).
//! All values are full-precision `f64`s; rounding lives in
//! [`crate::domain::format`].

use crate::domain::evaluator::Evaluator;
use crate::domain::expression::{self, Statement};
use crate::domain::format::{
    AngleMode, FormatSettings, Notation, format_display, format_plain,
};
use crate::domain::history::{History, HistoryEntry, LineResult};
use crate::domain::quantity::Quantity;
use crate::domain::units;
use crate::domain::variables::VariableStore;

/// Names that may not be used as variables because they collide with the
/// previous-answer keyword or meval's built-in constants.
const RESERVED_NAMES: &[&str] = &["ans", "pi", "e"];

/// The outcome of submitting a line, for the caller's status line.
#[derive(Debug, Clone, PartialEq)]
pub struct SubmitOutcome {
    /// The computed value, when the line succeeded.
    pub value: Option<Quantity>,
    /// The error message, when the line failed.
    pub error: Option<String>,
}

/// A non-mutating live preview of the current input, for typed feedback.
#[derive(Debug, Clone, PartialEq)]
pub enum Preview {
    /// Nothing to show (empty input or a `:` command).
    Empty,
    /// The input currently evaluates to this value.
    Value(Quantity),
    /// The input looks unfinished (still being typed); show no warning.
    Incomplete,
    /// The input looks complete but does not parse; show a warning.
    Invalid,
}

/// Orchestrates evaluation, history and variables behind one façade.
pub struct CalcService {
    evaluator: Box<dyn Evaluator>,
    variables: VariableStore,
    history: History,
    settings: FormatSettings,
}

impl CalcService {
    /// Builds a service with the given engine, settings and (possibly restored)
    /// history and variables.
    pub fn new(
        evaluator: Box<dyn Evaluator>,
        settings: FormatSettings,
        history: History,
        variables: VariableStore,
    ) -> Self {
        CalcService {
            evaluator,
            variables,
            history,
            settings,
        }
    }

    /// The current display settings (the source of truth for the settings bar).
    pub fn settings(&self) -> &FormatSettings {
        &self.settings
    }

    /// The calculation history.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// The defined variables.
    pub fn variables(&self) -> &VariableStore {
        &self.variables
    }

    /// Evaluates a new input line and appends it to the history.
    ///
    /// An errored line is still recorded (with its message) so the user can
    /// edit or delete it; its `ans` is `None` for the line below.
    pub fn submit(&mut self, input: &str) -> SubmitOutcome {
        let ans = self.history.last_value();
        let (value, error) = self.evaluate_line(input, ans);
        let outcome = SubmitOutcome {
            value: value.clone(),
            error: error.clone(),
        };
        self.history.push(HistoryEntry {
            input: input.to_string(),
            value,
            error,
        });
        outcome
    }

    /// Replaces the input of the entry at `index` and re-evaluates the tail.
    pub fn edit_entry(&mut self, index: usize, new_input: &str) {
        self.history.set_input(index, new_input.to_string());
        self.recompute(index);
    }

    /// Removes the entry at `index` and re-evaluates the tail.
    pub fn delete_entry(&mut self, index: usize) {
        self.history.remove(index);
        self.recompute(index);
    }

    /// Moves the entry at `index` by `delta` positions (clamped) and
    /// re-evaluates from the first affected line. Returns the new index, so the
    /// caller can follow the moved entry with its selection.
    pub fn move_entry(&mut self, index: usize, delta: isize) -> usize {
        let len = self.history.len();
        if len == 0 {
            return 0;
        }
        let target =
            (index as isize + delta).clamp(0, len as isize - 1) as usize;
        if target != index {
            self.history.swap(index, target);
            self.recompute(index.min(target));
        }
        target
    }

    /// Inserts a blank entry at `index` and re-evaluates the tail. The caller
    /// typically edits it immediately.
    pub fn insert_entry(&mut self, index: usize) {
        let blank = HistoryEntry {
            input: String::new(),
            value: None,
            error: None,
        };
        self.history.insert(index, blank);
        self.recompute(index);
    }

    /// Clears the history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Re-evaluates the entire history, regenerating values and errors under the
    /// current settings. Used on startup so restored entries are consistent with
    /// the active settings (and with each other).
    pub fn recompute_all(&mut self) {
        self.recompute(0);
    }

    /// Removes every variable.
    pub fn reset_variables(&mut self) {
        self.variables.clear();
    }

    /// Removes a single variable by name.
    pub fn remove_variable(&mut self, name: &str) {
        self.variables.remove(name);
    }

    /// Advances the notation (display only; values are unchanged).
    pub fn cycle_notation(&mut self) {
        self.settings.notation = self.settings.notation.next();
    }

    /// Sets the notation directly (display only), for the `:` commands.
    pub fn set_notation(&mut self, notation: Notation) {
        self.settings.notation = notation;
    }

    /// Sets the angle mode directly, recomputing only when it changes.
    pub fn set_angle_mode(&mut self, angle_mode: AngleMode) {
        if self.settings.angle_mode != angle_mode {
            self.settings.angle_mode = angle_mode;
            self.recompute(0);
        }
    }

    /// Sets the number of fractional digits (display only).
    pub fn set_decimals(&mut self, decimals: usize) {
        self.settings.decimals = decimals;
    }

    /// Toggles the angle mode and recomputes, since trig results change.
    pub fn toggle_angle_mode(&mut self) {
        self.settings.angle_mode = self.settings.angle_mode.toggled();
        self.recompute(0);
    }

    /// Toggles the decimal separator and recomputes, since it changes how input
    /// numbers are parsed.
    pub fn toggle_decimal_separator(&mut self) {
        self.settings.toggle_decimal_separator();
        self.recompute(0);
    }

    /// Toggles whether trailing fractional zeros are dropped (display only, so
    /// no recompute is needed).
    pub fn toggle_trim_trailing_zeros(&mut self) {
        self.settings.trim_trailing_zeros = !self.settings.trim_trailing_zeros;
    }

    /// Renders a quantity for display (rounded, grouped) - for the `Y` copy.
    pub fn format_display(&self, value: &Quantity) -> String {
        format_display(value, &self.settings)
    }

    /// Renders a quantity as a plain, full-precision value - for the `y` copy.
    pub fn format_plain(&self, value: &Quantity) -> String {
        format_plain(value, &self.settings)
    }

    /// Previews the current input without mutating history, variables or `ans`.
    ///
    /// Returns the value when it evaluates, [`Preview::Incomplete`] while the
    /// input still looks like it is being typed (so no warning is shown), and
    /// [`Preview::Invalid`] when a complete-looking input does not parse.
    pub fn preview(&self, input: &str) -> Preview {
        // The comment is not part of the calculation; a comment-only line (or a
        // command) shows no preview.
        let code = expression::strip_comment(input).trim();
        if code.is_empty() || code.starts_with(':') {
            return Preview::Empty;
        }
        match self.preview_value(code) {
            Some(value) => Preview::Value(value),
            None if expression::looks_incomplete(code) => Preview::Incomplete,
            None => Preview::Invalid,
        }
    }

    /// Evaluates the input for the preview, reusing the submit pipeline but
    /// reading (not mutating) the variable store.
    fn preview_value(&self, input: &str) -> Option<Quantity> {
        let ans = self.history.last_value();
        match expression::classify(input) {
            Statement::SaveAns(name) => {
                if reject_name(&name).is_some() {
                    return None;
                }
                ans
            }
            Statement::Assign { name, expr } => {
                if reject_name(&name).is_some() {
                    return None;
                }
                eval_expression(
                    self.evaluator.as_ref(),
                    &self.variables,
                    &self.settings,
                    &expr,
                    ans,
                )
                .ok()
            }
            Statement::Expression(expr) => eval_expression(
                self.evaluator.as_ref(),
                &self.variables,
                &self.settings,
                &expr,
                ans,
            )
            .ok(),
        }
    }

    /// Re-evaluates the history from `start`, threading `ans` and applying
    /// variable assignments in order. Borrows the engine, variables and settings
    /// disjointly from the history so the closure can mutate the store.
    fn recompute(&mut self, start: usize) {
        let evaluator = self.evaluator.as_ref();
        let variables = &mut self.variables;
        let settings = &self.settings;
        self.history.recompute_from(start, |input, ans| {
            evaluate_line(evaluator, variables, settings, input, ans)
        });
    }

    /// Evaluates a single line against the current engine, variables and
    /// settings (used by [`submit`](Self::submit)).
    fn evaluate_line(
        &mut self,
        input: &str,
        ans: Option<Quantity>,
    ) -> LineResult {
        evaluate_line(
            self.evaluator.as_ref(),
            &mut self.variables,
            &self.settings,
            input,
            ans,
        )
    }
}

/// Evaluates one line: a `=name` save, a `name = expr` assignment or a plain
/// expression. Variable assignments mutate `variables`; reserved and malformed
/// names are rejected with a message rather than a panic.
fn evaluate_line(
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
                Err(message) => (None, Some(message)),
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
    if let Some(message) = reject_name(name) {
        return (None, Some(message));
    }
    match ans {
        Some(value) => {
            variables.set(name, value.clone());
            (Some(value), None)
        }
        None => (None, Some("no previous answer to save".to_string())),
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
    if let Some(message) = reject_name(name) {
        return (None, Some(message));
    }
    match eval_expression(evaluator, variables, settings, expr, ans) {
        Ok(value) => {
            variables.set(name, value.clone());
            (Some(value), None)
        }
        Err(message) => (None, Some(message)),
    }
}

/// Evaluates `expr` into a [`Quantity`].
///
/// A sole `ans` or variable reference returns the stored quantity verbatim (so
/// its unit and full precision survive). Otherwise the line is routed: a
/// unit-free expression goes to meval (preserving functions and the angle
/// mode), while anything involving units - a conversion, a unit literal, or
/// arithmetic on unit-bearing values - goes to rink (see [`needs_units`]).
fn eval_expression(
    evaluator: &dyn Evaluator,
    variables: &VariableStore,
    settings: &FormatSettings,
    expr: &str,
    ans: Option<Quantity>,
) -> Result<Quantity, String> {
    let trimmed = expr.trim();
    if trimmed == "ans" {
        return ans.ok_or_else(|| "no previous answer".to_string());
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
    let leading_operator =
        expr.trim_start().starts_with(['+', '-', '*', '/', '^']);
    if ans.is_some_and(|a| !a.is_dimensionless())
        && (leading_operator || expression::references(expr, "ans"))
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
) -> Result<Quantity, String> {
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
) -> Result<String, String> {
    let mut prepared = prepare_units_expr(expr, settings.decimal_separator);
    let leading_operator =
        prepared.trim_start().starts_with(['+', '-', '*', '/', '^']);
    if leading_operator && ans.is_some() {
        prepared = format!("ans {prepared}");
    }
    if expression::references(&prepared, "ans") {
        let value = ans
            .as_ref()
            .ok_or_else(|| "no previous answer".to_string())?;
        prepared = expression::substitute_identifier_with(
            &prepared,
            "ans",
            &quantity_literal(value),
        );
    }
    for (name, value) in variables.iter() {
        if expression::references(&prepared, name) {
            prepared = expression::substitute_identifier_with(
                &prepared,
                name,
                &quantity_literal(value),
            );
        }
    }
    Ok(prepared)
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
) -> Result<f64, String> {
    let prepared = expression::preprocess(expr, settings.decimal_separator);
    let ans_number = ans
        .as_ref()
        .filter(|a| a.is_dimensionless())
        .map(Quantity::display_value);

    let prepared = expression::prepend_ans(&prepared, ans_number);
    let mut prepared = expression::substitute_ans(&prepared, ans_number);
    for (name, value) in variables.iter() {
        if expression::references(&prepared, name) && value.is_dimensionless() {
            prepared = expression::substitute_identifier(
                &prepared,
                name,
                value.display_value(),
            );
        }
    }
    evaluator
        .eval(&prepared, settings.angle_mode)
        .map_err(|error| error.to_string())
}

/// Returns an error message when `name` is invalid or reserved, else `None`.
fn reject_name(name: &str) -> Option<String> {
    if !expression::is_valid_var_name(name) {
        return Some(format!("invalid variable name: '{name}'"));
    }
    if RESERVED_NAMES.contains(&name) {
        return Some(format!("'{name}' is reserved and cannot be a variable"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evaluator::MevalEvaluator;
    use crate::domain::format::{AngleMode, Notation};

    fn settings() -> FormatSettings {
        FormatSettings {
            notation: Notation::Decimal,
            decimals: 3,
            angle_mode: AngleMode::Rad,
            decimal_separator: '.',
            thousands_separator: " ".to_string(),
            trim_trailing_zeros: false,
        }
    }

    fn service() -> CalcService {
        CalcService::new(
            Box::new(MevalEvaluator::new()),
            settings(),
            History::new(100),
            VariableStore::new(),
        )
    }

    fn value_at(service: &CalcService, index: usize) -> Option<f64> {
        service.history().entries()[index]
            .value
            .as_ref()
            .map(Quantity::display_value)
    }

    fn last(service: &CalcService) -> Option<f64> {
        service.history().last_value().map(|q| q.display_value())
    }

    fn var(service: &CalcService, name: &str) -> Option<f64> {
        service.variables().get(name).map(Quantity::display_value)
    }

    fn outval(outcome: &SubmitOutcome) -> Option<f64> {
        outcome.value.as_ref().map(Quantity::display_value)
    }

    /// A dimensionless value preview, for the preview assertions.
    fn val(value: f64) -> Preview {
        Preview::Value(Quantity::dimensionless(value))
    }

    #[test]
    fn submit_evaluates_and_records_history() {
        let mut service = service();
        let outcome = service.submit("2+3");
        assert_eq!(outval(&outcome), Some(5.0));
        assert_eq!(last(&service), Some(5.0));
    }

    #[test]
    fn ans_continues_from_the_previous_line() {
        let mut service = service();
        service.submit("10");
        service.submit("+5");
        assert_eq!(value_at(&service, 1), Some(15.0));
        service.submit("ans*2");
        assert_eq!(value_at(&service, 2), Some(30.0));
    }

    #[test]
    fn editing_a_line_recomputes_the_chain_below() {
        let mut service = service();
        service.submit("10");
        service.submit("ans+5");
        service.submit("ans*2");
        assert_eq!(value_at(&service, 2), Some(30.0));
        service.edit_entry(0, "20");
        assert_eq!(value_at(&service, 1), Some(25.0));
        assert_eq!(value_at(&service, 2), Some(50.0));
    }

    #[test]
    fn deleting_a_line_recomputes_the_chain_below() {
        let mut service = service();
        service.submit("10");
        service.submit("ans+5");
        service.submit("ans+100");
        service.delete_entry(1);
        // The former third line now follows the first: 10 + 100.
        assert_eq!(value_at(&service, 1), Some(110.0));
    }

    #[test]
    fn save_ans_and_assignment_define_variables() {
        let mut service = service();
        service.submit("7");
        service.submit("=x");
        assert_eq!(var(&service, "x"), Some(7.0));
        service.submit("y = x + 3");
        assert_eq!(var(&service, "y"), Some(10.0));
        service.submit("y*2");
        assert_eq!(last(&service), Some(20.0));
    }

    #[test]
    fn reserved_and_invalid_names_are_rejected() {
        let mut service = service();
        service.submit("5");
        let outcome = service.submit("=pi");
        assert!(outcome.error.is_some());
        let outcome = service.submit("1abc = 3");
        assert!(outcome.error.is_some());
    }

    #[test]
    fn an_errored_line_is_recorded_without_a_value() {
        let mut service = service();
        let outcome = service.submit("2+");
        assert!(outcome.error.is_some());
        assert_eq!(last(&service), None);
    }

    #[test]
    fn toggling_angle_mode_recomputes_history() {
        let mut service = service();
        service.submit("sin(90)");
        // In radians sin(90) is not 1.
        assert!((value_at(&service, 0).unwrap() - 1.0).abs() > 0.1);
        service.toggle_angle_mode();
        assert!((value_at(&service, 0).unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn toggling_decimal_separator_reparses_history_input() {
        let mut service = service();
        // With '.' decimal, the comma is a thousands separator: 1,5 -> 15.
        service.submit("1,5");
        assert_eq!(value_at(&service, 0), Some(15.0));
        service.toggle_decimal_separator();
        // With ',' decimal, 1,5 -> 1.5.
        assert_eq!(value_at(&service, 0), Some(1.5));
    }

    #[test]
    fn variable_used_before_assignment_recomputes_after_edit() {
        let mut service = service();
        service.submit("a = 2");
        service.submit("a * 10");
        assert_eq!(value_at(&service, 1), Some(20.0));
        service.edit_entry(0, "a = 5");
        assert_eq!(value_at(&service, 1), Some(50.0));
    }

    #[test]
    fn preview_reports_value_incomplete_and_invalid() {
        let mut service = service();
        service.submit("10");
        assert_eq!(service.preview("2+3"), val(5.0));
        assert_eq!(service.preview("ans+5"), val(15.0));
        assert_eq!(service.preview("2+"), Preview::Incomplete);
        assert_eq!(service.preview("2+3)"), Preview::Invalid);
        assert_eq!(service.preview(""), Preview::Empty);
        assert_eq!(service.preview(":d4"), Preview::Empty);
    }

    #[test]
    fn preview_handles_assignments_without_mutating_state() {
        let mut service = service();
        service.submit("7");
        assert_eq!(service.preview("x = ans + 3"), val(10.0));
        // Previewing must not define the variable or add to the history.
        assert!(service.variables().get("x").is_none());
        assert_eq!(service.history().len(), 1);
        // A reserved name is invalid, not a value.
        assert_eq!(service.preview("pi = 3"), Preview::Invalid);
    }

    #[test]
    fn inline_comments_are_ignored_but_kept_in_history() {
        let mut service = service();
        let outcome = service.submit("2+3 # the sum");
        assert_eq!(outval(&outcome), Some(5.0));
        // The full input, including the comment, is stored.
        assert_eq!(service.history().entries()[0].input, "2+3 # the sum");
        // Comments work on assignments too.
        service.submit("x = 5 # a note");
        assert_eq!(var(&service, "x"), Some(5.0));
    }

    #[test]
    fn a_comment_only_line_is_a_note_that_passes_ans_through() {
        let mut service = service();
        service.submit("5");
        let outcome = service.submit("# just a note");
        assert_eq!(outcome.value, None);
        assert_eq!(outcome.error, None);
        assert_eq!(service.history().entries()[1].input, "# just a note");
        // The note does not break the `ans` chain.
        service.submit("ans + 1");
        assert_eq!(value_at(&service, 2), Some(6.0));
    }

    #[test]
    fn moving_an_entry_recomputes_the_ans_chain() {
        let mut service = service();
        service.submit("10");
        service.submit("ans + 5"); // 15
        service.submit("ans * 2"); // 30
        // Move the last line up one: ["10", "ans*2", "ans+5"].
        let new_index = service.move_entry(2, -1);
        assert_eq!(new_index, 1);
        assert_eq!(value_at(&service, 1), Some(20.0)); // ans*2 with ans=10
        assert_eq!(value_at(&service, 2), Some(25.0)); // ans+5 with ans=20
    }

    #[test]
    fn inserting_a_blank_entry_shifts_and_recomputes() {
        let mut service = service();
        service.submit("10");
        service.submit("ans + 5"); // 15
        service.insert_entry(1);
        assert_eq!(service.history().len(), 3);
        assert_eq!(service.history().entries()[1].input, "");
        // The blank note passes ans through, so `ans + 5` is still 15.
        assert_eq!(value_at(&service, 2), Some(15.0));
    }

    #[test]
    fn preview_ignores_comments() {
        let mut service = service();
        service.submit("2");
        assert_eq!(service.preview("# note"), Preview::Empty);
        assert_eq!(service.preview("ans+3 # sum"), val(5.0));
    }

    #[test]
    fn converts_a_quantity_with_the_arrow() {
        let mut service = service();
        let outcome = service.submit("123 MPa -> bar");
        let quantity = outcome.value.unwrap();
        assert_eq!(quantity.unit_symbol(), Some("bar"));
        assert!((quantity.display_value() - 1230.0).abs() < 1e-6);
    }

    #[test]
    fn stores_a_quantity_variable_and_converts_it() {
        let mut service = service();
        service.submit("x = 50 kN");
        assert_eq!(
            service.variables().get("x").unwrap().unit_symbol(),
            Some("kN")
        );
        let outcome = service.submit("x -> N");
        let quantity = outcome.value.unwrap();
        assert_eq!(quantity.unit_symbol(), Some("N"));
        assert!((quantity.display_value() - 50_000.0).abs() < 1e-9);
    }

    #[test]
    fn ans_carries_its_unit_into_a_conversion() {
        let mut service = service();
        service.submit("2 bar");
        let outcome = service.submit("ans -> Pa");
        let quantity = outcome.value.unwrap();
        assert_eq!(quantity.unit_symbol(), Some("Pa"));
        assert!((quantity.display_value() - 200_000.0).abs() < 1e-6);
    }

    #[test]
    fn compound_and_volume_conversions_work() {
        let mut service = service();
        let litre = service.submit("1 l -> dm^3");
        assert!((outval(&litre).unwrap() - 1.0).abs() < 1e-9);
        let speed = service.submit("100 km/h -> m/s");
        assert!((outval(&speed).unwrap() - 27.7777778).abs() < 1e-6);
        assert_eq!(speed.value.unwrap().unit_symbol(), Some("m/s"));
    }

    #[test]
    fn adding_compatible_units_auto_picks_a_unit() {
        let mut service = service();
        let outcome = service.submit("1 m + 50 cm");
        assert!((outval(&outcome).unwrap() - 1.5).abs() < 1e-9);
        assert_eq!(outcome.value.unwrap().unit_symbol(), Some("m"));
    }

    #[test]
    fn addition_with_units_picks_a_single_unit() {
        let mut service = service();
        let outcome = service.submit("20 kN + 300 N");
        let quantity = outcome.value.unwrap();
        // 20 kN + 300 N = 20.3 kN, shown with the short unit symbol.
        assert!((quantity.display_value() - 20.3).abs() < 1e-9);
        assert_eq!(quantity.unit_symbol(), Some("kN"));
    }

    #[test]
    fn multiplying_quantities_yields_a_derived_unit() {
        let mut service = service();
        let outcome = service.submit("1 m * 2 m");
        let quantity = outcome.value.unwrap();
        assert!((quantity.display_value() - 2.0).abs() < 1e-9);
        assert_eq!(quantity.unit_symbol(), Some("m^2"));
    }

    #[test]
    fn dividing_quantities_pins_the_conversion_target() {
        let mut service = service();
        let outcome = service.submit("2 kN / 4 m^2 -> kN/m^2");
        let quantity = outcome.value.unwrap();
        assert!((quantity.display_value() - 0.5).abs() < 1e-9);
        assert_eq!(quantity.unit_symbol(), Some("kN/m^2"));
    }

    #[test]
    fn unit_arithmetic_with_a_unit_variable_routes_to_rink() {
        let mut service = service();
        service.submit("f = 20 kN");
        let outcome = service.submit("f + 300 N");
        let quantity = outcome.value.unwrap();
        assert!((quantity.display_value() - 20.3).abs() < 1e-9);
    }

    #[test]
    fn an_incompatible_conversion_errors() {
        let mut service = service();
        let outcome = service.submit("5 N -> bar");
        assert!(outcome.error.is_some());
    }

    #[test]
    fn pure_math_is_unaffected_by_the_units_router() {
        let mut service = service();
        // `e` is a constant (rink knows it too) but must stay on meval.
        assert!((outval(&service.submit("e^0")).unwrap() - 1.0).abs() < 1e-9);
        // `sin` is a function, not a unit, and respects the angle mode.
        service.toggle_angle_mode();
        let outcome = service.submit("sin(90)");
        assert!((outval(&outcome).unwrap() - 1.0).abs() < 1e-9);
    }
}
