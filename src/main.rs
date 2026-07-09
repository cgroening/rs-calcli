//! Composition root: load config and persisted state, wire the service and the
//! TUI, run the event loop and save the session on exit.

use std::process::ExitCode;

use anyhow::Context;
use log::LevelFilter;
use ratada::Tui;

use calcli::config::{Config, load_config};
use calcli::domain::evaluator::MevalEvaluator;
use calcli::domain::format::FormatSettings;
use calcli::domain::history::{History, HistoryEntry};
use calcli::domain::quantity::Quantity;
use calcli::domain::variables::VariableStore;
use calcli::services::CalcService;
use calcli::storage::{PersistedState, StateRepository, TomlStateRepository};
use calcli::tui::{self, App};
use calcli::util::{logging, paths};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("calcli: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Loads everything, runs the TUI and persists the session.
///
/// With `--demo`, the session is seeded with sample data and is *not* saved on
/// exit, so the real `state.toml` is left untouched.
fn run() -> anyhow::Result<()> {
    let demo = std::env::args().any(|arg| arg == "--demo");
    let config = load_config().context("loading configuration")?;
    let _ = logging::init(LevelFilter::Info, Some(&paths::log_file()));

    let repository = TomlStateRepository::new(paths::state_file());
    let state = if demo {
        calcli::demo::demo_state()
    } else {
        repository.load().unwrap_or_else(|error| {
            log::warn!("ignoring unreadable state: {error}");
            PersistedState::default()
        })
    };

    let service = build_service(&config, &state);
    let mut app = App::new(service, &config, &state.ui);

    let mut tui = Tui::new().context("initializing the terminal")?;
    let outcome = tui::run(&mut app, &mut tui);
    drop(tui);
    outcome?;

    if !demo {
        repository
            .save(&app.persisted_state())
            .context("saving the session")?;
    }
    Ok(())
}

/// Builds the calculator service from config and the restored state.
fn build_service(config: &Config, state: &PersistedState) -> CalcService {
    let settings = resolve_settings(config, state);
    let variables = VariableStore::from_pairs(state.variables.iter().map(
        |(name, stored)| {
            let quantity =
                Quantity::from_persisted(stored.value, stored.unit.as_deref());
            (name.clone(), quantity)
        },
    ));
    let entries = state
        .history
        .iter()
        .map(|entry| HistoryEntry {
            input: entry.input.clone(),
            value: entry.value.map(|value| {
                Quantity::from_persisted(value, entry.unit.as_deref())
            }),
            error: None,
        })
        .collect();
    let history = History::from_entries(entries, config.max_history);

    let mut service = CalcService::new(
        Box::new(MevalEvaluator::new()),
        settings,
        history,
        variables,
    );
    // Regenerate values/errors so restored entries match the active settings.
    service.recompute_all();
    service
}

/// Resolves the active settings: config defaults, overridden by the last
/// session when `restore_last_settings` is enabled.
fn resolve_settings(config: &Config, state: &PersistedState) -> FormatSettings {
    let mut settings = config.format_settings();
    if !config.restore_last_settings {
        return settings;
    }
    let Some(persisted) = &state.settings else {
        return settings;
    };
    settings.notation = persisted.notation;
    settings.decimals = persisted.decimals;
    settings.angle_mode = persisted.angle_mode;
    settings.trim_trailing_zeros = persisted.trim_trailing_zeros;
    if let Some(separator) = persisted.decimal_separator.chars().next()
        && matches!(separator, '.' | ',')
    {
        settings.decimal_separator = separator;
    }
    settings
}
