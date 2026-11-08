//! The key path: one press in, one [`Flow`] out.
//!
//! A press is offered to the suggestion dropdown first, then resolved against
//! the keymap in the current [`Context`](crate::keymap::Context), then handed
//! to the matching view handler. Whatever nobody claims and could be text falls
//! through to the editor, which is what keeps `q`, `?`, `y` and `d` typeable.
//!
//! Dialogs open through the [`Interaction`] port rather than a live terminal,
//! so the whole path can be driven key by key in a test.

use anyhow::Result;
use crossterm::event::KeyEvent;
use ratada::Flow;
use ratada::autocomplete::{AcOutcome, Autocomplete};
use ratada::command_palette::CommandItem;

use crate::domain::completion;
use crate::keymap::{Action, Context};
use crate::tui::app::App;
use crate::tui::bindings;
use crate::tui::interaction::{Interaction, Selection};
use crate::tui::text_edit::TextCursor;
use crate::tui::views::ViewId;
use crate::tui::views::calc::Mode;

impl App {
    /// Handles one key press against `io`, the port the dialogs open through.
    ///
    /// Separated from [`Screen::handle_key`](ratada::Screen::handle_key) so a
    /// test can drive the whole key path with a terminal-free [`Interaction`].
    ///
    /// # Errors
    ///
    /// Propagates whatever a dialog reports while drawing or reading input.
    pub(crate) fn dispatch(
        &mut self,
        key: KeyEvent,
        io: &mut dyn Interaction,
    ) -> Result<Flow> {
        // A transient status clears on the next key press.
        self.status = None;
        // `F5` reloads the configuration from every context, like `Ctrl+Q`
        // quits from every context.
        if ratada::reload::is_reload_key(key) {
            self.request_reload(io)?;
            return Ok(self.flow());
        }
        let context = self.context();

        if self.completion_consumed(key) {
            return Ok(self.flow());
        }

        let handled = match self.keymap.action_for(&key, context) {
            Some(action) => self.apply_action(action, io)?,
            None => false,
        };
        // Anything the keymap did not claim belongs to the text editor.
        if !handled && context.is_text_editing() {
            self.apply_edit_key(key);
        }
        self.refresh_completion();
        Ok(self.flow())
    }

    /// `F5`: checks `config.toml`; its problems are shown and nothing is
    /// reloaded, a clean file ends the TUI with the reload signal so `main`
    /// builds everything anew.
    fn request_reload(&self, io: &mut dyn Interaction) -> Result<()> {
        let path = self
            .config_path
            .clone()
            .unwrap_or_else(crate::util::paths::config_file);
        let problems = crate::config::config_problems(&path);
        let checked = if problems.is_empty() {
            Ok(())
        } else {
            Err(ratada::reload::ConfigProblems {
                title: format!("{} has problems", path.display()),
                body: problems.join("\n"),
            })
        };
        io.reload_config(self, checked)?;
        Ok(())
    }

    /// Whether an open suggestion dropdown took `key` for itself.
    ///
    /// It claims its own navigation first, so `Up`/`Down`/`Enter`/`Esc` steer
    /// the dropdown rather than the input field; keys it ignores fall through
    /// to the normal path.
    fn completion_consumed(&mut self, key: KeyEvent) -> bool {
        if !self.autocomplete.is_open() {
            return false;
        }
        match self.autocomplete.on_key(key) {
            AcOutcome::Accepted(value) => {
                self.accept_completion(&value);
                true
            }
            AcOutcome::Navigated | AcOutcome::Closed => true,
            AcOutcome::Ignored => false,
        }
    }

    /// Rebuilds the suggestion dropdown from the current variables and the
    /// built-in names, matched against the identifier under the caret. Only the
    /// input field completes; every other context leaves the dropdown closed.
    fn refresh_completion(&mut self) {
        // Only the input field completes. `mode` alone is not enough: it stays
        // `Input` after switching to another view, so gate on the full context
        // or the dropdown would linger there and steal `Up`/`Down`.
        let query = if self.context() == Context::Input {
            let range =
                completion::identifier_before(&self.input, self.cursor.pos);
            self.input
                .chars()
                .take(range.end)
                .skip(range.start)
                .collect()
        } else {
            String::new()
        };
        self.autocomplete =
            Autocomplete::new(completion::candidates(self.service.variables()));
        self.autocomplete.refresh(&query);
    }

    /// Replaces the whole identifier under the caret with an accepted
    /// suggestion, so completing mid-word rewrites all of it rather than
    /// leaving its tail behind.
    fn accept_completion(&mut self, value: &str) {
        let range = completion::identifier_at(&self.input, self.cursor.pos);
        self.cursor.move_to(range.start);
        self.cursor.extend_to(range.end);
        self.replace_selection(value);
    }

    /// Applies `action`, returning whether it was consumed.
    ///
    /// # Errors
    ///
    /// Propagates whatever a dialog opened by the action reports.
    pub(super) fn apply_action(
        &mut self,
        action: Action,
        io: &mut dyn Interaction,
    ) -> Result<bool> {
        if self.apply_global_action(action, io)? {
            return Ok(true);
        }
        match self.context() {
            Context::Input => self.apply_input_action(action, io),
            Context::Edit => self.apply_edit_action(action),
            Context::History => self.apply_history_action(action, io),
            Context::Variables => self.apply_variables_action(action, io),
            Context::Settings => self.apply_settings_action(action, io),
        }
    }

    /// Applies an action that works in every view; returns whether it did.
    fn apply_global_action(
        &mut self,
        action: Action,
        io: &mut dyn Interaction,
    ) -> Result<bool> {
        match action {
            Action::ViewCalc => self.switch_to(ViewId::Calc),
            Action::ViewVariables => self.switch_to(ViewId::Variables),
            Action::ViewSettings => self.switch_to(ViewId::Settings),
            Action::CycleNotation => {
                self.service.cycle_notation();
                self.report(format!(
                    "notation: {}",
                    self.service.settings().notation.label()
                ));
            }
            Action::ToggleAngle => {
                self.service.toggle_angle_mode();
                self.report(format!(
                    "angle: {}",
                    self.service.settings().angle_mode.label()
                ));
            }
            Action::ToggleDecimalSeparator => {
                self.service.toggle_decimal_separator();
                self.report(format!(
                    "decimal separator: {}",
                    self.service.settings().decimal_separator
                ));
            }
            Action::ToggleTrim => {
                self.service.toggle_trim_trailing_zeros();
                let state = if self.service.settings().trim_trailing_zeros {
                    "trimmed"
                } else {
                    "fixed"
                };
                self.report(format!("trailing zeros: {state}"));
            }
            Action::CopyLast => match self.service.history().last_value() {
                Some(value) => self.copy_plain(&value),
                None => self.report("no result yet".to_string()),
            },
            Action::SearchHistory => self.search_history(io)?,
            Action::OpenPalette => self.open_palette(io)?,
            Action::OpenHelp => {
                let answer = io.help(self)?;
                self.absorb(answer);
            }
            Action::Quit => self.request_quit(io),
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Switches views, unless an in-place edit locks the tab bar.
    fn switch_to(&mut self, view: ViewId) {
        if matches!(self.mode, Mode::Edit(_)) {
            self.report("finish the edit first".to_string());
            return;
        }
        self.active = view;
    }

    /// Raises the soft quit, which the toolkit questions when configured to.
    pub(super) fn request_quit(&mut self, io: &mut dyn Interaction) {
        if io.may_quit(self) {
            self.quit = true;
        }
    }

    /// Opens the fuzzy history finder and recalls the chosen expression into
    /// the input. A `Ctrl+Q` inside it quits the app.
    fn search_history(&mut self, io: &mut dyn Interaction) -> Result<()> {
        let items: Vec<String> = self
            .service
            .history()
            .entries()
            .iter()
            .rev()
            .map(|entry| entry.input.clone())
            .collect();
        if items.is_empty() {
            self.report("no history yet".to_string());
            return Ok(());
        }
        match io.pick(self, " Search history ", &items)? {
            Selection::Index(index) => self.recall(&items[index]),
            Selection::None => {}
            Selection::ForcedQuit => self.quit = true,
        }
        Ok(())
    }

    /// Recalls `expression` into the input field on the Calc view, caret at the
    /// end, ready to edit or submit.
    fn recall(&mut self, expression: &str) {
        self.active = ViewId::Calc;
        self.mode = Mode::Input;
        self.selected = None;
        self.input = expression.to_string();
        self.cursor = TextCursor::at(self.input.chars().count());
    }

    /// Opens the command palette and runs the chosen action. Every action but
    /// the palette itself is listed; one unavailable in the current context is
    /// shown dimmed and cannot be picked. A `Ctrl+Q` inside it quits the app.
    fn open_palette(&mut self, io: &mut dyn Interaction) -> Result<()> {
        let context = self.context();
        let actions: Vec<Action> = Action::all()
            .filter(|a| *a != Action::OpenPalette)
            .collect();
        // The key hints are owned here so the borrowed `CommandItem`s can point
        // into them for the length of the call.
        let keys: Vec<String> = actions
            .iter()
            .map(|action| self.keymap.labels_for(*action).join(", "))
            .collect();
        let items: Vec<CommandItem<'_>> = actions
            .iter()
            .zip(&keys)
            .map(|(action, key_hint)| CommandItem {
                label: action.description(),
                category: bindings::category_of(*action),
                key_hint,
                enabled: action.scope().is_active_in(context),
            })
            .collect();
        match io.palette(self, &items)? {
            Selection::Index(index) => {
                self.apply_action(actions[index], io)?;
            }
            Selection::None => {}
            Selection::ForcedQuit => self.quit = true,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
