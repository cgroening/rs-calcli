//! The settings list: navigate the rows and step the focused value live; a
//! colour row opens the colour chooser and saves the choice to `config.toml`.

use anyhow::Result;
use ratada::nav;
use ratada::theme::GlyphVariant;

use crate::config::{ConfigError, writer};
use crate::keymap::Action;
use crate::tui::app::App;
use crate::tui::interaction::{ColorPick, Interaction};
use crate::tui::views::ViewId;
use crate::tui::views::settings::{self, Row};

impl App {
    /// Applies an action in the settings list.
    ///
    /// # Errors
    ///
    /// Propagates a failure to draw the colour chooser.
    pub(super) fn apply_settings_action(
        &mut self,
        action: Action,
        io: &mut dyn Interaction,
    ) -> Result<bool> {
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
            Action::PreviousValue | Action::NextValue
                if Row::at(self.setting_selected).is_color() =>
            {
                self.pick_color(Row::at(self.setting_selected), io)?;
            }
            Action::PreviousValue => self.step_setting(-1),
            Action::NextValue => self.step_setting(1),
            Action::ResetValue => {
                let row = Row::at(self.setting_selected);
                if row.is_color() {
                    self.set_color(row, None);
                }
            }
            Action::Back => self.active = ViewId::Calc,
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Opens the colour chooser for the colour `row` and applies the choice;
    /// removing the colour returns it to the theme's value.
    fn pick_color(&mut self, row: Row, io: &mut dyn Interaction) -> Result<()> {
        let initial = self.setting_values().color_of(row);
        match io.pick_color(self, &row.label(), initial)? {
            ColorPick::Color(color) => {
                self.set_color(row, Some(&color.to_hex()))
            }
            ColorPick::Cleared => self.set_color(row, None),
            ColorPick::Cancelled => {}
            ColorPick::ForcedQuit => self.quit = true,
        }
        Ok(())
    }

    /// Sets (or with `None` removes) the colour of `row`, previews it at once
    /// and saves it to the config file.
    pub(super) fn set_color(&mut self, row: Row, color: Option<&str>) {
        match row {
            Row::Color(name) => {
                match color {
                    Some(value) => {
                        self.overrides
                            .insert(name.to_string(), value.to_string());
                    }
                    None => {
                        self.overrides.remove(name);
                    }
                }
                self.save_setting(|path| writer::set_color(path, name, color));
            }
            Row::GroupColor { group, key } => {
                self.color_groups.set_override(group, key, color);
                self.save_setting(|path| {
                    writer::set_group_color(path, group, key, color)
                });
            }
            _ => return,
        }
        let theme = self.theme.clone();
        self.set_theme(&theme);
    }

    /// Saves one setting to the config file, reporting where it went; without
    /// a config file the change lasts this session only.
    fn save_setting(
        &mut self,
        write: impl FnOnce(&std::path::Path) -> Result<(), ConfigError>,
    ) {
        let Some(path) = self.config_path.clone() else {
            self.report("Not saved: no config file".to_string());
            return;
        };
        match write(&path) {
            Ok(()) => self.report(format!("Saved to {}", path.display())),
            Err(error) => self.report(error.to_string()),
        }
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
            Row::DimAmount => {
                let amount = settings::step_dim_amount(self.dim_amount, delta);
                self.dim_amount = amount;
                self.apply_dim();
                self.save_setting(|path| writer::set_dim_amount(path, amount));
            }
            Row::Color(_) | Row::GroupColor { .. } => {}
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

    /// Selects the settings row `row`.
    fn select(app: &mut App, row: Row) {
        app.active = ViewId::Settings;
        app.setting_selected = Row::all()
            .iter()
            .position(|candidate| *candidate == row)
            .expect("the row is listed");
    }

    #[test]
    fn enter_on_a_colour_row_picks_and_saves_the_colour() {
        use crate::theme::Color;
        use crate::tui::interaction::{ColorPick, Headless};

        let dir = std::env::temp_dir()
            .join(format!("calcli-settings-{}", std::process::id()));
        let path = dir.join("config.toml");
        let config = crate::config::Config {
            path: Some(path.clone()),
            ..crate::config::Config::default()
        };
        let mut app = crate::tui::app::testing::app_with(&config);
        let row = Row::GroupColor {
            group: "highlight",
            key: "function",
        };
        select(&mut app, row);
        let mut io = Headless::accepting()
            .choosing_color(ColorPick::Color(Color::Rgb(1, 2, 3)));

        app.apply_settings_action(Action::NextValue, &mut io)
            .unwrap();

        assert_eq!(io.asked, 1, "the chooser opened");
        assert_eq!(
            app.setting_values().color_of(row),
            Some(Color::Rgb(1, 2, 3))
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[appearance.colors.highlight]"), "{text}");

        app.apply_settings_action(Action::ResetValue, &mut io)
            .unwrap();
        assert_ne!(
            app.setting_values().color_of(row),
            Some(Color::Rgb(1, 2, 3)),
            "reset returns the built-in colour"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_dim_colour_reaches_the_toolkit() {
        use crate::tui::interaction::Headless;

        let mut app = test_app();
        app.set_color(Row::Color("dim"), Some("#040506"));
        let render = ratada::dim::dim_render().expect("dim render set");
        assert_eq!(render.foreground.to_hex(), "#040506");
        select(&mut app, Row::DimAmount);
        let mut io = Headless::accepting();
        app.apply_settings_action(Action::NextValue, &mut io)
            .unwrap();
        assert_eq!(app.dim_amount, 45);
        app.set_color(Row::Color("dim"), None);
        assert_eq!(ratada::dim::dim_render(), None);
    }

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
