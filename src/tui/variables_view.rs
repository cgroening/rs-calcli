//! The variables overlay: a list of saved variables with insert, copy, delete
//! and reset-all actions.

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState,
};

use crate::tui::App;
use crate::tui::colors;
use crate::tui::widgets::{centered_rect, hint_line};

/// Renders the variables overlay centred over `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let accent = app.accent();
    let rect = centered_rect(area.width.saturating_sub(8).min(64), 16, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(" variables ");
    let inner = rect.inner(Margin::new(1, 1));
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);

    let variables: Vec<(String, f64)> = app
        .service()
        .variables()
        .iter()
        .map(|(name, value)| (name.clone(), *value))
        .collect();

    let list_height = inner.height.saturating_sub(2) as usize;
    if variables.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "no variables yet — use =name or name = expr",
            colors::dim(),
        )));
        frame.render_widget(empty, inner);
        render_hints(frame, inner, accent);
        return;
    }

    let offset = list_offset(app, variables.len(), list_height);
    let end = (offset + list_height).min(variables.len());
    let lines: Vec<Line> = (offset..end)
        .map(|index| variable_line(app, &variables[index], index))
        .collect();

    // Reserve a gutter column before the scrollbar when it is shown.
    let overflow = variables.len() > list_height;
    let list_width = if overflow {
        inner.width.saturating_sub(1)
    } else {
        inner.width
    };
    let list_area = Rect {
        width: list_width,
        height: list_height as u16,
        ..inner
    };
    frame.render_widget(Paragraph::new(lines), list_area);
    render_list_scrollbar(frame, rect, variables.len(), list_height, offset);
    render_hints(frame, inner, accent);
}

/// One variable row: `name = value`, highlighted when selected.
fn variable_line(
    app: &App,
    entry: &(String, f64),
    index: usize,
) -> Line<'static> {
    let (name, value) = entry;
    let formatted = app.service().format_display(*value);
    let spans = vec![
        Span::styled(
            format!("{name} "),
            Style::default()
                .fg(app.accent())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("= ".to_string(), colors::dim()),
        Span::raw(formatted),
    ];
    let line = Line::from(spans);
    if app.var_selected() == index {
        return line.style(Style::default().bg(colors::SELECTION_BG));
    }
    line
}

/// Renders the action hints on the bottom line of the overlay.
fn render_hints(frame: &mut Frame, inner: Rect, accent: ratatui::style::Color) {
    if inner.height < 2 {
        return;
    }
    let hints = [
        ("\u{2191}\u{2193}", "select"),
        ("Enter", "insert"),
        ("y/Y", "copy"),
        ("d", "delete"),
        ("R", "reset all"),
        ("Esc", "close"),
    ];
    let line = hint_line(&hints, accent, inner.width as usize);
    let area = Rect {
        y: inner.y + inner.height - 1,
        height: 1,
        ..inner
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// Keeps the selected variable visible within the list viewport.
fn list_offset(app: &App, total: usize, height: usize) -> usize {
    if height == 0 || total <= height {
        app.set_var_offset(0);
        return 0;
    }
    let max_offset = total - height;
    let selected = app.var_selected();
    let mut offset = app.var_offset().min(max_offset);
    if selected < offset {
        offset = selected;
    } else if selected >= offset + height {
        offset = selected + 1 - height;
    }
    app.set_var_offset(offset);
    offset
}

/// Draws the list scrollbar when the variables overflow the viewport.
fn render_list_scrollbar(
    frame: &mut Frame,
    rect: Rect,
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
    frame.render_stateful_widget(scrollbar, rect, &mut state);
}
