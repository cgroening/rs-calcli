//! Loads [`Config`] as defaults → TOML file → environment overrides.
//!
//! Each stage overlays the previous one, so later sources win field by field:
//! [`Config::default`] provides the baseline, an optional `config.toml` is
//! merged over it (a missing file is not an error), and `CALCLI_*` variables
//! overlay the result. Merging is per field, not per section: setting one
//! colour leaves the others alone.
//!
//! Unknown TOML keys are rejected so a typo surfaces instead of being silently
//! ignored; a single out-of-range value (e.g. an unsupported decimal separator)
//! is logged and the default kept, so one typo never blocks startup.
//!
//! # Backwards compatibility
//!
//! calcli 0.2 wrote a flat `[theme]` table and top-level `glyphs`. Those keys
//! are still accepted and mapped onto the new shape: the accent and the three
//! chrome colours become `[appearance.colors]` overrides, and the eight token
//! colours become `[highlight]`. The new sections win where both are present.

mod env;
mod legacy;
mod raw;
mod validate;

use std::io;
use std::path::Path;

use crate::config::appearance::{Appearance, MAX_DIM_AMOUNT};
use crate::config::color_groups::{ColorGroups, ColorMap};
use crate::config::highlight::HighlightColors;
use crate::config::loader::legacy::RawLegacyTheme;
use crate::config::loader::raw::{RawAppearance, RawConfig};
use crate::config::loader::validate::{SplitColors, report_unknown};
use crate::config::{Config, ConfigError};
use crate::theme::{
    GlyphVariant, ThemeColors, parse_color as parse_theme_color,
};
use crate::util::paths;

pub use crate::config::loader::env::apply_env;

/// Loads the configuration from the default config path, then applies the
/// environment overrides.
///
/// A missing file is not an error; the defaults are used in that case.
///
/// # Errors
///
/// Returns [`ConfigError`] when the file exists but cannot be read or parsed.
pub fn load_config() -> Result<Config, ConfigError> {
    load_from(&paths::config_file())
}

/// Loads the configuration from an explicit path, then applies env overrides.
///
/// # Errors
///
/// Returns [`ConfigError`] when the file exists but cannot be read or parsed.
pub fn load_from(path: &Path) -> Result<Config, ConfigError> {
    let mut config = match std::fs::read_to_string(path) {
        Ok(content) => {
            log::info!("loaded config from {}", path.display());
            parse(&content, path)?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            log::debug!("no config file at {}, using defaults", path.display());
            Config::default()
        }
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.display().to_string(),
                source,
            });
        }
    };
    apply_env(&mut config);
    config.path = Some(path.to_path_buf());
    Ok(config)
}

/// Every problem the config file at `path` has: one it cannot be read or
/// parsed for, and each value `load_from` would only report and replace with
/// its default (an unknown colour, the old `[highlight]` table, a
/// `dim_amount` above 100, an unsupported decimal separator). Empty for a
/// clean file and for a missing one. `F5` reloads only a clean
/// configuration.
pub fn problems(path: &Path) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => content_problems(&content),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => vec![format!("cannot read {}: {error}", path.display())],
    }
}

/// The problems of a config file's `content` (see [`problems`]).
fn content_problems(content: &str) -> Vec<String> {
    let raw: RawConfig = match toml::from_str(content) {
        Ok(raw) => raw,
        Err(error) => return vec![error.to_string()],
    };
    let mut problems = Vec::new();
    if raw.highlight.is_some() {
        problems.push(
            "[highlight] moved to [appearance.colors.highlight]".to_string(),
        );
    }
    if let Some(separator) = &raw.decimal_separator
        && !matches!(separator.as_str(), "." | ",")
    {
        problems.push(format!("unsupported decimal_separator {separator:?}"));
    }
    let appearance = raw.appearance.unwrap_or_default();
    if let Some(amount) = appearance.dim_amount
        && amount > MAX_DIM_AMOUNT
    {
        problems.push(format!(
            "appearance.dim_amount = {amount} is above {MAX_DIM_AMOUNT}"
        ));
    }
    let colors = SplitColors::new(appearance.colors, raw.themes);
    problems.extend(colors.unknown().into_iter().map(|(section, name)| {
        format!("unknown colour '{name}' in [{section}]")
    }));
    problems
}

/// Parses and merges a TOML config string over the defaults (no env applied).
///
/// # Errors
///
/// Returns [`ConfigError::Parse`] when the string is not valid config TOML.
pub fn config_from_str(content: &str) -> Result<Config, ConfigError> {
    parse(content, Path::new("<memory>"))
}

fn parse(content: &str, path: &Path) -> Result<Config, ConfigError> {
    let raw: RawConfig =
        toml::from_str(content).map_err(|source| ConfigError::Parse {
            path: path.display().to_string(),
            source,
        })?;
    Ok(merge(raw))
}

/// Merges a raw config over the defaults.
fn merge(raw: RawConfig) -> Config {
    let defaults = Config::default();
    let legacy = raw.theme.unwrap_or_default();
    if raw.highlight.is_some() {
        log::warn!(
            "[highlight] moved to [appearance.colors.highlight], ignoring it"
        );
    }
    let mut appearance = raw.appearance.unwrap_or_default();
    let mut colors =
        SplitColors::new(std::mem::take(&mut appearance.colors), raw.themes);
    report_unknown(&colors.unknown());
    add_legacy_highlight(&mut colors.override_groups, &legacy);
    let appearance = merge_appearance(
        appearance,
        std::mem::take(&mut colors.palette),
        raw.glyphs,
        &legacy,
        defaults.appearance,
    );
    let color_groups = ColorGroups {
        themes: colors.theme_groups,
        overrides: colors.override_groups,
    };
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
        restore_last_settings: raw
            .restore_last_settings
            .unwrap_or(defaults.restore_last_settings),
        live_feedback: raw.live_feedback.unwrap_or(defaults.live_feedback),
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
        confirm_delete: raw.confirm_delete.unwrap_or(defaults.confirm_delete),
        confirm_quit: raw.confirm_quit.unwrap_or(defaults.confirm_quit),
        highlight: HighlightColors::resolve(&color_groups, &appearance.theme),
        appearance,
        color_groups,
        themes: colors
            .themes
            .iter()
            .map(|(name, theme)| (name.clone(), theme_colors(theme)))
            .collect(),
        keys: raw
            .keys
            .into_iter()
            .map(|(action, binding)| (action, binding.into_keys()))
            .collect(),
        path: None,
    }
}

/// Merges `[appearance]` over the defaults, folding in the legacy `[theme]`
/// chrome colours and the legacy top-level `glyphs`. The new keys win.
fn merge_appearance(
    raw: RawAppearance,
    overrides: ColorMap,
    legacy_glyphs: Option<GlyphVariant>,
    legacy: &RawLegacyTheme,
    defaults: Appearance,
) -> Appearance {
    let mut colors = defaults.colors;
    for (name, value) in legacy::chrome_colors(legacy) {
        colors.insert(name.to_string(), value);
    }
    colors.extend(overrides);
    Appearance {
        theme: raw.theme.unwrap_or(defaults.theme),
        colors,
        glyphs: raw.glyphs.or(legacy_glyphs).unwrap_or(defaults.glyphs),
        dim_amount: dim_amount(raw.dim_amount, defaults.dim_amount),
    }
}

/// The configured `dim_amount`, or `default` when it is absent or above 100
/// (reported).
fn dim_amount(raw: Option<u8>, default: u8) -> u8 {
    match raw {
        Some(amount) if amount <= MAX_DIM_AMOUNT => amount,
        Some(amount) => {
            log::warn!(
                "appearance.dim_amount = {amount} is above {MAX_DIM_AMOUNT}, \
                 using {default}"
            );
            default
        }
        None => default,
    }
}

/// Adds the legacy (0.2) `[theme].*_color` token colors to the `highlight`
/// override group where `[appearance.colors.highlight]` is silent.
fn add_legacy_highlight(
    groups: &mut std::collections::BTreeMap<String, ColorMap>,
    legacy: &RawLegacyTheme,
) {
    for (key, value) in legacy::highlight_colors(legacy) {
        groups
            .entry("highlight".to_string())
            .or_default()
            .entry(key.to_string())
            .or_insert(value);
    }
}

/// Converts a raw custom theme colour table into [`ThemeColors`].
fn theme_colors(raw: &ColorMap) -> ThemeColors {
    ThemeColors::from_lookup(|name| {
        raw.get(name)
            .map(String::as_str)
            .and_then(parse_theme_color)
    })
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

#[cfg(test)]
mod problem_tests {
    use super::content_problems;

    #[test]
    fn problems_list_what_loading_would_only_report() {
        assert!(content_problems("").is_empty());
        let found = content_problems(
            "decimal_separator = \";\"\n[appearance]\ndim_amount = 140\n",
        );
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(content_problems("notation = ").len(), 1);
    }
}

#[cfg(test)]
mod tests;
