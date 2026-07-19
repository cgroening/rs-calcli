//! Browsing and reshaping the history list.
//!
//! Every mutation here goes through the service, which re-evaluates the lines
//! below the change so `ans` and the variable assignments stay consistent with
//! the new order.
//!
//! Navigation deliberately does *not* wrap. The history is a list that is open
//! at the bottom: stepping past the last line hands the keyboard back to the
//! input field below it, which is where the user was heading. Wrapping around
//! to the top instead would skip that. Stepping up past the first line clamps
//! for the same reason - there is nothing above it to reach.

use anyhow::Result;

use crate::keymap::Action;
use crate::tui::app::App;
use crate::tui::interaction::Interaction;
use crate::tui::views::calc::Mode;

impl App {
    /// Enters history navigation, selecting the most recent line.
    pub(super) fn enter_history(&mut self) {
        let total = self.service.history().len();
        if total == 0 {
            return;
        }
        self.mode = Mode::History;
        self.selected = Some(total - 1);
    }

    /// Returns from history navigation to the input field.
    pub(super) fn leave_history(&mut self) {
        self.mode = Mode::Input;
        self.selected = None;
    }

    /// Applies an action while browsing the history.
    ///
    /// # Errors
    ///
    /// Propagates whatever a confirmation dialog reports.
    pub(super) fn apply_history_action(
        &mut self,
        action: Action,
        io: &mut dyn Interaction,
    ) -> Result<bool> {
        let Some(index) = self.selected else {
            self.mode = Mode::Input;
            return Ok(false);
        };
        let total = self.service.history().len();
        let last = total.saturating_sub(1);
        let page = self.metrics.get().view_height.max(1);
        match action {
            Action::Up => self.selected = Some(index.saturating_sub(1)),
            Action::Down => {
                if index < last {
                    self.selected = Some(index + 1);
                } else {
                    self.leave_history();
                }
            }
            Action::Top => self.selected = Some(0),
            Action::Bottom => self.selected = Some(last),
            Action::PageUp => self.selected = Some(index.saturating_sub(page)),
            Action::PageDown => self.selected = Some((index + page).min(last)),
            Action::MoveUp => self.move_selected(index, -1),
            Action::MoveDown => self.move_selected(index, 1),
            Action::CopyPlain => self.copy_selected(index, false),
            Action::CopyDisplay => self.copy_selected(index, true),
            Action::EditEntry => self.start_edit(index),
            Action::InsertBelow => self.insert_line(index + 1),
            Action::InsertAbove => self.insert_line(index),
            Action::DeleteEntry => self.delete_entry(index, io)?,
            Action::ClearHistory => self.clear_history(io)?,
            Action::Back => self.leave_history(),
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Moves the selected entry by `delta` and follows it with the selection.
    fn move_selected(&mut self, index: usize, delta: isize) {
        self.selected = Some(self.service.move_entry(index, delta));
    }

    /// Inserts a blank entry at `at` and starts editing it immediately.
    fn insert_line(&mut self, at: usize) {
        self.service.insert_entry(at);
        self.selected = Some(at);
        self.start_edit(at);
        self.editing_inserted = true;
    }

    /// Begins editing history entry `index` in place.
    fn start_edit(&mut self, index: usize) {
        let Some(entry) = self.service.history().entries().get(index) else {
            return;
        };
        self.input = entry.input.clone();
        self.cursor =
            crate::tui::text_edit::TextCursor::at(self.input.chars().count());
        self.mode = Mode::Edit(index);
        self.editing_inserted = false;
    }

    /// Asks, then deletes history entry `index`.
    fn delete_entry(
        &mut self,
        index: usize,
        io: &mut dyn Interaction,
    ) -> Result<()> {
        if !self.confirm(io, " Delete this line? ")? {
            return Ok(());
        }
        self.service.delete_entry(index);
        let total = self.service.history().len();
        if total == 0 {
            self.leave_history();
        } else {
            self.selected = Some(index.min(total - 1));
        }
        self.report("line deleted".to_string());
        Ok(())
    }

    /// Asks, then clears the whole history.
    ///
    /// # Errors
    ///
    /// Propagates whatever the confirmation dialog reports.
    pub(super) fn clear_history(
        &mut self,
        io: &mut dyn Interaction,
    ) -> Result<()> {
        if self.service.history().is_empty() {
            self.report("history is already empty".to_string());
            return Ok(());
        }
        if !self.confirm(io, " Clear the entire history? ")? {
            return Ok(());
        }
        self.service.clear_history();
        self.leave_history();
        self.report("history cleared".to_string());
        Ok(())
    }

    /// Copies the value of history entry `index`, plain or as displayed.
    fn copy_selected(&mut self, index: usize, as_displayed: bool) {
        let value = self
            .service
            .history()
            .entries()
            .get(index)
            .and_then(|entry| entry.value.clone());
        match value {
            Some(value) if as_displayed => self.copy_display(&value),
            Some(value) => self.copy_plain(&value),
            None => self.report("no value to copy".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;
    use crate::config::Config;
    use crate::tui::app::testing::{
        app_with, chord, key, press, render_to_string, submit, test_app,
        value_at,
    };
    use crate::tui::interaction::Headless;
    use crate::tui::views::ViewId;

    // --- History navigation and editing ---

    #[test]
    fn up_enters_history_and_an_edit_recomputes_the_chain() {
        let mut app = test_app();
        submit(&mut app, "10");
        submit(&mut app, "ans+5");
        render_to_string(&app, 80, 24);

        press(&mut app, key(KeyCode::Up));
        assert_eq!(app.mode, Mode::History);
        press(&mut app, key(KeyCode::Home));
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Edit(0));

        for _ in 0..2 {
            press(&mut app, key(KeyCode::Backspace));
        }
        for character in "20".chars() {
            press(&mut app, key(KeyCode::Char(character)));
        }
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(value_at(&app, 1), Some(25.0));
    }

    #[test]
    fn history_reorder_delete_and_clear_via_keys() {
        let mut app = test_app();
        submit(&mut app, "10");
        submit(&mut app, "ans+5");
        submit(&mut app, "ans*2");
        render_to_string(&app, 80, 24);

        press(&mut app, key(KeyCode::Up));
        assert_eq!(app.selected, Some(2));
        press(&mut app, chord(KeyCode::Up, KeyModifiers::ALT));
        assert_eq!(app.selected, Some(1));
        assert_eq!(value_at(&app, 1), Some(20.0));
        assert_eq!(value_at(&app, 2), Some(25.0));

        press(&mut app, key(KeyCode::Char('d')));
        assert_eq!(app.service().history().len(), 2);

        press(&mut app, key(KeyCode::Char('D')));
        assert_eq!(app.service().history().len(), 0);
    }

    #[test]
    fn a_declined_confirmation_keeps_the_entry() {
        let mut app = test_app();
        submit(&mut app, "10");
        render_to_string(&app, 80, 24);
        press(&mut app, key(KeyCode::Up));

        let mut declining = Headless::declining();
        app.dispatch(key(KeyCode::Char('d')), &mut declining)
            .unwrap();
        assert_eq!(declining.asked, 1);
        assert_eq!(app.service().history().len(), 1);
    }

    #[test]
    fn confirm_delete_off_deletes_without_asking() {
        let config = Config {
            confirm_delete: false,
            ..Config::default()
        };
        let mut app = app_with(&config);
        submit(&mut app, "10");
        render_to_string(&app, 80, 24);
        press(&mut app, key(KeyCode::Up));

        let mut declining = Headless::declining();
        app.dispatch(key(KeyCode::Char('d')), &mut declining)
            .unwrap();
        assert_eq!(declining.asked, 0, "no dialog when confirm_delete is off");
        assert_eq!(app.service().history().len(), 0);
    }

    #[test]
    fn inserting_a_line_enters_edit_and_recomputes() {
        let mut app = test_app();
        submit(&mut app, "10");
        submit(&mut app, "ans+5");
        render_to_string(&app, 80, 24);

        press(&mut app, key(KeyCode::Up));
        press(&mut app, key(KeyCode::Home));
        press(&mut app, key(KeyCode::Char('o')));
        assert_eq!(app.mode, Mode::Edit(1));
        for character in "ans*3".chars() {
            press(&mut app, key(KeyCode::Char(character)));
        }
        press(&mut app, key(KeyCode::Enter));

        // ["10", "ans*3", "ans+5"] -> 10, 30, 35.
        assert_eq!(value_at(&app, 1), Some(30.0));
        assert_eq!(value_at(&app, 2), Some(35.0));
    }

    #[test]
    fn cancelling_an_inserted_line_removes_it() {
        let mut app = test_app();
        submit(&mut app, "10");
        render_to_string(&app, 80, 24);
        press(&mut app, key(KeyCode::Up));
        press(&mut app, key(KeyCode::Char('o')));
        assert_eq!(app.service().history().len(), 2);
        press(&mut app, key(KeyCode::Esc));
        assert_eq!(app.service().history().len(), 1);
        assert_eq!(app.mode, Mode::History);
    }

    #[test]
    fn an_in_place_edit_locks_the_tab_bar() {
        let mut app = test_app();
        submit(&mut app, "10");
        render_to_string(&app, 80, 24);
        press(&mut app, key(KeyCode::Up));
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Edit(0));

        press(&mut app, chord(KeyCode::Char('2'), KeyModifiers::ALT));
        assert_eq!(app.active, ViewId::Calc, "the tab switch is locked");
        assert_eq!(app.status.as_deref(), Some("finish the edit first"));
    }
}
