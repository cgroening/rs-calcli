//! Renders the calculation history. Each entry shows its syntax-highlighted
//! input (soft-wrapped over as many lines as it needs) followed by the
//! right-aligned result (or error) below it, then a configurable gap with an
//! optional separator line. Because entries have variable height, the viewport
//! is windowed by line: it keeps the selected entry visible and otherwise pins
//! to the tail.

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
use crate::tui::{App, Mode, colors, text_edit};

/// Content lines per entry without wrapping (used only for the paging step).
const MIN_ENTRY_LINES: usize = 2;

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
    let full_width = inner.width as usize;
    if height == 0 || full_width == 0 {
        return;
    }

    let entries = app.service().history().entries();
    if entries.is_empty() {
        let hint = Line::from(Span::styled(
            "type an expression and press Enter",
            colors::dim(),
        ));
        frame.render_widget(Paragraph::new(hint), inner);
        app.set_history_width(full_width);
        return;
    }

    let spacing = app.history_spacing();
    let separator = app.history_separator();

    // Decide whether a scrollbar is needed at full width; if so, reserve one
    // column as a gutter between the content and the scrollbar.
    let probe = entry_starts(app, entries, full_width, spacing);
    let overflow = *probe.last().expect("trailing sentinel") > height;
    let width = if overflow {
        full_width.saturating_sub(1).max(1)
    } else {
        full_width
    };
    app.set_history_width(width);

    let starts = if overflow {
        entry_starts(app, entries, width, spacing)
    } else {
        probe
    };
    let total_lines = *starts.last().expect("starts has a trailing sentinel");
    let start_line = window_start(total_lines, height, app.selected(), &starts);
    app.set_view_height((height / (MIN_ENTRY_LINES + spacing)).max(1));

    // Pass 2: build only the entries intersecting the window.
    let window_end = start_line + height;
    let mut lines: Vec<Line> = Vec::with_capacity(height);
    for (index, entry) in entries.iter().enumerate() {
        if starts[index + 1] <= start_line {
            continue;
        }
        if starts[index] >= window_end {
            break;
        }
        let block = entry_block(app, entry, index, width, spacing, separator);
        for (offset, line) in block.into_iter().enumerate() {
            let global = starts[index] + offset;
            if global >= start_line && global < window_end {
                lines.push(line);
            }
        }
    }
    // Render into the (possibly narrowed) content area, leaving the gutter.
    let content = Rect {
        width: width as u16,
        ..inner
    };
    frame.render_widget(Paragraph::new(lines), content);

    render_scrollbar(frame, area, total_lines, height, start_line);
}

/// The starting line of each entry plus a trailing sentinel (total line count).
fn entry_starts(
    app: &App,
    entries: &[HistoryEntry],
    width: usize,
    spacing: usize,
) -> Vec<usize> {
    let mut starts = Vec::with_capacity(entries.len() + 1);
    let mut acc = 0;
    for (index, entry) in entries.iter().enumerate() {
        starts.push(acc);
        acc += entry_height(app, entry, index, width, spacing);
    }
    starts.push(acc);
    starts
}

/// The total number of lines an entry occupies (input + result + gap).
fn entry_height(
    app: &App,
    entry: &HistoryEntry,
    index: usize,
    width: usize,
    spacing: usize,
) -> usize {
    let input_lines =
        text_edit::wrap_offsets(entry_input(app, entry, index), width).len();
    let (result_text, _) = result_span(app, entry);
    let result_lines = result_line_count(&result_text, width);
    input_lines + result_lines + spacing
}

/// The first visible line so the selected entry stays in view; otherwise the
/// tail (newest) is pinned to the bottom.
fn window_start(
    total_lines: usize,
    height: usize,
    selected: Option<usize>,
    starts: &[usize],
) -> usize {
    let mut start = total_lines.saturating_sub(height);
    if let Some(index) = selected {
        let top = starts[index];
        let bottom = starts[index + 1];
        if top < start {
            start = top;
        }
        if bottom > start + height {
            start = bottom - height;
        }
        // An entry taller than the viewport shows from its top.
        if bottom - top > height {
            start = top;
        }
    }
    start
}

/// The text to display for an entry's input: the live buffer while editing it,
/// otherwise the stored input.
fn entry_input<'a>(
    app: &'a App,
    entry: &'a HistoryEntry,
    index: usize,
) -> &'a str {
    if app.mode() == Mode::Edit(index) {
        app.input()
    } else {
        &entry.input
    }
}

/// Builds all lines of one entry: wrapped input, wrapped result, then the gap.
fn entry_block(
    app: &App,
    entry: &HistoryEntry,
    index: usize,
    width: usize,
    spacing: usize,
    separator: Option<Color>,
) -> Vec<Line<'static>> {
    let bg = row_bg(app, index);
    let mut lines = input_lines(app, entry, index, width, bg);
    lines.extend(result_lines(app, entry, width, bg));
    push_gap(&mut lines, spacing, separator, width);
    lines
}

/// The wrapped, syntax-highlighted input lines (live editor on the edited row).
fn input_lines(
    app: &App,
    entry: &HistoryEntry,
    index: usize,
    width: usize,
    bg: Option<Color>,
) -> Vec<Line<'static>> {
    let text = entry_input(app, entry, index);
    let kinds = highlight::classify(text, app.service().variables());
    let styles = colors::styles_for(&kinds, app.highlight());

    let wrapped = if app.mode() == Mode::Edit(index) {
        text_edit::multiline_spans_styled(text, app.cursor(), width, &styles)
    } else {
        text_edit::wrapped_spans(text, &styles, width)
    };
    wrapped
        .into_iter()
        .map(|line| fill_row(line.spans, width, bg))
        .collect()
}

/// The wrapped, right-aligned result (or error) lines.
fn result_lines(
    app: &App,
    entry: &HistoryEntry,
    width: usize,
    bg: Option<Color>,
) -> Vec<Line<'static>> {
    let (text, style) = result_span(app, entry);
    if text.is_empty() {
        return Vec::new();
    }
    text_edit::wrap_offsets(&text, width)
        .into_iter()
        .map(|(segment, _)| {
            let padding = width.saturating_sub(segment.width());
            let spans = vec![
                Span::raw(" ".repeat(padding)),
                Span::styled(segment, style),
            ];
            styled_row(Line::from(spans), bg)
        })
        .collect()
}

/// Pads `spans` to the full `width` and applies the row background, so the tint
/// spans the whole line.
fn fill_row(
    mut spans: Vec<Span<'static>>,
    width: usize,
    bg: Option<Color>,
) -> Line<'static> {
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
    styled_row(Line::from(spans), bg)
}

/// The number of lines the result text occupies (0 when empty).
fn result_line_count(text: &str, width: usize) -> usize {
    if text.is_empty() {
        0
    } else {
        text_edit::wrap_offsets(text, width).len()
    }
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

/// Draws a dim vertical scrollbar when the content overflows the viewport.
fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    total_lines: usize,
    height: usize,
    start_line: usize,
) {
    if total_lines <= height {
        return;
    }
    let mut state =
        ScrollbarState::new(total_lines - height).position(start_line);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .style(colors::dim());
    frame.render_stateful_widget(scrollbar, area, &mut state);
}
