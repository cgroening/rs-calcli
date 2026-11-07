//! The shared harness the `App` tests are driven through.
//!
//! Dialogs go through [`Headless`], so the whole key path runs without a
//! terminal, and [`render_to_string`] draws into ratatui's `TestBackend` so a
//! layout can be asserted on as text.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratada::Screen;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::config::Config;
use crate::domain::evaluator::F64Evaluator;
use crate::domain::history::History;
use crate::domain::quantity::Quantity;
use crate::domain::variables::VariableStore;
use crate::services::CalcService;
use crate::storage::UiState;
use crate::tui::app::App;
use crate::tui::interaction::Headless;

/// A key with no modifiers.
pub(super) fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A key with `modifiers` held.
pub(super) fn chord(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// An app on the default config, with an empty history and no variables.
pub(super) fn test_app() -> App {
    app_with(&Config::default())
}

/// An app on `config`, so a test can vary one setting without rebuilding the
/// whole wiring.
pub(super) fn app_with(config: &Config) -> App {
    app_with_state(config, &UiState::default())
}

/// An app on `config`, restored from `ui` as a real restart would.
pub(super) fn app_with_state(config: &Config, ui: &UiState) -> App {
    let service = CalcService::new(
        Box::new(F64Evaluator::new()),
        config.format_settings(),
        History::new(100),
        VariableStore::new(),
    );
    App::new(service, config, ui)
}

/// Presses `key`, answering every confirmation with yes.
pub(super) fn press(app: &mut App, key: KeyEvent) {
    app.dispatch(key, &mut Headless::accepting())
        .expect("dispatch must not fail headlessly");
}

/// Types `text` one character at a time, without submitting it.
pub(super) fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        press(app, key(KeyCode::Char(character)));
    }
}

/// Types `text` then submits it.
pub(super) fn submit(app: &mut App, text: &str) {
    type_text(app, text);
    press(app, key(KeyCode::Enter));
}

/// The display value of history entry `index`.
pub(super) fn value_at(app: &App, index: usize) -> Option<f64> {
    app.service().history().entries()[index]
        .value
        .as_ref()
        .map(Quantity::display_value)
}

/// Renders `app` into a test terminal and returns the screen text.
pub(super) fn render_to_string(app: &App, width: u16, height: u16) -> String {
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

/// Renders `app` into a test terminal and returns the screen rows, trailing
/// blanks trimmed, one per line - the form a snapshot reads well in.
pub(super) fn render_rows(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let row: String =
                (0..width).map(|x| buffer[(x, y)].symbol()).collect();
            row.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
