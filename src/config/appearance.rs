//! Appearance settings: theme name, optional colour overrides and glyphs.

use std::collections::BTreeMap;

use crate::theme::GlyphVariant;

/// The theme calcli ships with and selects by default.
pub const CALCLI_THEME: &str = "calcli";

/// Block-cursor colour (a muted red), distinct from the accent and selection.
/// Lives here rather than in [`super::CALCLI_COLORS`] because `cursor` is a
/// palette colour derived by `ratada`, not a theme base colour.
pub const DEFAULT_CURSOR_COLOR: &str = "#d65c5c";

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
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            theme: CALCLI_THEME.to_string(),
            colors: BTreeMap::from([(
                "cursor".to_string(),
                DEFAULT_CURSOR_COLOR.to_string(),
            )]),
            glyphs: GlyphVariant::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_select_the_calcli_theme_and_a_red_cursor() {
        let appearance = Appearance::default();
        assert_eq!(appearance.theme, CALCLI_THEME);
        assert_eq!(
            appearance.colors.get("cursor").map(String::as_str),
            Some(DEFAULT_CURSOR_COLOR),
        );
        assert_eq!(appearance.glyphs, GlyphVariant::Unicode);
    }
}
