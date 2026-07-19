//! The settings list: navigate the rows and step the focused value live.

use ratada::nav;
use ratada::theme::GlyphVariant;

use crate::keymap::Action;
use crate::tui::app::App;
use crate::tui::views::ViewId;
use crate::tui::views::settings::{self, Row};

impl App {
    /// Applies an action in the settings list.
    pub(super) fn apply_settings_action(&mut self, action: Action) -> bool {
        let rows = Row::all().len();
        match action {
            Action::Up => {
                self.setting_selected =
                    nav::cycle(self.setting_selected, rows, -1);
            }
            Action::Down => {
                self.setting_selected =
                    nav::cycle(self.setting_selected, rows, 1);
            }
            Action::PageUp => {
                self.setting_selected =
                    self.page_step(self.setting_selected, rows, -1);
            }
            Action::PageDown => {
                self.setting_selected =
                    self.page_step(self.setting_selected, rows, 1);
            }
            Action::Top => self.setting_selected = 0,
            Action::Bottom => self.setting_selected = rows.saturating_sub(1),
            Action::PreviousValue => self.step_setting(-1),
            Action::NextValue => self.step_setting(1),
            Action::Back => self.active = ViewId::Calc,
            _ => return false,
        }
        true
    }

    /// Steps the focused setting's value by `delta`, applied live.
    fn step_setting(&mut self, delta: isize) {
        let row = Row::at(self.setting_selected);
        self.apply_step(row, delta);
        let value = self.setting_values().of(row);
        self.report(format!("{}: {value}", row.label()));
    }

    /// Steps one row's value. The toggles ignore `delta`: with two states,
    /// stepping either way lands on the other one.
    fn apply_step(&mut self, row: Row, delta: isize) {
        let settings = self.service.settings();
        match row {
            Row::Notation => {
                let next = settings::step_notation(settings.notation, delta);
                self.service.set_notation(next);
            }
            Row::Decimals => {
                let next = settings::step_decimals(settings.decimals, delta);
                self.service.set_decimals(next);
            }
            Row::AngleMode => {
                let next = settings::step_angle(settings.angle_mode, delta);
                self.service.set_angle_mode(next);
            }
            Row::TrailingZeros => self.service.toggle_trim_trailing_zeros(),
            Row::DecimalSeparator => self.service.toggle_decimal_separator(),
            Row::ThousandsSeparator => {
                let next = settings::step_thousands(
                    &settings.thousands_separator,
                    delta,
                );
                self.service.set_thousands_separator(next);
            }
            Row::Glyphs => {
                self.set_glyphs(next_glyphs(self.skin.glyphs.variant))
            }
            Row::Theme => {
                let next = self.registry.next(&self.theme);
                self.set_theme(&next);
            }
        }
    }
}

/// The other glyph variant.
fn next_glyphs(current: GlyphVariant) -> GlyphVariant {
    match current {
        GlyphVariant::Unicode => GlyphVariant::Ascii,
        GlyphVariant::Ascii => GlyphVariant::Unicode,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;
    use crate::domain::format::Notation;
    use crate::tui::app::testing::{
        chord, key, press, render_to_string, test_app,
    };
    use crate::tui::views::ViewId;

    // --- The settings view ---

    #[test]
    fn the_settings_view_steps_a_value_live() {
        let mut app = test_app();
        press(&mut app, chord(KeyCode::Char('3'), KeyModifiers::ALT));
        assert_eq!(app.active, ViewId::Settings);
        assert_eq!(app.service().settings().notation, Notation::Decimal);

        // The first row is the notation.
        press(&mut app, key(KeyCode::Right));
        assert_eq!(app.service().settings().notation, Notation::Scientific);
        press(&mut app, key(KeyCode::Left));
        assert_eq!(app.service().settings().notation, Notation::Decimal);
    }

    #[test]
    fn the_settings_view_changes_the_focused_row() {
        let mut app = test_app();
        press(&mut app, chord(KeyCode::Char('3'), KeyModifiers::ALT));
        press(&mut app, key(KeyCode::Down));
        assert_eq!(app.setting_selected, 1);
        // The second row is the decimal count.
        press(&mut app, key(KeyCode::Right));
        assert_eq!(app.service().settings().decimals, 4);
    }

    /// Same trap as in the variables list: bound, dispatched, and silently
    /// dropped. The assertion is on the moved selection, not on the binding.
    #[test]
    fn page_keys_move_the_selection_in_the_settings_list() {
        let mut app = test_app();
        press(&mut app, chord(KeyCode::Char('3'), KeyModifiers::ALT));
        // Two rows visible, so a page is smaller than the list.
        render_to_string(&app, 80, 6);
        assert_eq!(app.setting_selected, 0);

        press(&mut app, key(KeyCode::PageDown));
        let moved = app.setting_selected;
        assert!(moved > 0, "PageDown must move the selection");

        press(&mut app, key(KeyCode::PageUp));
        assert!(app.setting_selected < moved, "PageUp must move it back");
    }

    #[test]
    fn home_and_end_jump_to_the_ends_of_the_settings_list() {
        let mut app = test_app();
        press(&mut app, chord(KeyCode::Char('3'), KeyModifiers::ALT));
        press(&mut app, key(KeyCode::End));
        assert_eq!(app.setting_selected, Row::all().len() - 1);
        press(&mut app, key(KeyCode::Home));
        assert_eq!(app.setting_selected, 0);
    }
}
