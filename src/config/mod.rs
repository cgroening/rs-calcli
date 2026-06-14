//! User-facing configuration.
//!
//! [`Config`] holds the default display settings and the look of the TUI. It is
//! loaded from `config.toml` by [`loader`]; missing keys fall back to the
//! defaults here, so an empty or absent file yields a working configuration.

pub mod loader;

pub use loader::load_config;

use serde::{Deserialize, Serialize};

use crate::domain::format::{AngleMode, FormatSettings, Notation};

/// Default number of fractional digits.
const DEFAULT_DECIMALS: usize = 3;

/// Default maximum number of history entries kept.
const DEFAULT_MAX_HISTORY: usize = 500;

/// Default accent colour (hex), a muted cyan.
pub const DEFAULT_ACCENT_COLOR: &str = "#82e38e";

/// Which glyph set the TUI renders.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum GlyphSet {
    /// Unicode symbols (default).
    #[default]
    Unicode,
    /// ASCII-only fallback.
    Ascii,
}

/// Theme colours; every field defaults to the built-in look so a missing key
/// changes nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Accent colour name or `#rrggbb` for borders, the active mode and labels.
    pub accent_color: String,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            accent_color: DEFAULT_ACCENT_COLOR.to_string(),
        }
    }
}

/// The resolved configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Default notation.
    pub notation: Notation,
    /// Default number of fractional digits.
    pub decimals: usize,
    /// Default angle mode.
    pub angle_mode: AngleMode,
    /// Decimal mark for display (`.` or `,`).
    pub decimal_separator: char,
    /// Thousands group separator for display (e.g. a space; empty disables it).
    pub thousands_separator: String,
    /// Maximum number of history entries kept.
    pub max_history: usize,
    /// Which glyph set to render.
    pub glyphs: GlyphSet,
    /// Whether to restore the last session's settings on startup; when `false`,
    /// the defaults above are used every time.
    pub restore_last_settings: bool,
    /// Theme colours.
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            notation: Notation::default(),
            decimals: DEFAULT_DECIMALS,
            angle_mode: AngleMode::default(),
            decimal_separator: '.',
            thousands_separator: " ".to_string(),
            max_history: DEFAULT_MAX_HISTORY,
            glyphs: GlyphSet::default(),
            restore_last_settings: true,
            theme: Theme::default(),
        }
    }
}

impl Config {
    /// Builds the initial [`FormatSettings`] from the configured defaults.
    pub fn format_settings(&self) -> FormatSettings {
        FormatSettings {
            notation: self.notation,
            decimals: self.decimals,
            angle_mode: self.angle_mode,
            decimal_separator: self.decimal_separator,
            thousands_separator: self.thousands_separator.clone(),
        }
    }
}
