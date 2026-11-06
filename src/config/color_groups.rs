//! App-specific colors: `[appearance.colors.<group>]` tables next to the
//! palette overrides, and the same `<group>` tables inside a `[themes.<name>]`.
//!
//! The palette covers what every app draws (frame, text, roles). A color only
//! one feature of an app needs - a mode badge, a syntax-highlight kind - lives
//! in a named group instead. Its built-in default sits in [`APP_COLOR_GROUPS`]
//! (the app's built-in theme for these colors); the active theme's group and
//! then `[appearance.colors.<group>]` override it, in that order.
//!
//! The template ships no group; an app lists its own in `config/app_colors.rs`
//! (the only file besides the toolkit's themes that holds color values), and
//! the settings overlay, the writer and the unknown-key report pick them up
//! from there.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::theme::{Color, parse_color};

/// A `name -> value` color table.
pub type ColorMap = BTreeMap<String, String>;

/// One app color group: its table name, the heading the settings overlay
/// shows, and every color with its built-in default.
#[derive(Debug)]
pub struct ColorGroup {
    /// The table name, e.g. `batch` for `[appearance.colors.batch]`.
    pub name: &'static str,
    /// The heading in the settings overlay.
    pub title: &'static str,
    /// Every color of the group with its built-in default.
    pub colors: &'static [(&'static str, Color)],
}

pub use super::app_colors::APP_COLOR_GROUPS;

/// One entry of `[appearance.colors]` or `[themes.<name>]`: a palette color
/// or a group table.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ColorEntry {
    /// A palette color (`accent = "#8bd3cd"`).
    Color(String),
    /// An app color group (`[appearance.colors.batch]`).
    Group(ColorMap),
}

/// Splits raw entries into the palette colors and the group tables.
pub(crate) fn split_entries(
    entries: BTreeMap<String, ColorEntry>,
) -> (ColorMap, BTreeMap<String, ColorMap>) {
    let mut colors = ColorMap::new();
    let mut groups = BTreeMap::new();
    for (name, entry) in entries {
        match entry {
            ColorEntry::Color(value) => {
                colors.insert(name, value);
            }
            ColorEntry::Group(table) => {
                groups.insert(name, table);
            }
        }
    }
    (colors, groups)
}

/// The configured group colors: each theme's groups and the
/// `[appearance.colors.<group>]` overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColorGroups {
    /// Group tables per custom theme name.
    pub themes: BTreeMap<String, BTreeMap<String, ColorMap>>,
    /// Group tables from `[appearance.colors]`, layered over every theme.
    pub overrides: BTreeMap<String, ColorMap>,
}

impl ColorGroups {
    /// The color `key` of `group` under `theme`: the override, else the
    /// theme's value, else the built-in default (`Color::Default` for a key
    /// no group defines). An unparseable value is skipped.
    #[must_use]
    pub fn color(&self, theme: &str, group: &str, key: &str) -> Color {
        let lookup = |tables: Option<&BTreeMap<String, ColorMap>>| {
            tables
                .and_then(|tables| tables.get(group))
                .and_then(|table| table.get(key))
                .and_then(|value| parse_color(value))
        };
        lookup(Some(&self.overrides))
            .or_else(|| lookup(self.themes.get(theme)))
            .unwrap_or_else(|| builtin(group, key))
    }

    /// Whether `[appearance.colors.<group>]` sets `key`.
    #[must_use]
    pub fn is_overridden(&self, group: &str, key: &str) -> bool {
        self.overrides
            .get(group)
            .is_some_and(|table| table.contains_key(key))
    }

    /// Sets (or with `None` removes) the override of `key` in `group`.
    pub fn set_override(
        &mut self,
        group: &str,
        key: &str,
        value: Option<&str>,
    ) {
        let table = self.overrides.entry(group.to_string()).or_default();
        match value {
            Some(value) => {
                table.insert(key.to_string(), value.to_string());
            }
            None => {
                table.remove(key);
            }
        }
    }
}

/// The built-in default of `key` in `group`.
fn builtin(group: &str, key: &str) -> Color {
    APP_COLOR_GROUPS
        .iter()
        .find(|candidate| candidate.name == group)
        .and_then(|candidate| {
            candidate.colors.iter().find(|(name, _)| *name == key)
        })
        .map_or(Color::Default, |(_, color)| *color)
}

/// The group names and keys `groups` holds that no app group defines, as
/// `(group, key)` pairs; an unknown group reports with an empty key.
pub(crate) fn unknown_group_keys(
    groups: &BTreeMap<String, ColorMap>,
) -> Vec<(String, String)> {
    let mut unknown = Vec::new();
    for (name, table) in groups {
        let Some(group) = APP_COLOR_GROUPS.iter().find(|g| g.name == name)
        else {
            unknown.push((name.clone(), String::new()));
            continue;
        };
        for key in table.keys() {
            if !group.colors.iter().any(|(known, _)| known == key) {
                unknown.push((name.clone(), key.clone()));
            }
        }
    }
    unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(pairs: &[(&str, &str)]) -> ColorMap {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn an_override_beats_the_theme_which_beats_the_default() {
        let mut groups = ColorGroups::default();
        let mut ocean = BTreeMap::new();
        ocean.insert("badge".to_string(), table(&[("bg", "#010203")]));
        groups.themes.insert("ocean".to_string(), ocean);

        assert_eq!(groups.color("ocean", "badge", "bg"), Color::Rgb(1, 2, 3));
        assert_eq!(groups.color("default", "badge", "bg"), Color::Default);

        groups.set_override("badge", "bg", Some("ansi:red"));
        assert_eq!(groups.color("ocean", "badge", "bg").to_hex(), "ansi:red");
        assert!(groups.is_overridden("badge", "bg"));

        groups.set_override("badge", "bg", None);
        assert!(!groups.is_overridden("badge", "bg"));
    }

    #[test]
    fn entries_split_into_colors_and_groups() {
        let raw: BTreeMap<String, ColorEntry> =
            toml::from_str("accent = \"#010203\"\n[badge]\nbg = \"#040506\"\n")
                .unwrap();
        let (colors, groups) = split_entries(raw);
        assert_eq!(colors.get("accent").map(String::as_str), Some("#010203"));
        assert_eq!(
            groups["badge"].get("bg").map(String::as_str),
            Some("#040506")
        );
    }

    #[test]
    fn a_group_the_app_does_not_define_is_unknown() {
        let mut groups = BTreeMap::new();
        groups.insert("badge".to_string(), table(&[("bg", "#040506")]));
        assert_eq!(
            unknown_group_keys(&groups),
            vec![("badge".to_string(), String::new())]
        );
    }
}
