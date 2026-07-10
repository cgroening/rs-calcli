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

use ratada::nav::ScrollView;
use ratada::theme::Skin;
use ratada::{chrome, scroll, style};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::domain::history::HistoryEntry;
use crate::tui::colors::{CaretColors, to_ratatui};
use crate::tui::text_edit::{self, SpanContext, TextCursor};

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

/// Floats the suggestion dropdown in a rounded box anchored to the bottom of
/// the history `area`, right above the input box. Draws nothing when there are
/// no suggestions or the history cannot hold even a one-line box.
fn render_completion(
    frame: &mut Frame,
    area: Rect,
    skin: &Skin,
    lines: &[Line<'static>],
) {
    if lines.is_empty() {
        return;
    }
    let content_width =
        lines.iter().map(|line| line.width()).max().unwrap_or(0) as u16;
    // Border on both sides plus one cell of horizontal padding each side.
    let width = (content_width + 4).min(area.width).max(3);
    let wanted = lines.len() as u16 + 2;
    let height = wanted.min(area.height);
    if height < 3 {
        return;
    }
    let box_area = Rect {
        x: area.x,
        y: area.y + area.height - height,
        width,
        height,
    };
    let visible = height as usize - 2;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style::border(&skin.palette))
        .style(style::bg(skin.palette.surface))
        .padding(Padding::horizontal(1));
    frame.render_widget(Clear, box_area);
    frame.render_widget(
        Paragraph::new(lines[..visible.min(lines.len())].to_vec()).block(block),
        box_area,
    );
}

/// Renders the history list, returning its measurements.
fn render_history(
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

    // Decide whether a scrollbar is needed at full width; if so, reserve one
    // column as a gutter between the content and the scrollbar.
    let probe = entry_starts(view, full_width);
    let total = *probe.last().expect("entry_starts appends a sentinel");
    let overflow = total > height;
    let width = if overflow {
        full_width.saturating_sub(1).max(1)
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
        return Some(to_ratatui(skin.palette.input_bg_active));
    }
    if view.selected == Some(index) {
        return Some(to_ratatui(skin.palette.selection));
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

/// Columns available for wrapping the input text (inside borders and prompt).
fn input_wrap_width(total_width: u16) -> usize {
    (total_width as usize).saturating_sub(4).max(1)
}

/// The input box height (including borders): grows with the wrapped line count
/// in Input mode, clamped to `input_max_lines`; one content line otherwise.
fn input_box_height(view: &CalcView<'_>, total_width: u16) -> u16 {
    let content = if matches!(view.mode, Mode::Input) {
        text_edit::wrap_offsets(view.input, input_wrap_width(total_width)).len()
    } else {
        1
    };
    content.clamp(1, view.input_max_lines.max(1)) as u16 + 2
}

/// Renders the input box, returning the wrap width it recorded.
fn render_input(
    frame: &mut Frame,
    area: Rect,
    skin: &Skin,
    view: &CalcView<'_>,
) -> usize {
    // A field, not a modal: a rounded border with the shared inset title
    // (`╭─ input ─`) over the input fill.
    //
    // The border is read from a skin whose `border` follows the focus (see
    // `focus_skin`), because `chrome::border_title` styles the title's leading
    // stroke from the palette itself.
    let framed = focus_skin(skin, view.mode);
    let title = chrome::border_title(
        &framed,
        "input",
        style::accent(&framed.palette).add_modifier(Modifier::BOLD),
    );
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style::border(&framed.palette))
        .style(style::bg(input_fill(skin, view.mode)))
        .title(title);
    if let Some(feedback) = view.feedback.clone() {
        block = block.title_bottom(feedback.right_aligned());
    }
    let inner = area.inner(Margin::new(1, 1));
    frame.render_widget(block, area);

    let width = inner.width.saturating_sub(2) as usize;
    let lines = match view.mode {
        Mode::Input => input_editor_lines(view, skin, inner, width),
        Mode::History => vec![Line::from(Span::styled(
            "browsing history - \u{2191}\u{2193} select, Enter edit, Esc back",
            style::secondary(&skin.palette),
        ))],
        Mode::Edit(_) => vec![Line::from(Span::styled(
            "editing line - Enter apply, Esc cancel",
            style::secondary(&skin.palette),
        ))],
    };
    frame.render_widget(Paragraph::new(lines), inner);
    width.max(1)
}

/// The input field's fill: brighter while it has the keyboard.
fn input_fill(skin: &Skin, mode: Mode) -> ratada::theme::Color {
    if matches!(mode, Mode::Input) {
        skin.palette.input_bg_active
    } else {
        skin.palette.input_bg
    }
}

/// The skin the input's frame is drawn from: a copy whose `border` is the
/// palette's `border_focus` while the field has the keyboard.
///
/// The focused fill is lighter than the resting one, so a fixed border would
/// lose most of its contrast against it and all but vanish. `border_focus`
/// keeps the frame legible in both states and is configurable, either under
/// `[appearance.colors]` or in a `[themes.<name>]`.
///
/// It is swapped into `border` rather than only handed to `border_style`,
/// because `chrome::border_title` reads the title's leading stroke from
/// `palette.border`: that one stroke would otherwise stay dark.
fn focus_skin(skin: &Skin, mode: Mode) -> Skin {
    if !matches!(mode, Mode::Input) {
        return *skin;
    }
    let mut focused = *skin;
    focused.palette.border = skin.palette.border_focus;
    focused
}

/// Builds the soft-wrapped, syntax-highlighted editor lines for the input
/// field, each prefixed with the prompt (`> `) or a continuation indent.
fn input_editor_lines(
    view: &CalcView<'_>,
    skin: &Skin,
    inner: Rect,
    width: usize,
) -> Vec<Line<'static>> {
    let ctx = SpanContext {
        width: width.max(1),
        styles: view.input_styles,
        caret: view.caret,
    };
    let display =
        text_edit::multiline_spans_styled(view.input, view.cursor, &ctx);

    let height = (inner.height as usize).max(1);
    let offsets = text_edit::wrap_offsets(view.input, ctx.width);
    let total = view.input.chars().count();
    let (cursor_line, _) =
        text_edit::cursor_to_display(&offsets, total, view.cursor.pos);
    let start = window_start_line(display.len(), height, cursor_line);

    let prompt_style =
        style::fg(skin.palette.accent).add_modifier(Modifier::BOLD);
    display
        .into_iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, line)| {
            let prefix = if index == 0 {
                Span::styled("> ", prompt_style)
            } else {
                Span::raw("  ")
            };
            let mut spans = line.spans;
            spans.insert(0, prefix);
            Line::from(spans)
        })
        .collect()
}

/// The first visible display line so the cursor line stays within `height`.
fn window_start_line(total: usize, height: usize, cursor_line: usize) -> usize {
    if total <= height {
        return 0;
    }
    if cursor_line < height {
        0
    } else {
        (cursor_line + 1 - height).min(total - height)
    }
}

#[cfg(test)]
mod tests {
    use ratada::theme::{
        ColorOverrides, GlyphVariant, Glyphs, Palette, ThemeRegistry,
    };

    use super::*;
    use crate::config::Config;

    /// A view carrying nothing but what `row_bg` reads.
    fn row_view() -> CalcView<'static> {
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

    fn skin() -> Skin {
        Skin::new(
            Config::default().palette(),
            Glyphs::new(GlyphVariant::Unicode),
        )
    }

    /// The perceived brightness of a colour, for contrast comparisons.
    fn luminance(color: ratada::theme::Color) -> f32 {
        color.luminance()
    }

    /// How far the border stands out from the fill it is drawn against.
    fn contrast(mode: Mode) -> f32 {
        let skin = skin();
        let border = focus_skin(&skin, mode).palette.border;
        (luminance(border) - luminance(input_fill(&skin, mode))).abs()
    }

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
            Some(to_ratatui(skin.palette.selection)),
        );

        let mut view = row_view();
        view.mode = Mode::Edit(1);
        assert_eq!(
            row_bg(&view, &skin, 1),
            Some(to_ratatui(skin.palette.input_bg_active)),
            "the row being edited wears the focused input fill",
        );
    }

    #[test]
    fn the_focused_frame_is_drawn_from_the_palette_focus_border() {
        let skin = skin();
        let resting = focus_skin(&skin, Mode::History).palette.border;
        let focused = focus_skin(&skin, Mode::Input).palette.border;

        assert_eq!(resting, skin.palette.border, "unfocused stays as themed");
        assert_eq!(
            focused, skin.palette.border_focus,
            "the focused frame is configurable, not hard-derived",
        );
        assert!(luminance(focused) > luminance(resting), "focused is lifted");
    }

    #[test]
    fn a_configured_focus_border_reaches_the_frame() {
        // What a user sets under `[appearance.colors] border_focus` must be
        // exactly what the focused input box paints.
        let mut skin = skin();
        skin.palette.border_focus = ratada::theme::Color::Rgb(1, 2, 3);
        let focused = focus_skin(&skin, Mode::Input).palette.border;
        assert_eq!(focused, ratada::theme::Color::Rgb(1, 2, 3));
    }

    #[test]
    fn an_in_place_edit_leaves_the_border_at_rest() {
        let skin = skin();
        let editing = focus_skin(&skin, Mode::Edit(0)).palette.border;
        assert_eq!(editing, skin.palette.border);
    }

    #[test]
    fn the_border_keeps_its_contrast_against_the_brighter_focused_fill() {
        // The focused fill is lighter, so a fixed border would fade into it.
        // Whatever the two fills are tuned to, the frame must stay at least as
        // visible when focused as it is at rest.
        assert!(
            contrast(Mode::Input) >= contrast(Mode::History),
            "focused contrast {} fell below resting {}",
            contrast(Mode::Input),
            contrast(Mode::History),
        );
    }

    #[test]
    fn the_lifted_border_never_outshines_the_foreground() {
        let skin = skin();
        let focused = focus_skin(&skin, Mode::Input).palette.border;
        assert!(
            luminance(focused) < luminance(skin.palette.foreground),
            "the frame must not read as text",
        );
    }

    #[test]
    fn a_custom_theme_border_is_lifted_from_its_own_value() {
        let base = ThemeRegistry::builtin().resolve("monochrome");
        let palette = Palette::resolve(base, &ColorOverrides::default());
        let skin = Skin::new(palette, Glyphs::new(GlyphVariant::Unicode));
        let focused = focus_skin(&skin, Mode::Input).palette.border;
        assert!(luminance(focused) > luminance(skin.palette.border));
    }
}
