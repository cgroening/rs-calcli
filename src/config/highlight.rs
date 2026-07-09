//! Syntax-highlight colours for the expression tokens.
//!
//! These are calcli's own: `ratada`'s [`Palette`](crate::theme::Palette) has no
//! equivalent for "the colour of a function name", so they live in their own
//! `[highlight]` config section rather than in `[appearance.colors]`.

use crate::theme::Color;

/// Default token colours, matching the nox.nvim palette.
const DEFAULT_FUNCTION: Color = Color::hex("#78c2b3");
const DEFAULT_CONSTANT: Color = Color::hex("#7c94ff");
const DEFAULT_OPERATOR: Color = Color::hex("#fe7057");
const DEFAULT_NUMBER: Color = Color::hex("#e5cb49");
const DEFAULT_VARIABLE: Color = Color::hex("#b27cde");
const DEFAULT_ANS: Color = Color::hex("#7dcfff");
const DEFAULT_COMMENT: Color = Color::hex("#67c1e5");
const DEFAULT_UNIT: Color = Color::hex("#ff79c6");

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

impl Default for HighlightColors {
    fn default() -> Self {
        HighlightColors {
            function: DEFAULT_FUNCTION,
            constant: DEFAULT_CONSTANT,
            operator: DEFAULT_OPERATOR,
            number: DEFAULT_NUMBER,
            variable: DEFAULT_VARIABLE,
            ans: DEFAULT_ANS,
            comment: DEFAULT_COMMENT,
            unit: DEFAULT_UNIT,
        }
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
