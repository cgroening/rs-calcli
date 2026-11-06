//! Syntax-highlight colours for the expression tokens.
//!
//! These are calcli's own: `ratada`'s [`Palette`](crate::theme::Palette) has no
//! equivalent for "the colour of a function name", so they are the color
//! group `highlight` (`[appearance.colors.highlight]`, defaults in
//! `config/app_colors.rs`).

use crate::config::appearance::CALCLI_THEME;
use crate::config::color_groups::ColorGroups;
use crate::theme::Color;

/// The resolved colour of every highlighted token kind.
///
/// Parentheses are always dim and carry no colour of their own, so they have no
/// field here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightColors {
    /// Built-in functions (`sin`, `sqrt`).
    pub function: Color,
    /// Constants (`pi`, `e`).
    pub constant: Color,
    /// Operators, rendered bold.
    pub operator: Color,
    /// Numeric literals.
    pub number: Color,
    /// Defined variables.
    pub variable: Color,
    /// The `ans` keyword, rendered underlined.
    pub ans: Color,
    /// Inline comments, rendered italic.
    pub comment: Color,
    /// Unit symbols.
    pub unit: Color,
}

impl HighlightColors {
    /// The highlight colors of `groups` under `theme`.
    #[must_use]
    pub fn resolve(groups: &ColorGroups, theme: &str) -> Self {
        let color = |key: &str| groups.color(theme, "highlight", key);
        HighlightColors {
            function: color("function"),
            constant: color("constant"),
            operator: color("operator"),
            number: color("number"),
            variable: color("variable"),
            ans: color("ans"),
            comment: color("comment"),
            unit: color("unit"),
        }
    }
}

impl Default for HighlightColors {
    /// The built-in highlight colors.
    fn default() -> Self {
        Self::resolve(&ColorGroups::default(), CALCLI_THEME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_carry_the_nox_palette() {
        let colors = HighlightColors::default();
        assert_eq!(colors.function.to_hex(), "#78c2b3");
        assert_eq!(colors.unit.to_hex(), "#ff79c6");
    }
}
