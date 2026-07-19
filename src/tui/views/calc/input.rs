//! The growing input box at the bottom of the calc view.
//!
//! The box grows with the expression up to `input_max_lines` rows, and its
//! frame brightens while it has the keyboard. The focus tint is swapped into a
//! copy of the skin rather than applied as a style, because the box title
//! reads its leading stroke from the palette.

use ratada::theme::Skin;
use ratada::{chrome, style};
use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::tui::text_edit::{self, SpanContext};
use crate::tui::views::calc::{CalcView, Mode};

pub(super) fn input_wrap_width(total_width: u16) -> usize {
    (total_width as usize).saturating_sub(4).max(1)
}

/// The input box height (including borders): grows with the wrapped line count
/// in Input mode, clamped to `input_max_lines`; one content line otherwise.
pub(super) fn input_box_height(view: &CalcView<'_>, total_width: u16) -> u16 {
    let content = if matches!(view.mode, Mode::Input) {
        text_edit::wrap_offsets(view.input, input_wrap_width(total_width)).len()
    } else {
        1
    };
    content.clamp(1, view.input_max_lines.max(1)) as u16 + 2
}

/// Renders the input box, returning the wrap width it recorded.
pub(super) fn render_input(
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
pub(super) fn input_fill(skin: &Skin, mode: Mode) -> ratada::theme::Color {
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
pub(super) fn focus_skin(skin: &Skin, mode: Mode) -> Skin {
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
    use super::*;
    use ratada::theme::{
        ColorOverrides, GlyphVariant, Glyphs, Palette, ThemeRegistry,
    };

    use crate::tui::views::calc::testing::{contrast, luminance, skin};

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
