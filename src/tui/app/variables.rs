//! The variables list: navigate, insert a name, copy or remove a value.

use anyhow::Result;
use ratada::nav;

use crate::domain::quantity::Quantity;
use crate::keymap::Action;
use crate::tui::app::App;
use crate::tui::interaction::Interaction;
use crate::tui::views::ViewId;
use crate::tui::views::calc::Mode;

impl App {
    /// Applies an action in the variables list.
    ///
    /// # Errors
    ///
    /// Propagates whatever the reset confirmation reports.
    pub(super) fn apply_variables_action(
        &mut self,
        action: Action,
        io: &mut dyn Interaction,
    ) -> Result<bool> {
        let total = self.service.variables().len();
        match action {
            Action::Up => {
                self.var_selected = nav::cycle(self.var_selected, total, -1);
            }
            Action::Down => {
                self.var_selected = nav::cycle(self.var_selected, total, 1);
            }
            Action::PageUp => {
                self.var_selected =
                    self.page_step(self.var_selected, total, -1);
            }
            Action::PageDown => {
                self.var_selected = self.page_step(self.var_selected, total, 1);
            }
            Action::Top => self.var_selected = 0,
            Action::Bottom => self.var_selected = total.saturating_sub(1),
            Action::InsertVariable => self.insert_variable(),
            Action::CopyPlain => self.copy_variable(false),
            Action::CopyDisplay => self.copy_variable(true),
            Action::DeleteVariable => self.delete_variable(),
            Action::ResetVariables => self.reset_variables(io)?,
            Action::Back => self.active = ViewId::Calc,
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// The name and value of the currently selected variable, if any.
    fn selected_variable(&self) -> Option<(String, Quantity)> {
        self.service
            .variables()
            .iter()
            .nth(self.var_selected)
            .map(|(name, value)| (name.clone(), value.clone()))
    }

    /// Inserts the selected variable's name into the input and returns to Calc.
    fn insert_variable(&mut self) {
        let Some((name, _)) = self.selected_variable() else {
            return;
        };
        self.replace_selection(&name);
        self.active = ViewId::Calc;
        self.mode = Mode::Input;
        self.report(format!("inserted {name}"));
    }

    /// Copies the selected variable's value, plain or as displayed.
    fn copy_variable(&mut self, as_displayed: bool) {
        let Some((_, value)) = self.selected_variable() else {
            return;
        };
        if as_displayed {
            self.copy_display(&value);
        } else {
            self.copy_plain(&value);
        }
    }

    /// Deletes the selected variable and keeps a valid selection.
    fn delete_variable(&mut self) {
        let Some((name, _)) = self.selected_variable() else {
            return;
        };
        self.service.remove_variable(&name);
        let total = self.service.variables().len();
        self.var_selected = self.var_selected.min(total.saturating_sub(1));
        self.report(format!("removed {name}"));
    }

    /// Asks, then removes every variable.
    fn reset_variables(&mut self, io: &mut dyn Interaction) -> Result<()> {
        if self.service.variables().is_empty() {
            return Ok(());
        }
        if !self.confirm(io, " Reset all variables? ")? {
            return Ok(());
        }
        self.service.reset_variables();
        self.var_selected = 0;
        self.report("variables reset".to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;

    use super::*;
    use crate::tui::app::testing::{
        key, press, render_to_string, submit, test_app,
    };

    // --- The variables view ---

    #[test]
    fn variables_view_lists_and_resets_after_confirmation() {
        let mut app = test_app();
        submit(&mut app, "7");
        submit(&mut app, "=x");
        assert_eq!(
            app.service()
                .variables()
                .get("x")
                .map(Quantity::display_value),
            Some(7.0),
        );

        press(&mut app, key(KeyCode::F(4)));
        assert_eq!(app.active, ViewId::Variables);
        assert!(render_to_string(&app, 80, 24).contains("x = 7"));

        press(&mut app, key(KeyCode::Char('R')));
        assert_eq!(app.service().variables().len(), 0);
    }

    #[test]
    fn inserting_a_variable_returns_to_the_calc_view() {
        let mut app = test_app();
        submit(&mut app, "7");
        submit(&mut app, "=x");
        press(&mut app, key(KeyCode::F(4)));
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(app.active, ViewId::Calc);
        assert_eq!(app.mode, Mode::Input);
        assert_eq!(app.input, "x");
    }

    /// `PageUp`/`PageDown` resolve in this view's scope, but that alone proves
    /// nothing: they were bound and dispatched into a `_ => false` arm for
    /// several releases, doing nothing at all. This drives the real key path
    /// and asserts the selection actually moved.
    #[test]
    fn page_keys_move_the_selection_in_the_variables_list() {
        let mut app = test_app();
        for index in 0..30 {
            submit(&mut app, &format!("{index}"));
            submit(&mut app, &format!("=v{index:02}"));
        }
        press(&mut app, key(KeyCode::F(4)));
        // The page size is whatever the last draw measured.
        render_to_string(&app, 80, 12);
        assert_eq!(app.var_selected, 0);

        press(&mut app, key(KeyCode::PageDown));
        let after_page_down = app.var_selected;
        assert!(after_page_down > 0, "PageDown must move the selection");

        press(&mut app, key(KeyCode::PageUp));
        assert!(
            app.var_selected < after_page_down,
            "PageUp must move it back",
        );
    }

    /// Page jumps clamp; only the arrows wrap.
    #[test]
    fn a_page_jump_stops_at_the_ends_instead_of_wrapping() {
        let mut app = test_app();
        submit(&mut app, "1");
        submit(&mut app, "=a");
        submit(&mut app, "2");
        submit(&mut app, "=b");
        press(&mut app, key(KeyCode::F(4)));
        render_to_string(&app, 80, 12);

        press(&mut app, key(KeyCode::PageUp));
        assert_eq!(app.var_selected, 0, "clamped at the top");

        press(&mut app, key(KeyCode::PageDown));
        press(&mut app, key(KeyCode::PageDown));
        assert_eq!(app.var_selected, 1, "clamped at the bottom");
    }

    /// A long value must not be cut off without a mark.
    #[test]
    fn an_overlong_variable_row_is_truncated_with_an_ellipsis() {
        let mut app = test_app();
        submit(&mut app, "123456789012345678901234567890");
        submit(&mut app, "=verylongvariablename");
        press(&mut app, key(KeyCode::F(4)));

        let screen = render_to_string(&app, 24, 12);
        assert!(screen.contains('\u{2026}'), "got: {screen}");
    }

    #[test]
    fn esc_leaves_a_list_view_for_the_calc_view() {
        let mut app = test_app();
        press(&mut app, key(KeyCode::F(4)));
        press(&mut app, key(KeyCode::Esc));
        assert_eq!(app.active, ViewId::Calc);
    }
}
