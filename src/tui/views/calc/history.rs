//! The scrollable history list.
//!
//! Entries have variable height - an input soft-wraps over as many rows as it
//! needs, and its result adds more - so the viewport is windowed by *line*
//! rather than by row. That is what [`window_start`] does, and it is why the
//! history cannot use [`ratada::list`], which assumes uniform rows. The
//! scrollbar still comes from [`ratada::scroll`].

use ratada::nav::ScrollView;
use ratada::theme::Skin;
use ratada::{scroll, style};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::tui::text_edit::{self, SpanContext};
use crate::tui::views::calc::{CalcView, MIN_ENTRY_LINES, Metrics, Mode, Row};

pub(super) fn render_history(
    frame: &mut Frame,
    area: Rect,
    skin: &Skin,
    view: &CalcView<'_>,
) -> Metrics {
    let height = area.height as usize;
    let full_width = area.width as usize;
    let mut metrics = Metrics {
        history_width: full_width.max(1),
        view_height: 1,
        ..Metrics::default()
    };
    if height == 0 || full_width == 0 {
        return metrics;
    }
    if view.rows.is_empty() {
        let hint = Line::from(Span::styled(
            "type an expression and press Enter",
            style::secondary(&skin.palette),
        ));
        frame.render_widget(Paragraph::new(hint), area);
        return metrics;
    }

    // Decide whether a scrollbar is needed at full width; if so, reserve two
    // columns: one blank gutter plus the scrollbar column itself.
    let probe = entry_starts(view, full_width);
    let total = *probe.last().expect("entry_starts appends a sentinel");
    let overflow = total > height;
    let width = if overflow {
        full_width.saturating_sub(2).max(1)
    } else {
        full_width
    };
    metrics.history_width = width;

    let starts = if overflow {
        entry_starts(view, width)
    } else {
        probe
    };
    let total_lines = *starts.last().expect("entry_starts appends a sentinel");
    let start_line = window_start(total_lines, height, view.selected, &starts);
    metrics.view_height =
        (height / (MIN_ENTRY_LINES + view.style.spacing)).max(1);

    let window_end = start_line + height;
    let mut lines: Vec<Line> = Vec::with_capacity(height);
    for (index, row) in view.rows.iter().enumerate() {
        if starts[index + 1] <= start_line {
            continue;
        }
        if starts[index] >= window_end {
            break;
        }
        for (offset, line) in entry_block(view, skin, row, index, width)
            .into_iter()
            .enumerate()
        {
            let global = starts[index] + offset;
            if global >= start_line && global < window_end {
                lines.push(line);
            }
        }
    }
    let content = Rect {
        width: width as u16,
        ..area
    };
    frame.render_widget(Paragraph::new(lines), content);

    scroll::render_scrollbar(
        frame,
        area,
        skin,
        ScrollView {
            total: total_lines,
            offset: start_line,
            viewport: height,
        },
    );
    metrics
}

/// The starting line of each entry plus a trailing sentinel (total line count).
fn entry_starts(view: &CalcView<'_>, width: usize) -> Vec<usize> {
    let mut starts = Vec::with_capacity(view.rows.len() + 1);
    let mut acc = 0;
    for (index, row) in view.rows.iter().enumerate() {
        starts.push(acc);
        acc += entry_height(view, row, index, width);
    }
    starts.push(acc);
    starts
}

/// The total number of lines an entry occupies (input + result + gap).
fn entry_height(
    view: &CalcView<'_>,
    row: &Row<'_>,
    index: usize,
    width: usize,
) -> usize {
    let input_lines =
        text_edit::wrap_offsets(entry_input(view, row, index), width).len();
    let (result_text, _) = result_span(view, row);
    let result_lines = if result_text.is_empty() {
        0
    } else {
        text_edit::wrap_offsets(&result_text, width).len()
    };
    input_lines + result_lines + view.style.spacing
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
    let Some(index) = selected else {
        return start;
    };
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
    start
}

/// The text to display for an entry's input: the live buffer while editing it,
/// otherwise the stored input.
fn entry_input<'a>(
    view: &'a CalcView<'a>,
    row: &'a Row<'a>,
    index: usize,
) -> &'a str {
    if view.mode == Mode::Edit(index) {
        view.input
    } else {
        &row.entry.input
    }
}

/// Builds all lines of one entry: wrapped input, wrapped result, then the gap.
fn entry_block(
    view: &CalcView<'_>,
    skin: &Skin,
    row: &Row<'_>,
    index: usize,
    width: usize,
) -> Vec<Line<'static>> {
    let bg = row_bg(view, skin, index);
    let mut lines = input_lines(view, row, index, width, bg);
    lines.extend(result_lines(view, row, width, bg));
    push_gap(&mut lines, view.style.spacing, view.style.separator, width);
    lines
}

/// The wrapped, syntax-highlighted input lines (live editor on the edited row).
fn input_lines(
    view: &CalcView<'_>,
    row: &Row<'_>,
    index: usize,
    width: usize,
    bg: Option<Color>,
) -> Vec<Line<'static>> {
    let text = entry_input(view, row, index);
    let editing = view.mode == Mode::Edit(index);
    let styles: &[Style] = if editing {
        view.input_styles
    } else {
        view.row_styles.get(index).map_or(&[], Vec::as_slice)
    };
    let ctx = SpanContext {
        width,
        styles,
        caret: view.caret,
    };
    let wrapped = if editing {
        text_edit::multiline_spans_styled(text, view.cursor, &ctx)
    } else {
        text_edit::wrapped_spans(text, &ctx)
    };
    wrapped
        .into_iter()
        .map(|line| fill_row(line.spans, width, bg))
        .collect()
}

/// The wrapped, right-aligned result (or error) lines.
fn result_lines(
    view: &CalcView<'_>,
    row: &Row<'_>,
    width: usize,
    bg: Option<Color>,
) -> Vec<Line<'static>> {
    let (text, style) = result_span(view, row);
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

/// The background tint for entry `index`: the focused fill while it is being
/// edited, the selection tint while it is selected, and nothing otherwise.
fn row_bg(view: &CalcView<'_>, skin: &Skin, index: usize) -> Option<Color> {
    if view.mode == Mode::Edit(index) {
        return Some(style::to_ratatui(skin.palette.input_bg_active));
    }
    if view.selected == Some(index) {
        return Some(style::to_ratatui(skin.palette.selection));
    }
    None
}

/// Pushes the `spacing` gap lines after an entry. When a separator colour is
/// given, the last gap line is a full-width rule; the rest stay blank.
fn push_gap(
    lines: &mut Vec<Line<'static>>,
    spacing: usize,
    separator: Option<Color>,
    width: usize,
) {
    for index in 0..spacing {
        if index + 1 == spacing
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

/// The result text and its style: the value, the error, or empty.
fn result_span(view: &CalcView<'_>, row: &Row<'_>) -> (String, Style) {
    if let Some(error) = &row.entry.error {
        let text = format!("{} {}", view.warn, error);
        return (text, Style::default().fg(view.error_color));
    }
    if row.result.is_empty() {
        return (String::new(), Style::default());
    }
    let style = Style::default()
        .fg(view.accent_color)
        .add_modifier(Modifier::BOLD);
    (format!("= {}", row.result), style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::history::HistoryEntry;
    use crate::tui::views::calc::testing::{row_view, skin};

    /// Only the row the user is on carries a tint; the separator rules do the
    /// work of telling the other entries apart.
    #[test]
    fn a_history_row_is_tinted_only_while_selected_or_edited() {
        let skin = skin();

        let mut view = row_view();
        assert_eq!(row_bg(&view, &skin, 0), None, "at rest nothing is tinted");
        assert_eq!(row_bg(&view, &skin, 1), None);

        view.selected = Some(1);
        assert_eq!(row_bg(&view, &skin, 0), None, "only the selected row");
        assert_eq!(
            row_bg(&view, &skin, 1),
            Some(style::to_ratatui(skin.palette.selection)),
        );

        let mut view = row_view();
        view.mode = Mode::Edit(1);
        assert_eq!(
            row_bg(&view, &skin, 1),
            Some(style::to_ratatui(skin.palette.input_bg_active)),
            "the row being edited wears the focused input fill",
        );
    }

    fn entry(input: &str, result: &str) -> (HistoryEntry, String) {
        let entry = HistoryEntry {
            input: input.to_string(),
            value: None,
            error: None,
        };
        (entry, result.to_string())
    }

    /// Builds a view over `rows`, each already paired with its result text.
    fn view_over<'a>(
        rows: &'a [Row<'a>],
        selected: Option<usize>,
    ) -> CalcView<'a> {
        CalcView {
            rows,
            selected,
            ..row_view()
        }
    }

    #[test]
    fn an_entry_is_as_tall_as_its_wrapped_input_result_and_gap() {
        let (stored, result) = entry("1+1", "2");
        let rows = [Row {
            entry: &stored,
            result,
        }];
        let view = view_over(&rows, None);
        // One input line, one result line, one line of spacing.
        assert_eq!(entry_height(&view, &rows[0], 0, 20), 3);
    }

    #[test]
    fn an_entry_without_a_result_reserves_no_result_line() {
        let (stored, result) = entry("# a note", "");
        let rows = [Row {
            entry: &stored,
            result,
        }];
        let view = view_over(&rows, None);
        assert_eq!(entry_height(&view, &rows[0], 0, 20), 2);
    }

    #[test]
    fn a_wrapped_input_grows_the_entry_by_a_line_per_wrap() {
        let (stored, result) = entry("aaaa bbbb cccc dddd", "0");
        let rows = [Row {
            entry: &stored,
            result,
        }];
        let view = view_over(&rows, None);
        let narrow = entry_height(&view, &rows[0], 0, 10);
        let wide = entry_height(&view, &rows[0], 0, 40);
        assert!(narrow > wide, "narrow: {narrow}, wide: {wide}");
    }

    #[test]
    fn entry_starts_accumulate_and_end_in_a_total_sentinel() {
        let (first, first_result) = entry("1", "1");
        let (second, second_result) = entry("2", "2");
        let rows = [
            Row {
                entry: &first,
                result: first_result,
            },
            Row {
                entry: &second,
                result: second_result,
            },
        ];
        let view = view_over(&rows, None);
        let starts = entry_starts(&view, 20);

        assert_eq!(starts.len(), rows.len() + 1, "one sentinel past the end");
        assert_eq!(starts[0], 0);
        assert_eq!(starts[1], entry_height(&view, &rows[0], 0, 20));
        assert_eq!(*starts.last().expect("a sentinel"), starts[1] * 2);
    }

    /// With nothing selected the newest lines are pinned to the bottom, which
    /// is what makes the view read like a terminal session.
    #[test]
    fn without_a_selection_the_window_shows_the_tail() {
        assert_eq!(window_start(30, 10, None, &[0, 30]), 20);
        assert_eq!(window_start(5, 10, None, &[0, 5]), 0, "no scroll needed");
    }

    #[test]
    fn the_window_follows_a_selection_above_and_below_it() {
        let starts = [0, 3, 6, 9, 12, 15];

        // Selecting an entry above the window pulls the window up to it.
        assert_eq!(window_start(15, 6, Some(0), &starts), 0);
        // One below pushes the window down until its bottom line fits.
        assert_eq!(window_start(15, 6, Some(4), &starts), 9);
    }

    /// An entry taller than the viewport cannot be shown whole, so it is shown
    /// from its top: the input is what the user needs to read first.
    #[test]
    fn an_entry_taller_than_the_viewport_is_shown_from_its_top() {
        let starts = [0, 10];
        assert_eq!(window_start(10, 4, Some(0), &starts), 0);
    }
}
