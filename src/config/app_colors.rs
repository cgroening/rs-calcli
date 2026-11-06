//! calcli's built-in colors: its own theme, the palette colors it presets
//! over the toolkit's derivation, and the syntax-highlight group. This is the
//! one place in the app that holds color values (`tests/theme_only_colours.rs`
//! keeps it so).

use super::color_groups::ColorGroup;
use crate::theme::{Color, ThemeColors};

/// calcli's own theme: the long-standing green accent over the panel bands the
/// clibase layout expects. The header/footer bands sit a step darker than the
/// content surface, so the content reads as a raised plane; `background` stays
/// [`Color::Default`] so the outer fill and the shortcut hints show the
/// terminal's own background.
pub const CALCLI_COLORS: ThemeColors = ThemeColors {
    accent: Color::hex("#82e38e"),
    foreground: Color::hex("#e5e5e5"),
    background: Color::Default,
    header: Color::hex("#0b0f0c"),
    footer: Color::hex("#0a0d0b"),
    // A step above `surface`, the raised plane a toolkit widget lifts onto.
    // calcli draws no such widget today, so nothing on screen wears it yet.
    panel: Color::hex("#1b1926"),
    surface: Color::hex("#16141d"),
    border: Color::hex("#3e3e3e"),
    // The border the focused input box draws. It has to hold its contrast
    // against the brighter focused fill, where a fixed `border` would fade out.
    border_focus: Color::hex("#4a7c60"),
    success: Color::hex("#a3c995"),
    warning: Color::hex("#ded483"),
    error: Color::hex("#e06c6c"),
    info: Color::hex("#7fb3d4"),
    dim: Color::Default,
};

/// Palette colors calcli presets in `[appearance.colors]` because the
/// toolkit's derivation does not suit its near-black surface. Each stays
/// overridable there.
///
/// - `cursor`: a muted red block cursor, distinct from accent and selection.
/// - `input_bg` / `input_bg_active`: the input field's fills. The derivation
///   lightens `surface` and overshoots here; at rest the field recedes below
///   the surface, focused it lifts clearly above it but stays below its border.
/// - `selection`: the selected history entry and list row, an accent tint at
///   roughly the weight of the focused input field instead of the derived mix,
///   which lands brighter than the border.
pub const DEFAULT_PALETTE_OVERRIDES: &[(&str, &str)] = &[
    ("cursor", "#d65c5c"),
    ("input_bg", "#100e15"),
    ("input_bg_active", "#262336"),
    ("selection", "#354440"),
];

/// The app's color groups with their built-in defaults.
pub const APP_COLOR_GROUPS: &[ColorGroup] = &[
    ColorGroup {
        name: "highlight",
        title: "HIGHLIGHT",
        // The nox.nvim palette.
        colors: &[
            ("function", Color::hex("#78c2b3")),
            ("constant", Color::hex("#7c94ff")),
            ("operator", Color::hex("#fe7057")),
            ("number", Color::hex("#e5cb49")),
            ("variable", Color::hex("#b27cde")),
            ("ans", Color::hex("#7dcfff")),
            ("comment", Color::hex("#67c1e5")),
            ("unit", Color::hex("#ff79c6")),
        ],
    },
    ColorGroup {
        name: "completion",
        title: "COMPLETION",
        // The suggestion text of the input's autocomplete dropdown.
        colors: &[("text", Color::hex("#bebebe"))],
    },
];
