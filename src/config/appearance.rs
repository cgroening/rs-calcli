//! Appearance settings: theme name, optional colour overrides and glyphs.

use std::collections::BTreeMap;

use crate::theme::GlyphVariant;

/// The theme calcli ships with and selects by default.
pub const CALCLI_THEME: &str = "calcli";

/// Block-cursor colour (a muted red), distinct from the accent and selection.
/// Lives here rather than in `super::CALCLI_COLORS` because `cursor` is a
/// palette colour derived by `ratada`, not a theme base colour.
pub const DEFAULT_CURSOR_COLOR: &str = "#d65c5c";

/// The input field's resting and focused fills.
///
/// `ratada` derives these by lightening `surface`, which assumes a mid-dark
/// surface. calcli's surface is near-black, so the derivation overshoots and
/// the field ends up brighter than its own border.
///
/// At rest the field sits *below* the content surface, receding into it. With
/// the keyboard it lifts clearly above the surface, so where you are typing is
/// never in doubt - but it stays below the border drawn around it. Both are
/// overridable under `[appearance.colors]`.
pub const DEFAULT_INPUT_BG: &str = "#100e15";
/// The fill of the input field while it holds the keyboard, lifted clearly
/// above the content surface so the caret's location is never in doubt.
pub const DEFAULT_INPUT_BG_ACTIVE: &str = "#262336";

/// The tint behind the selected history entry and the selected list row.
///
/// `ratada` derives it as `surface.mix(accent, 0.35)`, which on calcli's
/// near-black surface lands brighter than the border and shouts. This keeps the
/// accent tint - the row still reads as "here" - at roughly the weight of the
/// focused input field. Overridable under `[appearance.colors]`.
pub const DEFAULT_SELECTION_COLOR: &str = "#354440";

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
            colors: BTreeMap::from([
                ("cursor".to_string(), DEFAULT_CURSOR_COLOR.to_string()),
                ("input_bg".to_string(), DEFAULT_INPUT_BG.to_string()),
                (
                    "input_bg_active".to_string(),
                    DEFAULT_INPUT_BG_ACTIVE.to_string(),
                ),
                ("selection".to_string(), DEFAULT_SELECTION_COLOR.to_string()),
            ]),
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

    #[test]
    fn the_selection_tint_is_defaulted_rather_than_derived() {
        let appearance = Appearance::default();
        assert_eq!(
            appearance.colors.get("selection").map(String::as_str),
            Some(DEFAULT_SELECTION_COLOR),
        );
    }

    #[test]
    fn the_input_fills_are_defaulted_rather_than_derived() {
        let appearance = Appearance::default();
        assert_eq!(
            appearance.colors.get("input_bg").map(String::as_str),
            Some(DEFAULT_INPUT_BG),
        );
        assert_eq!(
            appearance.colors.get("input_bg_active").map(String::as_str),
            Some(DEFAULT_INPUT_BG_ACTIVE),
        );
    }
}
