//! The Ratatui front-end.
//!
//! Layout, top to bottom: a tinted header panel carrying only the brand and the
//! tab bar, the raised content surface with the active view, a tinted status
//! band (the active settings on the left, the transient status on the right)
//! and the backgroundless shortcut hints. The frame itself has no border lines;
//! rounded borders belong to individual widgets. See [`appframe`].
//!
//! The calculator core lives in [`crate::services`]; this module only
//! translates keys into service calls and renders the result.

pub mod app;
pub mod appframe;
pub mod bindings;
pub mod colors;
pub mod interaction;
pub mod text_edit;
pub mod views;

use ratada::Tui;

pub use crate::tui::app::App;

/// Runs the event loop until the user quits.
///
/// The toolkit's driver owns the global chords: `F1` toggles the shortcut hints
/// and `Ctrl+Q` quits at once, so neither ever reaches [`App`].
///
/// # Errors
///
/// Propagates any error from drawing, reading input or handling a key.
pub fn run(app: &mut App, tui: &mut Tui) -> anyhow::Result<()> {
    ratada::run(tui, app)
}
