//! Renders the calculation history. Each entry spans two lines: the
//! syntax-highlighted input, then the right-aligned result (or error) below it.
//! Entries alternate a subtle background (zebra striping); the selected entry
//! and the one being edited get their own tint over both lines.

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState,
};
use unicode_width::UnicodeWidthStr;

use crate::domain::highlight;
use crate::domain::history::HistoryEntry;
use crate::tui::widgets::truncate;
use crate::tui::{App, Mode, colors, text_edit};

/// Content lines per history entry (input line + result line), before spacing.
const CONTENT_LINES: usize = 2;

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

    let spacing = app.history_spacing();
    let per_entry = CONTENT_LINES + spacing;
    let visible = (height / per_entry).max(1);
    app.set_view_height(visible);
    let offset = visible_offset(app, total, visible);
    let end = (offset + visible).min(total);

    let mut lines: Vec<Line> = Vec::with_capacity((end - offset) * per_entry);
    for (index, entry) in
        entries.iter().enumerate().skip(offset).take(end - offset)
    {
        let bg = row_bg(app, index);
        lines.push(input_line(app, entry, index, width, bg));
        lines.push(result_line(app, entry, width, bg));
        push_gap(&mut lines, spacing, app.history_separator(), width);
    }
    frame.render_widget(Paragraph::new(lines), inner);

    render_scrollbar(frame, area, total, visible, offset);
}

/// Computes the scroll offset (in entries), keeping the selection visible and
/// otherwise pinning to the tail (most recent). Stores it back for paging.
fn visible_offset(app: &App, total: usize, visible: usize) -> usize {
    if total <= visible {
        app.set_history_offset(0);
        return 0;
    }
    let max_offset = total - visible;
    let mut offset = app.history_offset().min(max_offset);
    match app.selected() {
        None => offset = max_offset,
        Some(selected) if selected < offset => offset = selected,
        Some(selected) if selected >= offset + visible => {
            offset = selected + 1 - visible;
        }
        Some(_) => {}
    }
    app.set_history_offset(offset);
    offset
}

/// The background tint for entry `index`: focus while editing, selection when
/// selected, else the zebra stripe on every second entry.
fn row_bg(app: &App, index: usize) -> Option<Color> {
    if app.mode() == Mode::Edit(index) {
        Some(colors::FOCUS_BG)
    } else if app.selected() == Some(index) {
        Some(colors::SELECTION_BG)
    } else if index % 2 == 1 {
        app.history_alt_bg()
    } else {
        None
    }
}

/// The top line of an entry: the syntax-highlighted input, padded to `width`.
fn input_line(
    app: &App,
    entry: &HistoryEntry,
    index: usize,
    width: usize,
    bg: Option<Color>,
) -> Line<'static> {
    let mut spans = input_spans(app, entry, index, width);
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
    styled_row(Line::from(spans), bg)
}

/// The bottom line of an entry: the right-aligned result or error, over the
/// full `width`.
fn result_line(
    app: &App,
    entry: &HistoryEntry,
    width: usize,
    bg: Option<Color>,
) -> Line<'static> {
    let (text, style) = result_span(app, entry);
    let text = truncate(&text, width);
    let padding = width.saturating_sub(text.width());
    let spans = vec![Span::raw(" ".repeat(padding)), Span::styled(text, style)];
    styled_row(Line::from(spans), bg)
}

/// Pushes the `spacing` gap lines after an entry. When a separator colour is
/// given, the last gap line is a full-width rule; the rest stay blank.
fn push_gap(
    lines: &mut Vec<Line<'static>>,
    spacing: usize,
    separator: Option<Color>,
    width: usize,
) {
    for i in 0..spacing {
        if i + 1 == spacing
            && let Some(color) = separator
        {
            let rule = "\u{2500}".repeat(width);
            lines.push(Line::from(Span::styled(
                rule,
                Style::default().fg(color),
            )));
        } else {
            lines.push(Line::default());
        }
    }
}

/// Applies the row background to a line, if any.
fn styled_row(line: Line<'static>, bg: Option<Color>) -> Line<'static> {
    match bg {
        Some(color) => line.style(Style::default().bg(color)),
        None => line,
    }
}

/// The input spans for an entry: syntax-highlighted, switching to the live
/// editor on the edited entry.
fn input_spans(
    app: &App,
    entry: &HistoryEntry,
    index: usize,
    width: usize,
) -> Vec<Span<'static>> {
    let variables = app.service().variables();
    if app.mode() == Mode::Edit(index) {
        let kinds = highlight::classify(app.input(), variables);
        let styles = colors::styles_for(&kinds, app.highlight());
        return text_edit::single_line_spans_styled(
            app.input(),
            app.cursor(),
            width,
            &styles,
        );
    }
    let kinds = highlight::classify(&entry.input, variables);
    let styles = colors::styles_for(&kinds, app.highlight());
    text_edit::highlighted_spans(&entry.input, &styles, width)
}

/// The result text and its style: the value (accent), the error (red), or empty.
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

/// Draws a dim vertical scrollbar when the entries overflow the viewport.
fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    total: usize,
    visible: usize,
    offset: usize,
) {
    if total <= visible {
        return;
    }
    let mut state =
        ScrollbarState::new(total.saturating_sub(visible)).position(offset);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .style(colors::dim());
    frame.render_stateful_widget(scrollbar, area, &mut state);
}
