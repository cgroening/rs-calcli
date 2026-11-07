//! Composition root: parse the arguments, load config and persisted state,
//! wire the service and the TUI, run the event loop and save the session on
//! exit.

use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use log::LevelFilter;
use ratada::Tui;

use calcli::config::{Config, load_config};
use calcli::domain::evaluator::F64Evaluator;
use calcli::domain::format::FormatSettings;
use calcli::domain::history::{History, HistoryEntry};
use calcli::domain::quantity::Quantity;
use calcli::domain::variables::VariableStore;
use calcli::services::CalcService;
use calcli::storage::{PersistedState, StateRepository, TomlStateRepository};
use calcli::tui::{self, App};
use calcli::util::{logging, paths};

/// calcli takes no subcommands: it has exactly one job, so an argument-less
/// call starts the calculator rather than printing the help.
#[derive(Debug, Parser)]
#[command(
    name = calcli::APP_NAME,
    version,
    about = calcli::APP_ABOUT,
    long_about = "Starts the interactive calculator. Called without \
                  arguments, calcli opens its terminal interface; there are \
                  no subcommands."
)]
struct Cli {
    /// Fills the session with sample data and leaves the saved state alone.
    #[arg(long)]
    demo: bool,
}

fn main() -> ExitCode {
    // clap reports an unknown option or a bad value itself and exits with 2,
    // which is what the CLI conventions ask for; nothing is caught here.
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}: error: {error:#}", calcli::APP_NAME);
            ExitCode::FAILURE
        }
    }
}

/// Loads everything, runs the TUI and persists the session.
///
/// With `--demo`, the session is seeded with sample data and is *not* saved on
/// exit, so the real `state.toml` is left untouched.
fn run(cli: &Cli) -> anyhow::Result<()> {
    let demo = cli.demo;
    // Before `load_config`, which logs which file it read and what it ignored.
    // Installed later, those records would be dropped on the floor.
    if let Err(error) =
        logging::init(LevelFilter::Info, Some(&paths::log_file()))
    {
        eprintln!(
            "{}: warning: continuing without a log file: {error:#}",
            calcli::APP_NAME,
        );
    }
    let mut config = load_config().context("loading configuration")?;

    let repository = TomlStateRepository::new(paths::state_file());
    let mut state = if demo {
        calcli::demo::demo_state()
    } else {
        repository.load().unwrap_or_else(|error| {
            log::warn!("ignoring unreadable state: {error}");
            PersistedState::default()
        })
    };

    // `F5` in the TUI comes back here: the session is kept, the
    // configuration read again (the TUI checked it is clean) and everything
    // built anew from both.
    loop {
        let service = build_service(&config, &state);
        let mut app = App::new(service, &config, &state.ui);

        let mut tui = Tui::new().context("initializing the terminal")?;
        let outcome = tui::run(&mut app, &mut tui);
        drop(tui);
        let reload = matches!(&outcome, Err(error) if is_reload(error));
        if !reload {
            outcome?;
        }

        state = app.persisted_state();
        if !demo {
            repository.save(&state).context("saving the session")?;
        }
        if !reload {
            return Ok(());
        }
        config = load_config().context("reloading configuration")?;
        log::info!("configuration reloaded");
    }
}

/// Whether `error` is the TUI's reload signal (`F5`).
fn is_reload(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(ratada::reload::is_reload)
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
        Box::new(F64Evaluator::new()),
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
