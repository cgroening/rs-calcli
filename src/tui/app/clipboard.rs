//! Copying a value out, in the two shapes the user can ask for.
//!
//! `y` copies the raw value at full `f64` precision and without grouping, so
//! pasting it elsewhere loses nothing. `Y` copies what the screen shows:
//! rounded, grouped and carrying its unit.

use ratada::clipboard;

use crate::domain::quantity::Quantity;
use crate::tui::app::App;

impl App {
    /// Copies `value` as a plain, full-precision value (the `y` behaviour).
    pub(super) fn copy_plain(&mut self, value: &Quantity) {
        let text = self.service.format_plain(value);
        self.report(copy_status(&text));
    }

    /// Copies `value` as displayed: rounded and grouped (the `Y` behaviour).
    pub(super) fn copy_display(&mut self, value: &Quantity) {
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
