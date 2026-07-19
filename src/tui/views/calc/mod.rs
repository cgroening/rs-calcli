//! The calculator view: the scrollable history above the growing input field.
//!
//! Each history entry shows its syntax-highlighted input (soft-wrapped over as
//! many lines as it needs) followed by the right-aligned result (or error),
//! then a configurable gap with an optional separator line.
//!
//! Entries have variable height, so the viewport is windowed by line rather
//! than by row: it keeps the selected entry visible and otherwise pins to the
//! tail. That is why the history cannot use [`ratada::list`], which assumes
//! uniform rows; the scrollbar still comes from [`ratada::scroll`].

mod completion;
mod history;
mod input;
#[cfg(test)]
mod testing;

use ratada::theme::Skin;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;

use crate::domain::history::HistoryEntry;
use crate::tui::colors::CaretColors;
use crate::tui::text_edit::TextCursor;
use crate::tui::views::calc::completion::render_completion;
use crate::tui::views::calc::history::render_history;
use crate::tui::views::calc::input::{input_box_height, render_input};

/// Content lines per entry without wrapping (used only for the paging step).
const MIN_ENTRY_LINES: usize = 2;

/// Where keyboard focus sits in the calc view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Typing a new expression in the input field.
    Input,
    /// Browsing the history with a row selected.
    History,
    /// Editing the input of the history row at this index, in place.
    Edit(usize),
}

impl Mode {
    /// Whether a text field currently owns the keyboard, so a bare character
    /// must reach it rather than trigger an action.
    pub fn is_text_editing(self) -> bool {
        matches!(self, Mode::Input | Mode::Edit(_))
    }
}

/// A rendered history row, already formatted by the caller.
pub struct Row<'a> {
    /// The entry itself.
    pub entry: &'a HistoryEntry,
    /// The formatted result, or an empty string for a note.
    pub result: String,
}

/// How the history list is styled and laid out.
pub struct HistoryStyle {
    /// Blank lines inserted after each entry's result.
    pub spacing: usize,
    /// Colour of the separator rule in the gap, or `None` when disabled.
    pub separator: Option<Color>,
}

/// Everything the calc view renders from.
pub struct CalcView<'a> {
    /// The history rows, oldest first.
    pub rows: &'a [Row<'a>],
    /// The selected entry, when browsing the history.
    pub selected: Option<usize>,
    /// The current focus mode.
    pub mode: Mode,
    /// The live input buffer (also the in-place editor's buffer).
    pub input: &'a str,
    /// The input caret.
    pub cursor: TextCursor,
    /// The per-character styles for the input buffer.
    pub input_styles: &'a [Style],
    /// The per-entry per-character styles for the history inputs.
    pub row_styles: &'a [Vec<Style>],
    /// The suggestion dropdown lines to float above the input, empty when the
    /// autocomplete is closed.
    pub completion: Vec<Line<'static>>,
    /// The caret and selection colours.
    pub caret: CaretColors,
    /// How the history list is spaced and tinted.
    pub style: HistoryStyle,
    /// The accent colour, used for a result line.
    pub accent_color: Color,
    /// The error colour, used for a failed line.
    pub error_color: Color,
    /// The warning marker for the active glyph set.
    pub warn: &'a str,
    /// The live-feedback line for the input border, if any.
    pub feedback: Option<Line<'static>>,
    /// The height the input box may grow to, in text lines.
    pub input_max_lines: usize,
}

/// The measurements a draw recorded, which the app needs for its key handling.
#[derive(Debug, Clone, Copy, Default)]
pub struct Metrics {
    /// The wrap width of the input field.
    pub input_width: usize,
    /// The wrap width of a history row.
    pub history_width: usize,
    /// The history viewport height, in entries, for paging.
    pub view_height: usize,
}

/// Renders the calc view and returns the measurements the app needs.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    skin: &Skin,
    view: &CalcView<'_>,
) -> Metrics {
    let input_height = input_box_height(view, area.width);
    let rows = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(input_height),
    ])
    .split(area);

    let mut metrics = render_history(frame, rows[0], skin, view);
    metrics.input_width = render_input(frame, rows[1], skin, view);
    // The suggestion dropdown floats over the foot of the history, just above
    // the input box, so it never grows the bordered field itself. It is bound
    // to the history area, so it never spills over the header above.
    render_completion(frame, rows[0], skin, &view.completion);
    metrics
}
