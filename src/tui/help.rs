//! The help overlay: a scrollable, grouped list of every shortcut.

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState,
};

use crate::tui::App;
use crate::tui::colors;
use crate::tui::widgets::centered_rect;

/// A group of related shortcuts.
struct Group {
    title: &'static str,
    bindings: &'static [(&'static str, &'static str)],
}

/// Every shortcut, grouped for the overlay. Keep in sync with the footer hints
/// and the README.
const GROUPS: &[Group] = &[
    Group {
        title: "Input",
        bindings: &[
            ("Enter", "evaluate the expression"),
            ("\u{2191} \u{2193}", "move the caret across wrapped lines"),
            ("\u{2191} (top line)", "enter the history"),
            ("Ctrl+Y", "copy the last result (plain)"),
            ("Ctrl+C / X / V", "copy / cut / paste in the input"),
            ("Esc", "clear the input"),
        ],
    },
    Group {
        title: "History",
        bindings: &[
            ("\u{2191} \u{2193}", "select a line"),
            ("PgUp / PgDn", "page through the history"),
            ("Home / End", "jump to the first / last line"),
            ("Enter / e", "edit the selected line"),
            ("d / Del", "delete the selected line"),
            ("y", "copy the value (plain, full precision)"),
            ("Y", "copy the value (as shown, grouped)"),
            ("Esc", "back to the input"),
        ],
    },
    Group {
        title: "Editing a line",
        bindings: &[
            ("Enter", "apply and recompute below"),
            ("Esc", "cancel the edit"),
        ],
    },
    Group {
        title: "Settings",
        bindings: &[
            ("F2", "cycle notation (dec / sci / SI)"),
            ("F3", "toggle angle mode (deg / rad)"),
            ("F5", "toggle decimal separator (. / ,)"),
            (":d[n] :s[n] :si[n]", "set notation and decimals"),
            (":deg :rad", "set the angle mode"),
            (":clear", "clear the history"),
        ],
    },
    Group {
        title: "Variables",
        bindings: &[
            ("F4", "open the variables overlay"),
            ("=name", "save the last answer to a variable"),
            ("name = expr", "assign an expression to a variable"),
            ("Enter", "insert the variable into the input"),
            ("y / Y", "copy the value"),
            ("d", "delete the variable"),
            ("R", "reset all variables"),
        ],
    },
    Group {
        title: "Global",
        bindings: &[
            ("F1", "toggle this help"),
            ("Ctrl+Q", "quit (saving the session)"),
        ],
    },
];

/// Renders the help overlay centred over `area`, scrolled by `app.help_scroll`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let accent = app.accent();
    let rect = centered_rect(
        area.width.saturating_sub(6).min(70),
        area.height.saturating_sub(4).min(26),
        area,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(" help \u{00b7} F1 / Esc to close ");
    let inner = rect.inner(Margin::new(1, 1));
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);

    let lines = build_lines(accent);
    let height = inner.height as usize;
    let max_scroll = lines.len().saturating_sub(height);
    let scroll = app.help_scroll().min(max_scroll);
    let visible: Vec<Line> =
        lines.into_iter().skip(scroll).take(height).collect();
    frame.render_widget(Paragraph::new(visible), inner);

    if max_scroll > 0 {
        let mut state = ScrollbarState::new(max_scroll).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .style(colors::dim());
        frame.render_stateful_widget(scrollbar, rect, &mut state);
    }
}

/// The total number of rendered lines, for clamping the scroll position.
pub fn line_count() -> usize {
    GROUPS
        .iter()
        .map(|group| group.bindings.len() + 2)
        .sum::<usize>()
        .saturating_sub(1)
}

/// Builds the flat list of help lines from the grouped bindings.
fn build_lines(accent: Color) -> Vec<Line<'static>> {
    let title_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let key_style = Style::default().fg(accent);
    let mut lines: Vec<Line> = Vec::new();
    for (index, group) in GROUPS.iter().enumerate() {
        if index != 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(group.title, title_style)));
        for (key, description) in group.bindings {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<18}"), key_style),
                Span::styled((*description).to_string(), colors::dim()),
            ]));
        }
    }
    lines
}
