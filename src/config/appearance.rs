//! Appearance settings: theme name, optional colour overrides and glyphs.

use std::collections::BTreeMap;

use crate::config::app_colors::DEFAULT_PALETTE_OVERRIDES;
use crate::theme::GlyphVariant;

/// The theme calcli ships with and selects by default.
pub const CALCLI_THEME: &str = "calcli";

/// User-facing appearance configuration. `theme` names a theme (built-in,
/// calcli's own, or one defined under `[themes.<name>]`); `colors` holds
/// optional per-colour overrides keyed by palette colour name, resolved by
/// [`crate::theme`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appearance {
    /// The selected theme name.
    pub theme: String,
    /// Per-colour overrides, keyed by palette colour name. A missing key leaves
    /// the theme/derived colour untouched.
    pub colors: BTreeMap<String, String>,
    /// The glyph variant (Unicode icons or an ASCII fallback).
    pub glyphs: GlyphVariant,
    /// How far dimmed colored text is darkened while the palette color `dim`
    /// is set, in percent (0 = unchanged, 100 = black).
    pub dim_amount: u8,
}

/// The default of [`Appearance::dim_amount`].
pub const DEFAULT_DIM_AMOUNT: u8 = 40;

/// The largest [`Appearance::dim_amount`].
pub const MAX_DIM_AMOUNT: u8 = 100;

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            theme: CALCLI_THEME.to_string(),
            colors: DEFAULT_PALETTE_OVERRIDES
                .iter()
                .map(|(name, value)| {
                    ((*name).to_string(), (*value).to_string())
                })
                .collect(),
            glyphs: GlyphVariant::default(),
            dim_amount: DEFAULT_DIM_AMOUNT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// calcli's preset for the palette color `name`.
    fn preset(name: &str) -> &'static str {
        DEFAULT_PALETTE_OVERRIDES
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
            .expect("a preset color")
    }

    #[test]
    fn defaults_select_the_calcli_theme_and_a_red_cursor() {
        let appearance = Appearance::default();
        assert_eq!(appearance.theme, CALCLI_THEME);
        assert_eq!(
            appearance.colors.get("cursor").map(String::as_str),
            Some(preset("cursor")),
        );
        assert_eq!(appearance.glyphs, GlyphVariant::Unicode);
    }

    #[test]
    fn the_selection_tint_is_defaulted_rather_than_derived() {
        let appearance = Appearance::default();
        assert_eq!(
            appearance.colors.get("selection").map(String::as_str),
            Some(preset("selection")),
        );
    }

    #[test]
    fn the_input_fills_are_defaulted_rather_than_derived() {
        let appearance = Appearance::default();
        assert_eq!(
            appearance.colors.get("input_bg").map(String::as_str),
            Some(preset("input_bg")),
        );
        assert_eq!(
            appearance.colors.get("input_bg_active").map(String::as_str),
            Some(preset("input_bg_active")),
        );
    }
}
