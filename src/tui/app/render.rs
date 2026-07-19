//! Drawing one frame: the shared app frame, then the active view.
//!
//! Every view is drawn through [`crate::tui::appframe::render_frame`], so the
//! header, status band and hint band look the same wherever the user is. The
//! calc view reports its measurements back into `App::metrics`, which the key
//! path needs to know how wide the input wraps and how tall a page is.

use ratada::style;
use ratada::theme::Color;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::domain::highlight;
use crate::services::Preview;
use crate::tui::app::App;
use crate::tui::appframe::{self, FrameContext};
use crate::tui::colors::{CaretColors, styles_for};
use crate::tui::views::ViewId;
use crate::tui::views::calc::{self, CalcView, HistoryStyle, Mode, Row};
use crate::tui::views::settings::{self, SettingsView};
use crate::tui::views::variables::{self, Variable, VariablesView};

impl App {
    /// Draws the whole UI for one frame.
    pub(super) fn render_body(&self, frame: &mut Frame) {
        let hints = self.hint_groups();
        let status_right = self.status.clone().unwrap_or_default();
        let content = appframe::render_frame(
            frame,
            &FrameContext {
                skin: &self.skin,
                tabs: ViewId::tabs(),
                active_tab: self.active.index(),
                tabs_locked: matches!(self.mode, Mode::Edit(_)),
                status_left: &self.settings_summary(),
                status_right: &status_right,
                hints: &hints,
            },
        );
        match self.active {
            ViewId::Calc => self.render_calc(frame, content),
            ViewId::Variables => self.render_variables(frame, content),
            ViewId::Settings => self.render_settings(frame, content),
        }
    }

    /// The always-visible settings summary in the status band.
    fn settings_summary(&self) -> String {
        let settings = self.service.settings();
        let values = [
            settings.angle_mode.label().to_string(),
            settings.notation.label().to_string(),
            format!("{} dp", settings.decimals),
            if settings.trim_trailing_zeros {
                "trim".to_string()
            } else {
                "fixed".to_string()
            },
            settings.decimal_separator.to_string(),
            settings::thousands_label(&settings.thousands_separator)
                .to_string(),
            settings::glyphs_label(self.skin.glyphs.variant).to_string(),
        ];
        values.join(" \u{b7} ")
    }

    /// Renders the calculator view and records its measurements.
    fn render_calc(&self, frame: &mut Frame, area: Rect) {
        let entries = self.service.history().entries();
        let rows: Vec<Row<'_>> = entries
            .iter()
            .map(|entry| Row {
                entry,
                result: entry
                    .value
                    .as_ref()
                    .map(|value| self.service.format_display(value))
                    .unwrap_or_default(),
            })
            .collect();
        let row_styles: Vec<Vec<Style>> = entries
            .iter()
            .map(|entry| self.styles_of(&entry.input))
            .collect();
        let input_styles = self.styles_of(&self.input);
        // Colour of the autocomplete suggestion text. Set a concrete RGB here.
        let completion_text = Color::Rgb(190, 190, 190);
        let completion = self.autocomplete.lines(
            &self.skin.palette,
            0,
            style::fg(completion_text),
        );

        let view = CalcView {
            rows: &rows,
            selected: self.selected,
            mode: self.mode,
            input: &self.input,
            cursor: self.cursor,
            input_styles: &input_styles,
            row_styles: &row_styles,
            completion,
            caret: CaretColors::from_palette(&self.skin.palette),
            style: HistoryStyle {
                spacing: self.history.spacing,
                separator: self.history.separator,
            },
            accent_color: style::to_ratatui(self.skin.palette.accent),
            error_color: style::to_ratatui(self.skin.palette.error),
            warn: self.warn(),
            feedback: self.live_feedback_line(),
            input_max_lines: self.input_max_lines,
        };
        self.metrics
            .set(calc::render(frame, area, &self.skin, &view));
    }

    /// The per-character highlight styles for `text`.
    fn styles_of(&self, text: &str) -> Vec<Style> {
        let kinds = highlight::classify(text, self.service.variables());
        styles_for(&kinds, &self.highlight)
    }

    /// The dim live-feedback line for the input border: a result preview when
    /// the expression is valid, a muted warning when it looks complete but
    /// invalid, and nothing while typing, empty or disabled.
    fn live_feedback_line(&self) -> Option<Line<'static>> {
        if !self.live_feedback
            || self.mode != Mode::Input
            || self.input.is_empty()
        {
            return None;
        }
        match self.service.preview(&self.input) {
            Preview::Value(value) => {
                let text =
                    format!(" = {} ", self.service.format_display(&value));
                Some(feedback_line(text, self.skin.palette.accent))
            }
            Preview::Invalid => {
                let text = format!(" {} ", self.warn());
                Some(feedback_line(text, self.skin.palette.error))
            }
            Preview::Empty | Preview::Incomplete => None,
        }
    }

    /// Renders the variables view, recording its height for the page keys.
    fn render_variables(&self, frame: &mut Frame, area: Rect) {
        let variables = self.variable_rows();
        let height = variables::render(
            frame,
            area,
            &self.skin,
            &VariablesView {
                variables: &variables,
                selected: self.var_selected,
                offset: &self.var_offset,
            },
        );
        self.list_height.set(height);
    }

    /// The variables, formatted for display.
    fn variable_rows(&self) -> Vec<Variable> {
        self.service
            .variables()
            .iter()
            .map(|(name, value)| Variable {
                name: name.clone(),
                value: self.service.format_display(value),
            })
            .collect()
    }

    /// Renders the settings view, recording its height for the page keys.
    fn render_settings(&self, frame: &mut Frame, area: Rect) {
        let height = settings::render(
            frame,
            area,
            &self.skin,
            &SettingsView {
                values: self.setting_values(),
                selected: self.setting_selected,
                offset: &self.setting_offset,
            },
        );
        self.list_height.set(height);
    }

    /// The values the settings rows read from.
    pub(super) fn setting_values(&self) -> settings::Values<'_> {
        settings::Values {
            settings: self.service.settings(),
            glyphs: self.skin.glyphs.variant,
            theme: &self.theme,
        }
    }
}

/// One dim feedback line in `color`, as drawn into the input border.
fn feedback_line(text: String, color: Color) -> Line<'static> {
    let style = style::fg(color).add_modifier(Modifier::DIM);
    Line::from(Span::styled(text, style))
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratada::shortcut_hints;

    use crate::config::{CALCLI_THEME, Config};
    use crate::storage::UiState;
    use crate::tui::app::testing::{
        app_with_state, chord, key, press, render_to_string, submit, test_app,
    };
    use crate::tui::views::ViewId;

    // --- Rendering ---

    #[test]
    fn the_frame_shows_the_brand_the_tabs_and_the_status_band() {
        let app = test_app();
        let screen = render_to_string(&app, 80, 24);
        assert!(screen.contains("calcli"), "the brand");
        assert!(screen.contains("Calc"), "the tab bar");
        assert!(screen.contains("input"), "the input box");
        // The settings summary lives in the status band, dot-separated.
        assert!(screen.contains("RAD \u{b7} DEC \u{b7} 3 dp \u{b7} trim"));
    }

    #[test]
    fn the_footer_closes_with_the_global_group() {
        let app = test_app();
        let groups = app.hint_groups();
        let (label, hints) = groups.last().expect("a Global group");
        assert_eq!(label, "Global");
        let keys: Vec<&str> = hints.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"f12/?"), "help: {keys:?}");
        assert!(keys.contains(&"q"), "quit: {keys:?}");
        assert!(keys.iter().any(|k| k.contains("ctrl+q")), "{keys:?}");
        assert!(keys.iter().any(|k| k.contains("f1")), "{keys:?}");
    }

    #[test]
    fn the_help_overlay_ends_with_the_same_global_group_as_the_footer() {
        let app = test_app();
        let footer = app.hint_groups();
        let help = app.help_sections();
        assert_eq!(footer.last(), help.last(), "one source, two callers");
    }

    #[test]
    fn no_hint_anywhere_hardcodes_ctrl_q_outside_the_global_group() {
        let app = test_app();
        let sections = app.help_sections();
        let (_, global) = sections.last().expect("a Global group");
        for (label, hints) in &sections[..sections.len() - 1] {
            for (keys, _) in hints {
                assert!(
                    !keys.contains("ctrl+q"),
                    "ctrl+q hardcoded in {label:?}",
                );
            }
        }
        assert!(global.iter().any(|(keys, _)| keys.contains("ctrl+q")));
    }

    #[test]
    fn the_edit_mode_footer_offers_no_tab_switch() {
        let mut app = test_app();
        submit(&mut app, "10");
        render_to_string(&app, 80, 24);
        press(&mut app, key(KeyCode::Up));
        press(&mut app, key(KeyCode::Enter));

        let groups = app.hint_groups();
        let labels: Vec<&str> =
            groups.iter().map(|(label, _)| label.as_str()).collect();
        assert!(!labels.contains(&"Views"), "{labels:?}");
        assert!(labels.contains(&"Editing"), "{labels:?}");
    }

    #[test]
    fn live_feedback_previews_valid_and_warns_on_invalid() {
        let mut app = test_app();
        for character in "2+3".chars() {
            press(&mut app, key(KeyCode::Char(character)));
        }
        assert!(render_to_string(&app, 80, 24).contains("= 5"));

        press(&mut app, key(KeyCode::Char(')')));
        let screen = render_to_string(&app, 80, 24);
        assert!(screen.contains('\u{26a0}'));
        assert!(!screen.contains("= 5"));
    }

    #[test]
    fn live_feedback_stays_silent_while_typing() {
        let mut app = test_app();
        for character in "2+".chars() {
            press(&mut app, key(KeyCode::Char(character)));
        }
        let screen = render_to_string(&app, 80, 24);
        assert!(!screen.contains('\u{26a0}'));
        assert!(!screen.contains('='));
    }

    #[test]
    fn a_highlighted_input_keeps_every_typed_character() {
        let mut app = test_app();
        for character in "sin(pi)+x".chars() {
            press(&mut app, key(KeyCode::Char(character)));
        }
        assert!(render_to_string(&app, 80, 24).contains("sin(pi)+x"));
    }

    #[test]
    fn a_long_history_entry_wraps_instead_of_truncating() {
        let mut app = test_app();
        submit(&mut app, "111111111111111111+7");
        press(&mut app, key(KeyCode::Esc));

        // Narrow enough that the entry must wrap, tall enough that the grouped
        // hint band still leaves the history a viewport.
        let screen = render_to_string(&app, 24, 40);
        assert!(!screen.contains('\u{2026}'), "no ellipsis");
        assert!(screen.contains("+7"), "the tail wrapped");
    }

    #[test]
    fn a_short_terminal_drops_the_hints_rather_than_the_input_box() {
        let app = test_app();
        let screen = render_to_string(&app, 40, 12);
        assert!(screen.contains("input"), "the input box survives");
        assert!(!screen.contains("f12/?"), "the hint band gives way");
    }

    #[test]
    fn restoring_a_persisted_theme_keeps_the_configured_colour_overrides() {
        // `restore` runs `set_theme` for the saved theme name, so a real
        // restart went through the theme path. Resolving without the overrides
        // there silently undid calcli's own defaults: the red block cursor
        // turned accent-green and the input field turned bright.
        let config = Config::default();
        let expected = config.palette();

        let restored = UiState {
            theme: Some(CALCLI_THEME.to_string()),
            ..UiState::default()
        };
        let app = app_with_state(&config, &restored);

        let palette = app.skin().palette;
        assert_eq!(palette.cursor, expected.cursor, "the red block cursor");
        assert_eq!(palette.input_bg, expected.input_bg);
        assert_eq!(palette.input_bg_active, expected.input_bg_active);
        assert_ne!(palette.cursor, palette.accent);
    }

    #[test]
    fn cycling_the_theme_at_runtime_keeps_the_colour_overrides() {
        let mut app = test_app();
        let cursor = app.skin().palette.cursor;
        app.set_theme("monochrome");
        assert_eq!(app.skin().palette.cursor, cursor, "the override survives");
        // The theme's own colours did change.
        assert_ne!(
            app.skin().palette.accent,
            Config::default().palette().accent
        );
    }

    #[test]
    fn the_ui_state_round_trips_through_persistence() {
        let mut app = test_app();
        press(&mut app, chord(KeyCode::Char('3'), KeyModifiers::ALT));
        let state = app.persisted_state();
        assert_eq!(state.ui.active_view, ViewId::Settings.index());
        assert_eq!(state.ui.theme.as_deref(), Some("calcli"));
        assert_eq!(state.ui.hints_visible, Some(shortcut_hints::visible()));
    }

    #[test]
    fn every_view_renders_at_a_tiny_size_without_panicking() {
        let mut app = test_app();
        for view in ViewId::all() {
            app.active = view;
            render_to_string(&app, 12, 5);
        }
    }
}
