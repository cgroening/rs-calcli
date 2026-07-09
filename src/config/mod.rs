//! User-facing configuration.
//!
//! [`Config`] holds the default display settings, the look of the TUI and the
//! key bindings. It is loaded from `config.toml` by [`loader`]; missing keys
//! fall back to the defaults here, so an empty or absent file yields a working
//! configuration. [`Config::default`] is the single source of truth for those
//! defaults.

pub mod appearance;
pub mod highlight;
pub mod loader;

use std::collections::BTreeMap;

use thiserror::Error;

pub use crate::config::appearance::{Appearance, CALCLI_THEME};
pub use crate::config::highlight::HighlightColors;
pub use crate::config::loader::load_config;
use crate::domain::format::{AngleMode, FormatSettings, Notation};
use crate::keymap::Keymap;
use crate::theme::{
    Color, ColorOverrides, Palette, ThemeColors, ThemeRegistry,
};

/// Default number of fractional digits.
const DEFAULT_DECIMALS: usize = 3;

/// Default maximum number of history entries kept.
const DEFAULT_MAX_HISTORY: usize = 500;

/// calcli's own theme: the long-standing green accent over the panel bands the
/// clibase layout expects. The header/footer bands sit a step darker than the
/// content surface, so the content reads as a raised plane; `background` stays
/// [`Color::Default`] so the outer fill and the shortcut hints show the
/// terminal's own background.
const CALCLI_COLORS: ThemeColors = ThemeColors {
    accent: Color::hex("#82e38e"),
    foreground: Color::hex("#e5e5e5"),
    background: Color::Default,
    header: Color::hex("#0e0c12"),
    footer: Color::hex("#0e0c12"),
    panel: Color::hex("#16141d"),
    surface: Color::hex("#16141d"),
    border: Color::hex("#3e3e3e"),
    // The lifted border the focused input box draws, so its frame keeps its
    // contrast against the brighter focused fill. `border.lighten(0.15)`, the
    // toolkit's own derivation, spelled out because `lighten` is not `const`.
    border_focus: Color::hex("#4a7c60"),
    success: Color::hex("#a3c995"),
    warning: Color::hex("#ded483"),
    error: Color::hex("#e06c6c"),
    info: Color::hex("#7fb3d4"),
};

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
    /// Whether trailing fractional zeros are dropped on display (`12` not
    /// `12.000`); `decimals` still caps the precision.
    pub trim_trailing_zeros: bool,
    /// Maximum number of history entries kept.
    pub max_history: usize,
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
    /// Maximum height (in text lines) the input field grows to as it wraps.
    pub input_max_lines: usize,
    /// Appearance settings from `[appearance]`.
    pub appearance: Appearance,
    /// User-defined themes from `[themes.<name>]`, layered over the built-ins.
    pub themes: Vec<(String, ThemeColors)>,
    /// Syntax-highlight colours from `[highlight]`.
    pub highlight: HighlightColors,
    /// Per-action key overrides from `[keys]`, resolved by [`crate::keymap`].
    pub keys: BTreeMap<String, Vec<String>>,
    /// Whether destructive actions ask for confirmation first.
    pub confirm_delete: bool,
    /// Whether a soft quit (`q`, `:q`) asks first. The hard `Ctrl+Q` chord
    /// always quits at once. Defaults to `false`, matching calcli's long-
    /// standing behaviour of quitting without a prompt.
    pub confirm_quit: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            notation: Notation::default(),
            decimals: DEFAULT_DECIMALS,
            angle_mode: AngleMode::default(),
            decimal_separator: '.',
            thousands_separator: " ".to_string(),
            trim_trailing_zeros: true,
            max_history: DEFAULT_MAX_HISTORY,
            restore_last_settings: true,
            live_feedback: true,
            history_zebra: false,
            history_spacing: 1,
            history_separator: true,
            input_max_lines: 5,
            appearance: Appearance::default(),
            themes: Vec::new(),
            highlight: HighlightColors::default(),
            keys: BTreeMap::new(),
            confirm_delete: true,
            confirm_quit: false,
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
            trim_trailing_zeros: self.trim_trailing_zeros,
        }
    }

    /// The resolved key map: compiled-in defaults with the `[keys]` overrides
    /// applied. The single source for TUI dispatch and the shortcut hints.
    pub fn keymap(&self) -> Keymap {
        Keymap::from_overrides(&self.keys)
    }

    /// The configured colour overrides, layered over the selected theme. The
    /// palette colour set is the single source (see
    /// [`ColorOverrides::from_lookup`]), so a new colour needs no change here.
    pub fn color_overrides(&self) -> ColorOverrides<'_> {
        ColorOverrides::from_lookup(|name| {
            self.appearance.colors.get(name).map_or("", String::as_str)
        })
    }

    /// The theme registry: the built-ins, calcli's own theme, then any theme
    /// configured under `[themes.<name>]` (which may replace either).
    pub fn theme_registry(&self) -> ThemeRegistry {
        ThemeRegistry::builtin()
            .with_custom([(CALCLI_THEME.to_string(), CALCLI_COLORS)])
            .with_custom(self.themes.iter().cloned())
    }

    /// Resolves the configured theme plus overrides into a [`Palette`].
    pub fn palette(&self) -> Palette {
        let base = self.theme_registry().resolve(&self.appearance.theme);
        Palette::resolve(base, &self.color_overrides())
    }
}

/// Errors raised while loading configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The config file exists but could not be read.
    #[error("cannot read config file {path}")]
    Read {
        /// Path of the config file that could not be read.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The config file was read but is not valid TOML for [`Config`].
    #[error("invalid config file {path}")]
    Parse {
        /// Path of the config file that failed to parse.
        path: String,
        /// The underlying TOML parse error.
        #[source]
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calcli_theme_is_registered_and_selected_by_default() {
        let config = Config::default();
        assert_eq!(config.appearance.theme, CALCLI_THEME);
        assert!(config.theme_registry().contains(CALCLI_THEME));
    }

    #[test]
    fn the_palette_keeps_the_calcli_accent_and_panel_bands() {
        let palette = Config::default().palette();
        assert_eq!(palette.accent, Color::hex("#82e38e"));
        assert_eq!(palette.header, Color::hex("#0e0c12"));
        assert_eq!(palette.footer, Color::hex("#0e0c12"));
        assert_eq!(palette.surface, Color::hex("#16141d"));
    }

    #[test]
    fn the_content_surface_is_lighter_than_the_bands() {
        let palette = Config::default().palette();
        assert!(palette.surface.luminance() > palette.header.luminance());
    }

    #[test]
    fn the_outer_background_is_the_terminal_default() {
        assert_eq!(Config::default().palette().background, Color::Default);
    }

    /// The input fills are hand-tuned rather than derived, so these pin the
    /// relationships that make the field readable, not the exact hex values.
    /// Retuning a colour should not break a test; inverting the design should.
    #[test]
    fn the_input_field_recedes_at_rest_and_lifts_when_focused() {
        let palette = Config::default().palette();
        let surface = palette.surface.luminance();

        // At rest the field sinks into the content it sits on ...
        assert!(palette.input_bg.luminance() < surface, "resting fill");
        // ... but never below the header/status bands, or it would read as a
        // hole punched through the panel.
        assert!(palette.input_bg.luminance() > palette.header.luminance());

        // Focused, it lifts clearly above the surface, so where the keyboard is
        // is never in doubt ...
        assert!(
            palette.input_bg_active.luminance() > surface,
            "focused fill"
        );
        // ... yet stays below its own border, which is exactly what `ratada`'s
        // lighten-the-surface derivation gets wrong on a near-black surface.
        assert!(
            palette.input_bg_active.luminance() < palette.border.luminance()
        );
    }

    /// Hand-tuned like the input fills, so pin the relationships rather than
    /// the hex value: retuning the tint must not break a test.
    #[test]
    fn the_selected_row_is_marked_without_shouting() {
        let palette = Config::default().palette();

        // Visible against the content it tints ...
        assert!(palette.selection.luminance() > palette.surface.luminance());
        // ... never brighter than the brightest chrome stroke, which is what
        // `ratada`'s mix-a-third-of-the-accent derivation produced here ...
        assert!(
            palette.selection.luminance() < palette.border_focus.luminance()
        );
        // ... and always far enough below the text drawn on it.
        assert!(palette.selection.luminance() < palette.foreground.luminance());

        // It still carries the accent's hue, so the row reads as "here" rather
        // than as a plain grey block.
        let (red, green, blue) =
            palette.selection.rgb().expect("a concrete colour");
        assert!(green > red && green > blue, "the accent tint survives");
    }

    #[test]
    fn a_selection_override_still_wins() {
        let config = loader::config_from_str(
            "[appearance.colors]\nselection = \"#010203\"\n",
        )
        .expect("parses");
        assert_eq!(config.palette().selection, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn an_input_fill_override_still_wins() {
        let config = loader::config_from_str(
            "[appearance.colors]\ninput_bg = \"#010203\"\n",
        )
        .expect("parses");
        assert_eq!(config.palette().input_bg, Color::Rgb(1, 2, 3));
        // The other default survives the partial override.
        assert_eq!(
            config.palette().input_bg_active,
            Color::hex(appearance::DEFAULT_INPUT_BG_ACTIVE),
        );
    }

    #[test]
    fn the_block_cursor_stays_red_rather_than_following_the_accent() {
        let palette = Config::default().palette();
        assert_eq!(palette.cursor, Color::hex("#d65c5c"));
        assert_ne!(palette.cursor, palette.accent);
    }

    #[test]
    fn a_user_theme_replaces_the_built_in_calcli_theme() {
        let custom =
            ThemeColors::from_accent(Color::Rgb(1, 2, 3), Color::Rgb(4, 5, 6));
        let config = Config {
            themes: vec![(CALCLI_THEME.to_string(), custom)],
            ..Config::default()
        };
        assert_eq!(config.palette().accent, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn confirm_quit_defaults_off_and_confirm_delete_defaults_on() {
        let config = Config::default();
        assert!(!config.confirm_quit);
        assert!(config.confirm_delete);
    }
}
