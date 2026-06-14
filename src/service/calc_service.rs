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
use crate::domain::variables::VariableStore;

/// Names that may not be used as variables because they collide with the
/// previous-answer keyword or meval's built-in constants.
const RESERVED_NAMES: &[&str] = &["ans", "pi", "e"];

/// The outcome of submitting a line, for the caller's status line.
#[derive(Debug, Clone, PartialEq)]
pub struct SubmitOutcome {
    /// The computed value, when the line succeeded.
    pub value: Option<f64>,
    /// The error message, when the line failed.
    pub error: Option<String>,
}

/// A non-mutating live preview of the current input, for typed feedback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Preview {
    /// Nothing to show (empty input or a `:` command).
    Empty,
    /// The input currently evaluates to this value.
    Value(f64),
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
        let entry = HistoryEntry {
            input: input.to_string(),
            value,
            error: error.clone(),
        };
        self.history.push(entry);
        SubmitOutcome { value, error }
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

    /// Renders `value` for display (rounded, grouped) — for the `Y` copy.
    pub fn format_display(&self, value: f64) -> String {
        format_display(value, &self.settings)
    }

    /// Renders `value` as a plain, full-precision number — for the `y` copy.
    pub fn format_plain(&self, value: f64) -> String {
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
    fn preview_value(&self, input: &str) -> Option<f64> {
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
    fn evaluate_line(&mut self, input: &str, ans: Option<f64>) -> LineResult {
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
    ans: Option<f64>,
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
    ans: Option<f64>,
) -> LineResult {
    if let Some(message) = reject_name(name) {
        return (None, Some(message));
    }
    match ans {
        Some(value) => {
            variables.set(name, value);
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
    ans: Option<f64>,
) -> LineResult {
    if let Some(message) = reject_name(name) {
        return (None, Some(message));
    }
    match eval_expression(evaluator, variables, settings, expr, ans) {
        Ok(value) => {
            variables.set(name, value);
            (Some(value), None)
        }
        Err(message) => (None, Some(message)),
    }
}

/// Preprocesses, substitutes `ans`/variables and evaluates `expr`.
fn eval_expression(
    evaluator: &dyn Evaluator,
    variables: &VariableStore,
    settings: &FormatSettings,
    expr: &str,
    ans: Option<f64>,
) -> Result<f64, String> {
    let prepared = expression::preprocess(expr, settings.decimal_separator);
    let prepared = expression::prepend_ans(&prepared, ans);
    let mut prepared = expression::substitute_ans(&prepared, ans);
    for (name, value) in variables.iter() {
        prepared = expression::substitute_identifier(&prepared, name, *value);
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
        service.history().entries()[index].value
    }

    #[test]
    fn submit_evaluates_and_records_history() {
        let mut service = service();
        let outcome = service.submit("2+3");
        assert_eq!(outcome.value, Some(5.0));
        assert_eq!(service.history().last_value(), Some(5.0));
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
        assert_eq!(service.variables().get("x"), Some(7.0));
        service.submit("y = x + 3");
        assert_eq!(service.variables().get("y"), Some(10.0));
        service.submit("y*2");
        assert_eq!(service.history().last_value(), Some(20.0));
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
        assert_eq!(service.history().last_value(), None);
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
        assert_eq!(service.preview("2+3"), Preview::Value(5.0));
        assert_eq!(service.preview("ans+5"), Preview::Value(15.0));
        assert_eq!(service.preview("2+"), Preview::Incomplete);
        assert_eq!(service.preview("2+3)"), Preview::Invalid);
        assert_eq!(service.preview(""), Preview::Empty);
        assert_eq!(service.preview(":d4"), Preview::Empty);
    }

    #[test]
    fn preview_handles_assignments_without_mutating_state() {
        let mut service = service();
        service.submit("7");
        assert_eq!(service.preview("x = ans + 3"), Preview::Value(10.0));
        // Previewing must not define the variable or add to the history.
        assert_eq!(service.variables().get("x"), None);
        assert_eq!(service.history().len(), 1);
        // A reserved name is invalid, not a value.
        assert_eq!(service.preview("pi = 3"), Preview::Invalid);
    }

    #[test]
    fn inline_comments_are_ignored_but_kept_in_history() {
        let mut service = service();
        let outcome = service.submit("2+3 # the sum");
        assert_eq!(outcome.value, Some(5.0));
        // The full input, including the comment, is stored.
        assert_eq!(service.history().entries()[0].input, "2+3 # the sum");
        // Comments work on assignments too.
        service.submit("x = 5 # a note");
        assert_eq!(service.variables().get("x"), Some(5.0));
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
        assert_eq!(service.history().entries()[2].value, Some(6.0));
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
        assert_eq!(service.preview("ans+3 # sum"), Preview::Value(5.0));
    }
}
