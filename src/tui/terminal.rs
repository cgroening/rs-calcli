//! Terminal RAII guard and event reading.
//!
//! [`Tui`] enables raw mode and the alternate screen on creation and restores
//! the terminal on drop, so the shell is left clean even on error. Unlike the
//! reference projects, the global quit chord is returned to the caller (not
//! intercepted here) so the app can persist its state before exiting.

use std::io::{self, Stdout, stdout};

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// The concrete terminal type used throughout the TUI.
pub type Backend = CrosstermBackend<Stdout>;

/// Owns the terminal in raw/alternate-screen mode for the session.
pub struct Tui {
    /// The ratatui terminal; draw through this.
    pub terminal: Terminal<Backend>,
}

impl Tui {
    /// Enters raw mode and the alternate screen.
    ///
    /// # Errors
    /// Returns an I/O error if the terminal cannot be configured.
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(out))?;
        Ok(Tui { terminal })
    }

    /// Reads the next key press, ignoring release/repeat and non-key events.
    ///
    /// # Errors
    /// Returns an I/O error if reading from the terminal fails.
    pub fn read_key(&self) -> io::Result<KeyEvent> {
        loop {
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                return Ok(key);
            }
        }
    }
}

/// Whether `key` is the global quit chord (`Ctrl+Q`).
pub fn is_global_quit(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('q')
}

/// Leaves raw mode and the alternate screen and shows the cursor again.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
}

impl Drop for Tui {
    fn drop(&mut self) {
        restore_terminal();
    }
}
