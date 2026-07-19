//! The `CALCLI_*` environment overlay, applied last.
//!
//! Env variables win over both the defaults and `config.toml`, so a single run
//! can be re-themed or re-formatted without editing the file. An unparseable
//! value is ignored rather than fatal: the environment is not something the
//! user is editing deliberately at that moment.

use crate::config::Config;
use crate::config::loader::parse_separator;
use crate::theme::GlyphVariant;

/// Applies `CALCLI_*` environment overrides on top of the file/defaults.
pub fn apply_env(config: &mut Config) {
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
    if let Ok(value) = std::env::var("CALCLI_ACCENT")
        && !value.is_empty()
    {
        config.appearance.colors.insert("accent".to_string(), value);
    }
    if let Ok(value) = std::env::var("CALCLI_THEME")
        && !value.is_empty()
    {
        // The name is validated when resolved against the theme registry.
        config.appearance.theme = value;
    }
    if let Ok(value) = std::env::var("CALCLI_GLYPHS") {
        match value.to_ascii_lowercase().as_str() {
            "ascii" => config.appearance.glyphs = GlyphVariant::Ascii,
            "unicode" => config.appearance.glyphs = GlyphVariant::Unicode,
            _ => {}
        }
    }
    if let Ok(value) = std::env::var("CALCLI_CONFIRM_QUIT")
        && let Ok(flag) = value.parse::<bool>()
    {
        config.confirm_quit = flag;
    }
    if let Ok(value) = std::env::var("CALCLI_CONFIRM_DELETE")
        && let Ok(flag) = value.parse::<bool>()
    {
        config.confirm_delete = flag;
    }
}
