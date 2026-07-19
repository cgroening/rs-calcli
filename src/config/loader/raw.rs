//! The on-disk shape of `config.toml`.
//!
//! Every field is optional so a partial file merges cleanly, and every table
//! carries `deny_unknown_fields` so a typo is reported instead of silently
//! ignored. These types mirror the file, not [`Config`](crate::config::Config):
//! keeping them apart is what lets the resolved config change shape without
//! breaking files already on disk.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::config::loader::legacy::RawLegacyTheme;
use crate::domain::format::{AngleMode, Notation};
use crate::theme::GlyphVariant;

/// A colour-override table (`name -> value`), used for `[appearance.colors]`
/// and each `[themes.<name>]`.
pub(super) type ColorMap = BTreeMap<String, String>;

/// The on-disk shape: every field optional so partial files merge cleanly, and
/// wide enough to cover both the 0.2 layout and the current one.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RawConfig {
    pub(super) notation: Option<Notation>,
    pub(super) decimals: Option<usize>,
    pub(super) angle_mode: Option<AngleMode>,
    pub(super) decimal_separator: Option<String>,
    pub(super) thousands_separator: Option<String>,
    pub(super) trim_trailing_zeros: Option<bool>,
    pub(super) max_history: Option<usize>,
    pub(super) restore_last_settings: Option<bool>,
    pub(super) live_feedback: Option<bool>,
    pub(super) history_spacing: Option<usize>,
    pub(super) history_separator: Option<bool>,
    pub(super) input_max_lines: Option<usize>,
    pub(super) confirm_delete: Option<bool>,
    pub(super) confirm_quit: Option<bool>,

    /// Legacy (0.2) top-level glyph set; superseded by `[appearance].glyphs`.
    pub(super) glyphs: Option<GlyphVariant>,
    /// Legacy (0.2) flat `[theme]` colour table. Not to be confused with the
    /// theme *name*, which is the scalar `theme` inside `[appearance]`.
    pub(super) theme: Option<RawLegacyTheme>,

    pub(super) appearance: Option<RawAppearance>,
    /// Syntax-highlight token colours (`[highlight]`).
    pub(super) highlight: Option<RawHighlight>,
    /// User-defined themes, each a `[themes.<name>]` colour table.
    pub(super) themes: BTreeMap<String, ColorMap>,
    /// Per-action key overrides (`[keys]`), each a string or a list of strings.
    pub(super) keys: BTreeMap<String, KeyBinding>,
}

/// The `[appearance]` table, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RawAppearance {
    pub(super) theme: Option<String>,
    pub(super) glyphs: Option<GlyphVariant>,
    /// Per-colour overrides (`[appearance.colors]`), keyed by palette colour.
    pub(super) colors: ColorMap,
}

/// The `[highlight]` table, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RawHighlight {
    pub(super) function: Option<String>,
    pub(super) constant: Option<String>,
    pub(super) operator: Option<String>,
    pub(super) number: Option<String>,
    pub(super) variable: Option<String>,
    pub(super) ans: Option<String>,
    pub(super) comment: Option<String>,
    pub(super) unit: Option<String>,
}

/// A key binding from config: either one key or a list of keys.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum KeyBinding {
    /// A single key.
    One(String),
    /// Several alternative keys.
    Many(Vec<String>),
}

impl KeyBinding {
    pub(super) fn into_keys(self) -> Vec<String> {
        match self {
            KeyBinding::One(key) => vec![key],
            KeyBinding::Many(keys) => keys,
        }
    }
}
