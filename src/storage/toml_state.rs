//! A [`StateRepository`] backed by a single TOML file.

use std::fs;
use std::path::PathBuf;

use crate::storage::errors::{IoResultExt, StorageResult};
use crate::storage::repository::{PersistedState, StateRepository};
use crate::util::fs::write_atomic;

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
    fn load(&self) -> StorageResult<PersistedState> {
        if !self.path.exists() {
            return Ok(PersistedState::default());
        }
        let text = fs::read_to_string(&self.path)
            .io_context(format!("read {}", self.path.display()))?;
        Ok(toml::from_str(&text)?)
    }

    fn save(&self, state: &PersistedState) -> StorageResult<()> {
        let text = toml::to_string_pretty(state)?;
        write_atomic(&self.path, text.as_bytes())
            .io_context(format!("write {}", self.path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::repository::{
        PersistedEntry, PersistedSettings, PersistedValue, UiState,
    };
    use crate::theme::GlyphVariant;

    fn temp_path(tag: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "calcli-test-{tag}-{}-{:?}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the clock is after the Unix epoch")
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
                trim_trailing_zeros: false,
            }),
            ui: UiState {
                active_view: 1,
                theme: Some("monochrome".to_string()),
                glyphs: Some(GlyphVariant::Ascii),
                hints_visible: Some(false),
            },
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
        assert_eq!(loaded.ui, state.ui);
        let x = loaded.variables.get("x").unwrap();
        assert_eq!(x.value, 50.0);
        assert_eq!(x.unit.as_deref(), Some("kN"));
        assert_eq!(loaded.history.len(), 3);
        assert_eq!(loaded.history[0].value, Some(1230.0));
        assert_eq!(loaded.history[0].unit.as_deref(), Some("bar"));
        assert_eq!(loaded.history[2].value, None);
    }

    #[test]
    fn saving_leaves_no_temporary_file_next_to_the_state() {
        let path = temp_path("atomic");
        let repo = TomlStateRepository::new(path.clone());
        repo.save(&PersistedState::default()).unwrap();
        assert!(path.exists());

        let temporaries: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| {
                name.starts_with("calcli-test-atomic") && name.contains(".tmp.")
            })
            .collect();
        let _ = fs::remove_file(&path);
        assert!(temporaries.is_empty(), "left behind: {temporaries:?}");
    }

    #[test]
    fn an_unparseable_state_file_reports_a_deserialization_error() {
        let path = temp_path("broken");
        fs::write(&path, "= not toml").unwrap();
        let repo = TomlStateRepository::new(path.clone());
        let error = repo.load().unwrap_err();
        let _ = fs::remove_file(&path);
        assert_eq!(error.to_string(), "cannot parse the state file");
    }
}
