//! Pure string transforms that turn raw user input into a meval-ready
//! expression, plus the classification of an input line into a [`Statement`].
//!
//! Nothing here evaluates or holds state; the service drives the pipeline,
//! supplying the previous answer and variable values. The separator handling is
//! deliberately lenient on input (see [`preprocess`]) while display formatting
//! stays strict in [`crate::domain::format`].

use std::sync::LazyLock;

// https://crates.io/crates/regex
use regex::{Captures, Regex};

/// What a single input line asks the calculator to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// `=name` - store the previous answer in a variable.
    SaveAns(String),
    /// `name = expr` - evaluate `expr` and store it in `name`.
    Assign {
        /// The variable name receiving the result.
        name: String,
        /// The expression evaluated for the stored value.
        expr: String,
    },
    /// A bare expression to evaluate.
    Expression(String),
}

/// The character that starts an inline comment (to the end of the line).
pub const COMMENT_CHAR: char = '#';

/// Returns the code part of `input`: everything before the first comment marker.
/// The comment itself is kept by the caller (history/display) but never
/// evaluated.
pub fn strip_comment(input: &str) -> &str {
    match input.split_once(COMMENT_CHAR) {
        Some((code, _comment)) => code,
        None => input,
    }
}

/// SI prefixes accepted on input, mapped to their power-of-ten exponent.
const INPUT_PREFIXES: &[(char, i32)] = &[
    ('p', -12),
    ('n', -9),
    ('µ', -6),
    ('u', -6),
    ('m', -3),
    ('k', 3),
    ('M', 6),
    ('G', 9),
    ('T', 12),
];

/// Whether `c` is an accepted SI-prefix letter (the single source of truth for
/// both SI expansion and number highlighting).
pub fn is_si_prefix(c: char) -> bool {
    INPUT_PREFIXES.iter().any(|(prefix, _)| *prefix == c)
}

/// Matches a number directly followed by an SI prefix letter, both at word
/// boundaries so it never fires inside an identifier (`a5k`, `pi`, `5km`).
static SI_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d+(?:\.\d+)?)([pnµumkMGT])\b")
        .expect("the SI-prefix pattern is a valid, fixed regex")
});

/// Classifies a raw input line as a save, an assignment or an expression.
///
/// Detection works on names only, so it runs before [`preprocess`]; the caller
/// validates the name and preprocesses the expression part afterwards.
pub fn classify(input: &str) -> Statement {
    let trimmed = input.trim();
    if let Some(name) = trimmed.strip_prefix('=') {
        return Statement::SaveAns(name.trim().to_string());
    }
    match trimmed.split_once('=') {
        Some((name, expr)) => Statement::Assign {
            name: name.trim().to_string(),
            expr: expr.trim().to_string(),
        },
        None => Statement::Expression(trimmed.to_string()),
    }
}

/// Turns a raw expression into a meval-ready one: `**`→`^`, lenient separator
/// normalization and SI-prefix expansion.
///
/// `decimal_separator` is the configured decimal mark; whichever of `.`/`,` is
/// not the mark is treated (with spaces and `_`) as a thousands separator and
/// removed. Function arguments are always separated by `;`, which becomes the
/// meval `,`.
pub fn preprocess(expr: &str, decimal_separator: char) -> String {
    let replaced = expr.replace("**", "^");
    let normalized = normalize_separators(&replaced, decimal_separator);
    expand_si_prefixes(&normalized)
}

/// Removes thousands separators, maps the decimal mark to `.` and `;` to `,`.
fn normalize_separators(expr: &str, decimal_separator: char) -> String {
    let thousands = if decimal_separator == '.' { ',' } else { '.' };
    let mut out = String::with_capacity(expr.len());
    for ch in expr.chars() {
        match ch {
            ' ' | '_' => continue,
            c if c == thousands => continue,
            c if c == decimal_separator => out.push('.'),
            ';' => out.push(','),
            other => out.push(other),
        }
    }
    out
}

/// Expands `3.3k` into `3.3*1e3` and so on, for every SI-prefixed number.
fn expand_si_prefixes(expr: &str) -> String {
    SI_PATTERN
        .replace_all(expr, |caps: &Captures| {
            let number = &caps[1];
            let prefix = caps[2].chars().next().expect("group 2 is one char");
            let exponent = INPUT_PREFIXES
                .iter()
                .find(|(c, _)| *c == prefix)
                .map(|(_, e)| *e)
                .expect("the regex only matches known prefixes");
            format!("{number}*1e{exponent}")
        })
        .into_owned()
}

/// Prepends `ans` when `expr` begins with a binary operator and a previous
/// answer exists, so `+5` continues from the last result.
pub fn prepend_ans(expr: &str, ans: Option<f64>) -> String {
    let starts_with_operator = expr
        .chars()
        .next()
        .is_some_and(|c| matches!(c, '+' | '-' | '*' | '/' | '^'));
    if starts_with_operator && ans.is_some() {
        return format!("ans{expr}");
    }
    expr.to_string()
}

/// Replaces the `ans` token with the previous answer (parenthesized for safety).
pub fn substitute_ans(expr: &str, ans: Option<f64>) -> String {
    match ans {
        Some(value) => substitute_identifier(expr, "ans", value),
        None => expr.to_string(),
    }
}

/// Replaces every whole-word occurrence of `name` with `(value)`.
pub fn substitute_identifier(expr: &str, name: &str, value: f64) -> String {
    let pattern = format!(r"\b{}\b", regex::escape(name));
    let Ok(re) = Regex::new(&pattern) else {
        return expr.to_string();
    };
    re.replace_all(expr, format!("({value})")).into_owned()
}

/// Replaces every whole-word occurrence of `name` with `replacement` verbatim.
///
/// Unlike [`substitute_identifier`], the replacement is inserted as-is (the
/// caller is expected to have wrapped it), so a unit-bearing literal such as
/// `(50 kN)` can be substituted into a unit expression for rink.
pub fn substitute_identifier_with(
    expr: &str,
    name: &str,
    replacement: &str,
) -> String {
    let pattern = format!(r"\b{}\b", regex::escape(name));
    let Ok(re) = Regex::new(&pattern) else {
        return expr.to_string();
    };
    // `$` is special in a replacement string; use the closure form to insert
    // the replacement literally (units never contain `$`, but be safe).
    re.replace_all(expr, |_: &Captures| replacement.to_string())
        .into_owned()
}

/// Whether `expr` references the identifier `name` at a word boundary.
pub fn references(expr: &str, name: &str) -> bool {
    let pattern = format!(r"\b{}\b", regex::escape(name));
    Regex::new(&pattern)
        .map(|re| re.is_match(expr))
        .unwrap_or(false)
}

/// Whether `name` is a valid variable name: non-empty `[A-Za-z0-9_]` not
/// starting with a digit.
pub fn is_valid_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether `input` looks like it is still being typed rather than genuinely
/// malformed, so a live validity warning should be suppressed.
///
/// True when the input is empty, ends on a token that clearly expects more
/// (an operator, an open paren, a separator, a decimal point or a scientific
/// `e`), or has more open than closing parentheses. More closing than opening
/// parentheses is a real error, so it is *not* treated as incomplete.
pub fn looks_incomplete(input: &str) -> bool {
    let trimmed = input.trim_end();
    let Some(last) = trimmed.chars().last() else {
        return true;
    };
    if matches!(
        last,
        '+' | '-' | '*' | '/' | '^' | '(' | '.' | ',' | ';' | 'e' | 'E'
    ) {
        return true;
    }
    let open = trimmed.matches('(').count();
    let close = trimmed.matches(')').count();
    open > close
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_comment_keeps_only_the_code_part() {
        assert_eq!(strip_comment("2*pi*r # circumference"), "2*pi*r ");
        assert_eq!(strip_comment("# just a note"), "");
        assert_eq!(strip_comment("2+3"), "2+3");
    }

    #[test]
    fn classify_detects_save_assign_and_expression() {
        assert_eq!(classify("=x"), Statement::SaveAns("x".to_string()));
        assert_eq!(
            classify("y = 5 + 1"),
            Statement::Assign {
                name: "y".to_string(),
                expr: "5 + 1".to_string(),
            }
        );
        assert_eq!(classify("2+2"), Statement::Expression("2+2".to_string()));
    }

    #[test]
    fn preprocess_replaces_power_operator() {
        assert_eq!(preprocess("2**8", '.'), "2^8");
    }

    #[test]
    fn preprocess_strips_thousands_with_dot_decimal() {
        assert_eq!(preprocess("1 000 000.5", '.'), "1000000.5");
        assert_eq!(preprocess("1_000_000.5", '.'), "1000000.5");
        assert_eq!(preprocess("1,000,000.5", '.'), "1000000.5");
    }

    #[test]
    fn preprocess_strips_thousands_with_comma_decimal() {
        assert_eq!(preprocess("1.000.000,5", ','), "1000000.5");
        assert_eq!(preprocess("1 000 000,5", ','), "1000000.5");
        assert_eq!(preprocess("1_000_000,5", ','), "1000000.5");
    }

    #[test]
    fn preprocess_maps_semicolon_to_argument_comma() {
        assert_eq!(preprocess("max(1;2)", '.'), "max(1,2)");
    }

    #[test]
    fn preprocess_expands_si_prefixes() {
        assert_eq!(preprocess("3.3k", '.'), "3.3*1e3");
        assert_eq!(preprocess("100u", '.'), "100*1e-6");
        assert_eq!(preprocess("2M+5", '.'), "2*1e6+5");
    }

    #[test]
    fn si_expansion_leaves_identifiers_alone() {
        // `pi` and `5km` must not be mistaken for prefixes.
        assert_eq!(expand_si_prefixes("pi"), "pi");
        assert_eq!(expand_si_prefixes("5km"), "5km");
        assert_eq!(expand_si_prefixes("a5k"), "a5k");
    }

    #[test]
    fn prepend_ans_only_with_a_leading_operator_and_a_value() {
        assert_eq!(prepend_ans("+5", Some(3.0)), "ans+5");
        assert_eq!(prepend_ans("+5", None), "+5");
        assert_eq!(prepend_ans("5+5", Some(3.0)), "5+5");
    }

    #[test]
    fn substitute_wraps_values_in_parentheses() {
        assert_eq!(substitute_ans("ans+1", Some(-5.0)), "(-5)+1");
        assert_eq!(substitute_identifier("2*x", "x", 4.0), "2*(4)");
        assert_eq!(substitute_identifier("xy+x", "x", 1.0), "xy+(1)");
    }

    #[test]
    fn valid_variable_names() {
        assert!(is_valid_var_name("x"));
        assert!(is_valid_var_name("alpha_1"));
        assert!(is_valid_var_name("_tmp"));
        assert!(!is_valid_var_name(""));
        assert!(!is_valid_var_name("1x"));
        assert!(!is_valid_var_name("a b"));
    }

    #[test]
    fn incomplete_input_suppresses_warnings_while_complete_input_does_not() {
        // Still typing: trailing operator, open paren, separator, more "(".
        assert!(looks_incomplete(""));
        assert!(looks_incomplete("2+"));
        assert!(looks_incomplete("2 * "));
        assert!(looks_incomplete("sin("));
        assert!(looks_incomplete("(2+3"));
        assert!(looks_incomplete("1e"));
        // Complete-looking (a real error or a valid expression).
        assert!(!looks_incomplete("2+3"));
        assert!(!looks_incomplete("2+3)"));
        assert!(!looks_incomplete("sin(90)"));
    }
}
