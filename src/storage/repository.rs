//! The persistence port and the serializable shape of the saved state.
//!
//! [`PersistedState`] is what survives between sessions: the last-active
//! settings, the variables and the history (input plus computed value). It is
//! both the public API of this layer and the TOML wire format, so the binary
//! edge converts it to and from the domain types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;
use crate::domain::format::{AngleMode, Notation};

/// Everything calcli persists across sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedState {
    /// The last-active settings, or `None` when never saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<PersistedSettings>,
    /// Variables by name, in sorted order.
    #[serde(default)]
    pub variables: BTreeMap<String, f64>,
    /// History entries, oldest first.
    #[serde(default)]
    pub history: Vec<PersistedEntry>,
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
}

/// One persisted history line: the raw input and its computed value (absent for
/// a line that errored).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedEntry {
    /// The raw input as typed.
    pub input: String,
    /// The computed value, omitted when the line errored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

/// Loads and stores the persisted state.
pub trait StateRepository {
    /// Loads the saved state, or a default (empty) state when none exists.
    ///
    /// # Errors
    /// Returns an error when the backing store exists but cannot be read or
    /// parsed.
    fn load(&self) -> Result<PersistedState>;

    /// Saves `state`, replacing any previous contents.
    ///
    /// # Errors
    /// Returns an error when the state cannot be serialized or written.
    fn save(&self, state: &PersistedState) -> Result<()>;
}
