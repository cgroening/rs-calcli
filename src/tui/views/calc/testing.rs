//! Shared fixtures for the calc view's tests.

use ratada::theme::{GlyphVariant, Glyphs, Skin};

use crate::config::Config;
use crate::tui::colors::CaretColors;
use crate::tui::text_edit::TextCursor;
use crate::tui::views::calc::input::{focus_skin, input_fill};
use crate::tui::views::calc::{CalcView, HistoryStyle, Mode};

pub(super) fn row_view() -> CalcView<'static> {
    CalcView {
        rows: &[],
        selected: None,
        mode: Mode::Input,
        input: "",
        cursor: TextCursor::at(0),
        input_styles: &[],
        row_styles: &[],
        completion: Vec::new(),
        caret: CaretColors {
            cursor: ratatui::style::Color::Reset,
            selection: ratatui::style::Color::Reset,
        },
        style: HistoryStyle {
            spacing: 1,
            separator: None,
        },
        accent_color: ratatui::style::Color::Reset,
        error_color: ratatui::style::Color::Reset,
        warn: "!",
        feedback: None,
        input_max_lines: 5,
    }
}

pub(super) fn skin() -> Skin {
    Skin::new(
        Config::default().palette(),
        Glyphs::new(GlyphVariant::Unicode),
    )
}

/// The perceived brightness of a colour, for contrast comparisons.
pub(super) fn luminance(color: ratada::theme::Color) -> f32 {
    color.luminance()
}

/// How far the border stands out from the fill it is drawn against.
pub(super) fn contrast(mode: Mode) -> f32 {
    let skin = skin();
    let border = focus_skin(&skin, mode).palette.border;
    (luminance(border) - luminance(input_fill(&skin, mode))).abs()
}
