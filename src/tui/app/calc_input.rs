//! Typing an expression, and editing a history line in place.
//!
//! Both modes share the input buffer and its caret; what differs is where the
//! text ends up on `Enter` and how wide it wraps. The `:` commands live here
//! too, because they arrive through the same submit path as an expression.

use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::domain::expression;
use crate::domain::format::{AngleMode, Notation};
use crate::keymap::Action;
use crate::services::SubmitOutcome;
use crate::tui::app::App;
use crate::tui::interaction::Interaction;
use crate::tui::text_edit;
use crate::tui::views::calc::Mode;

impl App {
    /// Applies an action while typing an expression.
    ///
    /// # Errors
    ///
    /// Propagates whatever a dialog opened by the action reports.
    pub(super) fn apply_input_action(
        &mut self,
        action: Action,
        io: &mut dyn Interaction,
    ) -> Result<bool> {
        match action {
            Action::Submit => self.submit_input(io)?,
            Action::ClearInput => self.reset_input(),
            Action::EnterHistory => {
                // `Up` only leaves the field from its first display line, so a
                // wrapped expression stays navigable.
                if !self.cursor_is_on_first_line() {
                    return Ok(false);
                }
                self.enter_history();
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Whether the caret sits on the input's first display line.
    fn cursor_is_on_first_line(&self) -> bool {
        let width = self.metrics.get().input_width.max(1);
        let offsets = text_edit::wrap_offsets(&self.input, width);
        let total = self.input.chars().count();
        let (line, _) =
            text_edit::cursor_to_display(&offsets, total, self.cursor.pos);
        line == 0
    }

    /// Evaluates the input buffer (or runs a `:` command) and clears it.
    fn submit_input(&mut self, io: &mut dyn Interaction) -> Result<()> {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }
        // Drop the inline comment to detect a `:` command; otherwise the full
        // text is submitted (a comment-only line becomes a note entry).
        let code = expression::strip_comment(&text).trim();
        if let Some(command) = code.strip_prefix(':') {
            let command = command.to_string();
            self.reset_input();
            let message = self.run_command(&command, io)?;
            self.status = Some(message);
            return Ok(());
        }
        let outcome = self.service.submit(&text);
        self.status = Some(self.outcome_status(&outcome));
        self.selected = None;
        self.reset_input();
        Ok(())
    }

    /// The status message for a submitted line.
    fn outcome_status(&self, outcome: &SubmitOutcome) -> String {
        match (&outcome.value, &outcome.error) {
            (_, Some(error)) => error.clone(),
            (Some(value), None) => {
                format!("= {}", self.service.format_display(value))
            }
            (None, None) => String::new(),
        }
    }

    /// Runs a `:` command (without the leading colon), returning a status line.
    fn run_command(
        &mut self,
        command: &str,
        io: &mut dyn Interaction,
    ) -> Result<String> {
        let message = match command.trim() {
            // The soft quit, questioned when `confirm_quit` is on. `Ctrl+Q`
            // stays the unconditional escape hatch.
            "q" | "quit" => {
                self.request_quit(io);
                String::new()
            }
            "deg" => {
                self.service.set_angle_mode(AngleMode::Deg);
                "angle: DEG".to_string()
            }
            "rad" => {
                self.service.set_angle_mode(AngleMode::Rad);
                "angle: RAD".to_string()
            }
            "clear" => {
                self.clear_history(io)?;
                self.status.clone().unwrap_or_default()
            }
            other => self.run_notation_command(other),
        };
        Ok(message)
    }

    /// Parses the `:d`/`:s`/`:si` notation commands with optional decimals.
    fn run_notation_command(&mut self, command: &str) -> String {
        let Some((notation, rest)) = split_notation_command(command) else {
            return format!("unknown command ':{command}'");
        };
        self.service.set_notation(notation);
        if rest.is_empty() {
            return format!("notation: {}", notation.label());
        }
        match rest.parse::<usize>() {
            Ok(decimals) => {
                self.service.set_decimals(decimals);
                format!("notation: {} ({decimals} dp)", notation.label())
            }
            Err(_) => format!("invalid decimals: '{rest}'"),
        }
    }

    /// Applies an action while editing a history line in place.
    ///
    /// # Errors
    ///
    /// Never fails today; the `Result` matches the other action handlers so
    /// [`App::apply_action`] can treat them alike.
    pub(super) fn apply_edit_action(&mut self, action: Action) -> Result<bool> {
        let Mode::Edit(index) = self.mode else {
            return Ok(false);
        };
        match action {
            Action::ApplyEdit => {
                let text = self.input.clone();
                self.service.edit_entry(index, &text);
                self.editing_inserted = false;
                self.finish_edit(index);
                self.report("line updated".to_string());
            }
            Action::CancelEdit => {
                if self.editing_inserted {
                    self.cancel_insert(index);
                } else {
                    self.finish_edit(index);
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Leaves edit mode back to history navigation.
    pub(super) fn finish_edit(&mut self, index: usize) {
        self.mode = Mode::History;
        self.selected = Some(index);
        self.reset_input();
    }

    /// Cancels a freshly inserted line: removes the blank entry and returns to
    /// history navigation.
    fn cancel_insert(&mut self, index: usize) {
        self.editing_inserted = false;
        self.service.delete_entry(index);
        self.reset_input();
        let total = self.service.history().len();
        if total == 0 {
            self.leave_history();
        } else {
            self.mode = Mode::History;
            self.selected = Some(index.min(total - 1));
        }
    }

    /// Feeds a key the keymap did not claim to the text editor.
    ///
    /// The mode is `Wrapped`, not `Multiline`: an expression is one logical
    /// line, so `Enter` stays the caller's (it submits) and a multi-line paste
    /// collapses instead of smuggling a `\n` into the buffer.
    pub(super) fn apply_edit_key(&mut self, key: KeyEvent) {
        let mode = text_edit::EditMode::Wrapped {
            width: self.edit_width(),
            height: self.input_max_lines.max(1),
        };
        if text_edit::handle_clipboard(
            &mut self.input,
            &mut self.cursor,
            key,
            mode,
        ) {
            return;
        }
        let mode = text_edit::EditMode::Wrapped {
            width: self.edit_width(),
            height: self.input_max_lines.max(1),
        };
        text_edit::apply_edit_key(
            &mut self.input,
            &mut self.cursor,
            key,
            mode,
            None,
        );
    }

    /// The column count the input wraps at: the history width while editing a
    /// line in place, so the text sits exactly where the entry is shown.
    fn edit_width(&self) -> usize {
        let metrics = self.metrics.get();
        match self.mode {
            Mode::Edit(_) => metrics.history_width,
            _ => metrics.input_width,
        }
        .max(1)
    }
}

/// Splits a notation command into its notation and the decimals that follow.
fn split_notation_command(command: &str) -> Option<(Notation, &str)> {
    if let Some(rest) = command.strip_prefix("si") {
        return Some((Notation::SiPrefixed, rest));
    }
    if let Some(rest) = command.strip_prefix('d') {
        return Some((Notation::Decimal, rest));
    }
    command
        .strip_prefix('s')
        .map(|rest| (Notation::Scientific, rest))
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;
    use crate::tui::app::testing::{
        chord, key, press, render_to_string, submit, test_app,
    };
    use crate::tui::views::ViewId;

    // --- Typing and evaluating ---

    #[test]
    fn typing_and_enter_records_a_result() {
        let mut app = test_app();
        submit(&mut app, "2+3");
        assert_eq!(
            app.service()
                .history()
                .last_value()
                .map(|q| q.display_value()),
            Some(5.0),
        );
        assert!(render_to_string(&app, 80, 24).contains("= 5"));
    }

    #[test]
    fn a_bare_digit_is_typed_rather_than_switching_tabs() {
        let mut app = test_app();
        for character in "123".chars() {
            press(&mut app, key(KeyCode::Char(character)));
        }
        assert_eq!(app.input, "123");
        assert_eq!(app.active, ViewId::Calc);
    }

    #[test]
    fn q_and_question_mark_are_typed_into_an_expression() {
        let mut app = test_app();
        press(&mut app, key(KeyCode::Char('q')));
        press(&mut app, key(KeyCode::Char('?')));
        assert_eq!(app.input, "q?");
        assert!(!app.quit);
    }

    #[test]
    fn alt_digits_switch_views_while_typing() {
        let mut app = test_app();
        press(&mut app, chord(KeyCode::Char('2'), KeyModifiers::ALT));
        assert_eq!(app.active, ViewId::Variables);
        press(&mut app, chord(KeyCode::Char('3'), KeyModifiers::ALT));
        assert_eq!(app.active, ViewId::Settings);
        press(&mut app, chord(KeyCode::Char('1'), KeyModifiers::ALT));
        assert_eq!(app.active, ViewId::Calc);
        press(&mut app, key(KeyCode::F(4)));
        assert_eq!(app.active, ViewId::Variables);
    }

    #[test]
    fn alt_gr_types_a_digit_instead_of_switching_views() {
        let mut app = test_app();
        let alt_gr = KeyModifiers::ALT | KeyModifiers::CONTROL;
        press(&mut app, chord(KeyCode::Char('2'), alt_gr));
        assert_eq!(app.active, ViewId::Calc, "AltGr must not switch tabs");
    }

    #[test]
    fn function_keys_toggle_settings_from_the_input() {
        let mut app = test_app();
        assert_eq!(app.service().settings().angle_mode, AngleMode::Rad);
        press(&mut app, key(KeyCode::F(3)));
        assert_eq!(app.service().settings().angle_mode, AngleMode::Deg);

        assert_eq!(app.service().settings().decimal_separator, '.');
        press(&mut app, key(KeyCode::F(5)));
        assert_eq!(app.service().settings().decimal_separator, ',');

        assert!(app.service().settings().trim_trailing_zeros);
        press(&mut app, key(KeyCode::F(6)));
        assert!(!app.service().settings().trim_trailing_zeros);
        assert_eq!(app.status.as_deref(), Some("trailing zeros: fixed"));
    }
}
