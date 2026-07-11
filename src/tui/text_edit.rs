//! Single-line text editing for the input field and inline history edits.
//!
//! A character caret with an optional selection anchor over a `String`, the key
//! handling that moves, selects and edits at it, plus block-cursor rendering
//! that scrolls to keep the caret visible. The value stays owned by the caller;
//! this module only mutates `(text, cursor)` and renders. Adapted (single-line
//! only) from the shared `text_edit` of the reference projects.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratada::clipboard;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::colors::CaretColors;

/// A character caret over an input value plus an optional selection anchor.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct TextCursor {
    /// The moving caret as a character index in `0..=len`.
    pub pos: usize,
    /// Where the current selection began, or `None` when nothing is selected.
    pub anchor: Option<usize>,
}

impl TextCursor {
    /// A caret at `pos` with no selection.
    pub fn at(pos: usize) -> Self {
        TextCursor { pos, anchor: None }
    }

    /// Moves the caret to `pos`, dropping any selection.
    pub fn move_to(&mut self, pos: usize) {
        self.pos = pos;
        self.anchor = None;
    }

    /// Moves the caret to `pos`, seeding the anchor when no selection is active.
    pub fn extend_to(&mut self, pos: usize) {
        if self.anchor.is_none() {
            self.anchor = Some(self.pos);
        }
        self.pos = pos;
    }

    /// Selects the whole value of `len` characters.
    pub fn select_all(&mut self, len: usize) {
        self.anchor = Some(0);
        self.pos = len;
    }

    /// The selection as an ordered half-open `(start, end)`, or `None`.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        if anchor == self.pos {
            return None;
        }
        Some((anchor.min(self.pos), anchor.max(self.pos)))
    }

    /// Whether a non-empty selection is active.
    pub fn has_selection(&self) -> bool {
        self.selection().is_some()
    }
}

/// Where to paint the caret and selection within the rendered line.
#[derive(Clone, Copy, Default)]
struct LineCaret {
    /// The caret column, or `None` when the caret is off-screen.
    cursor: Option<usize>,
    /// The selected column range as a half-open `(start, end)`.
    selection: Option<(usize, usize)>,
}

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

/// Whether an input is a single logical line or soft-wrapped at a display width.
#[derive(Clone, Copy)]
pub enum EditMode {
    /// One logical line: `Home`/`End` jump to the value's start/end.
    SingleLine,
    /// Soft-wrapped at `width` columns: `Home`/`End` act on the display line and
    /// `Up`/`Down` move across wrapped lines. The value never contains `\n`.
    Multiline {
        /// The column count at which the value soft-wraps.
        width: usize,
    },
}

/// Applies an editing key to `(text, cursor)`, returning `true` when the key was
/// an editing key. Steering keys the caller owns (`Esc`, a confirming `Enter`,
/// other chords) must be handled before delegating here.
pub fn apply_edit_key(
    text: &mut String,
    cursor: &mut TextCursor,
    key: KeyEvent,
    mode: EditMode,
) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    if let Some(target) = motion_target(text, cursor.pos, key, mode) {
        if shift {
            cursor.extend_to(target);
        } else {
            cursor.move_to(target);
        }
        return true;
    }
    match key.code {
        KeyCode::Char('a') if ctrl => cursor.select_all(char_count(text)),
        KeyCode::Char('u') if ctrl => {
            cursor.anchor = Some(line_start(text, cursor.pos, mode));
            replace_selection(text, cursor, "");
        }
        KeyCode::Char('k') if ctrl => {
            cursor.anchor = Some(line_end(text, cursor.pos, mode));
            replace_selection(text, cursor, "");
        }
        KeyCode::Char(c) if !ctrl => {
            replace_selection(text, cursor, &c.to_string());
        }
        KeyCode::Backspace if cursor.has_selection() => {
            replace_selection(text, cursor, "");
        }
        KeyCode::Delete if cursor.has_selection() => {
            replace_selection(text, cursor, "");
        }
        KeyCode::Backspace => delete_before(text, cursor),
        KeyCode::Delete => delete_after(text, cursor),
        _ => return false,
    }
    true
}

/// Handles the clipboard chords: `Ctrl+C` copies the selection, `Ctrl+X` cuts
/// it, `Ctrl+V` pastes. Returns `true` when the key was one of them.
pub fn handle_clipboard(
    text: &mut String,
    cursor: &mut TextCursor,
    key: KeyEvent,
) -> bool {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }
    match key.code {
        KeyCode::Char('c') => {
            if let Some(selected) = selected_text(text, cursor) {
                clipboard::copy(&selected);
            }
        }
        KeyCode::Char('x') => {
            if let Some(selected) = selected_text(text, cursor) {
                clipboard::copy(&selected);
                replace_selection(text, cursor, "");
            }
        }
        KeyCode::Char('v') => {
            if let Some(pasted) = clipboard::paste() {
                let one_line = pasted.replace(['\n', '\r'], " ");
                replace_selection(text, cursor, &one_line);
            }
        }
        _ => return false,
    }
    true
}

/// The motion target for a navigation key, or `None` when it is not one. A
/// vertical move that cannot go further returns the unchanged `pos`.
fn motion_target(
    text: &str,
    pos: usize,
    key: KeyEvent,
    mode: EditMode,
) -> Option<usize> {
    let multiline = matches!(mode, EditMode::Multiline { .. });
    let target = match key.code {
        KeyCode::Left => pos.saturating_sub(1),
        KeyCode::Right => (pos + 1).min(char_count(text)),
        KeyCode::Home => line_start(text, pos, mode),
        KeyCode::End => line_end(text, pos, mode),
        KeyCode::Up if multiline => display_line_target(text, pos, mode, -1),
        KeyCode::Down if multiline => display_line_target(text, pos, mode, 1),
        _ => return None,
    };
    Some(target)
}

/// The cursor index for `Home`: the value's start, or the display line's start.
fn line_start(text: &str, cursor: usize, mode: EditMode) -> usize {
    match mode {
        EditMode::SingleLine => 0,
        EditMode::Multiline { width } => {
            let lines = wrap_offsets(text, width);
            let (display_line, _) =
                cursor_to_display(&lines, char_count(text), cursor);
            display_to_cursor(&lines, display_line, 0)
        }
    }
}

/// The cursor index for `End`: the value's end, or the display line's end.
fn line_end(text: &str, cursor: usize, mode: EditMode) -> usize {
    match mode {
        EditMode::SingleLine => char_count(text),
        EditMode::Multiline { width } => {
            let lines = wrap_offsets(text, width);
            let (display_line, _) =
                cursor_to_display(&lines, char_count(text), cursor);
            let col = lines[display_line].0.chars().count();
            display_to_cursor(&lines, display_line, col)
        }
    }
}

/// The cursor index one display line up/down, keeping the column where possible;
/// returns `pos` unchanged when there is no line in that direction.
fn display_line_target(
    text: &str,
    pos: usize,
    mode: EditMode,
    delta: i32,
) -> usize {
    let EditMode::Multiline { width } = mode else {
        return pos;
    };
    let lines = wrap_offsets(text, width);
    let (display_line, col) = cursor_to_display(&lines, char_count(text), pos);
    let target = display_line as i32 + delta;
    if target < 0 || target >= lines.len() as i32 {
        return pos;
    }
    display_to_cursor(&lines, target as usize, col)
}

/// Replaces the active selection (or inserts at the caret) with `s`.
pub fn replace_selection(text: &mut String, cursor: &mut TextCursor, s: &str) {
    if let Some((start, end)) = cursor.selection() {
        let mut chars: Vec<char> = text.chars().collect();
        let end = end.min(chars.len());
        let start = start.min(end);
        chars.drain(start..end);
        *text = chars.into_iter().collect();
        cursor.pos = start;
    }
    cursor.anchor = None;
    insert_raw(text, cursor, s);
}

/// The selected substring, or `None` when nothing is selected.
fn selected_text(text: &str, cursor: &TextCursor) -> Option<String> {
    let (start, end) = cursor.selection()?;
    let chars: Vec<char> = text.chars().collect();
    let end = end.min(chars.len());
    let start = start.min(end);
    Some(chars[start..end].iter().collect())
}

/// The number of characters (not bytes) in `text`.
fn char_count(text: &str) -> usize {
    text.chars().count()
}

/// Inserts `s` at the caret, advancing it.
fn insert_raw(text: &mut String, cursor: &mut TextCursor, s: &str) {
    let mut chars: Vec<char> = text.chars().collect();
    let mut at = cursor.pos.min(chars.len());
    for c in s.chars() {
        chars.insert(at, c);
        at += 1;
    }
    cursor.pos = at;
    *text = chars.into_iter().collect();
}

/// Deletes the character before the caret (Backspace).
fn delete_before(text: &mut String, cursor: &mut TextCursor) {
    cursor.anchor = None;
    let mut chars: Vec<char> = text.chars().collect();
    let at = cursor.pos.min(chars.len());
    if at == 0 {
        return;
    }
    chars.remove(at - 1);
    cursor.pos = at - 1;
    *text = chars.into_iter().collect();
}

/// Deletes the character at the caret (forward Delete).
fn delete_after(text: &mut String, cursor: &mut TextCursor) {
    cursor.anchor = None;
    let mut chars: Vec<char> = text.chars().collect();
    let at = cursor.pos.min(chars.len());
    if at >= chars.len() {
        return;
    }
    chars.remove(at);
    *text = chars.into_iter().collect();
}

/// The overlap of `[s, e)` with `[lo, hi)`, or `None` when they don't meet.
fn intersect(
    s: usize,
    e: usize,
    lo: usize,
    hi: usize,
) -> Option<(usize, usize)> {
    let start = s.max(lo);
    let end = e.min(hi);
    (start < end).then_some((start, end))
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
    let total = char_count(value);
    let (cursor_line, _) = cursor_to_display(&lines, total, cursor.pos);
    let selection = cursor.selection();

    let mut out: Vec<Line<'static>> = Vec::with_capacity(lines.len());
    for (index, (text, start)) in lines.iter().enumerate() {
        let len = text.chars().count();
        let styles = line_styles(ctx.styles, *start, len);
        let column = (index == cursor_line)
            .then(|| cursor.pos.saturating_sub(*start).min(len));
        let line_selection = selection
            .and_then(|(s, e)| intersect(s, e, *start, *start + len))
            .map(|(s, e)| (s - *start, e - *start));
        let caret = LineCaret {
            cursor: column,
            selection: line_selection,
        };
        out.push(Line::from(cursor_spans_styled(
            text, caret, styles, ctx.caret,
        )));
    }
    out
}

/// The display line and column of cursor index `cursor` within `lines`.
pub fn cursor_to_display(
    lines: &[(String, usize)],
    total: usize,
    cursor: usize,
) -> (usize, usize) {
    let cursor = cursor.min(total);
    let mut display_line = 0;
    for (index, (_, start)) in lines.iter().enumerate() {
        if *start <= cursor {
            display_line = index;
        } else {
            break;
        }
    }
    let (text, start) = &lines[display_line];
    (display_line, (cursor - start).min(text.chars().count()))
}

/// Maps a `(display line, column)` back to a cursor char index.
fn display_to_cursor(
    lines: &[(String, usize)],
    line: usize,
    col: usize,
) -> usize {
    let (text, start) = &lines[line];
    start + col.min(text.chars().count())
}

/// Soft-wraps `text` to `width` columns, returning each display line with the
/// character offset (into `text`) at which it starts. An over-long word is
/// hard-split; the value carries no explicit newlines.
pub fn wrap_offsets(text: &str, width: usize) -> Vec<(String, usize)> {
    let width = width.max(1);
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<(String, usize)> = Vec::new();
    wrap_logical(&chars, 0, width, &mut out);
    if out.is_empty() {
        out.push((String::new(), 0));
    }
    out
}

/// Greedily wraps `line` (starting at char offset `base`) into `out`.
fn wrap_logical(
    line: &[char],
    base: usize,
    width: usize,
    out: &mut Vec<(String, usize)>,
) {
    if line.is_empty() {
        out.push((String::new(), base));
        return;
    }
    let len = line.len();
    let mut start = 0usize;
    while start < len {
        let end = (start + width).min(len);
        if end == len {
            out.push((line[start..end].iter().collect(), base + start));
            break;
        }
        // Break at the last space in the window; else hard-split.
        let break_at = (start + 1..=end).rev().find(|&p| line[p - 1] == ' ');
        match break_at {
            Some(p) if p - 1 > start => {
                out.push((line[start..p - 1].iter().collect(), base + start));
                start = p; // consume the break space
            }
            _ => {
                out.push((line[start..end].iter().collect(), base + start));
                start = end;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn apply(
        text: &str,
        cursor: TextCursor,
        key: KeyEvent,
    ) -> (String, TextCursor) {
        let mut text = text.to_string();
        let mut cursor = cursor;
        apply_edit_key(&mut text, &mut cursor, key, EditMode::SingleLine);
        (text, cursor)
    }

    fn apply_multiline(
        text: &str,
        cursor: TextCursor,
        key: KeyEvent,
        width: usize,
    ) -> (String, TextCursor) {
        let mut text = text.to_string();
        let mut cursor = cursor;
        let mode = EditMode::Multiline { width };
        apply_edit_key(&mut text, &mut cursor, key, mode);
        (text, cursor)
    }

    #[test]
    fn typing_inserts_at_the_caret() {
        let (text, cursor) =
            apply("ac", TextCursor::at(1), key(KeyCode::Char('b')));
        assert_eq!((text.as_str(), cursor.pos), ("abc", 2));
    }

    #[test]
    fn home_and_end_jump_to_the_edges() {
        assert_eq!(
            apply("abc", TextCursor::at(2), key(KeyCode::Home)).1.pos,
            0
        );
        assert_eq!(apply("abc", TextCursor::at(0), key(KeyCode::End)).1.pos, 3);
    }

    #[test]
    fn ctrl_u_and_ctrl_k_delete_to_the_edges() {
        let (text, _) =
            apply("hello", TextCursor::at(3), ctrl(KeyCode::Char('u')));
        assert_eq!(text, "lo");
        let (text, _) =
            apply("hello", TextCursor::at(3), ctrl(KeyCode::Char('k')));
        assert_eq!(text, "hel");
    }

    #[test]
    fn backspace_deletes_the_previous_character() {
        let (text, cursor) =
            apply("abc", TextCursor::at(2), key(KeyCode::Backspace));
        assert_eq!((text.as_str(), cursor.pos), ("ac", 1));
    }

    #[test]
    fn typing_replaces_an_active_selection() {
        let cursor = TextCursor {
            pos: 3,
            anchor: Some(1),
        };
        let (text, cursor) = apply("abcd", cursor, key(KeyCode::Char('X')));
        assert_eq!((text.as_str(), cursor.pos), ("aXd", 2));
    }

    #[test]
    fn wrap_offsets_breaks_on_words_and_long_words() {
        let lines = |t: &str, w| {
            wrap_offsets(t, w)
                .into_iter()
                .map(|(s, _)| s)
                .collect::<Vec<_>>()
        };
        assert_eq!(lines("alpha beta gamma", 11), vec!["alpha beta", "gamma"]);
        assert_eq!(lines("abcdef", 3), vec!["abc", "def"]);
        // A soft-wrapped value carries no newlines; offsets track char indices.
        let offsets = wrap_offsets("alpha beta gamma", 11);
        assert_eq!(offsets[1].1, 11);
        let total = "alpha beta gamma".chars().count();
        assert_eq!(cursor_to_display(&offsets, total, 13), (1, 2));
    }

    #[test]
    fn multiline_up_down_move_across_wrapped_lines() {
        let mode_width = 11;
        // "alpha beta gamma" wraps to ["alpha beta", "gamma"]; pos 13 is on
        // line 1, column 2. Up keeps the column on line 0.
        let (_, cursor) = apply_multiline(
            "alpha beta gamma",
            TextCursor::at(13),
            key(KeyCode::Up),
            mode_width,
        );
        assert_eq!(cursor.pos, 2);
        // Down from the top line returns to the lower line at the same column.
        let (_, cursor) = apply_multiline(
            "alpha beta gamma",
            TextCursor::at(2),
            key(KeyCode::Down),
            mode_width,
        );
        assert_eq!(cursor.pos, 13);
    }

    #[test]
    fn multiline_home_and_end_act_on_the_display_line() {
        let (_, cursor) = apply_multiline(
            "alpha beta gamma",
            TextCursor::at(13),
            key(KeyCode::Home),
            11,
        );
        assert_eq!(cursor.pos, 11);
        let (_, cursor) = apply_multiline(
            "alpha beta gamma",
            TextCursor::at(13),
            key(KeyCode::End),
            11,
        );
        assert_eq!(cursor.pos, 16);
    }
}
