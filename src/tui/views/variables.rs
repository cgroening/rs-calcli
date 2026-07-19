//! The variables view: the saved variables with insert, copy, delete and
//! reset-all actions.

use std::cell::Cell;

use ratada::theme::Skin;
use ratada::{list, markdown, style};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// A variable row as the view needs it: the name and its formatted value.
pub struct Variable {
    /// The variable name.
    pub name: String,
    /// The value, already formatted for display.
    pub value: String,
}

/// Everything the variables view renders from.
pub struct VariablesView<'a> {
    /// The variables, in sorted order.
    pub variables: &'a [Variable],
    /// The focused row index.
    pub selected: usize,
    /// The scroll offset, kept across frames.
    pub offset: &'a Cell<usize>,
}

/// Renders the variables list into `area` and returns its viewport height in
/// rows, which the key path uses as the page size.
///
/// The `position/total` badge lands in a reserved last row rather than over a
/// row, so it never hides a variable.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    skin: &Skin,
    view: &VariablesView,
) -> usize {
    if view.variables.is_empty() {
        let hint = Line::from(Span::styled(
            "no variables yet - use =name or name = expr",
            style::secondary(&skin.palette),
        ));
        frame.render_widget(Paragraph::new(hint), area);
        return 0;
    }

    let width = area.width as usize;
    let rows: Vec<Line<'static>> = view
        .variables
        .iter()
        .map(|variable| variable_line(skin, variable, width))
        .collect();

    list::render_counted(
        frame,
        area,
        skin,
        list::ListView {
            rows,
            selected: view.selected.min(view.variables.len() - 1),
            offset: view.offset,
        },
    )
}

/// One variable row: `name = value`, clipped to `width` columns.
///
/// A name or value can be arbitrarily long, and the list widget hands its rows
/// straight to ratatui, which cuts them off mid-glyph without a mark. Clipping
/// here is what puts the `…` on.
fn variable_line(
    skin: &Skin,
    variable: &Variable,
    width: usize,
) -> Line<'static> {
    let palette = &skin.palette;
    let spans = vec![
        Span::styled(
            format!(" {} ", variable.name),
            style::fg(palette.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled("= ", style::secondary(palette)),
        Span::raw(variable.value.clone()),
    ];
    Line::from(markdown::clip_spans(spans, width, style::muted(palette)))
}

#[cfg(test)]
mod tests {
    use ratada::theme::{
        ColorOverrides, GlyphVariant, Glyphs, Palette, ThemeRegistry,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn skin() -> Skin {
        let base = ThemeRegistry::builtin().resolve("default");
        Skin::new(
            Palette::resolve(base, &ColorOverrides::default()),
            Glyphs::new(GlyphVariant::Unicode),
        )
    }

    fn draw(variables: &[Variable], selected: usize) -> String {
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let offset = Cell::new(0);
        let view = VariablesView {
            variables,
            selected,
            offset: &offset,
        };
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &skin(), &view);
            })
            .expect("draw");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn variable(name: &str, value: &str) -> Variable {
        Variable {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn an_empty_store_shows_a_hint_instead_of_a_list() {
        let screen = draw(&[], 0);
        assert!(screen.contains("no variables yet"));
    }

    #[test]
    fn variables_render_as_name_equals_value() {
        let screen = draw(&[variable("g", "9.81")], 0);
        assert!(screen.contains("g = 9.81"), "got: {screen}");
    }

    #[test]
    fn the_position_badge_reports_the_selection_not_the_row_count() {
        let variables = [variable("a", "1"), variable("b", "2")];
        let screen = draw(&variables, 1);
        // `render_counted` writes a 1-based `selected/total` badge.
        assert!(screen.contains("2/2"), "got: {screen}");
    }

    #[test]
    fn an_out_of_range_selection_is_clamped_rather_than_panicking() {
        let screen = draw(&[variable("g", "9.81")], 99);
        assert!(screen.contains("9.81"));
    }
}
