//! Loads [`Config`] from `config.toml`, merging over the defaults.
//!
//! Every key is optional: an absent file or a partial file yields a valid
//! config. A parse error is surfaced (the user wrote invalid TOML); a single
//! out-of-range value (e.g. an unsupported decimal separator) is logged and the
//! default kept, so one typo never blocks startup.

use std::fs;

use serde::Deserialize;

use crate::config::{Config, GlyphSet, Theme};
use crate::domain::error::{Error, Result};
use crate::domain::format::{AngleMode, Notation};
use crate::util::paths;

/// The on-disk shape: every field optional so partial files merge cleanly.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    notation: Option<Notation>,
    decimals: Option<usize>,
    angle_mode: Option<AngleMode>,
    decimal_separator: Option<String>,
    thousands_separator: Option<String>,
    trim_trailing_zeros: Option<bool>,
    max_history: Option<usize>,
    glyphs: Option<GlyphSet>,
    restore_last_settings: Option<bool>,
    live_feedback: Option<bool>,
    history_zebra: Option<bool>,
    history_spacing: Option<usize>,
    history_separator: Option<bool>,
    input_max_lines: Option<usize>,
    theme: Option<RawTheme>,
}

/// The `[theme]` table, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    accent_color: Option<String>,
    function_color: Option<String>,
    constant_color: Option<String>,
    operator_color: Option<String>,
    number_color: Option<String>,
    variable_color: Option<String>,
    ans_color: Option<String>,
    comment_color: Option<String>,
    unit_color: Option<String>,
    settings_bar_bg: Option<String>,
    history_alt_bg: Option<String>,
    history_separator_color: Option<String>,
}

/// Loads the configuration from the default config path.
///
/// # Errors
/// Returns [`Error::Config`] when the file exists but is not valid TOML.
pub fn load_config() -> Result<Config> {
    let raw = read_raw_config()?;
    let mut config = merge(raw);
    apply_env(&mut config);
    Ok(config)
}

/// Reads and parses `config.toml`, or returns the empty raw config when absent.
fn read_raw_config() -> Result<RawConfig> {
    let path = paths::config_file();
    if !path.exists() {
        return Ok(RawConfig::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| Error::config(&path, e.to_string()))?;
    toml::from_str(&text).map_err(|e| Error::config(&path, e.to_string()))
}

/// Merges a raw config over the defaults.
fn merge(raw: RawConfig) -> Config {
    let defaults = Config::default();
    Config {
        notation: raw.notation.unwrap_or(defaults.notation),
        decimals: raw.decimals.unwrap_or(defaults.decimals),
        angle_mode: raw.angle_mode.unwrap_or(defaults.angle_mode),
        decimal_separator: raw
            .decimal_separator
            .and_then(|s| parse_separator(&s))
            .unwrap_or(defaults.decimal_separator),
        thousands_separator: raw
            .thousands_separator
            .unwrap_or(defaults.thousands_separator),
        trim_trailing_zeros: raw
            .trim_trailing_zeros
            .unwrap_or(defaults.trim_trailing_zeros),
        max_history: raw.max_history.unwrap_or(defaults.max_history).max(1),
        glyphs: raw.glyphs.unwrap_or(defaults.glyphs),
        restore_last_settings: raw
            .restore_last_settings
            .unwrap_or(defaults.restore_last_settings),
        live_feedback: raw.live_feedback.unwrap_or(defaults.live_feedback),
        history_zebra: raw.history_zebra.unwrap_or(defaults.history_zebra),
        history_spacing: raw
            .history_spacing
            .unwrap_or(defaults.history_spacing),
        history_separator: raw
            .history_separator
            .unwrap_or(defaults.history_separator),
        input_max_lines: raw
            .input_max_lines
            .unwrap_or(defaults.input_max_lines)
            .max(1),
        theme: merge_theme(raw.theme),
    }
}

/// Merges the optional `[theme]` table over the default theme.
fn merge_theme(raw: Option<RawTheme>) -> Theme {
    let defaults = Theme::default();
    let Some(raw) = raw else {
        return defaults;
    };
    Theme {
        accent_color: raw.accent_color.unwrap_or(defaults.accent_color),
        function_color: raw.function_color.unwrap_or(defaults.function_color),
        constant_color: raw.constant_color.unwrap_or(defaults.constant_color),
        operator_color: raw.operator_color.unwrap_or(defaults.operator_color),
        number_color: raw.number_color.unwrap_or(defaults.number_color),
        variable_color: raw.variable_color.unwrap_or(defaults.variable_color),
        ans_color: raw.ans_color.unwrap_or(defaults.ans_color),
        comment_color: raw.comment_color.unwrap_or(defaults.comment_color),
        unit_color: raw.unit_color.unwrap_or(defaults.unit_color),
        settings_bar_bg: raw
            .settings_bar_bg
            .unwrap_or(defaults.settings_bar_bg),
        history_alt_bg: raw.history_alt_bg.unwrap_or(defaults.history_alt_bg),
        history_separator_color: raw
            .history_separator_color
            .unwrap_or(defaults.history_separator_color),
    }
}

/// Parses a decimal-separator string, accepting only `.` or `,`.
fn parse_separator(value: &str) -> Option<char> {
    match value {
        "." => Some('.'),
        "," => Some(','),
        other => {
            log::warn!(
                "ignoring unsupported decimal_separator {other:?}; \
                 using the default"
            );
            None
        }
    }
}

/// Applies `CALCLI_*` environment overrides on top of the file/defaults.
fn apply_env(config: &mut Config) {
    if let Ok(value) = std::env::var("CALCLI_DECIMAL_SEPARATOR")
        && let Some(separator) = parse_separator(&value)
    {
        config.decimal_separator = separator;
    }
    if let Ok(value) = std::env::var("CALCLI_DECIMALS")
        && let Ok(decimals) = value.parse::<usize>()
    {
        config.decimals = decimals;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_raw_config_yields_defaults() {
        let config = merge(RawConfig::default());
        assert_eq!(config, Config::default());
    }

    #[test]
    fn partial_config_overrides_only_given_keys() {
        let raw = RawConfig {
            decimals: Some(6),
            decimal_separator: Some(",".to_string()),
            ..RawConfig::default()
        };
        let config = merge(raw);
        assert_eq!(config.decimals, 6);
        assert_eq!(config.decimal_separator, ',');
        // Untouched keys keep their defaults.
        assert_eq!(config.notation, Config::default().notation);
        assert_eq!(config.thousands_separator, " ");
    }

    #[test]
    fn an_unsupported_separator_falls_back_to_the_default() {
        let raw = RawConfig {
            decimal_separator: Some(";".to_string()),
            ..RawConfig::default()
        };
        assert_eq!(merge(raw).decimal_separator, '.');
    }

    #[test]
    fn max_history_is_at_least_one() {
        let raw = RawConfig {
            max_history: Some(0),
            ..RawConfig::default()
        };
        assert_eq!(merge(raw).max_history, 1);
    }

    #[test]
    fn live_feedback_defaults_on_and_can_be_disabled() {
        assert!(merge(RawConfig::default()).live_feedback);
        let raw: RawConfig = toml::from_str("live_feedback = false\n").unwrap();
        assert!(!merge(raw).live_feedback);
    }

    #[test]
    fn trim_trailing_zeros_defaults_on_and_can_be_disabled() {
        assert!(merge(RawConfig::default()).trim_trailing_zeros);
        let raw: RawConfig =
            toml::from_str("trim_trailing_zeros = false\n").unwrap();
        assert!(!merge(raw).trim_trailing_zeros);
    }

    #[test]
    fn input_max_lines_defaults_and_is_at_least_one() {
        assert_eq!(merge(RawConfig::default()).input_max_lines, 5);
        let raw = RawConfig {
            input_max_lines: Some(0),
            ..RawConfig::default()
        };
        assert_eq!(merge(raw).input_max_lines, 1);
    }

    #[test]
    fn theme_colours_default_and_override_individually() {
        let defaults = merge(RawConfig::default()).theme;
        assert_eq!(defaults, Theme::default());

        let raw: RawConfig =
            toml::from_str("[theme]\nfunction_color = \"#112233\"\n").unwrap();
        let theme = merge(raw).theme;
        assert_eq!(theme.function_color, "#112233");
        // Other colours keep their defaults.
        assert_eq!(theme.constant_color, Theme::default().constant_color);
        assert_eq!(theme.accent_color, Theme::default().accent_color);
    }

    #[test]
    fn parses_notation_and_angle_mode_from_toml() {
        let raw: RawConfig =
            toml::from_str("notation = \"scientific\"\nangle_mode = \"deg\"\n")
                .unwrap();
        let config = merge(raw);
        assert_eq!(config.notation, Notation::Scientific);
        assert_eq!(config.angle_mode, AngleMode::Deg);
    }
}
