//! Number formatting and the display settings that drive it.
//!
//! The internal value is always a full-precision `f64`; rounding happens only
//! here, on the way to the screen or the clipboard. Two render paths exist:
//! [`format_display`] (rounded, grouped, for the history and the `Y` copy) and
//! [`format_plain`] (full precision, ungrouped, for the `y` copy).

use serde::{Deserialize, Serialize};

/// How an evaluated value is rendered.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Notation {
    /// Fixed-point decimal (e.g. `1 234.500`).
    #[default]
    Decimal,
    /// Scientific notation (e.g. `1.235e3`).
    Scientific,
    /// Engineering notation with an SI prefix (e.g. `1.235 k`).
    SiPrefixed,
}

impl Notation {
    /// The next notation in the cycle, for the toggle shortcut.
    pub fn next(self) -> Self {
        match self {
            Notation::Decimal => Notation::Scientific,
            Notation::Scientific => Notation::SiPrefixed,
            Notation::SiPrefixed => Notation::Decimal,
        }
    }

    /// A short label for the settings bar.
    pub fn label(self) -> &'static str {
        match self {
            Notation::Decimal => "DEC",
            Notation::Scientific => "SCI",
            Notation::SiPrefixed => "SI",
        }
    }
}

/// Whether trigonometric functions interpret arguments as degrees or radians.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum AngleMode {
    /// Radians (the mathematical default; `sin(pi/2) == 1`).
    #[default]
    Rad,
    /// Degrees (`sin(90) == 1`).
    Deg,
}

impl AngleMode {
    /// The opposite mode, for the toggle shortcut.
    pub fn toggled(self) -> Self {
        match self {
            AngleMode::Rad => AngleMode::Deg,
            AngleMode::Deg => AngleMode::Rad,
        }
    }

    /// A short label for the settings bar.
    pub fn label(self) -> &'static str {
        match self {
            AngleMode::Rad => "RAD",
            AngleMode::Deg => "DEG",
        }
    }
}

/// The full set of display settings, shared by the service and the views.
///
/// This is the single source of truth for the settings bar; toggling a value
/// here changes both the rendering and what the bar shows.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatSettings {
    /// How values are rendered.
    pub notation: Notation,
    /// Number of fractional digits in decimal and scientific notation.
    pub decimals: usize,
    /// How trigonometric functions read their arguments.
    pub angle_mode: AngleMode,
    /// Decimal mark for display (`.` or `,`).
    pub decimal_separator: char,
    /// Thousands group separator for display (e.g. a space; empty disables it).
    pub thousands_separator: String,
}

impl FormatSettings {
    /// The input character that acts as a thousands separator alongside spaces
    /// and `_`: whichever of `.`/`,` is *not* the decimal separator.
    pub fn input_thousands_char(&self) -> char {
        if self.decimal_separator == '.' {
            ','
        } else {
            '.'
        }
    }

    /// Toggles the decimal separator between `.` and `,`.
    pub fn toggle_decimal_separator(&mut self) {
        self.decimal_separator = self.input_thousands_char();
    }
}

/// SI prefixes from `1e-12` to `1e12`, ordered by ascending exponent. The empty
/// label marks the `1e0` bucket where no prefix is shown.
const SI_PREFIXES: &[(i32, &str)] = &[
    (-12, "p"),
    (-9, "n"),
    (-6, "µ"),
    (-3, "m"),
    (0, ""),
    (3, "k"),
    (6, "M"),
    (9, "G"),
    (12, "T"),
];

/// Renders `value` for display: rounded to `settings.decimals`, with the
/// configured decimal mark and (in decimal notation) thousands grouping. This
/// is what the history shows and what `Y` copies.
pub fn format_display(value: f64, settings: &FormatSettings) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    match settings.notation {
        Notation::Decimal => format_decimal(value, settings),
        Notation::Scientific => format_scientific(value, settings),
        Notation::SiPrefixed => format_si(value, settings),
    }
}

/// Renders `value` as a plain, directly reusable number: full `f64` precision,
/// the configured decimal mark, and no thousands grouping. This is what `y`
/// copies.
pub fn format_plain(value: f64, settings: &FormatSettings) -> String {
    swap_decimal_mark(&value.to_string(), settings.decimal_separator)
}

/// Fixed-point with grouping on the integer part.
fn format_decimal(value: f64, settings: &FormatSettings) -> String {
    let raw = format!("{value:.prec$}", prec = settings.decimals);
    let (sign, digits) = split_sign(&raw);
    let (int_part, frac_part) = match digits.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (digits, None),
    };
    let grouped = group_thousands(int_part, &settings.thousands_separator);
    let mut out = format!("{sign}{grouped}");
    if let Some(frac) = frac_part {
        out.push(settings.decimal_separator);
        out.push_str(frac);
    }
    out
}

/// `{:.prec$e}` with the decimal mark swapped in.
fn format_scientific(value: f64, settings: &FormatSettings) -> String {
    let raw = format!("{value:.prec$e}", prec = settings.decimals);
    swap_decimal_mark(&raw, settings.decimal_separator)
}

/// Engineering notation: a mantissa in `[1, 1000)` times an SI prefix.
fn format_si(value: f64, settings: &FormatSettings) -> String {
    if value == 0.0 {
        return format_decimal(0.0, settings);
    }
    let exponent = value.abs().log10().floor() as i32;
    let bucket = (exponent.div_euclid(3)) * 3;
    let Some((_, prefix)) = SI_PREFIXES.iter().find(|(e, _)| *e == bucket)
    else {
        // Outside the supported range: fall back to scientific notation.
        return format_scientific(value, settings);
    };
    let mantissa = value / 10f64.powi(bucket);
    let raw = format!("{mantissa:.prec$}", prec = settings.decimals);
    let mantissa = swap_decimal_mark(&raw, settings.decimal_separator);
    if prefix.is_empty() {
        mantissa
    } else {
        format!("{mantissa} {prefix}")
    }
}

/// Splits a leading `-` from a formatted number, returning `(sign, digits)`.
fn split_sign(formatted: &str) -> (&str, &str) {
    match formatted.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", formatted),
    }
}

/// Inserts `separator` every three digits from the right of `int_digits`.
fn group_thousands(int_digits: &str, separator: &str) -> String {
    if separator.is_empty() || int_digits.len() <= 3 {
        return int_digits.to_string();
    }
    let digits: Vec<char> = int_digits.chars().collect();
    let first = digits.len() % 3;
    let mut out: String = digits[..first].iter().collect();
    for chunk in digits[first..].chunks(3) {
        if !out.is_empty() {
            out.push_str(separator);
        }
        out.extend(chunk);
    }
    out
}

/// Replaces the `.` decimal mark in a Rust-formatted number with `mark`.
fn swap_decimal_mark(formatted: &str, mark: char) -> String {
    if mark == '.' {
        return formatted.to_string();
    }
    formatted.replace('.', &mark.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> FormatSettings {
        FormatSettings {
            notation: Notation::Decimal,
            decimals: 3,
            angle_mode: AngleMode::Rad,
            decimal_separator: '.',
            thousands_separator: " ".to_string(),
        }
    }

    #[test]
    fn decimal_groups_thousands_and_keeps_precision() {
        let s = settings();
        assert_eq!(format_display(1234.5, &s), "1 234.500");
        assert_eq!(format_display(-1_000_000.0, &s), "-1 000 000.000");
        assert_eq!(format_display(12.0, &s), "12.000");
    }

    #[test]
    fn decimal_respects_a_comma_mark_and_dot_grouping() {
        let mut s = settings();
        s.decimal_separator = ',';
        s.thousands_separator = ".".to_string();
        assert_eq!(format_display(1234.5, &s), "1.234,500");
    }

    #[test]
    fn scientific_uses_the_configured_mark() {
        let mut s = settings();
        s.notation = Notation::Scientific;
        s.decimals = 3;
        assert_eq!(format_display(1234.0, &s), "1.234e3");
        s.decimal_separator = ',';
        assert_eq!(format_display(1234.0, &s), "1,234e3");
    }

    #[test]
    fn si_prefix_picks_engineering_buckets() {
        let mut s = settings();
        s.notation = Notation::SiPrefixed;
        assert_eq!(format_display(1500.0, &s), "1.500 k");
        assert_eq!(format_display(0.0033, &s), "3.300 m");
        assert_eq!(format_display(2_000_000.0, &s), "2.000 M");
        assert_eq!(format_display(42.0, &s), "42.000");
    }

    #[test]
    fn plain_keeps_full_precision_without_grouping() {
        let s = settings();
        assert_eq!(format_plain(1234.5, &s), "1234.5");
        assert_eq!(
            format_plain(std::f64::consts::PI, &s),
            std::f64::consts::PI.to_string()
        );
    }

    #[test]
    fn plain_swaps_the_decimal_mark() {
        let mut s = settings();
        s.decimal_separator = ',';
        assert_eq!(format_plain(1234.5, &s), "1234,5");
    }
}
