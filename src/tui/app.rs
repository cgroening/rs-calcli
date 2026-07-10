//! The TUI state and its [`Screen`] implementation.
//!
//! [`App`] owns the calculator service and the view-only state, translates keys
//! into service calls through [`crate::keymap`] and renders through the shared
//! [`appframe`]. Modals are blocking (`ratada::modal`), so they dim the live
//! view behind them instead of a black screen.

use std::cell::Cell;
use std::collections::BTreeMap;

use anyhow::Result;
use crossterm::event::KeyEvent;
use ratada::quit::QuitConfirm;
use ratada::theme::{
    ColorOverrides, DEFAULT_THEME, GlyphVariant, Glyphs, Palette, Skin,
    ThemeRegistry,
};
use ratada::{Flow, Screen, Tui, clipboard, modal, quit, shortcut_hints};
use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::config::Config;
use crate::domain::format::{AngleMode, Notation};
use crate::domain::highlight;
use crate::domain::quantity::Quantity;
use crate::keymap::{Action, Context, Keymap};
use crate::services::{CalcService, Preview};
use crate::storage::{
    PersistedEntry, PersistedSettings, PersistedState, PersistedValue, UiState,
};
use crate::tui::appframe::{self, FrameContext, HintGroups};
use crate::tui::bindings;
use crate::tui::colors::{CaretColors, Highlight, styles_for, to_ratatui};
use crate::tui::interaction::{Answer, Interaction, Modals};
use crate::tui::text_edit::{self, TextCursor};
use crate::tui::views::ViewId;
use crate::tui::views::calc::{
    self, CalcView, HistoryStyle, Metrics, Mode, Row,
};
use crate::tui::views::settings::{self, SettingsView};
use crate::tui::views::variables::{self, Variable, VariablesView};

/// The label of the closing hint group, shown in the footer and the help.
const GLOBAL_GROUP: &str = "Global";

/// The whole TUI state: the calculator service plus view-only fields.
pub struct App {
    service: CalcService,
    skin: Skin,
    registry: ThemeRegistry,
    theme: String,
    /// The `[appearance.colors]` overrides, re-applied whenever the theme
    /// changes so a runtime theme switch never loses them.
    overrides: BTreeMap<String, String>,
    keymap: Keymap,
    highlight: Highlight,
    confirm_delete: bool,

    history: HistoryStyle,
    input_max_lines: usize,
    live_feedback: bool,

    active: ViewId,
    input: String,
    cursor: TextCursor,
    mode: Mode,
    selected: Option<usize>,
    var_selected: usize,
    setting_selected: usize,

    status: Option<String>,
    quit: bool,
    editing_inserted: bool,

    metrics: Cell<Metrics>,
    var_offset: Cell<usize>,
    setting_offset: Cell<usize>,
}

impl App {
    /// Builds the app from a configured service, the loaded config and the
    /// restored UI state.
    pub fn new(service: CalcService, config: &Config, ui: &UiState) -> Self {
        let registry = config.theme_registry();
        let skin =
            Skin::new(config.palette(), Glyphs::new(config.appearance.glyphs));
        let mut app = App {
            service,
            skin,
            registry,
            theme: config.appearance.theme.clone(),
            overrides: config.appearance.colors.clone(),
            keymap: config.keymap(),
            highlight: Highlight::new(&config.highlight),
            confirm_delete: config.confirm_delete,
            history: HistoryStyle {
                spacing: config.history_spacing,
                separator: config
                    .history_separator
                    .then(|| to_ratatui(config.palette().border)),
            },
            input_max_lines: config.input_max_lines,
            live_feedback: config.live_feedback,
            active: ViewId::Calc,
            input: String::new(),
            cursor: TextCursor::at(0),
            mode: Mode::Input,
            selected: None,
            var_selected: 0,
            setting_selected: 0,
            status: None,
            quit: false,
            editing_inserted: false,
            metrics: Cell::new(Metrics::default()),
            var_offset: Cell::new(0),
            setting_offset: Cell::new(0),
        };
        for conflict in app.keymap.conflicts() {
            log::warn!(
                "key '{}' for '{}' is shadowed by '{}'",
                conflict.key,
                conflict.action.config_name(),
                conflict.claimed_by.config_name(),
            );
        }
        app.restore(ui);
        // Only the soft quit is questioned; `Ctrl+Q` stays the unconditional
        // escape hatch either way.
        quit::set_confirm(if config.confirm_quit {
            QuitConfirm::Soft
        } else {
            QuitConfirm::Never
        });
        app.install_quit_guard();
        app
    }

    /// Registers how the quit confirmation is drawn, with the current skin.
    /// Re-run from [`Self::set_skin`] so the dialog follows the active theme.
    fn install_quit_guard(&self) {
        let skin = self.skin;
        quit::set_guard(move |tui, _kind, background| {
            modal::confirm(tui, &skin, " Quit? ", background)
        });
    }

    /// The single seam for changing the skin: the quit guard holds a copy of it
    /// and has to be re-registered whenever the theme or the glyphs change.
    fn set_skin(&mut self, skin: Skin) {
        self.skin = skin;
        self.install_quit_guard();
    }

    /// Switches the active theme by name, falling back to the default when the
    /// name is unknown.
    ///
    /// The configured colour overrides are re-applied on top, exactly as at
    /// startup. Dropping them here would undo calcli's own defaults (the red
    /// block cursor, the input fills) on every restart, because restoring the
    /// persisted theme name comes through this path.
    fn set_theme(&mut self, name: &str) {
        let name = if self.registry.contains(name) {
            name.to_string()
        } else {
            log::warn!("unknown theme '{name}', using '{DEFAULT_THEME}'");
            DEFAULT_THEME.to_string()
        };
        let base = self.registry.resolve(&name);
        let mut skin = self.skin;
        skin.palette = Palette::resolve(base, &self.color_overrides());
        self.set_skin(skin);
        self.theme = name;
    }

    /// The configured colour overrides, layered over whichever theme is active.
    fn color_overrides(&self) -> ColorOverrides<'_> {
        ColorOverrides::from_lookup(|name| {
            self.overrides.get(name).map_or("", String::as_str)
        })
    }

    /// Switches the glyph variant.
    fn set_glyphs(&mut self, variant: GlyphVariant) {
        let mut skin = self.skin;
        skin.glyphs = Glyphs::new(variant);
        self.set_skin(skin);
    }

    /// Applies the persisted UI state.
    fn restore(&mut self, ui: &UiState) {
        self.active = ViewId::from_index(ui.active_view);
        if let Some(theme) = &ui.theme {
            self.set_theme(theme);
        }
        if let Some(glyphs) = ui.glyphs {
            self.set_glyphs(glyphs);
        }
        if let Some(visible) = ui.hints_visible {
            shortcut_hints::set_visible(visible);
        }
    }

    /// Snapshots the session for persistence: settings, UI, variables, history.
    pub fn persisted_state(&self) -> PersistedState {
        let settings = self.service.settings();
        let history =
            self.service
                .history()
                .entries()
                .iter()
                .map(|entry| PersistedEntry {
                    input: entry.input.clone(),
                    value: entry.value.as_ref().map(Quantity::display_value),
                    unit: entry.value.as_ref().and_then(|value| {
                        value.unit_symbol().map(str::to_string)
                    }),
                })
                .collect();
        let variables = self
            .service
            .variables()
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    PersistedValue {
                        value: value.display_value(),
                        unit: value.unit_symbol().map(str::to_string),
                    },
                )
            })
            .collect();
        PersistedState {
            settings: Some(PersistedSettings {
                notation: settings.notation,
                decimals: settings.decimals,
                angle_mode: settings.angle_mode,
                decimal_separator: settings.decimal_separator.to_string(),
                trim_trailing_zeros: settings.trim_trailing_zeros,
            }),
            ui: UiState {
                active_view: self.active.index(),
                theme: Some(self.theme.clone()),
                glyphs: Some(self.skin.glyphs.variant),
                hints_visible: Some(shortcut_hints::visible()),
            },
            variables,
            history,
        }
    }

    /// The calculator service, for the integration tests.
    pub fn service(&self) -> &CalcService {
        &self.service
    }

    /// The warning marker for the current glyph set.
    fn warn(&self) -> &'static str {
        match self.skin.glyphs.variant {
            GlyphVariant::Unicode => "\u{26a0}",
            GlyphVariant::Ascii => "!",
        }
    }

    /// Which keymap context the current view and mode select.
    fn context(&self) -> Context {
        match (self.active, self.mode) {
            (ViewId::Calc, Mode::Input) => Context::Input,
            (ViewId::Calc, Mode::Edit(_)) => Context::Edit,
            (ViewId::Calc, Mode::History) => Context::History,
            (ViewId::Variables, _) => Context::Variables,
            (ViewId::Settings, _) => Context::Settings,
        }
    }

    /// The active skin, for the dialogs drawn over this view.
    pub(crate) fn skin(&self) -> &Skin {
        &self.skin
    }

    /// Asks before a destructive action, unless `confirm_delete` is off.
    ///
    /// A `Ctrl+Q` inside the dialog quits at once and the action is abandoned:
    /// the toolkit honours the hard quit everywhere, modals included.
    fn confirm(
        &mut self,
        io: &mut dyn Interaction,
        prompt: &str,
    ) -> Result<bool> {
        if !self.confirm_delete {
            return Ok(true);
        }
        let answer = io.confirm(self, prompt)?;
        self.absorb(answer);
        Ok(answer.is_yes())
    }

    /// Records a forced quit raised from inside a dialog.
    fn absorb(&mut self, answer: Answer) {
        if answer.is_forced_quit() {
            self.quit = true;
        }
    }
}

// --- Hints and help ---------------------------------------------------------

impl App {
    /// The closing `Global` group: the app's own chords first, then the
    /// toolkit's (`f1 toggle hints`, `ctrl+q force quit`). One helper feeds
    /// both the footer and the help overlay, so they cannot drift apart.
    fn global_group(&self) -> (String, Vec<(String, String)>) {
        let mut hints = self.keymap.hints(&[Action::OpenHelp, Action::Quit]);
        hints.extend(shortcut_hints::global_bindings());
        (GLOBAL_GROUP.to_string(), hints)
    }

    /// Renders one hint table into owned groups, dropping any that ended up
    /// empty because every action in it was unbound.
    fn groups_of(&self, table: &[bindings::Group]) -> HintGroups {
        table
            .iter()
            .filter_map(|(label, actions)| {
                let hints = self.keymap.hints(actions);
                (!hints.is_empty()).then(|| ((*label).to_string(), hints))
            })
            .collect()
    }

    /// The hint groups of the current view: its own groups, then the settings
    /// chords and the tab switches, closed by the `Global` group.
    ///
    /// An in-place edit locks the tab bar, so the `Views` group is left out
    /// there rather than promising a switch that will not happen.
    fn hint_groups(&self) -> HintGroups {
        let table = match (self.active, self.mode) {
            (ViewId::Calc, Mode::Input) => bindings::INPUT_HINT_GROUPS,
            (ViewId::Calc, Mode::Edit(_)) => bindings::EDIT_HINT_GROUPS,
            (ViewId::Calc, Mode::History) => bindings::HISTORY_HINT_GROUPS,
            (ViewId::Variables, _) => bindings::VARIABLES_HINT_GROUPS,
            (ViewId::Settings, _) => bindings::SETTINGS_HINT_GROUPS,
        };
        let mut groups = self.groups_of(table);
        groups.extend(self.groups_of(&[bindings::SETTINGS_CHORDS]));
        if !matches!(self.mode, Mode::Edit(_)) {
            groups.extend(self.groups_of(&[bindings::VIEW_GROUP]));
        }
        groups.push(self.global_group());
        groups
    }

    /// The help overlay's sections: every view's groups, the shared chords,
    /// then the same `Global` group the footer closes with.
    pub(crate) fn help_sections(&self) -> HintGroups {
        let mut sections: HintGroups = Vec::new();
        for (view, table) in bindings::HELP_TABLES {
            for (label, hints) in self.groups_of(table) {
                sections.push((format!("{view} \u{b7} {label}"), hints));
            }
        }
        sections.extend(self.groups_of(&[bindings::SETTINGS_CHORDS]));
        sections.extend(self.groups_of(&[bindings::VIEW_GROUP]));
        sections.push(self.global_group());
        sections
    }
}

// --- Rendering --------------------------------------------------------------

impl App {
    /// Draws the whole UI for one frame.
    fn render_body(&self, frame: &mut Frame) {
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
    fn render_calc(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
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

        let view = CalcView {
            rows: &rows,
            selected: self.selected,
            mode: self.mode,
            input: &self.input,
            cursor: self.cursor,
            input_styles: &input_styles,
            row_styles: &row_styles,
            caret: CaretColors::from_palette(&self.skin.palette),
            style: HistoryStyle {
                spacing: self.history.spacing,
                separator: self.history.separator,
            },
            accent_color: to_ratatui(self.skin.palette.accent),
            error_color: to_ratatui(self.skin.palette.error),
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
                let style = Style::default()
                    .fg(to_ratatui(self.skin.palette.accent))
                    .add_modifier(Modifier::DIM);
                Some(Line::from(Span::styled(text, style)))
            }
            Preview::Invalid => {
                let text = format!(" {} ", self.warn());
                let style = Style::default()
                    .fg(to_ratatui(self.skin.palette.error))
                    .add_modifier(Modifier::DIM);
                Some(Line::from(Span::styled(text, style)))
            }
            Preview::Empty | Preview::Incomplete => None,
        }
    }

    /// Renders the variables view.
    fn render_variables(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let variables = self.variable_rows();
        variables::render(
            frame,
            area,
            &self.skin,
            &VariablesView {
                variables: &variables,
                selected: self.var_selected,
                offset: &self.var_offset,
            },
        );
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

    /// Renders the settings view.
    fn render_settings(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        settings::render(
            frame,
            area,
            &self.skin,
            &SettingsView {
                values: self.setting_values(),
                selected: self.setting_selected,
                offset: &self.setting_offset,
            },
        );
    }

    /// The values the settings rows read from.
    fn setting_values(&self) -> settings::Values<'_> {
        settings::Values {
            settings: self.service.settings(),
            glyphs: self.skin.glyphs.variant,
            theme: &self.theme,
        }
    }
}

// --- Key handling -----------------------------------------------------------

impl Screen for App {
    type Error = anyhow::Error;

    fn render(&self, frame: &mut Frame) {
        self.render_body(frame);
    }

    fn handle_key(&mut self, key: KeyEvent, tui: &mut Tui) -> Result<Flow> {
        let mut modals = Modals::new(tui);
        self.dispatch(key, &mut modals)
    }
}

impl App {
    /// Whether the loop should keep running.
    fn flow(&self) -> Flow {
        if self.quit {
            Flow::Quit
        } else {
            Flow::Continue
        }
    }

    /// Handles one key press against `io`, the port the dialogs open through.
    ///
    /// Separated from [`Screen::handle_key`] so a test can drive the whole key
    /// path with a terminal-free [`Interaction`].
    pub(crate) fn dispatch(
        &mut self,
        key: KeyEvent,
        io: &mut dyn Interaction,
    ) -> Result<Flow> {
        // A transient status clears on the next key press.
        self.status = None;
        let context = self.context();

        if let Some(action) = self.keymap.action_for(&key, context)
            && self.apply_action(action, io)?
        {
            return Ok(self.flow());
        }
        // Anything the keymap did not claim belongs to the text editor.
        if context.is_text_editing() {
            self.apply_edit_key(key);
        }
        Ok(self.flow())
    }

    /// Applies `action`, returning whether it was consumed.
    fn apply_action(
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
    fn request_quit(&mut self, io: &mut dyn Interaction) {
        if io.may_quit(self) {
            self.quit = true;
        }
    }

    /// Records a transient status message.
    fn report(&mut self, message: String) {
        self.status = Some(message);
    }
}

// --- The calc view ----------------------------------------------------------

impl App {
    /// Applies an action while typing an expression.
    fn apply_input_action(
        &mut self,
        action: Action,
        io: &mut dyn Interaction,
    ) -> Result<bool> {
        match action {
            Action::Submit => self.submit_input(io)?,
            Action::ClearInput => {
                self.input.clear();
                self.cursor = TextCursor::at(0);
            }
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
        let code = crate::domain::expression::strip_comment(&text).trim();
        if let Some(command) = code.strip_prefix(':') {
            let command = command.to_string();
            self.input.clear();
            self.cursor = TextCursor::at(0);
            let message = self.run_command(&command, io)?;
            self.status = Some(message);
            return Ok(());
        }
        let outcome = self.service.submit(&text);
        self.status = Some(self.outcome_status(&outcome));
        self.selected = None;
        self.input.clear();
        self.cursor = TextCursor::at(0);
        Ok(())
    }

    /// The status message for a submitted line.
    fn outcome_status(
        &self,
        outcome: &crate::services::SubmitOutcome,
    ) -> String {
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
        let (notation, rest) = if let Some(rest) = command.strip_prefix("si") {
            (Notation::SiPrefixed, rest)
        } else if let Some(rest) = command.strip_prefix('d') {
            (Notation::Decimal, rest)
        } else if let Some(rest) = command.strip_prefix('s') {
            (Notation::Scientific, rest)
        } else {
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

    /// Enters history navigation, selecting the most recent line.
    fn enter_history(&mut self) {
        let total = self.service.history().len();
        if total == 0 {
            return;
        }
        self.mode = Mode::History;
        self.selected = Some(total - 1);
    }

    /// Returns from history navigation to the input field.
    fn leave_history(&mut self) {
        self.mode = Mode::Input;
        self.selected = None;
    }

    /// Applies an action while browsing the history.
    fn apply_history_action(
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
        self.cursor = TextCursor::at(self.input.chars().count());
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
    fn clear_history(&mut self, io: &mut dyn Interaction) -> Result<()> {
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

    /// Applies an action while editing a history line in place.
    fn apply_edit_action(&mut self, action: Action) -> Result<bool> {
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
    fn finish_edit(&mut self, index: usize) {
        self.mode = Mode::History;
        self.selected = Some(index);
        self.input.clear();
        self.cursor = TextCursor::at(0);
    }

    /// Cancels a freshly inserted line: removes the blank entry and returns to
    /// history navigation.
    fn cancel_insert(&mut self, index: usize) {
        self.editing_inserted = false;
        self.service.delete_entry(index);
        self.input.clear();
        self.cursor = TextCursor::at(0);
        let total = self.service.history().len();
        if total == 0 {
            self.leave_history();
        } else {
            self.mode = Mode::History;
            self.selected = Some(index.min(total - 1));
        }
    }

    /// Feeds a key the keymap did not claim to the text editor.
    fn apply_edit_key(&mut self, key: KeyEvent) {
        if text_edit::handle_clipboard(&mut self.input, &mut self.cursor, key) {
            return;
        }
        // The in-place editor wraps at the history width, as the entry is
        // shown.
        let metrics = self.metrics.get();
        let width = match self.mode {
            Mode::Edit(_) => metrics.history_width,
            _ => metrics.input_width,
        }
        .max(1);
        text_edit::apply_edit_key(
            &mut self.input,
            &mut self.cursor,
            key,
            text_edit::EditMode::Multiline { width },
        );
    }
}

// --- The variables view -----------------------------------------------------

impl App {
    /// Applies an action in the variables list.
    fn apply_variables_action(
        &mut self,
        action: Action,
        io: &mut dyn Interaction,
    ) -> Result<bool> {
        let total = self.service.variables().len();
        match action {
            Action::Up => {
                self.var_selected =
                    ratada::nav::cycle(self.var_selected, total, -1);
            }
            Action::Down => {
                self.var_selected =
                    ratada::nav::cycle(self.var_selected, total, 1);
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
        text_edit::replace_selection(&mut self.input, &mut self.cursor, &name);
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

// --- The settings view ------------------------------------------------------

impl App {
    /// Applies an action in the settings list.
    fn apply_settings_action(&mut self, action: Action) -> bool {
        let rows = settings::Row::all().len();
        match action {
            Action::Up => {
                self.setting_selected =
                    ratada::nav::cycle(self.setting_selected, rows, -1);
            }
            Action::Down => {
                self.setting_selected =
                    ratada::nav::cycle(self.setting_selected, rows, 1);
            }
            Action::Top => self.setting_selected = 0,
            Action::Bottom => self.setting_selected = rows - 1,
            Action::PreviousValue => self.step_setting(-1),
            Action::NextValue => self.step_setting(1),
            Action::Back => self.active = ViewId::Calc,
            _ => return false,
        }
        true
    }

    /// Steps the focused setting's value by `delta`, applied live.
    fn step_setting(&mut self, delta: isize) {
        let row = settings::Row::at(self.setting_selected);
        match row {
            settings::Row::Notation => {
                let next = settings::step_notation(
                    self.service.settings().notation,
                    delta,
                );
                self.service.set_notation(next);
            }
            settings::Row::Decimals => {
                let next = settings::step_decimals(
                    self.service.settings().decimals,
                    delta,
                );
                self.service.set_decimals(next);
            }
            settings::Row::AngleMode => {
                let next = settings::step_angle(
                    self.service.settings().angle_mode,
                    delta,
                );
                self.service.set_angle_mode(next);
            }
            settings::Row::TrailingZeros => {
                self.service.toggle_trim_trailing_zeros();
            }
            settings::Row::DecimalSeparator => {
                self.service.toggle_decimal_separator();
            }
            settings::Row::ThousandsSeparator => {
                let next = settings::step_thousands(
                    &self.service.settings().thousands_separator,
                    delta,
                );
                self.service.set_thousands_separator(next);
            }
            settings::Row::Glyphs => {
                let next = match self.skin.glyphs.variant {
                    GlyphVariant::Unicode => GlyphVariant::Ascii,
                    GlyphVariant::Ascii => GlyphVariant::Unicode,
                };
                self.set_glyphs(next);
            }
            settings::Row::Theme => {
                let next = self.registry.next(&self.theme);
                self.set_theme(&next);
            }
        }
        let value = self.setting_values().of(row);
        self.report(format!("{}: {value}", row.label()));
    }
}

// --- Clipboard --------------------------------------------------------------

impl App {
    /// Copies `value` as a plain, full-precision value (the `y` behaviour).
    fn copy_plain(&mut self, value: &Quantity) {
        let text = self.service.format_plain(value);
        self.report(copy_status(&text));
    }

    /// Copies `value` as displayed: rounded and grouped (the `Y` behaviour).
    fn copy_display(&mut self, value: &Quantity) {
        let text = self.service.format_display(value);
        self.report(copy_status(&text));
    }
}

/// Copies `text` to the clipboard and returns a status message either way.
fn copy_status(text: &str) -> String {
    if clipboard::copy(text) {
        format!("copied {text}")
    } else {
        "clipboard unavailable".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::config::CALCLI_THEME;
    use crate::domain::evaluator::MevalEvaluator;
    use crate::domain::history::History;
    use crate::domain::variables::VariableStore;
    use crate::tui::interaction::Headless;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn chord(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn test_app() -> App {
        let config = Config::default();
        let service = CalcService::new(
            Box::new(MevalEvaluator::new()),
            config.format_settings(),
            History::new(100),
            VariableStore::new(),
        );
        App::new(service, &config, &UiState::default())
    }

    /// Presses `key`, answering every confirmation with yes.
    fn press(app: &mut App, key: KeyEvent) {
        app.dispatch(key, &mut Headless::accepting())
            .expect("dispatch must not fail headlessly");
    }

    /// Types `text` then submits it.
    fn submit(app: &mut App, text: &str) {
        for character in text.chars() {
            press(app, key(KeyCode::Char(character)));
        }
        press(app, key(KeyCode::Enter));
    }

    /// The display value of history entry `index`.
    fn value_at(app: &App, index: usize) -> Option<f64> {
        app.service().history().entries()[index]
            .value
            .as_ref()
            .map(Quantity::display_value)
    }

    /// Renders `app` into a test terminal and returns the screen text.
    fn render_to_string(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        // The measurements the key handling reads are recorded while drawing.
        terminal.draw(|frame| app.render(frame)).expect("draw");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

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
        let service = CalcService::new(
            Box::new(MevalEvaluator::new()),
            config.format_settings(),
            History::new(100),
            VariableStore::new(),
        );
        let mut app = App::new(service, &config, &UiState::default());
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

    #[test]
    fn esc_leaves_a_list_view_for_the_calc_view() {
        let mut app = test_app();
        press(&mut app, key(KeyCode::F(4)));
        press(&mut app, key(KeyCode::Esc));
        assert_eq!(app.active, ViewId::Calc);
    }

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

        let service = CalcService::new(
            Box::new(MevalEvaluator::new()),
            config.format_settings(),
            History::new(100),
            VariableStore::new(),
        );
        let restored = UiState {
            theme: Some(CALCLI_THEME.to_string()),
            ..UiState::default()
        };
        let app = App::new(service, &config, &restored);

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
