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
    max_history: Option<usize>,
    glyphs: Option<GlyphSet>,
    restore_last_settings: Option<bool>,
    theme: Option<RawTheme>,
}

/// The `[theme]` table, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    accent_color: Option<String>,
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
        max_history: raw.max_history.unwrap_or(defaults.max_history).max(1),
        glyphs: raw.glyphs.unwrap_or(defaults.glyphs),
        restore_last_settings: raw
            .restore_last_settings
            .unwrap_or(defaults.restore_last_settings),
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
    fn parses_notation_and_angle_mode_from_toml() {
        let raw: RawConfig =
            toml::from_str("notation = \"scientific\"\nangle_mode = \"deg\"\n")
                .unwrap();
        let config = merge(raw);
        assert_eq!(config.notation, Notation::Scientific);
        assert_eq!(config.angle_mode, AngleMode::Deg);
    }
}
