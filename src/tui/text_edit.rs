//! Single-line text editing for the input field and inline history edits.
//!
//! A character caret with an optional selection anchor over a `String`, the key
//! handling that moves, selects and edits at it, plus block-cursor rendering
//! that scrolls to keep the caret visible. The value stays owned by the caller;
//! this module only mutates `(text, cursor)` and renders. Adapted (single-line
//! only) from the shared `text_edit` of the reference projects.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::tui::colors;
use crate::util::clipboard;

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
pub struct LineCaret {
    /// The caret column, or `None` when the caret is off-screen.
    pub cursor: Option<usize>,
    /// The selected column range as a half-open `(start, end)`.
    pub selection: Option<(usize, usize)>,
}

/// Applies an editing key to `(text, cursor)`, returning `true` when the key was
/// an editing key. Steering keys the caller owns (`Esc`, a confirming `Enter`,
/// other chords) must be handled before delegating here.
pub fn apply_edit_key(
    text: &mut String,
    cursor: &mut TextCursor,
    key: KeyEvent,
) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    if let Some(target) = motion_target(text, cursor.pos, key) {
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
            cursor.anchor = Some(0);
            replace_selection(text, cursor, "");
        }
        KeyCode::Char('k') if ctrl => {
            cursor.anchor = Some(char_count(text));
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

/// The motion target for a navigation key, or `None` when it is not one.
fn motion_target(text: &str, pos: usize, key: KeyEvent) -> Option<usize> {
    let target = match key.code {
        KeyCode::Left => pos.saturating_sub(1),
        KeyCode::Right => (pos + 1).min(char_count(text)),
        KeyCode::Home => 0,
        KeyCode::End => char_count(text),
        _ => return None,
    };
    Some(target)
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
pub fn selected_text(text: &str, cursor: &TextCursor) -> Option<String> {
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

/// Builds the spans for a single-line value of at most `width` columns with the
/// block cursor at `cursor.pos`, scrolling to keep the cursor visible and
/// marking clipped ends with a dim `…`.
pub fn single_line_spans(
    value: &str,
    cursor: TextCursor,
    width: usize,
    base: Style,
) -> Vec<Span<'static>> {
    let width = width.max(1);
    let chars: Vec<char> = value.chars().collect();
    let n = chars.len();
    let pos = cursor.pos.min(n);
    let end_cursor = pos >= n;
    if n + usize::from(end_cursor) <= width {
        let caret = LineCaret {
            cursor: Some(pos),
            selection: cursor.selection(),
        };
        return cursor_spans(value, caret, base);
    }
    let max_start = (n + usize::from(end_cursor)).saturating_sub(width - 1);
    let room = width.saturating_sub(2).max(1);
    let start = if pos < room {
        0
    } else {
        (pos + 1 - room).min(max_start)
    };
    let lead = usize::from(start > 0);
    let reach = (n - start) + usize::from(end_cursor) <= width - lead;
    let show = if reach { n - start } else { width - (lead + 1) };
    let visible_end = start + show;
    let visible: String = chars[start..visible_end.min(n)].iter().collect();
    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut spans = Vec::new();
    if lead == 1 {
        spans.push(Span::styled("\u{2026}".to_string(), dim));
    }
    let col = (pos >= start && pos <= visible_end).then(|| pos - start);
    let selection = cursor
        .selection()
        .and_then(|(s, e)| intersect(s, e, start, visible_end))
        .map(|(s, e)| (s - start, e - start));
    let caret = LineCaret {
        cursor: col,
        selection,
    };
    spans.extend(cursor_spans(&visible, caret, base));
    if !reach {
        spans.push(Span::styled("\u{2026}".to_string(), dim));
    }
    spans
}

/// Splits `visible` into styled spans painting the selection and the block
/// cursor (a `█` past the text, or the covered character on a tinted cell).
pub fn cursor_spans(
    visible: &str,
    caret: LineCaret,
    base: Style,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = visible.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = base;
    for (i, c) in chars.iter().enumerate() {
        let style = cell_style(i, caret, base);
        if !run.is_empty() && style != run_style {
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
        }
        if run.is_empty() {
            run_style = style;
        }
        run.push(*c);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    if caret.cursor == Some(chars.len()) {
        spans.push(colors::cursor_block_span());
    }
    spans
}

/// The style of one rendered cell: the caret cell, a selected cell, or `base`.
fn cell_style(i: usize, caret: LineCaret, base: Style) -> Style {
    if caret.cursor == Some(i) {
        return base.bg(colors::INPUT_CURSOR);
    }
    if let Some((start, end)) = caret.selection
        && i >= start
        && i < end
    {
        return base.bg(colors::SELECTION_BG);
    }
    base
}

/// Like [`single_line_spans`], but the base style of each character `g` is
/// `styles[g]` (its highlight colour). `styles` is indexed by the value's
/// character position; missing entries fall back to the default style.
/// Adapted (single-line only) from numcli's `single_line_spans_styled`.
pub fn single_line_spans_styled(
    value: &str,
    cursor: TextCursor,
    width: usize,
    styles: &[Style],
) -> Vec<Span<'static>> {
    let width = width.max(1);
    let chars: Vec<char> = value.chars().collect();
    let n = chars.len();
    let pos = cursor.pos.min(n);
    let end_cursor = pos >= n;
    if n + usize::from(end_cursor) <= width {
        let caret = LineCaret {
            cursor: Some(pos),
            selection: cursor.selection(),
        };
        return cursor_spans_styled(value, caret, styles);
    }
    let max_start = (n + usize::from(end_cursor)).saturating_sub(width - 1);
    let room = width.saturating_sub(2).max(1);
    let start = if pos < room {
        0
    } else {
        (pos + 1 - room).min(max_start)
    };
    let lead = usize::from(start > 0);
    let reach = (n - start) + usize::from(end_cursor) <= width - lead;
    let show = if reach { n - start } else { width - (lead + 1) };
    let visible_end = start + show;
    let visible: String = chars[start..visible_end.min(n)].iter().collect();
    let visible_styles =
        &styles[start.min(styles.len())..visible_end.min(n).min(styles.len())];
    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut spans = Vec::new();
    if lead == 1 {
        spans.push(Span::styled("\u{2026}".to_string(), dim));
    }
    let col = (pos >= start && pos <= visible_end).then(|| pos - start);
    let selection = cursor
        .selection()
        .and_then(|(s, e)| intersect(s, e, start, visible_end))
        .map(|(s, e)| (s - start, e - start));
    let caret = LineCaret {
        cursor: col,
        selection,
    };
    spans.extend(cursor_spans_styled(&visible, caret, visible_styles));
    if !reach {
        spans.push(Span::styled("\u{2026}".to_string(), dim));
    }
    spans
}

/// Like [`cursor_spans`], but each visible character `i` uses `styles[i]` as its
/// base style (missing entries fall back to the default).
pub fn cursor_spans_styled(
    visible: &str,
    caret: LineCaret,
    styles: &[Style],
) -> Vec<Span<'static>> {
    let chars: Vec<char> = visible.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::default();
    for (i, c) in chars.iter().enumerate() {
        let base = styles.get(i).copied().unwrap_or_default();
        let style = cell_style(i, caret, base);
        if !run.is_empty() && style != run_style {
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
        }
        if run.is_empty() {
            run_style = style;
        }
        run.push(*c);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    if caret.cursor == Some(chars.len()) {
        spans.push(colors::cursor_block_span());
    }
    spans
}

/// Builds styled spans for `value` truncated to `width` characters (no cursor),
/// appending a dim `…` when clipped. `styles` is indexed per character. Used for
/// the read-only history rows.
pub fn highlighted_spans(
    value: &str,
    styles: &[Style],
    width: usize,
) -> Vec<Span<'static>> {
    let width = width.max(1);
    let chars: Vec<char> = value.chars().collect();
    let clipped = chars.len() > width;
    let show = if clipped { width - 1 } else { chars.len() };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::default();
    for (i, c) in chars.iter().take(show).enumerate() {
        let style = styles.get(i).copied().unwrap_or_default();
        if !run.is_empty() && style != run_style {
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
        }
        if run.is_empty() {
            run_style = style;
        }
        run.push(*c);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    if clipped {
        spans.push(Span::styled(
            "\u{2026}".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    spans
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
        apply_edit_key(&mut text, &mut cursor, key);
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
}
