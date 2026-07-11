//! Syntax-highlight styles and the caret colours the text editor paints with.
//!
//! Everything the shared palette already names (accent, selection, borders,
//! errors) is read from [`Palette`]. Only the per-token
//! expression colours live here, because no palette colour means "the colour of
//! a function name".

use ratatui::style::{Color, Modifier, Style};

use crate::config::HighlightColors;
use crate::domain::highlight::TokenKind;
use crate::theme::Color as ThemeColor;
use crate::theme::Palette;

/// The colours the block caret and the selection are painted with, resolved
/// once from the palette and handed to the text editor.
#[derive(Debug, Clone, Copy)]
pub struct CaretColors {
    /// Background of the cell the caret covers.
    pub cursor: Color,
    /// Background of the selected cells.
    pub selection: Color,
}

impl CaretColors {
    /// Reads the caret colours from the active palette.
    pub fn from_palette(palette: &Palette) -> Self {
        CaretColors {
            cursor: to_ratatui(palette.cursor),
            selection: to_ratatui(palette.selection),
        }
    }
}

/// Converts a toolkit colour into a ratatui one.
pub fn to_ratatui(color: ThemeColor) -> Color {
    match color.rgb() {
        Some((red, green, blue)) => Color::Rgb(red, green, blue),
        None => Color::Reset,
    }
}

/// A dim style for secondary text.
pub fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// The resolved per-category styles for syntax highlighting.
#[derive(Debug, Clone)]
pub struct Highlight {
    function: Style,
    constant: Style,
    operator: Style,
    number: Style,
    variable: Style,
    unit: Style,
    paren: Style,
    ans: Style,
    comment: Style,
    plain: Style,
}

impl Highlight {
    /// Builds the highlight styles from the configured token colours.
    ///
    /// Operators are rendered bold, parentheses dim, `ans` underlined and
    /// comments italic; the rest use their configured colour plainly.
    pub fn new(colors: &HighlightColors) -> Self {
        let fg = |color: ThemeColor| Style::default().fg(to_ratatui(color));
        Highlight {
            function: fg(colors.function),
            constant: fg(colors.constant),
            operator: fg(colors.operator).add_modifier(Modifier::BOLD),
            number: fg(colors.number),
            variable: fg(colors.variable),
            unit: fg(colors.unit),
            paren: dim(),
            ans: fg(colors.ans).add_modifier(Modifier::UNDERLINED),
            comment: fg(colors.comment).add_modifier(Modifier::ITALIC),
            plain: Style::default(),
        }
    }

    /// The style for a token kind.
    pub fn style(&self, kind: TokenKind) -> Style {
        match kind {
            TokenKind::Function => self.function,
            TokenKind::Constant => self.constant,
            TokenKind::Operator => self.operator,
            TokenKind::Number => self.number,
            TokenKind::Variable => self.variable,
            TokenKind::Unit => self.unit,
            TokenKind::Paren => self.paren,
            TokenKind::Ans => self.ans,
            TokenKind::Comment => self.comment,
            TokenKind::Plain => self.plain,
        }
    }
}

/// Maps per-character token kinds to per-character styles.
pub fn styles_for(kinds: &[TokenKind], highlight: &Highlight) -> Vec<Style> {
    kinds.iter().map(|kind| highlight.style(*kind)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn a_toolkit_colour_maps_onto_a_ratatui_rgb() {
        let rgb = to_ratatui(ThemeColor::hex("#010203"));
        assert_eq!(rgb, Color::Rgb(1, 2, 3));
        assert_eq!(to_ratatui(ThemeColor::Default), Color::Reset);
    }

    #[test]
    fn highlight_maps_the_token_colours_and_adds_the_modifiers() {
        let colors = HighlightColors::default();
        let highlight = Highlight::new(&colors);

        assert_eq!(
            highlight.style(TokenKind::Function).fg,
            Some(to_ratatui(colors.function)),
        );

        let ans = highlight.style(TokenKind::Ans);
        assert_eq!(ans.fg, Some(to_ratatui(colors.ans)));
        assert!(ans.add_modifier.contains(Modifier::UNDERLINED));

        let operator = highlight.style(TokenKind::Operator);
        assert_eq!(operator.fg, Some(to_ratatui(colors.operator)));
        assert!(operator.add_modifier.contains(Modifier::BOLD));

        let comment = highlight.style(TokenKind::Comment);
        assert!(comment.add_modifier.contains(Modifier::ITALIC));

        assert!(
            highlight
                .style(TokenKind::Paren)
                .add_modifier
                .contains(Modifier::DIM)
        );
    }

    #[test]
    fn the_caret_reads_its_colours_from_the_palette() {
        let palette = Config::default().palette();
        let caret = CaretColors::from_palette(&palette);
        assert_eq!(caret.cursor, to_ratatui(palette.cursor));
        assert_eq!(caret.selection, to_ratatui(palette.selection));
        assert_ne!(caret.cursor, caret.selection);
    }

    #[test]
    fn styles_for_maps_one_style_per_character() {
        let highlight = Highlight::new(&HighlightColors::default());
        let kinds = [TokenKind::Number, TokenKind::Operator, TokenKind::Number];
        let styles = styles_for(&kinds, &highlight);
        assert_eq!(styles.len(), 3);
        assert!(styles[1].add_modifier.contains(Modifier::BOLD));
    }
}
