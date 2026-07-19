//! Reporting colour names a section cannot carry.
//!
//! The two colour sections take different key sets and mixing them up would
//! otherwise be silent, so every name is checked against the set its own
//! section accepts.

use crate::config::loader::raw::{ColorMap, RawConfig};
use crate::theme::{Palette, ThemeColors};

/// Every colour the file names in a section that cannot carry it, as
/// `(section, name)` pairs.
///
/// The two sections take different colour sets, and mixing them up is silent:
/// `[appearance.colors]` overrides any palette colour, while a
/// `[themes.<name>]` contributes only the [`ThemeColors`] a palette is derived
/// *from*. Validating a theme against `Palette::KEYS` accepts the derived ones
/// (`selection`, `cursor`, `input_bg`, …) and then drops the value without a
/// word.
pub(super) fn unknown_color_names(raw: &RawConfig) -> Vec<(String, String)> {
    let mut unknown = Vec::new();
    if let Some(appearance) = &raw.appearance {
        for name in unknown_colors(&appearance.colors, Palette::KEYS) {
            unknown.push(("appearance.colors".to_string(), name.to_string()));
        }
    }
    for (theme, colors) in &raw.themes {
        for name in unknown_colors(colors, ThemeColors::KEYS) {
            unknown.push((format!("themes.{theme}"), name.to_string()));
        }
    }
    unknown
}

/// The keys of `colors` that are not in `known`, in file order.
fn unknown_colors<'a>(colors: &'a ColorMap, known: &[&str]) -> Vec<&'a str> {
    colors
        .keys()
        .map(String::as_str)
        .filter(|name| !known.contains(name))
        .collect()
}

/// Warns about each unusable colour name, so a typo (or a colour in the wrong
/// section) surfaces instead of being silently ignored.
pub(super) fn report_unknown(unknown: &[(String, String)]) {
    for (section, name) in unknown {
        log::warn!("unknown colour '{name}' in [{section}], ignoring");
    }
}
