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

/// Default syntax-highlight colours, matching the nox.nvim palette.
pub const DEFAULT_FUNCTION_COLOR: &str = "#78c2b3";
pub const DEFAULT_CONSTANT_COLOR: &str = "#7c94ff";
pub const DEFAULT_OPERATOR_COLOR: &str = "#fe7057";
pub const DEFAULT_NUMBER_COLOR: &str = "#e5cb49";
pub const DEFAULT_VARIABLE_COLOR: &str = "#b27cde";
pub const DEFAULT_ANS_COLOR: &str = "#7dcfff";

/// Default background for the settings bar (a muted dark tint).
pub const DEFAULT_SETTINGS_BAR_BG: &str = "#252525";

/// Default background tint for alternating history entries (zebra striping).
pub const DEFAULT_HISTORY_ALT_BG: &str = "#1a1a1a";

/// Default colour of the separator line between history entries (a muted gray).
pub const DEFAULT_HISTORY_SEPARATOR_COLOR: &str = "#3e3e3e";

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
    /// Syntax-highlight colour for built-in functions (e.g. `sin`).
    pub function_color: String,
    /// Syntax-highlight colour for constants (`pi`, `e`).
    pub constant_color: String,
    /// Syntax-highlight colour for operators (rendered bold).
    pub operator_color: String,
    /// Syntax-highlight colour for numeric literals.
    pub number_color: String,
    /// Syntax-highlight colour for defined variables.
    pub variable_color: String,
    /// Syntax-highlight colour for the `ans` keyword (rendered underlined).
    pub ans_color: String,
    /// Background colour of the full-width settings bar.
    pub settings_bar_bg: String,
    /// Background tint of every second history entry (zebra striping).
    pub history_alt_bg: String,
    /// Colour of the separator line drawn between history entries.
    pub history_separator_color: String,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            accent_color: DEFAULT_ACCENT_COLOR.to_string(),
            function_color: DEFAULT_FUNCTION_COLOR.to_string(),
            constant_color: DEFAULT_CONSTANT_COLOR.to_string(),
            operator_color: DEFAULT_OPERATOR_COLOR.to_string(),
            number_color: DEFAULT_NUMBER_COLOR.to_string(),
            variable_color: DEFAULT_VARIABLE_COLOR.to_string(),
            ans_color: DEFAULT_ANS_COLOR.to_string(),
            settings_bar_bg: DEFAULT_SETTINGS_BAR_BG.to_string(),
            history_alt_bg: DEFAULT_HISTORY_ALT_BG.to_string(),
            history_separator_color: DEFAULT_HISTORY_SEPARATOR_COLOR
                .to_string(),
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
    /// Whether the input field shows a live result preview / validity warning.
    pub live_feedback: bool,
    /// Whether history entries alternate a background tint (zebra striping).
    pub history_zebra: bool,
    /// Blank lines inserted after each history entry's result.
    pub history_spacing: usize,
    /// Whether a separator line is drawn in the gap between history entries.
    pub history_separator: bool,
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
            live_feedback: true,
            history_zebra: false,
            history_spacing: 1,
            history_separator: true,
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
