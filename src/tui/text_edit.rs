//! Per-character colouring of a soft-wrapped input, and nothing else.
//!
//! This is calcli's one deliberate departure from the toolkit, and it covers
//! the *rendering* only. `ratada::input::InputField` and
//! `ratada::textarea::TextArea` draw plain text with no per-token style hook,
//! while calcli paints every character of an expression by its kind (function,
//! constant, operator, number, variable, unit, `ans`, comment). Everything
//! else - the caret, the selection, the editing keys, the clipboard and the
//! soft-wrap arithmetic - is the toolkit's, re-exported here so call sites keep
//! reaching for one path.
//!
//! In particular the key rules are **never** reimplemented: a terminal reports
//! `AltGr` as Control+Alt, so a hand-rolled Control check would swallow the
//! characters it produces (`@`, `\`, `[`, `~` on a German layout) instead of
//! typing them.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::colors::CaretColors;

pub use ratada::input::{
    EditMode, LineCaret, TextCursor, apply_edit_key, handle_clipboard,
    replace_selection,
};
pub use ratada::textarea::{cursor_to_display, wrap_offsets};

/// How to render one soft-wrapped value: the display `width`, the per-character
/// highlight `styles` and the `caret` colours to paint the cursor and selection
/// with. Bundled so the span builders stay within three parameters.
#[derive(Clone, Copy)]
pub struct SpanContext<'a> {
    /// The display width in columns.
    pub width: usize,
    /// One base style per character of the value.
    pub styles: &'a [Style],
    /// The caret and selection colours, resolved from the palette.
    pub caret: CaretColors,
}

/// Soft-wraps `value` and renders one [`Line`] per display line (no cursor),
/// applying the per-character highlight styles. Used for the read-only history
/// rows.
pub fn wrapped_spans(value: &str, ctx: &SpanContext<'_>) -> Vec<Line<'static>> {
    wrap_offsets(value, ctx.width)
        .iter()
        .map(|(text, start)| {
            let len = text.chars().count();
            let styles = line_styles(ctx.styles, *start, len);
            let caret = LineCaret::default();
            Line::from(cursor_spans_styled(text, caret, styles, ctx.caret))
        })
        .collect()
}

/// Soft-wraps `value` and renders one [`Line`] per display line, applying the
/// per-character highlight styles and painting the block cursor and selection
/// on the right line. Used by the growing input field and the in-place editor.
pub fn multiline_spans_styled(
    value: &str,
    cursor: TextCursor,
    ctx: &SpanContext<'_>,
) -> Vec<Line<'static>> {
    let lines = wrap_offsets(value, ctx.width);
    let total = value.chars().count();
    let (cursor_line, _) = cursor_to_display(&lines, total, cursor.pos);
    let selection = cursor.selection();

    lines
        .iter()
        .enumerate()
        .map(|(index, (text, start))| {
            let len = text.chars().count();
            let caret = LineCaret {
                cursor: (index == cursor_line)
                    .then(|| cursor.pos.saturating_sub(*start).min(len)),
                selection: selection
                    .and_then(|(s, e)| {
                        ratada::input::intersect(s, e, *start, *start + len)
                    })
                    .map(|(s, e)| (s - *start, e - *start)),
            };
            let styles = line_styles(ctx.styles, *start, len);
            Line::from(cursor_spans_styled(text, caret, styles, ctx.caret))
        })
        .collect()
}

/// Splits `visible` into styled spans painting the selection and the block
/// cursor, using `styles[i]` as character `i`'s base style (missing entries
/// fall back to the default). A caret past the end of the text renders as a
/// solid block.
fn cursor_spans_styled(
    visible: &str,
    caret: LineCaret,
    styles: &[Style],
    colors: CaretColors,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = visible.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::default();
    for (index, character) in chars.iter().enumerate() {
        let base = styles.get(index).copied().unwrap_or_default();
        let style = cell_style(index, caret, base, colors);
        if !run.is_empty() && style != run_style {
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
        }
        if run.is_empty() {
            run_style = style;
        }
        run.push(*character);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    if caret.cursor == Some(chars.len()) {
        spans.push(cursor_block_span(colors));
    }
    spans
}

/// The style of one rendered cell: the caret cell, a selected cell, or `base`.
fn cell_style(
    index: usize,
    caret: LineCaret,
    base: Style,
    colors: CaretColors,
) -> Style {
    if caret.cursor == Some(index) {
        return base.bg(colors.cursor);
    }
    if let Some((start, end)) = caret.selection
        && index >= start
        && index < end
    {
        return base.bg(colors.selection);
    }
    base
}

/// The block-cursor span (`\u{2588}`) painted past the end of a value.
fn cursor_block_span(colors: CaretColors) -> Span<'static> {
    Span::styled("\u{2588}", Style::default().fg(colors.cursor))
}

/// The slice of `styles` covering the characters `[start, start + len)`.
fn line_styles(styles: &[Style], start: usize, len: usize) -> &[Style] {
    let begin = start.min(styles.len());
    let end = (start + len).min(styles.len());
    &styles[begin..end]
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::Color;

    use super::*;

    /// Wraps at 8 columns, tall enough that the page keys have room.
    const MODE: EditMode = EditMode::Wrapped {
        width: 8,
        height: 4,
    };

    fn colors() -> CaretColors {
        CaretColors {
            cursor: Color::Red,
            selection: Color::Blue,
        }
    }

    fn context(width: usize, styles: &[Style]) -> SpanContext<'_> {
        SpanContext {
            width,
            styles,
            caret: colors(),
        }
    }

    /// The text of a rendered line, spans concatenated.
    fn text_of(line: &Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    fn press(
        text: &mut String,
        cursor: &mut TextCursor,
        key: KeyEvent,
    ) -> bool {
        apply_edit_key(text, cursor, key, MODE, None)
    }

    /// `AltGr` reaches the terminal as Control+Alt and must *type*, not be
    /// swallowed as a chord. This is the bug the local fork used to carry, and
    /// the reason the key rules are never reimplemented here.
    #[test]
    fn alt_gr_types_the_character_it_produces() {
        for character in ['@', '\\', '[', '~'] {
            let mut text = String::new();
            let mut cursor = TextCursor::at(0);
            let key = KeyEvent::new(
                KeyCode::Char(character),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            );

            assert!(press(&mut text, &mut cursor, key), "{character}");
            assert_eq!(text, character.to_string());
        }
    }

    /// The input holds an expression, which is one line by definition. The
    /// wrapped mode is what keeps a pasted newline out of it.
    #[test]
    fn the_input_mode_never_lets_a_newline_into_the_buffer() {
        let mut text = String::new();
        let mut cursor = TextCursor::at(0);

        ratada::input::paste_text(&mut text, &mut cursor, MODE, None, "1+\n2");
        assert!(!text.contains('\n'), "got: {text:?}");

        let enter = KeyEvent::from(KeyCode::Enter);
        assert!(
            !press(&mut text, &mut cursor, enter),
            "Enter belongs to the caller, which submits with it",
        );
    }

    #[test]
    fn a_value_is_split_into_one_line_per_wrapped_row() {
        let lines = wrapped_spans("aaa bbb ccc", &context(8, &[]));
        assert_eq!(lines.len(), 2);
        assert_eq!(text_of(&lines[0]), "aaa bbb");
        assert_eq!(text_of(&lines[1]), "ccc");
    }

    /// The one thing the toolkit cannot do: a base style per character.
    #[test]
    fn each_character_keeps_its_own_highlight_style() {
        let styles = [
            Style::default().fg(Color::Green),
            Style::default().fg(Color::Yellow),
        ];
        let lines = multiline_spans_styled(
            "ab",
            TextCursor::at(2),
            &context(8, &styles),
        );

        let spans = &lines[0].spans;
        assert_eq!(spans[0].content.as_ref(), "a");
        assert_eq!(spans[0].style.fg, Some(Color::Green));
        assert_eq!(spans[1].content.as_ref(), "b");
        assert_eq!(spans[1].style.fg, Some(Color::Yellow));
    }

    #[test]
    fn a_caret_past_the_end_renders_as_a_solid_block() {
        let lines =
            multiline_spans_styled("ab", TextCursor::at(2), &context(8, &[]));
        let last = lines[0].spans.last().expect("a caret span");
        assert_eq!(last.content.as_ref(), "\u{2588}");
        assert_eq!(last.style.fg, Some(Color::Red));
    }

    #[test]
    fn the_caret_cell_is_tinted_where_the_caret_sits() {
        let lines =
            multiline_spans_styled("ab", TextCursor::at(0), &context(8, &[]));
        let first = &lines[0].spans[0];
        assert_eq!(first.content.as_ref(), "a");
        assert_eq!(first.style.bg, Some(Color::Red));
    }

    #[test]
    fn a_selection_is_tinted_across_the_lines_it_spans() {
        let mut cursor = TextCursor::at(0);
        cursor.extend_to(10);
        let lines =
            multiline_spans_styled("aaa bbb ccc", cursor, &context(8, &[]));

        let selected = |line: &Line<'static>| {
            line.spans
                .iter()
                .any(|span| span.style.bg == Some(Color::Blue))
        };
        assert!(selected(&lines[0]), "the first row is inside the selection");
        assert!(selected(&lines[1]), "and so is the second");
    }

    /// A read-only history row carries no caret, so nothing is tinted.
    #[test]
    fn a_read_only_row_paints_no_caret() {
        let lines = wrapped_spans("ab", &context(8, &[]));
        assert!(
            lines[0].spans.iter().all(|span| span.style.bg.is_none()),
            "a history row must not show a cursor",
        );
    }
}
