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
            Context::Settings => Ok(self.apply_settings_action(action)),
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
            .map(|action| self.keymap.keys_for(*action).join(", "))
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
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratada::shortcut_hints;

    use super::*;
    use crate::domain::format::AngleMode;
    use crate::tui::app::testing::{
        chord, key, press, render_to_string, submit, test_app, type_text,
        value_at,
    };
    use crate::tui::interaction::Headless;

    // --- Autocomplete ---

    #[test]
    fn typing_an_identifier_prefix_opens_the_suggestion_dropdown() {
        let mut app = test_app();
        type_text(&mut app, "si");
        assert!(app.autocomplete.is_open());
        // Hide the hints so the history area has room for the dropdown box;
        // otherwise the grouped hints squeeze it out (content wins).
        shortcut_hints::set_visible(false);
        let screen = render_to_string(&app, 40, 24);
        shortcut_hints::set_visible(true);
        assert!(screen.contains("sin"));
    }

    #[test]
    fn a_caret_after_an_operator_offers_no_suggestions() {
        let mut app = test_app();
        type_text(&mut app, "2+");
        assert!(!app.autocomplete.is_open());
    }

    #[test]
    fn navigating_and_confirming_replaces_the_typed_prefix() {
        let mut app = test_app();
        type_text(&mut app, "si");
        press(&mut app, key(KeyCode::Down));
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input, "sin");
        assert!(!app.autocomplete.is_open());
    }

    #[test]
    fn a_suggestion_replaces_only_the_word_under_the_caret() {
        let mut app = test_app();
        type_text(&mut app, "2*co");
        press(&mut app, key(KeyCode::Down));
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input, "2*cos");
    }

    #[test]
    fn accepting_mid_word_rewrites_the_whole_identifier() {
        let mut app = test_app();
        render_to_string(&app, 80, 24);
        type_text(&mut app, "sinh");
        // Move the caret into the middle of the word: `si|nh`.
        press(&mut app, key(KeyCode::Left));
        press(&mut app, key(KeyCode::Left));
        // The first match for the `si` prefix is `sin`; accepting it must
        // replace the whole `sinh`, not just its head (which left `sinnh`).
        press(&mut app, key(KeyCode::Down));
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input, "sin");
    }

    #[test]
    fn a_bare_number_offers_no_suggestions() {
        let mut app = test_app();
        type_text(&mut app, "42");
        assert!(!app.autocomplete.is_open());
    }

    #[test]
    fn the_dropdown_never_overwrites_the_header_on_a_short_terminal() {
        let mut app = test_app();
        type_text(&mut app, "si");
        assert!(app.autocomplete.is_open());
        // Too short for the box above the input: it is dropped rather than
        // spilling up over the tab bar.
        let screen = render_to_string(&app, 40, 14);
        assert!(screen.contains("Calc"), "the tab bar must survive intact");
    }

    #[test]
    fn escape_closes_the_dropdown_before_clearing_the_input() {
        let mut app = test_app();
        type_text(&mut app, "si");
        press(&mut app, key(KeyCode::Esc));
        assert!(!app.autocomplete.is_open());
        assert_eq!(app.input, "si", "the first Esc only closes the dropdown");
        press(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input, "", "a second Esc clears the input");
    }

    #[test]
    fn enter_submits_when_no_suggestion_is_highlighted() {
        let mut app = test_app();
        type_text(&mut app, "1+2");
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(value_at(&app, 0), Some(3.0));
        assert!(!app.autocomplete.is_open());
    }

    #[test]
    fn a_defined_variable_is_offered_as_a_suggestion() {
        let mut app = test_app();
        submit(&mut app, "radius = 5");
        type_text(&mut app, "rad");
        assert!(app.autocomplete.is_open());
        press(&mut app, key(KeyCode::Down));
        press(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input, "radius");
    }

    #[test]
    fn switching_views_closes_the_dropdown() {
        let mut app = test_app();
        type_text(&mut app, "si");
        assert!(app.autocomplete.is_open());
        // The Variables view must own `Up`/`Down`, not a lingering dropdown.
        press(&mut app, chord(KeyCode::Char('2'), KeyModifiers::ALT));
        assert_eq!(app.active, ViewId::Variables);
        assert!(!app.autocomplete.is_open());
    }

    #[test]
    fn page_up_still_enters_history_while_the_dropdown_is_open() {
        let mut app = test_app();
        submit(&mut app, "7");
        type_text(&mut app, "si");
        // A wide render puts the caret on the first line, so `Up` would qualify
        // too; `PageUp` is the guaranteed escape while the dropdown owns `Up`.
        render_to_string(&app, 80, 24);
        assert!(app.autocomplete.is_open());
        press(&mut app, key(KeyCode::PageUp));
        assert_eq!(app.mode, Mode::History);
    }

    // --- History search ---

    #[test]
    fn searching_recalls_the_chosen_expression_into_the_input() {
        let mut app = test_app();
        submit(&mut app, "1+1");
        submit(&mut app, "2*3");
        // Items are newest first, so index 1 is the older "1+1".
        let mut io = Headless::accepting().picking(1);
        app.dispatch(chord(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut io)
            .expect("dispatch");
        assert_eq!(app.input, "1+1");
        assert_eq!(app.mode, Mode::Input);
        assert_eq!(app.cursor.pos, 3);
    }

    #[test]
    fn dismissing_the_search_leaves_the_input_untouched() {
        let mut app = test_app();
        submit(&mut app, "9-4");
        type_text(&mut app, "5+");
        let mut io = Headless::declining();
        app.dispatch(chord(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut io)
            .expect("dispatch");
        assert_eq!(app.input, "5+");
    }

    #[test]
    fn searching_an_empty_history_reports_and_opens_nothing() {
        let mut app = test_app();
        let mut io = Headless::accepting().picking(0);
        app.dispatch(chord(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut io)
            .expect("dispatch");
        assert_eq!(io.asked, 0, "no picker opens without history");
        assert_eq!(app.status.as_deref(), Some("no history yet"));
    }

    #[test]
    fn a_forced_quit_inside_the_search_quits_the_app() {
        let mut app = test_app();
        submit(&mut app, "1");
        let mut io = Headless::force_quitting();
        app.dispatch(chord(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut io)
            .expect("dispatch");
        assert!(app.quit);
    }

    // --- Command palette ---

    #[test]
    fn the_palette_runs_the_chosen_action() {
        let mut app = test_app();
        // The catalog lists ToggleAngle before the palette entry, so it keeps
        // its index once the palette itself is filtered out.
        let index = Action::all()
            .filter(|a| *a != Action::OpenPalette)
            .position(|a| a == Action::ToggleAngle)
            .expect("angle action is in the catalog");
        assert_eq!(app.service().settings().angle_mode, AngleMode::Rad);
        let mut io = Headless::accepting().picking(index);
        app.dispatch(chord(KeyCode::Char('p'), KeyModifiers::CONTROL), &mut io)
            .expect("dispatch");
        assert_eq!(app.service().settings().angle_mode, AngleMode::Deg);
    }

    #[test]
    fn a_forced_quit_inside_the_palette_quits_the_app() {
        let mut app = test_app();
        let mut io = Headless::force_quitting();
        app.dispatch(chord(KeyCode::Char('p'), KeyModifiers::CONTROL), &mut io)
            .expect("dispatch");
        assert!(app.quit);
    }

    // --- Quitting ---

    #[test]
    fn ctrl_q_inside_a_confirm_dialog_quits_and_abandons_the_action() {
        let mut app = test_app();
        submit(&mut app, "10");
        render_to_string(&app, 80, 24);
        press(&mut app, key(KeyCode::Up));

        let mut forced = Headless::force_quitting();
        let flow = app.dispatch(key(KeyCode::Char('d')), &mut forced).unwrap();
        assert!(matches!(flow, Flow::Quit));
        assert!(app.quit);
        assert_eq!(
            app.service().history().len(),
            1,
            "a forced quit is not a yes",
        );
    }

    #[test]
    fn ctrl_q_inside_the_help_overlay_quits() {
        let mut app = test_app();
        let mut forced = Headless::force_quitting();
        let flow = app.dispatch(key(KeyCode::F(12)), &mut forced).unwrap();
        assert!(matches!(flow, Flow::Quit));
        assert!(app.quit);
    }

    #[test]
    fn closing_the_help_overlay_normally_keeps_the_app_running() {
        let mut app = test_app();
        let mut declining = Headless::declining();
        let flow = app.dispatch(key(KeyCode::F(12)), &mut declining).unwrap();
        assert!(matches!(flow, Flow::Continue));
        assert!(!app.quit);
        assert_eq!(declining.asked, 1);
    }

    #[test]
    fn the_colon_q_command_quits() {
        let mut app = test_app();
        submit(&mut app, ":q");
        assert!(app.quit);
    }

    #[test]
    fn q_quits_from_a_list_view_but_not_from_the_input() {
        let mut app = test_app();
        press(&mut app, key(KeyCode::Char('q')));
        assert!(!app.quit, "q is a character in the input");

        press(&mut app, key(KeyCode::Esc));
        press(&mut app, key(KeyCode::F(4)));
        press(&mut app, key(KeyCode::Char('q')));
        assert!(app.quit);
    }
}
