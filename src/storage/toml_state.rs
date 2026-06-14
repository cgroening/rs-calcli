//! A [`StateRepository`] backed by a single TOML file.

use std::fs;
use std::path::PathBuf;

use crate::domain::error::{Error, Result};
use crate::storage::repository::{PersistedState, StateRepository};

/// Reads and writes the persisted state as `state.toml`.
pub struct TomlStateRepository {
    path: PathBuf,
}

impl TomlStateRepository {
    /// Creates a repository backed by the file at `path`.
    pub fn new(path: PathBuf) -> Self {
        TomlStateRepository { path }
    }
}

impl StateRepository for TomlStateRepository {
    fn load(&self) -> Result<PersistedState> {
        if !self.path.exists() {
            return Ok(PersistedState::default());
        }
        let text = fs::read_to_string(&self.path)
            .map_err(|e| Error::state(&self.path, e.to_string()))?;
        toml::from_str(&text)
            .map_err(|e| Error::state(&self.path, e.to_string()))
    }

    fn save(&self, state: &PersistedState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| Error::state(&self.path, e.to_string()))?;
        }
        let text = toml::to_string_pretty(state)
            .map_err(|e| Error::state(&self.path, e.to_string()))?;
        fs::write(&self.path, text)
            .map_err(|e| Error::state(&self.path, e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::repository::{
        PersistedEntry, PersistedSettings, PersistedValue,
    };

    fn temp_path(tag: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "calcli-test-{tag}-{}-{:?}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(unique);
        path
    }

    #[test]
    fn missing_file_loads_default_state() {
        let repo = TomlStateRepository::new(temp_path("missing"));
        let state = repo.load().unwrap();
        assert!(state.settings.is_none());
        assert!(state.variables.is_empty());
        assert!(state.history.is_empty());
    }

    #[test]
    fn save_then_load_round_trips() {
        let path = temp_path("roundtrip");
        let repo = TomlStateRepository::new(path.clone());
        let state = PersistedState {
            settings: Some(PersistedSettings {
                notation: crate::domain::format::Notation::Scientific,
                decimals: 5,
                angle_mode: crate::domain::format::AngleMode::Deg,
                decimal_separator: ",".to_string(),
            }),
            variables: [(
                "x".to_string(),
                PersistedValue {
                    value: 50.0,
                    unit: Some("kN".to_string()),
                },
            )]
            .into_iter()
            .collect(),
            history: vec![
                PersistedEntry {
                    input: "123 MPa -> bar".to_string(),
                    value: Some(1230.0),
                    unit: Some("bar".to_string()),
                },
                PersistedEntry {
                    input: "2+3".to_string(),
                    value: Some(5.0),
                    unit: None,
                },
                PersistedEntry {
                    input: "boom(".to_string(),
                    value: None,
                    unit: None,
                },
            ],
        };

        repo.save(&state).unwrap();
        let loaded = repo.load().unwrap();
        let _ = fs::remove_file(&path);

        assert_eq!(loaded.settings.unwrap().decimals, 5);
        let x = loaded.variables.get("x").unwrap();
        assert_eq!(x.value, 50.0);
        assert_eq!(x.unit.as_deref(), Some("kN"));
        assert_eq!(loaded.history.len(), 3);
        assert_eq!(loaded.history[0].value, Some(1230.0));
        assert_eq!(loaded.history[0].unit.as_deref(), Some("bar"));
        assert_eq!(loaded.history[2].value, None);
    }
}
