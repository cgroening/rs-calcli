//! The suggestion dropdown drawn over the history.
//!
//! It floats above the input box rather than pushing it down, so accepting or
//! dismissing a suggestion never makes the layout jump.

use ratada::style;
use ratada::theme::Skin;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};

/// Floats the suggestion dropdown in a rounded box anchored to the bottom of
/// the history `area`, right above the input box. Draws nothing when there are
pub(super) fn render_completion(
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
        .border_style(style::border_focus(&skin.palette))
        .style(style::bg(skin.palette.input_bg))
        .padding(Padding::horizontal(1));
    frame.render_widget(Clear, box_area);
    frame.render_widget(
        Paragraph::new(lines[..visible.min(lines.len())].to_vec()).block(block),
        box_area,
    );
}
