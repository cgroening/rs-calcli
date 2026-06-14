//! The Ratatui front-end: the [`App`] state, the event loop and rendering.
//!
//! Layout, top to bottom: a header, the scrollable history (input left, result
//! right), the fixed input field, a settings bar showing every active setting,
//! a transient status line and a footer of shortcut hints. Overlays (variables,
//! help, a confirm modal) draw on top. The calculator core lives in
//! [`crate::service`]; this module only translates keys into service calls and
//! renders the result.

pub mod colors;
pub mod help;
pub mod history_view;
pub mod terminal;
pub mod text_edit;
pub mod variables_view;
pub mod widgets;

use std::cell::Cell;
use std::io;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::config::{Config, GlyphSet};
use crate::domain::format::{AngleMode, Notation};
use crate::service::CalcService;
use crate::storage::{PersistedEntry, PersistedSettings, PersistedState};
use crate::tui::colors::parse_color;
use crate::tui::terminal::{Tui, is_global_quit};
use crate::tui::text_edit::TextCursor;
use crate::tui::widgets::{ConfirmModal, ConfirmResult, hint_line};
use crate::util::clipboard;

/// Where keyboard focus sits in the main view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Typing a new expression in the input field.
    Input,
    /// Browsing the history with a row selected.
    History,
    /// Editing the input of the history row at this index, in place.
    Edit(usize),
}

/// A pending destructive action awaiting confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmAction {
    /// Remove every variable.
    ResetVariables,
}

/// The overlay drawn on top of the main view, if any.
enum Overlay {
    /// No overlay.
    None,
    /// The variables list.
    Variables,
    /// The help screen.
    Help,
    /// A yes/no confirmation for `action`.
    Confirm(ConfirmModal, ConfirmAction),
}

/// The whole TUI state: the calculator service plus view-only fields.
pub struct App {
    service: CalcService,
    accent: Color,
    glyphs: GlyphSet,
    input: String,
    cursor: TextCursor,
    mode: Mode,
    selected: Option<usize>,
    overlay: Overlay,
    var_selected: usize,
    help_scroll: usize,
    status: Option<String>,
    quit: bool,
    history_offset: Cell<usize>,
    view_height: Cell<usize>,
    var_offset: Cell<usize>,
}

impl App {
    /// Builds the app from a configured service and the loaded config.
    pub fn new(service: CalcService, config: &Config) -> Self {
        App {
            service,
            accent: parse_color(&config.theme.accent_color),
            glyphs: config.glyphs,
            input: String::new(),
            cursor: TextCursor::at(0),
            mode: Mode::Input,
            selected: None,
            overlay: Overlay::None,
            var_selected: 0,
            help_scroll: 0,
            status: None,
            quit: false,
            history_offset: Cell::new(0),
            view_height: Cell::new(1),
            var_offset: Cell::new(0),
        }
    }

    /// Snapshots the session for persistence: settings, variables and history.
    pub fn persisted_state(&self) -> PersistedState {
        let settings = self.service.settings();
        let history = self
            .service
            .history()
            .entries()
            .iter()
            .map(|entry| PersistedEntry {
                input: entry.input.clone(),
                value: entry.value,
            })
            .collect();
        let variables = self
            .service
            .variables()
            .iter()
            .map(|(name, value)| (name.clone(), *value))
            .collect();
        PersistedState {
            settings: Some(PersistedSettings {
                notation: settings.notation,
                decimals: settings.decimals,
                angle_mode: settings.angle_mode,
                decimal_separator: settings.decimal_separator.to_string(),
            }),
            variables,
            history,
        }
    }

    // --- Accessors used by the render submodules ---

    /// The resolved accent colour.
    pub fn accent(&self) -> Color {
        self.accent
    }

    /// The calculator service.
    pub fn service(&self) -> &CalcService {
        &self.service
    }

    /// The current focus mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The selected history index, if any.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The current input buffer.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// The input caret.
    pub fn cursor(&self) -> TextCursor {
        self.cursor
    }

    /// The selected variable index in the overlay.
    pub fn var_selected(&self) -> usize {
        self.var_selected
    }

    /// The help overlay scroll position.
    pub fn help_scroll(&self) -> usize {
        self.help_scroll
    }

    /// The stored history scroll offset.
    pub fn history_offset(&self) -> usize {
        self.history_offset.get()
    }

    /// Records the history scroll offset chosen while rendering.
    pub fn set_history_offset(&self, offset: usize) {
        self.history_offset.set(offset);
    }

    /// Records the history viewport height (for paging).
    pub fn set_view_height(&self, height: usize) {
        self.view_height.set(height);
    }

    /// The stored variables scroll offset.
    pub fn var_offset(&self) -> usize {
        self.var_offset.get()
    }

    /// Records the variables scroll offset chosen while rendering.
    pub fn set_var_offset(&self, offset: usize) {
        self.var_offset.set(offset);
    }

    /// The warning marker for the current glyph set.
    pub fn warn(&self) -> &'static str {
        match self.glyphs {
            GlyphSet::Unicode => "\u{26a0}",
            GlyphSet::Ascii => "!",
        }
    }
}

/// Runs the event loop until the user quits, leaving the app holding the final
/// state for the caller to persist.
///
/// # Errors
/// Returns an I/O error if drawing or reading from the terminal fails.
pub fn run(app: &mut App, tui: &mut Tui) -> io::Result<()> {
    loop {
        tui.terminal.draw(|frame| render(app, frame))?;
        let key = tui.read_key()?;
        if is_global_quit(key) {
            return Ok(());
        }
        app.handle_key(key);
        if app.quit {
            return Ok(());
        }
    }
}

impl App {
    /// Dispatches a key to the right handler for the current overlay and mode.
    fn handle_key(&mut self, key: KeyEvent) {
        self.status = None;
        if let Overlay::Confirm(..) = self.overlay {
            self.handle_confirm_key(key);
            return;
        }
        if self.handle_global_key(key) {
            return;
        }
        match self.overlay {
            Overlay::Variables => self.handle_variables_key(key),
            Overlay::Help => self.handle_help_key(key),
            Overlay::None | Overlay::Confirm(..) => match self.mode {
                Mode::Input => self.handle_input_key(key),
                Mode::History => self.handle_history_key(key),
                Mode::Edit(index) => self.handle_edit_key(key, index),
            },
        }
    }

    /// Handles the global function-key shortcuts; returns whether it consumed
    /// the key.
    fn handle_global_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::F(1) => self.toggle_help(),
            KeyCode::F(2) => {
                self.service.cycle_notation();
                self.status = Some(format!(
                    "notation: {}",
                    self.service.settings().notation.label()
                ));
            }
            KeyCode::F(3) => {
                self.service.toggle_angle_mode();
                self.status = Some(format!(
                    "angle: {}",
                    self.service.settings().angle_mode.label()
                ));
            }
            KeyCode::F(4) => self.toggle_variables(),
            KeyCode::F(5) => {
                self.service.toggle_decimal_separator();
                self.status = Some(format!(
                    "decimal separator: {}",
                    self.service.settings().decimal_separator
                ));
            }
            _ => return false,
        }
        true
    }

    /// Toggles the help overlay.
    fn toggle_help(&mut self) {
        self.overlay = match self.overlay {
            Overlay::Help => Overlay::None,
            _ => {
                self.help_scroll = 0;
                Overlay::Help
            }
        };
    }

    /// Toggles the variables overlay.
    fn toggle_variables(&mut self) {
        self.overlay = match self.overlay {
            Overlay::Variables => Overlay::None,
            _ => {
                self.var_selected = 0;
                self.var_offset.set(0);
                Overlay::Variables
            }
        };
    }

    /// Handles keys while typing a new expression.
    fn handle_input_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => self.submit_input(),
            KeyCode::Up | KeyCode::PageUp => self.enter_history(),
            KeyCode::Char('y') if ctrl => {
                match self.service.history().last_value() {
                    Some(value) => self.copy_plain(value),
                    None => self.status = Some("no result yet".to_string()),
                }
            }
            KeyCode::Esc => {
                self.input.clear();
                self.cursor = TextCursor::at(0);
            }
            _ => {
                if text_edit::handle_clipboard(
                    &mut self.input,
                    &mut self.cursor,
                    key,
                ) {
                    return;
                }
                text_edit::apply_edit_key(
                    &mut self.input,
                    &mut self.cursor,
                    key,
                );
            }
        }
    }

    /// Evaluates the input buffer (or runs a `:` command) and clears it.
    fn submit_input(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(command) = text.strip_prefix(':') {
            let message = self.run_command(command);
            self.status = Some(message);
        } else {
            let outcome = self.service.submit(&text);
            self.status = Some(self.outcome_status(&outcome));
            self.selected = None;
        }
        self.input.clear();
        self.cursor = TextCursor::at(0);
    }

    /// The status message for a submitted line.
    fn outcome_status(
        &self,
        outcome: &crate::service::SubmitOutcome,
    ) -> String {
        match (&outcome.value, &outcome.error) {
            (_, Some(error)) => error.clone(),
            (Some(value), None) => {
                format!("= {}", self.service.format_display(*value))
            }
            (None, None) => String::new(),
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

    /// Handles keys while browsing the history.
    fn handle_history_key(&mut self, key: KeyEvent) {
        let Some(index) = self.selected else {
            self.mode = Mode::Input;
            return;
        };
        let total = self.service.history().len();
        let last = total.saturating_sub(1);
        let page = self.view_height.get().max(1);
        match key.code {
            KeyCode::Up => self.selected = Some(index.saturating_sub(1)),
            KeyCode::Down => {
                if index < last {
                    self.selected = Some(index + 1);
                } else {
                    self.leave_history();
                }
            }
            KeyCode::Home => self.selected = Some(0),
            KeyCode::End => self.selected = Some(last),
            KeyCode::PageUp => self.selected = Some(index.saturating_sub(page)),
            KeyCode::PageDown => self.selected = Some((index + page).min(last)),
            KeyCode::Char('y') => self.copy_selected(index, false),
            KeyCode::Char('Y') => self.copy_selected(index, true),
            KeyCode::Char('e') | KeyCode::Enter => self.start_edit(index),
            KeyCode::Char('d') | KeyCode::Delete => self.delete_selected(index),
            KeyCode::Esc => self.leave_history(),
            _ => {}
        }
    }

    /// Returns from history navigation to the input field.
    fn leave_history(&mut self) {
        self.mode = Mode::Input;
        self.selected = None;
    }

    /// Copies the value of history entry `index`, plain or as displayed.
    fn copy_selected(&mut self, index: usize, as_displayed: bool) {
        let value = self
            .service
            .history()
            .entries()
            .get(index)
            .and_then(|e| e.value);
        match value {
            Some(value) if as_displayed => self.copy_display(value),
            Some(value) => self.copy_plain(value),
            None => self.status = Some("no value to copy".to_string()),
        }
    }

    /// Begins editing history entry `index` in place.
    fn start_edit(&mut self, index: usize) {
        let Some(entry) = self.service.history().entries().get(index) else {
            return;
        };
        self.input = entry.input.clone();
        self.cursor = TextCursor::at(self.input.chars().count());
        self.mode = Mode::Edit(index);
    }

    /// Deletes history entry `index` and keeps a valid selection.
    fn delete_selected(&mut self, index: usize) {
        self.service.delete_entry(index);
        let total = self.service.history().len();
        if total == 0 {
            self.leave_history();
        } else {
            self.selected = Some(index.min(total - 1));
        }
        self.status = Some("line deleted".to_string());
    }

    /// Handles keys while editing a history line in place.
    fn handle_edit_key(&mut self, key: KeyEvent, index: usize) {
        match key.code {
            KeyCode::Enter => {
                let text = self.input.clone();
                self.service.edit_entry(index, &text);
                self.finish_edit(index);
                self.status = Some("line updated".to_string());
            }
            KeyCode::Esc => self.finish_edit(index),
            _ => {
                if text_edit::handle_clipboard(
                    &mut self.input,
                    &mut self.cursor,
                    key,
                ) {
                    return;
                }
                text_edit::apply_edit_key(
                    &mut self.input,
                    &mut self.cursor,
                    key,
                );
            }
        }
    }

    /// Leaves edit mode back to history navigation.
    fn finish_edit(&mut self, index: usize) {
        self.mode = Mode::History;
        self.selected = Some(index);
        self.input.clear();
        self.cursor = TextCursor::at(0);
    }

    /// Handles keys in the variables overlay.
    fn handle_variables_key(&mut self, key: KeyEvent) {
        let total = self.service.variables().len();
        match key.code {
            KeyCode::Esc => self.overlay = Overlay::None,
            KeyCode::Up => {
                self.var_selected = cycle(self.var_selected, -1, total)
            }
            KeyCode::Down => {
                self.var_selected = cycle(self.var_selected, 1, total)
            }
            KeyCode::Enter => self.insert_variable(),
            KeyCode::Char('y') => self.copy_variable(false),
            KeyCode::Char('Y') => self.copy_variable(true),
            KeyCode::Char('d') | KeyCode::Delete => self.delete_variable(),
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if total > 0 {
                    self.overlay = Overlay::Confirm(
                        ConfirmModal::new("Reset all variables?"),
                        ConfirmAction::ResetVariables,
                    );
                }
            }
            _ => {}
        }
    }

    /// The name of the currently selected variable, if any.
    fn selected_variable(&self) -> Option<(String, f64)> {
        self.service
            .variables()
            .iter()
            .nth(self.var_selected)
            .map(|(name, value)| (name.clone(), *value))
    }

    /// Inserts the selected variable's name into the input and closes the
    /// overlay.
    fn insert_variable(&mut self) {
        let Some((name, _)) = self.selected_variable() else {
            return;
        };
        text_edit::replace_selection(&mut self.input, &mut self.cursor, &name);
        self.overlay = Overlay::None;
        self.mode = Mode::Input;
        self.status = Some(format!("inserted {name}"));
    }

    /// Copies the selected variable's value, plain or as displayed.
    fn copy_variable(&mut self, as_displayed: bool) {
        let Some((_, value)) = self.selected_variable() else {
            return;
        };
        if as_displayed {
            self.copy_display(value);
        } else {
            self.copy_plain(value);
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
        self.status = Some(format!("removed {name}"));
    }

    /// Handles keys for the confirm modal.
    fn handle_confirm_key(&mut self, key: KeyEvent) {
        let (result, action) = match &self.overlay {
            Overlay::Confirm(modal, action) => (modal.handle_key(key), *action),
            _ => return,
        };
        match result {
            ConfirmResult::Yes => {
                match action {
                    ConfirmAction::ResetVariables => {
                        self.service.reset_variables();
                        self.var_selected = 0;
                        self.status = Some("variables reset".to_string());
                    }
                }
                self.overlay = Overlay::Variables;
            }
            ConfirmResult::No => self.overlay = Overlay::Variables,
            ConfirmResult::Pending => {}
        }
    }

    /// Handles keys in the help overlay.
    fn handle_help_key(&mut self, key: KeyEvent) {
        let max_scroll = help::line_count();
        match key.code {
            KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?') => {
                self.overlay = Overlay::None;
            }
            KeyCode::Up => {
                self.help_scroll = self.help_scroll.saturating_sub(1)
            }
            KeyCode::Down => {
                self.help_scroll = (self.help_scroll + 1).min(max_scroll);
            }
            KeyCode::PageUp => {
                self.help_scroll = self.help_scroll.saturating_sub(10)
            }
            KeyCode::PageDown => {
                self.help_scroll = (self.help_scroll + 10).min(max_scroll);
            }
            _ => {}
        }
    }

    /// Runs a `:` command (without the leading colon) and returns a status line.
    fn run_command(&mut self, command: &str) -> String {
        match command.trim() {
            "deg" => {
                self.service.set_angle_mode(AngleMode::Deg);
                "angle: DEG".to_string()
            }
            "rad" => {
                self.service.set_angle_mode(AngleMode::Rad);
                "angle: RAD".to_string()
            }
            "clear" => {
                self.service.clear_history();
                self.leave_history();
                "history cleared".to_string()
            }
            other => self.run_notation_command(other),
        }
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

    /// Copies `value` as a plain, full-precision number (the `y` behaviour).
    fn copy_plain(&mut self, value: f64) {
        let text = self.service.format_plain(value);
        self.status = Some(copy_status(&text));
    }

    /// Copies `value` as displayed: rounded and grouped (the `Y` behaviour).
    fn copy_display(&mut self, value: f64) {
        let text = self.service.format_display(value);
        self.status = Some(copy_status(&text));
    }
}

/// Cyclically moves an index by `delta` within `len` items (empty stays 0).
fn cycle(index: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let next = index as isize + delta;
    next.rem_euclid(len as isize) as usize
}

/// Copies `text` to the clipboard and returns a status message either way.
fn copy_status(text: &str) -> String {
    if clipboard::copy(text) {
        format!("copied {text}")
    } else {
        "clipboard unavailable".to_string()
    }
}

/// Draws the whole UI for one frame.
fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(1),    // history
            Constraint::Length(3), // input
            Constraint::Length(1), // settings bar
            Constraint::Length(1), // status line
            Constraint::Length(1), // footer hints
        ])
        .split(area);

    render_header(frame, chunks[0], app);
    history_view::render(frame, chunks[1], app);
    render_input(frame, chunks[2], app);
    render_settings_bar(frame, chunks[3], app);
    render_status(frame, chunks[4], app);
    render_footer(frame, chunks[5], app);

    match &app.overlay {
        Overlay::Variables => variables_view::render(frame, area, app),
        Overlay::Help => help::render(frame, area, app),
        Overlay::Confirm(modal, _) => modal.render(frame, area, app.accent),
        Overlay::None => {}
    }
}

/// Renders the title header.
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let line = Line::from(vec![
        Span::styled(
            "calcli",
            Style::default().fg(app.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" v{}", crate::util::app_info::APP_VERSION),
            colors::dim(),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// Renders the fixed input field, adapting to the focus mode.
fn render_input(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.accent))
        .title(" input ");
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    frame.render_widget(block, area);

    let line = match app.mode {
        Mode::Input => {
            let prompt = Span::styled(
                "> ",
                Style::default().fg(app.accent).add_modifier(Modifier::BOLD),
            );
            let width = inner.width.saturating_sub(2) as usize;
            let mut spans = vec![prompt];
            spans.extend(text_edit::single_line_spans(
                &app.input,
                app.cursor,
                width,
                Style::default(),
            ));
            Line::from(spans)
        }
        Mode::History => Line::from(Span::styled(
            "browsing history \u{2014} \u{2191}\u{2193} select, Enter edit, Esc back",
            colors::dim(),
        )),
        Mode::Edit(_) => Line::from(Span::styled(
            "editing line \u{2014} Enter apply, Esc cancel",
            colors::dim(),
        )),
    };
    frame.render_widget(Paragraph::new(line), inner);
}

/// Renders the settings bar showing every active setting.
fn render_settings_bar(frame: &mut Frame, area: Rect, app: &App) {
    let settings = app.service.settings();
    let grouping = match settings.thousands_separator.as_str() {
        " " => "space".to_string(),
        "" => "none".to_string(),
        other => other.to_string(),
    };
    let glyphs = match app.glyphs {
        GlyphSet::Unicode => "unicode",
        GlyphSet::Ascii => "ascii",
    };
    let pairs = [
        ("angle", settings.angle_mode.label().to_string()),
        ("notation", settings.notation.label().to_string()),
        ("dp", settings.decimals.to_string()),
        ("sep", settings.decimal_separator.to_string()),
        ("group", grouping),
        ("glyphs", glyphs.to_string()),
    ];
    let value_style = Style::default().fg(app.accent);
    let mut spans: Vec<Span> = Vec::new();
    for (index, (label, value)) in pairs.iter().enumerate() {
        if index != 0 {
            spans.push(Span::styled(" \u{00b7} ", colors::dim()));
        }
        spans.push(Span::styled(format!("{label}: "), colors::dim()));
        spans.push(Span::styled(value.clone(), value_style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Renders the transient status line.
fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let Some(status) = &app.status else {
        return;
    };
    let line = Line::from(Span::styled(
        widgets::truncate(status, area.width as usize),
        Style::default().fg(app.accent),
    ));
    frame.render_widget(Paragraph::new(line), area);
}

/// Renders the footer shortcut hints for the current state.
fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let hints: &[(&str, &str)] = match (&app.overlay, app.mode) {
        (Overlay::Help, _) => &[("F1/Esc", "close help")],
        (Overlay::Variables, _) => &[
            ("\u{2191}\u{2193}", "select"),
            ("Enter", "insert"),
            ("y/Y", "copy"),
            ("d", "delete"),
            ("R", "reset"),
            ("Esc", "close"),
        ],
        (_, Mode::Edit(_)) => &[("Enter", "apply"), ("Esc", "cancel")],
        (_, Mode::History) => &[
            ("\u{2191}\u{2193}", "select"),
            ("Enter/e", "edit"),
            ("d", "delete"),
            ("y/Y", "copy"),
            ("Esc", "back"),
            ("F1", "help"),
        ],
        (_, Mode::Input) => &[
            ("Enter", "calc"),
            ("\u{2191}", "history"),
            ("F2", "notation"),
            ("F3", "deg/rad"),
            ("F4", "vars"),
            ("F5", ". ,"),
            ("F1", "help"),
            ("^Q", "quit"),
        ],
    };
    let line = hint_line(hints, app.accent, area.width as usize);
    frame.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::domain::evaluator::MevalEvaluator;
    use crate::domain::history::History;
    use crate::domain::variables::VariableStore;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn test_app() -> App {
        let config = Config::default();
        let service = CalcService::new(
            Box::new(MevalEvaluator::new()),
            config.format_settings(),
            History::new(100),
            VariableStore::new(),
        );
        App::new(service, &config)
    }

    /// Renders `app` into an 80x24 test terminal and returns the screen text.
    fn render_to_string(app: &App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(app, frame)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer.content().iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn renders_the_main_chrome_without_panicking() {
        let app = test_app();
        let screen = render_to_string(&app);
        assert!(screen.contains("calcli"));
        assert!(screen.contains("input"));
        assert!(screen.contains("history"));
    }

    #[test]
    fn typing_and_enter_records_a_result() {
        let mut app = test_app();
        for c in "2+3".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.service().history().last_value(), Some(5.0));
        let screen = render_to_string(&app);
        assert!(screen.contains("= 5"));
    }

    #[test]
    fn up_enters_history_and_edit_recomputes() {
        let mut app = test_app();
        for c in "10".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        for c in "ans+5".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));

        // Enter history, select the first line, edit it.
        app.handle_key(key(KeyCode::Up));
        app.handle_key(key(KeyCode::Home));
        app.handle_key(key(KeyCode::Enter)); // start editing line 0
        assert!(matches!(app.mode(), Mode::Edit(0)));
        for _ in 0..2 {
            app.handle_key(key(KeyCode::Backspace));
        }
        for c in "20".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter)); // apply
        assert_eq!(app.service().history().entries()[1].value, Some(25.0));
    }

    #[test]
    fn function_keys_toggle_settings() {
        let mut app = test_app();
        assert_eq!(app.service().settings().angle_mode, AngleMode::Rad);
        app.handle_key(key(KeyCode::F(3)));
        assert_eq!(app.service().settings().angle_mode, AngleMode::Deg);
        assert_eq!(app.service().settings().decimal_separator, '.');
        app.handle_key(key(KeyCode::F(5)));
        assert_eq!(app.service().settings().decimal_separator, ',');
    }

    #[test]
    fn variables_overlay_opens_and_resets() {
        let mut app = test_app();
        for c in "7".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        for c in "=x".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.service().variables().get("x"), Some(7.0));

        app.handle_key(key(KeyCode::F(4))); // open variables
        assert!(matches!(app.overlay, Overlay::Variables));
        app.handle_key(key(KeyCode::Char('R'))); // ask to reset
        assert!(matches!(app.overlay, Overlay::Confirm(..)));
        app.handle_key(key(KeyCode::Char('y'))); // confirm
        assert_eq!(app.service().variables().len(), 0);
    }
}
