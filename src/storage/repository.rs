//! The persistence port and the serializable shape of the saved state.
//!
//! [`PersistedState`] is what survives between sessions: the last-active
//! settings, the variables, the history (input plus computed value) and the UI
//! state (active view, theme, glyphs, hint visibility). Every field carries a
//! serde default, so a `state.toml` written by an older calcli still loads.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::format::{AngleMode, Notation};
use crate::storage::errors::StorageResult;
use crate::theme::GlyphVariant;

/// Everything calcli persists across sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedState {
    /// The last-active settings, or `None` when never saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<PersistedSettings>,
    /// The last-active UI state.
    #[serde(default)]
    pub ui: UiState,
    /// Variables by name, in sorted order.
    #[serde(default)]
    pub variables: BTreeMap<String, PersistedValue>,
    /// History entries, oldest first.
    #[serde(default)]
    pub history: Vec<PersistedEntry>,
}

/// The parts of the interface that survive a restart. Every field is optional
/// so a state file from before this section existed still loads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiState {
    /// Index of the active view (0 = Calc, 1 = Variables, 2 = Settings).
    #[serde(default)]
    pub active_view: usize,
    /// The theme chosen at runtime, overriding the configured one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// The glyph variant chosen at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyphs: Option<GlyphVariant>,
    /// Whether the shortcut-hint footer is shown (toggled with `F1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints_visible: Option<bool>,
}

/// A persisted value: the display number and its unit symbol, if any.
///
/// Written as a table (`x = { value = 50.0, unit = "kN" }`), but a bare number
/// (`g = 9.81`) is also accepted: that is how calcli wrote dimensionless
/// variables before units existed, and such a file must keep loading.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PersistedValue {
    /// The value in its display unit.
    pub value: f64,
    /// The display unit symbol, omitted when dimensionless.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

impl<'de> Deserialize<'de> for PersistedValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        /// The two shapes a persisted variable has ever had on disk.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            /// Pre-units: a dimensionless number.
            Bare(f64),
            /// Current: a value with an optional unit symbol.
            Table {
                value: f64,
                #[serde(default)]
                unit: Option<String>,
            },
        }

        Ok(match Raw::deserialize(deserializer)? {
            Raw::Bare(value) => PersistedValue { value, unit: None },
            Raw::Table { value, unit } => PersistedValue { value, unit },
        })
    }
}

/// The subset of display settings that persists (the rest comes from config).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedSettings {
    /// Active notation.
    pub notation: Notation,
    /// Active number of fractional digits.
    pub decimals: usize,
    /// Active angle mode.
    pub angle_mode: AngleMode,
    /// Active decimal mark, as a one-character string (`.` or `,`).
    pub decimal_separator: String,
    /// Whether trailing fractional zeros are dropped on display. Defaults to
    /// `true` so a state file written before this setting still loads.
    #[serde(default = "default_trim_trailing_zeros")]
    pub trim_trailing_zeros: bool,
}

/// The default for [`PersistedSettings::trim_trailing_zeros`] in older state.
fn default_trim_trailing_zeros() -> bool {
    true
}

/// One persisted history line: the raw input and its computed value (absent for
/// a line that errored).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedEntry {
    /// The raw input as typed.
    pub input: String,
    /// The computed value, omitted for a note or an errored line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The display unit symbol, omitted when dimensionless or absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// Loads and stores the persisted state.
pub trait StateRepository {
    /// Loads the saved state, or a default (empty) state when none exists.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`](crate::storage::errors::StorageError) when
    /// the backing store exists but cannot be read or parsed.
    fn load(&self) -> StorageResult<PersistedState>;

    /// Saves `state`, replacing any previous contents.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`](crate::storage::errors::StorageError) when
    /// the state cannot be serialized or written.
    fn save(&self, state: &PersistedState) -> StorageResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_file_without_a_ui_section_still_loads() {
        let legacy = "\
[settings]
notation = \"decimal\"
decimals = 3
angle_mode = \"deg\"
decimal_separator = \".\"

[variables]
g = 9.81

[[history]]
input = \"2+3\"
value = 5.0
";
        let state: PersistedState = toml::from_str(legacy).unwrap();
        assert_eq!(state.ui, UiState::default());
        assert_eq!(state.ui.active_view, 0);
        assert_eq!(state.ui.hints_visible, None);
        assert!(state.settings.unwrap().trim_trailing_zeros);
        assert_eq!(state.history.len(), 1);
    }

    #[test]
    fn an_empty_state_file_loads_as_the_default() {
        let state: PersistedState = toml::from_str("").unwrap();
        assert!(state.settings.is_none());
        assert!(state.variables.is_empty());
        assert!(state.history.is_empty());
        assert_eq!(state.ui, UiState::default());
    }

    #[test]
    fn the_ui_section_round_trips() {
        let state = PersistedState {
            ui: UiState {
                active_view: 2,
                theme: Some("monochrome".to_string()),
                glyphs: Some(GlyphVariant::Ascii),
                hints_visible: Some(false),
            },
            ..PersistedState::default()
        };
        let text = toml::to_string_pretty(&state).unwrap();
        let loaded: PersistedState = toml::from_str(&text).unwrap();
        assert_eq!(loaded.ui, state.ui);
    }

    #[test]
    fn unset_ui_fields_are_left_out_of_the_written_file() {
        let text = toml::to_string_pretty(&PersistedState::default()).unwrap();
        assert!(!text.contains("theme"));
        assert!(!text.contains("hints_visible"));
    }

    #[test]
    fn a_bare_number_still_loads_as_a_dimensionless_variable() {
        // How calcli wrote variables before units existed. Rejecting this made
        // `load` fail wholesale, silently resetting the entire session.
        let state: PersistedState =
            toml::from_str("[variables]\ng = 9.81\nr = 2.0\n").unwrap();
        assert_eq!(
            state.variables["g"],
            PersistedValue {
                value: 9.81,
                unit: None,
            },
        );
        assert_eq!(state.variables["r"].value, 2.0);
    }

    #[test]
    fn a_unit_bearing_variable_loads_from_its_table() {
        let state: PersistedState = toml::from_str(
            "[variables]\nx = { value = 50.0, unit = \"kN\" }\n",
        )
        .unwrap();
        assert_eq!(state.variables["x"].value, 50.0);
        assert_eq!(state.variables["x"].unit.as_deref(), Some("kN"));
    }

    #[test]
    fn variables_are_always_written_back_as_tables() {
        let state = PersistedState {
            variables: BTreeMap::from([(
                "g".to_string(),
                PersistedValue {
                    value: 9.81,
                    unit: None,
                },
            )]),
            ..PersistedState::default()
        };
        let text = toml::to_string_pretty(&state).unwrap();
        let reloaded: PersistedState = toml::from_str(&text).unwrap();
        assert_eq!(reloaded.variables["g"].value, 9.81);
    }
}
