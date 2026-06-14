//! Small reusable rendering helpers: a footer hint line, overlay placement,
//! width-aware truncation and a confirmation modal.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
// https://crates.io/crates/unicode-width
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::tui::colors;

/// Truncates `text` to `max_width` display columns, appending `…` when cut.
pub fn truncate(text: &str, max_width: usize) -> String {
    if text.width() <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let mut width = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max_width.saturating_sub(1) {
            break;
        }
        width += ch_width;
        out.push(ch);
    }
    out.push('\u{2026}');
    out
}

/// Builds a footer hint line from `(key, description)` pairs: each key in dimmed
/// accent, each description dim, separated by ` · ` and truncated to `width`.
pub fn hint_line(
    hints: &[(&str, &str)],
    accent: Color,
    width: usize,
) -> Line<'static> {
    let key_style = Style::default().fg(accent).add_modifier(Modifier::DIM);
    let desc_style = colors::dim();
    let separator = Span::styled(" \u{00b7} ", colors::dim());

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for (index, (key, description)) in hints.iter().enumerate() {
        let token = format!("{key} {description}");
        let extra = if index == 0 { 0 } else { 3 };
        if used + token.width() + extra > width {
            break;
        }
        if index != 0 {
            spans.push(separator.clone());
            used += 3;
        }
        spans.push(Span::styled(format!("{key} "), key_style));
        spans.push(Span::styled(description.to_string(), desc_style));
        used += token.width();
    }
    Line::from(spans)
}

/// Lays out shortcut hints across as many lines as needed for `width`, wrapping
/// rather than dropping the ones that do not fit (unlike [`hint_line`]). Always
/// returns at least one line.
pub fn hint_lines(
    hints: &[(&str, &str)],
    accent: Color,
    width: usize,
) -> Vec<Line<'static>> {
    let key_style = Style::default().fg(accent).add_modifier(Modifier::DIM);
    let desc_style = colors::dim();
    let separator = Span::styled(" \u{00b7} ", colors::dim());

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for (key, description) in hints {
        let token_width = format!("{key} {description}").width();
        // Start a new line when the next hint (plus its separator) overflows,
        // but never break before the first hint of a line.
        if !spans.is_empty() && used + 3 + token_width > width {
            lines.push(Line::from(std::mem::take(&mut spans)));
            used = 0;
        }
        if !spans.is_empty() {
            spans.push(separator.clone());
            used += 3;
        }
        spans.push(Span::styled(format!("{key} "), key_style));
        spans.push(Span::styled(description.to_string(), desc_style));
        used += token_width;
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

/// A centred rectangle `width`×`height` (clamped to `area`), for overlays.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width - width) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height - height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(horizontal[1])[1]
}

/// The outcome of feeding a key to a [`ConfirmModal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmResult {
    /// The user confirmed (`y`/`Enter`).
    Yes,
    /// The user declined (`n`/`Esc`).
    No,
    /// The key was not a decision; keep the modal open.
    Pending,
}

/// A yes/no confirmation prompt for destructive actions.
pub struct ConfirmModal {
    /// The question shown to the user.
    pub prompt: String,
}

impl ConfirmModal {
    /// Creates a modal asking `prompt`.
    pub fn new(prompt: impl Into<String>) -> Self {
        ConfirmModal {
            prompt: prompt.into(),
        }
    }

    /// Maps a key to a decision.
    pub fn handle_key(&self, key: KeyEvent) -> ConfirmResult {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                ConfirmResult::Yes
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                ConfirmResult::No
            }
            _ => ConfirmResult::Pending,
        }
    }

    /// Renders the modal centred over `area`.
    pub fn render(&self, frame: &mut Frame, area: Rect, accent: Color) {
        let width = (self.prompt.width() as u16 + 6).max(28).min(area.width);
        let rect = centered_rect(width, 5, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .title(" Confirm ");
        let text = vec![
            Line::from(self.prompt.clone()),
            Line::from(""),
            hint_line(&[("y", "yes"), ("n", "no")], accent, width as usize),
        ];
        let paragraph =
            Paragraph::new(text).block(block).wrap(Wrap { trim: true });
        frame.render_widget(Clear, rect);
        frame.render_widget(paragraph, rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HINTS: &[(&str, &str)] =
        &[("F1", "help"), ("F2", "notation"), ("F3", "deg/rad")];

    #[test]
    fn hint_lines_keep_every_hint_by_wrapping() {
        // Too narrow for one row: all three hints still appear, across rows.
        let lines = hint_lines(HINTS, Color::Reset, 12);
        assert!(lines.len() > 1);
        let text: String = lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.clone()))
            .collect();
        assert!(text.contains("help"));
        assert!(text.contains("notation"));
        assert!(text.contains("deg/rad"));
    }

    #[test]
    fn hint_lines_fit_on_one_row_when_wide_enough() {
        let lines = hint_lines(HINTS, Color::Reset, 200);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn hint_lines_returns_one_line_for_no_hints() {
        assert_eq!(hint_lines(&[], Color::Reset, 80).len(), 1);
    }
}
