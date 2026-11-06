//! Splitting the color tables into palette colors and app color groups, and
//! reporting color names a section cannot carry.
//!
//! The two color sections take different key sets and mixing them up would
//! otherwise be silent, so every name is checked against the set its own
//! section accepts.

use std::collections::BTreeMap;

use crate::config::color_groups::{
    ColorMap, split_entries, unknown_group_keys,
};
use crate::config::loader::raw::RawColors;
use crate::theme::{Palette, ThemeColors};

/// The color tables of a config file, split into palette colors and app color
/// groups.
pub(super) struct SplitColors {
    /// `[appearance.colors]` palette overrides.
    pub(super) palette: ColorMap,
    /// `[appearance.colors.<group>]` tables.
    pub(super) override_groups: BTreeMap<String, ColorMap>,
    /// Each `[themes.<name>]`'s theme colors.
    pub(super) themes: Vec<(String, ColorMap)>,
    /// Each `[themes.<name>]`'s group tables.
    pub(super) theme_groups: BTreeMap<String, BTreeMap<String, ColorMap>>,
}

impl SplitColors {
    /// Splits `[appearance.colors]` and every `[themes.<name>]`.
    pub(super) fn new(
        colors: RawColors,
        themes: BTreeMap<String, RawColors>,
    ) -> Self {
        let (palette, override_groups) = split_entries(colors);
        let mut split = Self {
            palette,
            override_groups,
            themes: Vec::new(),
            theme_groups: BTreeMap::new(),
        };
        for (name, entries) in themes {
            let (theme, groups) = split_entries(entries);
            split.themes.push((name.clone(), theme));
            split.theme_groups.insert(name, groups);
        }
        split
    }

    /// Every color or group the file names in a section that cannot carry
    /// it, as `(section, name)` pairs.
    ///
    /// The two sections take different color sets, and mixing them up is
    /// silent: `[appearance.colors]` overrides any palette color, while a
    /// `[themes.<name>]` contributes only the [`ThemeColors`] a palette is
    /// derived *from*. Validating a theme against `Palette::KEYS` accepts the
    /// derived ones (`selection`, `cursor`, `input_bg`, …) and then drops the
    /// value without a word.
    pub(super) fn unknown(&self) -> Vec<(String, String)> {
        let mut unknown =
            unknown_colors("appearance.colors", &self.palette, Palette::KEYS);
        unknown
            .extend(unknown_groups("appearance.colors", &self.override_groups));
        for (name, theme) in &self.themes {
            let section = format!("themes.{name}");
            unknown.extend(unknown_colors(&section, theme, ThemeColors::KEYS));
            if let Some(groups) = self.theme_groups.get(name) {
                unknown.extend(unknown_groups(&section, groups));
            }
        }
        unknown
    }
}

/// The colors of `section` that it cannot carry, as `(section, name)` pairs.
fn unknown_colors(
    section: &str,
    colors: &ColorMap,
    known: &[&str],
) -> Vec<(String, String)> {
    colors
        .keys()
        .filter(|name| !known.contains(&name.as_str()))
        .map(|name| (section.to_string(), name.clone()))
        .collect()
}

/// The group tables and keys of `section` no app color group defines.
fn unknown_groups(
    section: &str,
    groups: &BTreeMap<String, ColorMap>,
) -> Vec<(String, String)> {
    unknown_group_keys(groups)
        .into_iter()
        .map(|(group, key)| {
            if key.is_empty() {
                (section.to_string(), group)
            } else {
                (format!("{section}.{group}"), key)
            }
        })
        .collect()
}

/// Warns about each unusable colour name, so a typo (or a colour in the wrong
/// section) surfaces instead of being silently ignored.
pub(super) fn report_unknown(unknown: &[(String, String)]) {
    for (section, name) in unknown {
        log::warn!("unknown colour '{name}' in [{section}], ignoring");
    }
}
