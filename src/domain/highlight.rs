//! Syntax classification for expression highlighting.
//!
//! A pure tokenizer that labels each character of an input with a [`TokenKind`],
//! so the TUI can colour functions, constants, operators, numbers, variables,
//! parentheses and the `ans` keyword. It has no knowledge of styling (a Ratatui
//! concern); the view maps kinds to styles. Unknown identifiers stay
//! [`TokenKind::Plain`] so typing never flashes a colour mid-word.

use crate::domain::expression::is_si_prefix;
use crate::domain::units;
use crate::domain::variables::VariableStore;

/// The known built-in function names (mirrors the meval builtins calcli uses).
const FUNCTIONS: &[&str] = &[
    "sqrt", "exp", "ln", "abs", "sin", "cos", "tan", "asin", "acos", "atan",
    "sinh", "cosh", "tanh", "asinh", "acosh", "atanh", "floor", "ceil",
    "round", "signum", "atan2", "max", "min",
];

/// The known built-in constant names.
const CONSTANTS: &[&str] = &["pi", "e"];

/// The previous-answer keyword.
const ANS: &str = "ans";

/// What a character belongs to, for highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// A built-in function name (e.g. `sin`).
    Function,
    /// A built-in constant (`pi`, `e`).
    Constant,
    /// An operator (`+ - * / ^ =`).
    Operator,
    /// A numeric literal (digits, decimal point, exponent).
    Number,
    /// A defined variable name.
    Variable,
    /// A unit symbol (e.g. `MPa`, `kN`).
    Unit,
    /// A parenthesis.
    Paren,
    /// The `ans` keyword.
    Ans,
    /// An inline comment (from `#` to the end of the line).
    Comment,
    /// Anything else, including unknown identifiers and separators.
    Plain,
}

/// Classifies every character of `input`, returning one [`TokenKind`] per
/// character (so the result has the same length as `input.chars()`).
///
/// `variables` decides whether an identifier is a known variable; unknown names
/// fall back to [`TokenKind::Plain`].
pub fn classify(input: &str, variables: &VariableStore) -> Vec<TokenKind> {
    let chars: Vec<char> = input.chars().collect();
    let mut kinds = vec![TokenKind::Plain; chars.len()];
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if current == crate::domain::expression::COMMENT_CHAR {
            // Everything from here to the end of the line is a comment.
            kinds[index..].fill(TokenKind::Comment);
            break;
        }
        if is_identifier_start(current) {
            let end = identifier_end(&chars, index);
            let name: String = chars[index..end].iter().collect();
            let kind = classify_identifier(&name, variables);
            kinds[index..end].fill(kind);
            index = end;
        } else if current.is_ascii_digit() || current == '.' {
            let end = number_end(&chars, index);
            kinds[index..end].fill(TokenKind::Number);
            index = end;
        } else {
            kinds[index] = classify_symbol(current);
            index += 1;
        }
    }
    kinds
}

/// Whether `c` can start an identifier.
fn is_identifier_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// Whether `c` can continue an identifier.
fn is_identifier_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The exclusive end of the identifier starting at `start`.
fn identifier_end(chars: &[char], start: usize) -> usize {
    let mut end = start;
    while end < chars.len() && is_identifier_part(chars[end]) {
        end += 1;
    }
    end
}

/// The exclusive end of the number starting at `start`, including a trailing
/// `e`/`E` exponent (when followed by an optional sign and a digit) or a single
/// SI-prefix letter at a word boundary (`3.3k`, `100u`), mirroring the SI
/// expansion in [`crate::domain::expression`].
fn number_end(chars: &[char], start: usize) -> usize {
    let mut end = start;
    while end < chars.len()
        && (chars[end].is_ascii_digit() || chars[end] == '.')
    {
        end += 1;
    }
    if end < chars.len() && matches!(chars[end], 'e' | 'E') {
        let mut exponent = end + 1;
        if exponent < chars.len() && matches!(chars[exponent], '+' | '-') {
            exponent += 1;
        }
        if exponent < chars.len() && chars[exponent].is_ascii_digit() {
            end = exponent;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
        }
    } else if end < chars.len() && is_si_prefix(chars[end]) {
        // Only when the prefix is not part of a longer identifier (e.g. `5km`).
        let after_is_identifier =
            end + 1 < chars.len() && is_identifier_part(chars[end + 1]);
        if !after_is_identifier {
            end += 1;
        }
    }
    end
}

/// Classifies an identifier by name.
fn classify_identifier(name: &str, variables: &VariableStore) -> TokenKind {
    if name == ANS {
        TokenKind::Ans
    } else if CONSTANTS.contains(&name) {
        TokenKind::Constant
    } else if FUNCTIONS.contains(&name) {
        TokenKind::Function
    } else if variables.get(name).is_some() {
        TokenKind::Variable
    } else if units::is_unit(name) {
        TokenKind::Unit
    } else {
        TokenKind::Plain
    }
}

/// Classifies a single non-identifier, non-number character. `>` is included so
/// the conversion arrow `->` colours as an operator.
fn classify_symbol(c: char) -> TokenKind {
    match c {
        '+' | '-' | '*' | '/' | '^' | '=' | '>' => TokenKind::Operator,
        '(' | ')' => TokenKind::Paren,
        _ => TokenKind::Plain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::quantity::Quantity;

    fn vars(names: &[&str]) -> VariableStore {
        VariableStore::from_pairs(
            names
                .iter()
                .map(|name| (name.to_string(), Quantity::dimensionless(1.0))),
        )
    }

    /// Collapses per-char kinds into per-token runs for readable assertions.
    fn runs(
        input: &str,
        variables: &VariableStore,
    ) -> Vec<(String, TokenKind)> {
        let kinds = classify(input, variables);
        let chars: Vec<char> = input.chars().collect();
        let mut out: Vec<(String, TokenKind)> = Vec::new();
        for (ch, kind) in chars.into_iter().zip(kinds) {
            match out.last_mut() {
                Some((text, last)) if *last == kind => text.push(ch),
                _ => out.push((ch.to_string(), kind)),
            }
        }
        out
    }

    #[test]
    fn classify_has_one_kind_per_character() {
        assert_eq!(classify("sin(pi)", &vars(&[])).len(), 7);
        assert_eq!(classify("πλ", &vars(&[])).len(), 2);
    }

    #[test]
    fn functions_constants_parens_and_operators() {
        assert_eq!(
            runs("sin(pi)+1", &vars(&[])),
            vec![
                ("sin".to_string(), TokenKind::Function),
                ("(".to_string(), TokenKind::Paren),
                ("pi".to_string(), TokenKind::Constant),
                (")".to_string(), TokenKind::Paren),
                ("+".to_string(), TokenKind::Operator),
                ("1".to_string(), TokenKind::Number),
            ]
        );
    }

    #[test]
    fn defined_variables_colour_but_unknown_identifiers_stay_plain() {
        assert_eq!(
            runs("x*y", &vars(&["x"])),
            vec![
                ("x".to_string(), TokenKind::Variable),
                ("*".to_string(), TokenKind::Operator),
                ("y".to_string(), TokenKind::Plain),
            ]
        );
    }

    #[test]
    fn inline_comment_colours_from_the_hash_to_the_end() {
        assert_eq!(
            runs("2+3 # the sum", &vars(&[])),
            vec![
                ("2".to_string(), TokenKind::Number),
                ("+".to_string(), TokenKind::Operator),
                ("3".to_string(), TokenKind::Number),
                (" ".to_string(), TokenKind::Plain),
                ("# the sum".to_string(), TokenKind::Comment),
            ]
        );
    }

    #[test]
    fn units_and_the_conversion_arrow_are_highlighted() {
        assert_eq!(
            runs("123 MPa -> bar", &vars(&[])),
            vec![
                ("123".to_string(), TokenKind::Number),
                (" ".to_string(), TokenKind::Plain),
                ("MPa".to_string(), TokenKind::Unit),
                (" ".to_string(), TokenKind::Plain),
                ("->".to_string(), TokenKind::Operator),
                (" ".to_string(), TokenKind::Plain),
                ("bar".to_string(), TokenKind::Unit),
            ]
        );
    }

    #[test]
    fn ans_keyword_is_distinct() {
        assert_eq!(
            runs("ans/2", &vars(&[])),
            vec![
                ("ans".to_string(), TokenKind::Ans),
                ("/".to_string(), TokenKind::Operator),
                ("2".to_string(), TokenKind::Number),
            ]
        );
    }

    #[test]
    fn scientific_exponent_is_part_of_the_number() {
        assert_eq!(
            runs("1e3", &vars(&[])),
            vec![("1e3".to_string(), TokenKind::Number)]
        );
        assert_eq!(
            runs("2.5e-3", &vars(&[])),
            vec![("2.5e-3".to_string(), TokenKind::Number)]
        );
    }

    #[test]
    fn si_prefix_suffix_joins_the_number_at_a_word_boundary() {
        assert_eq!(
            runs("3.3k", &vars(&[])),
            vec![("3.3k".to_string(), TokenKind::Number)]
        );
        assert_eq!(
            runs("100u+2", &vars(&[])),
            vec![
                ("100u".to_string(), TokenKind::Number),
                ("+".to_string(), TokenKind::Operator),
                ("2".to_string(), TokenKind::Number),
            ]
        );
        // `5km` does not join the prefix (`k` continues into `km`); `km` is a
        // known unit, so it highlights as one.
        assert_eq!(
            runs("5km", &vars(&[])),
            vec![
                ("5".to_string(), TokenKind::Number),
                ("km".to_string(), TokenKind::Unit),
            ]
        );
    }
}
