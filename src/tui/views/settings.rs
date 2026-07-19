//! The settings view: an editable list of the active display settings.
//!
//! The arrow keys move between rows; `←`/`→` (and `Enter`) step the focused
//! row's value, applied live. The same settings stay reachable from every view
//! through their function keys, so this view is a discoverable surface rather
//! than the only way in.

use std::cell::Cell;

use ratada::theme::{GlyphVariant, Skin};
use ratada::{list, markdown, style};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::domain::format::{AngleMode, FormatSettings, Notation};

/// The thousands separators the view cycles through, and how each reads.
const THOUSANDS: &[(&str, &str)] =
    &[(" ", "space"), ("", "none"), (".", "."), (",", ",")];

/// One editable setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// How values are rendered (decimal, scientific, SI-prefixed).
    Notation,
    /// Fractional digits shown.
    Decimals,
    /// Whether trigonometric functions read degrees or radians.
    AngleMode,
    /// Whether trailing fractional zeros are dropped.
    TrailingZeros,
    /// The decimal mark.
    DecimalSeparator,
    /// The thousands group separator.
    ThousandsSeparator,
    /// Unicode glyphs or the ASCII fallback.
    Glyphs,
    /// The colour theme.
    Theme,
}

/// The largest number of fractional digits the view allows.
const MAX_DECIMALS: usize = 12;

impl Row {
    /// Every row, in display order.
    pub fn all() -> [Row; 8] {
        [
            Row::Notation,
            Row::Decimals,
            Row::AngleMode,
            Row::TrailingZeros,
            Row::DecimalSeparator,
            Row::ThousandsSeparator,
            Row::Glyphs,
            Row::Theme,
        ]
    }

    /// The row at `index`, clamped to the last row.
    pub fn at(index: usize) -> Row {
        let rows = Row::all();
        rows[index.min(rows.len() - 1)]
    }

    /// The label shown in the left column.
    pub fn label(self) -> &'static str {
        match self {
            Row::Notation => "Notation",
            Row::Decimals => "Decimals",
            Row::AngleMode => "Angle mode",
            Row::TrailingZeros => "Trailing zeros",
            Row::DecimalSeparator => "Decimal mark",
            Row::ThousandsSeparator => "Thousands mark",
            Row::Glyphs => "Glyphs",
            Row::Theme => "Theme",
        }
    }
}

/// The state every row reads its current value from.
#[derive(Clone, Copy)]
pub struct Values<'a> {
    /// The active display settings.
    pub settings: &'a FormatSettings,
    /// The active glyph variant.
    pub glyphs: GlyphVariant,
    /// The active theme name.
    pub theme: &'a str,
}

impl Values<'_> {
    /// The current value of `row`, as displayed.
    pub fn of(&self, row: Row) -> String {
        match row {
            Row::Notation => self.settings.notation.label().to_string(),
            Row::Decimals => self.settings.decimals.to_string(),
            Row::AngleMode => self.settings.angle_mode.label().to_string(),
            Row::TrailingZeros => trailing_zeros_label(self.settings),
            Row::DecimalSeparator => {
                self.settings.decimal_separator.to_string()
            }
            Row::ThousandsSeparator => {
                thousands_label(&self.settings.thousands_separator).to_string()
            }
            Row::Glyphs => glyphs_label(self.glyphs).to_string(),
            Row::Theme => self.theme.to_string(),
        }
    }
}

/// Everything the settings view renders from.
pub struct SettingsView<'a> {
    /// The values each row shows.
    pub values: Values<'a>,
    /// The focused row index.
    pub selected: usize,
    /// The scroll offset, kept across frames so a short terminal still follows
    /// the selection instead of snapping back each draw.
    pub offset: &'a Cell<usize>,
}

/// How the trailing-zero setting reads.
fn trailing_zeros_label(settings: &FormatSettings) -> String {
    if settings.trim_trailing_zeros {
        "trim".to_string()
    } else {
        "fixed".to_string()
    }
}

/// How a thousands separator reads in the list and the status band.
pub fn thousands_label(separator: &str) -> &str {
    THOUSANDS
        .iter()
        .find(|(value, _)| *value == separator)
        .map_or(separator, |(_, label)| *label)
}

/// The thousands separator `delta` steps after `current`, wrapping.
pub fn step_thousands(current: &str, delta: isize) -> String {
    let index = THOUSANDS
        .iter()
        .position(|(value, _)| *value == current)
        .unwrap_or(0);
    let next = ratada::nav::cycle(index, THOUSANDS.len(), delta);
    THOUSANDS[next].0.to_string()
}

/// How a glyph variant reads.
pub fn glyphs_label(glyphs: GlyphVariant) -> &'static str {
    match glyphs {
        GlyphVariant::Unicode => "unicode",
        GlyphVariant::Ascii => "ascii",
    }
}

/// The notation `delta` steps after `current`, wrapping.
pub fn step_notation(current: Notation, delta: isize) -> Notation {
    const ALL: [Notation; 3] = [
        Notation::Decimal,
        Notation::Scientific,
        Notation::SiPrefixed,
    ];
    let index = ALL.iter().position(|n| *n == current).unwrap_or(0);
    ALL[ratada::nav::cycle(index, ALL.len(), delta)]
}

/// The angle mode `delta` steps after `current` (two values, so it toggles).
pub fn step_angle(current: AngleMode, delta: isize) -> AngleMode {
    if delta == 0 {
        current
    } else {
        current.toggled()
    }
}

/// The decimal count `delta` steps after `current`, clamped to `0..=MAX`.
pub fn step_decimals(current: usize, delta: isize) -> usize {
    let next = current as isize + delta;
    next.clamp(0, MAX_DECIMALS as isize) as usize
}

/// Renders the settings list into `area`.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    skin: &Skin,
    view: &SettingsView,
) -> usize {
    let palette = &skin.palette;
    let width = Row::all()
        .iter()
        .map(|row| row.label().len())
        .max()
        .unwrap_or(0);

    let rows: Vec<Line<'static>> = Row::all()
        .iter()
        .map(|&row| {
            let value = view.values.of(row);
            let spans = vec![
                Span::styled(
                    format!(" {:width$}  ", row.label(), width = width),
                    style::secondary(palette),
                ),
                Span::styled("\u{2039} ", style::muted(palette)),
                Span::styled(
                    value,
                    style::fg(palette.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" \u{203a}", style::muted(palette)),
            ];
            Line::from(markdown::clip_spans(
                spans,
                area.width as usize,
                style::muted(palette),
            ))
        })
        .collect();

    list::render(
        frame,
        area,
        skin,
        list::ListView {
            rows,
            selected: view.selected.min(Row::all().len() - 1),
            offset: view.offset,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_has_a_label_and_a_stable_index() {
        for (index, row) in Row::all().iter().enumerate() {
            assert_eq!(Row::at(index), *row);
            assert!(!row.label().is_empty());
        }
    }

    #[test]
    fn an_out_of_range_index_clamps_to_the_last_row() {
        assert_eq!(Row::at(99), Row::Theme);
    }

    #[test]
    fn notation_cycles_forwards_and_backwards() {
        assert_eq!(step_notation(Notation::Decimal, 1), Notation::Scientific);
        assert_eq!(
            step_notation(Notation::SiPrefixed, 1),
            Notation::Decimal,
            "wraps at the end",
        );
        assert_eq!(
            step_notation(Notation::Decimal, -1),
            Notation::SiPrefixed,
            "wraps at the start",
        );
    }

    #[test]
    fn decimals_clamp_rather_than_wrap() {
        assert_eq!(step_decimals(3, 1), 4);
        assert_eq!(step_decimals(0, -1), 0);
        assert_eq!(step_decimals(MAX_DECIMALS, 1), MAX_DECIMALS);
    }

    #[test]
    fn the_angle_mode_toggles_in_either_direction() {
        assert_eq!(step_angle(AngleMode::Rad, 1), AngleMode::Deg);
        assert_eq!(step_angle(AngleMode::Rad, -1), AngleMode::Deg);
        assert_eq!(step_angle(AngleMode::Deg, 1), AngleMode::Rad);
    }

    #[test]
    fn the_thousands_separator_cycles_and_reads_by_name() {
        assert_eq!(thousands_label(" "), "space");
        assert_eq!(thousands_label(""), "none");
        assert_eq!(step_thousands(" ", 1), "");
        assert_eq!(step_thousands("", 1), ".");
        assert_eq!(step_thousands(" ", -1), ",", "wraps at the start");
    }

    #[test]
    fn glyph_variants_read_by_name() {
        assert_eq!(glyphs_label(GlyphVariant::Unicode), "unicode");
        assert_eq!(glyphs_label(GlyphVariant::Ascii), "ascii");
    }
}
