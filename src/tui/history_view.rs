//! Renders the calculation history: input on the left, result on the right,
//! with the selected row highlighted and in-place editing of the focused row.

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState,
};
use unicode_width::UnicodeWidthStr;

use crate::domain::history::HistoryEntry;
use crate::tui::colors;
use crate::tui::widgets::truncate;
use crate::tui::{App, Mode};

/// The gap between the input and the right-aligned result.
const GAP: usize = 2;

/// Renders the history list into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.accent()))
        .title(" history ");
    let inner = area.inner(Margin::new(1, 1));
    frame.render_widget(block, area);

    let height = inner.height as usize;
    let width = inner.width as usize;
    if height == 0 || width == 0 {
        return;
    }
    app.set_view_height(height);

    let entries = app.service().history().entries();
    let total = entries.len();
    if total == 0 {
        let hint = Line::from(Span::styled(
            "type an expression and press Enter",
            colors::dim(),
        ));
        frame.render_widget(Paragraph::new(hint), inner);
        return;
    }

    let offset = visible_offset(app, total, height);
    let end = (offset + height).min(total);
    let lines: Vec<Line> = (offset..end)
        .map(|index| row_line(app, entries, index, width))
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);

    render_scrollbar(frame, area, total, height, offset);
}

/// Computes the scroll offset, keeping the selection visible and otherwise
/// pinning to the tail (most recent). Stores it back for paging.
fn visible_offset(app: &App, total: usize, height: usize) -> usize {
    if total <= height {
        app.set_history_offset(0);
        return 0;
    }
    let max_offset = total - height;
    let mut offset = app.history_offset().min(max_offset);
    match app.selected() {
        None => offset = max_offset,
        Some(selected) if selected < offset => offset = selected,
        Some(selected) if selected >= offset + height => {
            offset = selected + 1 - height;
        }
        Some(_) => {}
    }
    app.set_history_offset(offset);
    offset
}

/// Builds one history row: input (left) and result or error (right).
fn row_line<'a>(
    app: &App,
    entries: &'a [HistoryEntry],
    index: usize,
    width: usize,
) -> Line<'a> {
    let entry = &entries[index];
    let (result_text, result_style) = result_span(app, entry);
    let result_width = result_text.width();
    let input_width = width.saturating_sub(result_width + GAP);

    let mut spans = input_spans(app, entry, index, input_width);
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    let padding = width.saturating_sub(used + result_width);
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(result_text, result_style));

    let line = Line::from(spans);
    if app.selected() == Some(index) && !matches!(app.mode(), Mode::Edit(_)) {
        return line.style(Style::default().bg(colors::SELECTION_BG));
    }
    line
}

/// The input spans for a row, switching to the live editor on the edited row.
fn input_spans(
    app: &App,
    entry: &HistoryEntry,
    index: usize,
    input_width: usize,
) -> Vec<Span<'static>> {
    if app.mode() == Mode::Edit(index) {
        let base = Style::default().bg(colors::FOCUS_BG);
        return crate::tui::text_edit::single_line_spans(
            app.input(),
            app.cursor(),
            input_width,
            base,
        );
    }
    vec![Span::raw(truncate(&entry.input, input_width))]
}

/// The right-hand result text and its style: the value (accent), the error
/// (red), or empty.
fn result_span(app: &App, entry: &HistoryEntry) -> (String, Style) {
    if let Some(error) = &entry.error {
        let text = format!("{} {}", app.warn(), error);
        return (text, Style::default().fg(colors::ERROR));
    }
    match entry.value {
        Some(value) => {
            let text = format!("= {}", app.service().format_display(value));
            let style = Style::default()
                .fg(app.accent())
                .add_modifier(Modifier::BOLD);
            (text, style)
        }
        None => (String::new(), Style::default()),
    }
}

/// Draws a dim vertical scrollbar when the content overflows the viewport.
fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    total: usize,
    height: usize,
    offset: usize,
) {
    if total <= height {
        return;
    }
    let mut state =
        ScrollbarState::new(total.saturating_sub(height)).position(offset);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .style(colors::dim());
    frame.render_stateful_widget(scrollbar, area, &mut state);
}
