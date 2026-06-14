//! End-to-end persistence check: a session snapshot survives a save/load cycle
//! and rebuilds an equivalent service.

use std::path::PathBuf;

use calcli::config::Config;
use calcli::domain::evaluator::MevalEvaluator;
use calcli::domain::history::{History, HistoryEntry};
use calcli::domain::variables::VariableStore;
use calcli::service::CalcService;
use calcli::storage::{StateRepository, TomlStateRepository};
use calcli::tui::App;

fn temp_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = format!(
        "calcli-it-{}-{:?}.toml",
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
fn a_session_survives_save_and_reload() {
    let config = Config::default();
    let mut service = CalcService::new(
        Box::new(MevalEvaluator::new()),
        config.format_settings(),
        History::new(config.max_history),
        VariableStore::new(),
    );
    service.submit("6 * 7");
    service.submit("=answer");
    service.submit("answer + 1");
    service.toggle_angle_mode();

    let app = App::new(service, &config);
    let snapshot = app.persisted_state();

    let path = temp_path();
    let repository = TomlStateRepository::new(path.clone());
    repository.save(&snapshot).unwrap();
    let loaded = repository.load().unwrap();
    let _ = std::fs::remove_file(&path);

    // Settings persisted.
    let settings = loaded.settings.expect("settings were saved");
    assert_eq!(settings.angle_mode, calcli::domain::format::AngleMode::Deg);

    // Variables persisted.
    assert_eq!(loaded.variables.get("answer"), Some(&42.0));

    // History persisted; rebuilding and recomputing reproduces the values.
    let entries = loaded
        .history
        .iter()
        .map(|entry| HistoryEntry {
            input: entry.input.clone(),
            value: entry.value,
            error: None,
        })
        .collect();
    let mut rebuilt = CalcService::new(
        Box::new(MevalEvaluator::new()),
        config.format_settings(),
        History::from_entries(entries, config.max_history),
        VariableStore::from_pairs(
            loaded
                .variables
                .iter()
                .map(|(name, value)| (name.clone(), *value)),
        ),
    );
    rebuilt.recompute_all();
    assert_eq!(rebuilt.history().entries()[2].value, Some(43.0));
    assert_eq!(rebuilt.variables().get("answer"), Some(42.0));
}
