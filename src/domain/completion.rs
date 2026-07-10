//! Identifier completion for the input field.
//!
//! Pure helpers that list the names the user can complete to and find the
//! identifier under the caret. No styling and no I/O; the TUI drives
//! [`ratada::autocomplete`] with these. The built-in name lists live in
//! [`crate::domain::highlight`], the single source shared with the syntax
//! highlighter, so a new function or constant needs adding in only one place.

use std::ops::Range;

use crate::domain::highlight::{self, ANS, CONSTANTS, FUNCTIONS};
use crate::domain::variables::VariableStore;

/// The completion candidates for the current session: the defined variables
/// first (most relevant while typing), then the built-in functions, the
/// constants and the `ans` keyword. A variable that shadows a built-in name is
/// listed once.
pub fn candidates(variables: &VariableStore) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (name, _) in variables.iter() {
        push_unique(&mut names, name);
    }
    for name in FUNCTIONS {
        push_unique(&mut names, name);
    }
    for name in CONSTANTS {
        push_unique(&mut names, name);
    }
    push_unique(&mut names, ANS);
    names
}

/// Appends `name` unless it is already present, keeping the list order stable.
fn push_unique(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

/// The character range of the identifier being typed immediately before
/// `cursor`, as an index into `input.chars()`. This is the prefix to match
/// against. Empty (`cursor..cursor`) when the run before the caret is not a
/// valid identifier, so a caret after an operator, a space or a bare number
/// (an identifier cannot start with a digit) offers nothing to complete.
pub fn identifier_before(input: &str, cursor: usize) -> Range<usize> {
    let chars: Vec<char> = input.chars().collect();
    let end = cursor.min(chars.len());
    let start = walk_back(&chars, end);
    if starts_identifier(&chars, start, end) {
        start..end
    } else {
        end..end
    }
}

/// The character range of the whole identifier the `cursor` sits in or at the
/// end of, spanning both directions. This is the span an accepted suggestion
/// replaces, so completing in the middle of a word rewrites all of it rather
/// than leaving its tail behind. Empty when the caret is not in an identifier.
pub fn identifier_at(input: &str, cursor: usize) -> Range<usize> {
    let chars: Vec<char> = input.chars().collect();
    let caret = cursor.min(chars.len());
    let start = walk_back(&chars, caret);
    let mut end = caret;
    while end < chars.len() && highlight::is_identifier_part(chars[end]) {
        end += 1;
    }
    if starts_identifier(&chars, start, end) {
        start..end
    } else {
        caret..caret
    }
}

/// The start of the run of identifier characters ending at `end`.
fn walk_back(chars: &[char], end: usize) -> usize {
    let mut start = end;
    while start > 0 && highlight::is_identifier_part(chars[start - 1]) {
        start -= 1;
    }
    start
}

/// Whether `chars[start..end]` is a non-empty run that begins an identifier
/// (its first character is a letter or `_`, never a digit).
fn starts_identifier(chars: &[char], start: usize, end: usize) -> bool {
    start < end && highlight::is_identifier_start(chars[start])
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

    #[test]
    fn candidates_list_variables_first_then_the_builtins() {
        let candidates = candidates(&vars(&["radius"]));
        assert_eq!(candidates.first().map(String::as_str), Some("radius"));
        assert!(candidates.iter().any(|name| name == "sin"));
        assert!(candidates.iter().any(|name| name == "pi"));
        assert_eq!(candidates.last().map(String::as_str), Some("ans"));
    }

    #[test]
    fn a_variable_shadowing_a_builtin_is_listed_once() {
        let candidates = candidates(&vars(&["min"]));
        assert_eq!(candidates.iter().filter(|name| *name == "min").count(), 1,);
    }

    #[test]
    fn identifier_before_spans_the_word_ending_at_the_caret() {
        assert_eq!(identifier_before("sin", 3), 0..3);
        // The caret in the middle of a word stops at the caret.
        assert_eq!(identifier_before("sinh", 2), 0..2);
        // Only the trailing identifier run counts, not an earlier one.
        assert_eq!(identifier_before("2*co", 4), 2..4);
    }

    #[test]
    fn identifier_before_is_empty_after_a_non_identifier_character() {
        assert_eq!(identifier_before("2+", 2), 2..2);
        assert_eq!(identifier_before("sin(", 4), 4..4);
        assert_eq!(identifier_before("", 0), 0..0);
    }

    #[test]
    fn identifier_before_ignores_a_bare_number() {
        // A digit cannot start an identifier, so a number offers nothing.
        assert_eq!(identifier_before("2", 1), 1..1);
        assert_eq!(identifier_before("1+2", 3), 3..3);
    }

    #[test]
    fn identifier_at_spans_the_whole_word_around_the_caret() {
        // The caret in the middle of `sinh` still spans all of it, so an
        // accepted suggestion replaces the whole word, not just its head.
        assert_eq!(identifier_at("sinh", 2), 0..4);
        assert_eq!(identifier_at("2*cos", 4), 2..5);
        // Not in an identifier: nothing to replace.
        assert_eq!(identifier_at("2+", 2), 2..2);
        assert_eq!(identifier_at("42", 1), 1..1);
    }
}
