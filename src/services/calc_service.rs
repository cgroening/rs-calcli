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
use crate::domain::variables::VariableStore;
use crate::services::eval::{self, eval_expression, reject_name};

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
    /// The history and variables before the last change to either, for the
    /// one-level undo (`u`).
    undo: Option<(History, VariableStore)>,
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
            undo: None,
        }
    }

    /// Puts the history and the variables back to how they were before the
    /// last change to either (one level). Returns whether there was one.
    pub fn undo(&mut self) -> bool {
        let Some((history, variables)) = self.undo.take() else {
            return false;
        };
        self.history = history;
        self.variables = variables;
        // The settings may have changed since; values follow the active ones.
        self.recompute_all();
        true
    }

    /// Whether [`undo`](Self::undo) has a change to take back.
    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    /// Keeps the current history and variables for [`undo`](Self::undo),
    /// right before a change to either.
    fn remember(&mut self) {
        self.undo = Some((self.history.clone(), self.variables.clone()));
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
    ///
    /// # Examples
    ///
    /// Each line may continue from the previous answer through `ans`:
    ///
    /// ```
    /// use calcli::config::Config;
    /// use calcli::domain::evaluator::F64Evaluator;
    /// use calcli::domain::history::History;
    /// use calcli::domain::quantity::Quantity;
    /// use calcli::domain::variables::VariableStore;
    /// use calcli::services::CalcService;
    ///
    /// let mut service = CalcService::new(
    ///     Box::new(F64Evaluator::new()),
    ///     Config::default().format_settings(),
    ///     History::new(100),
    ///     VariableStore::new(),
    /// );
    ///
    /// service.submit("10");
    /// let outcome = service.submit("ans * 3");
    ///
    /// assert_eq!(
    ///     outcome.value.as_ref().map(Quantity::display_value),
    ///     Some(30.0),
    /// );
    /// assert_eq!(service.history().len(), 2);
    /// ```
    pub fn submit(&mut self, input: &str) -> SubmitOutcome {
        self.remember();
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
        self.remember();
        self.history.set_input(index, new_input.to_string());
        self.recompute(index);
    }

    /// Removes the entry at `index` and re-evaluates the tail.
    pub fn delete_entry(&mut self, index: usize) {
        self.remember();
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
        let target = step_index(index, delta, len);
        if target != index {
            self.remember();
            self.history.swap(index, target);
            self.recompute(index.min(target));
        }
        target
    }

    /// Inserts a blank entry at `index` and re-evaluates the tail. The caller
    /// typically edits it immediately.
    pub fn insert_entry(&mut self, index: usize) {
        self.remember();
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
        self.remember();
        self.history.clear();
    }

    /// Re-evaluates the entire history, regenerating values and errors under
    /// the
    /// current settings. Used on startup so restored entries are consistent
    /// with
    /// the active settings (and with each other).
    pub fn recompute_all(&mut self) {
        self.recompute(0);
    }

    /// Removes every variable.
    pub fn reset_variables(&mut self) {
        self.remember();
        self.variables.clear();
    }

    /// Removes a single variable by name.
    pub fn remove_variable(&mut self, name: &str) {
        self.remember();
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

    /// Sets the thousands group separator used for display. Input parsing is
    /// tolerant of any grouping, so no recompute is needed.
    pub fn set_thousands_separator(&mut self, separator: String) {
        self.settings.thousands_separator = separator;
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
    /// variable assignments in order. Borrows the engine, variables and
    /// settings
    /// disjointly from the history so the closure can mutate the store.
    fn recompute(&mut self, start: usize) {
        let evaluator = self.evaluator.as_ref();
        let variables = &mut self.variables;
        let settings = &self.settings;
        self.history.recompute_from(start, |input, ans| {
            eval::evaluate_line(evaluator, variables, settings, input, ans)
        });
    }

    /// Evaluates a single line against the current engine, variables and
    /// settings (used by [`submit`](Self::submit)).
    fn evaluate_line(
        &mut self,
        input: &str,
        ans: Option<Quantity>,
    ) -> LineResult {
        eval::evaluate_line(
            self.evaluator.as_ref(),
            &mut self.variables,
            &self.settings,
            input,
            ans,
        )
    }
}

/// Steps `index` by `delta` within a list of `len` entries, clamped to its
/// ends. `len` must not be zero.
///
/// `index` and `delta` both cross the API boundary from the UI, so the step is
/// computed with `checked_*`: a plain `index as isize + delta` panics in debug
/// and wraps in release once the values grow past `isize`. Anything that does
/// not fit saturates at the end it was heading for, which is what clamping
/// would have done anyway.
fn step_index(index: usize, delta: isize, len: usize) -> usize {
    let last = len - 1;
    let Ok(current) = isize::try_from(index) else {
        return last;
    };
    let Some(target) = current.checked_add(delta) else {
        return if delta < 0 { 0 } else { last };
    };
    match usize::try_from(target) {
        Ok(target) => target.min(last),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests;
