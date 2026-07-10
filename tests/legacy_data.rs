//! Guards the on-disk formats calcli has ever written.
//!
//! A `config.toml` or `state.toml` from an older release must keep loading. The
//! fixtures in `tests/data/` are verbatim copies of what those releases wrote,
//! so a change to the shape breaks here rather than in a user's session.

use std::fs;
use std::path::PathBuf;

use calcli::config::Config;
use calcli::config::loader::config_from_str;
use calcli::domain::evaluator::MevalEvaluator;
use calcli::domain::history::{History, HistoryEntry};
use calcli::domain::quantity::Quantity;
use calcli::domain::variables::VariableStore;
use calcli::services::CalcService;
use calcli::storage::{PersistedState, StateRepository, TomlStateRepository};
use calcli::theme::{Color, GlyphVariant};

/// A unique scratch file, removed by the caller.
fn scratch(tag: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("calcli-legacy-{tag}-{}.toml", std::process::id()));
    path
}

/// Loads `fixture` through the real repository, as the binary would.
fn load_fixture(name: &str, tag: &str) -> PersistedState {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    let path = scratch(tag);
    fs::copy(&source, &path).expect("fixture is readable");
    let state = TomlStateRepository::new(path.clone())
        .load()
        .unwrap_or_else(|error| panic!("{name} must load: {error}"));
    let _ = fs::remove_file(&path);
    state
}

/// The verbatim 0.2 `config.toml`.
const LEGACY_CONFIG: &str = include_str!("data/config-0.2.toml");

/// `history_zebra` was removed, and `deny_unknown_fields` turns that into a
/// startup error rather than a shrug. That is the one shape a `config.toml` is
/// allowed to lose: the user reads the message and deletes a line. A
/// `state.toml` may never break this way, because `main` treats an unreadable
/// one as an empty session and the whole history goes with it.
///
/// `main` prints the cause chain (`{error:#}`), so the TOML error naming the
/// key is what reaches the terminal.
#[test]
fn a_0_2_config_file_is_rejected_by_name_for_its_zebra_key() {
    let error = config_from_str(LEGACY_CONFIG)
        .expect_err("history_zebra no longer exists");
    let cause = std::error::Error::source(&error)
        .expect("a parse failure carries the TOML error")
        .to_string();
    assert!(
        cause.contains("history_zebra"),
        "the message must name the offending key: {cause}",
    );
}

#[test]
fn a_0_2_config_file_still_loads_with_its_colours_mapped() {
    let text = LEGACY_CONFIG.replace("history_zebra = false\n", "");
    let config = config_from_str(&text).expect("the 0.2 config must load");

    assert_eq!(config.decimals, 3);
    assert_eq!(config.max_history, 500);
    assert_eq!(config.appearance.glyphs, GlyphVariant::Unicode);
    assert!(config.live_feedback);

    // The flat `[theme]` colours land on the palette and on `[highlight]`.
    // `history_alt_bg` is not among them: it tinted the zebra stripe, and the
    // key is now accepted without effect rather than aimed at an unused colour.
    let palette = config.palette();
    assert_eq!(palette.accent, Color::hex("#6dd0ff"));
    assert_eq!(palette.footer, Color::hex("#252525"));
    assert_eq!(palette.border, Color::hex("#3e3e3e"));
    assert_eq!(palette.panel, Config::default().palette().panel);
    assert_eq!(config.highlight.function, Color::hex("#78c2b3"));
    assert_eq!(config.highlight.unit, Color::hex("#ff79c6"));
}

#[test]
fn a_0_2_state_file_still_loads_with_its_history_and_settings() {
    let state = load_fixture("state-0.2.toml", "state02");

    let settings = state.settings.expect("the settings survive");
    assert_eq!(settings.decimal_separator, ",");
    assert!(settings.trim_trailing_zeros);

    assert!(state.history.len() >= 3, "the history survives");
    assert_eq!(state.history[0].input, "1+3");
    assert_eq!(state.history[0].value, Some(4.0));
    // A line that errored has no value, and must not be dropped.
    assert!(state.history.iter().any(|entry| entry.value.is_none()));

    // The `[ui]` section did not exist yet, so it defaults.
    assert_eq!(state.ui.active_view, 0);
    assert_eq!(state.ui.hints_visible, None);
}

#[test]
fn a_pre_units_state_file_still_loads_its_bare_variables() {
    // Before units, a dimensionless variable was written as a bare number
    // (`g = 9.81`). Rejecting that made `load` fail wholesale, which the binary
    // swallows by starting from an empty session: the user lost everything.
    let state = load_fixture("state-pre-units.toml", "preunits");

    assert_eq!(state.variables.len(), 2);
    assert_eq!(state.variables["g"].value, 9.81);
    assert_eq!(state.variables["g"].unit, None);
    assert_eq!(state.variables["r"].value, 2.0);
    assert_eq!(state.history.len(), 3);
}

#[test]
fn a_pre_units_session_recomputes_into_the_same_values() {
    let state = load_fixture("state-pre-units.toml", "replay");

    // Rebuild the service exactly as `main::build_service` does.
    let variables = VariableStore::from_pairs(state.variables.iter().map(
        |(name, stored)| {
            let quantity =
                Quantity::from_persisted(stored.value, stored.unit.as_deref());
            (name.clone(), quantity)
        },
    ));
    let entries: Vec<HistoryEntry> = state
        .history
        .iter()
        .map(|entry| HistoryEntry {
            input: entry.input.clone(),
            value: entry
                .value
                .map(|v| Quantity::from_persisted(v, entry.unit.as_deref())),
            error: None,
        })
        .collect();
    let mut service = CalcService::new(
        Box::new(MevalEvaluator::new()),
        Config::default().format_settings(),
        History::from_entries(entries, 500),
        variables,
    );
    service.recompute_all();

    let value_at = |index: usize| {
        service.history().entries()[index]
            .value
            .as_ref()
            .map(Quantity::display_value)
    };
    // `2 * pi * r` with r = 2, then `ans^2`, then `g * 3`.
    assert_eq!(value_at(0), Some(std::f64::consts::TAU * 2.0));
    assert_eq!(value_at(2), Some(9.81 * 3.0));
}

#[test]
fn a_legacy_state_file_is_rewritten_in_the_current_shape() {
    let state = load_fixture("state-pre-units.toml", "rewrite");
    let path = scratch("rewrite-out");
    let repository = TomlStateRepository::new(path.clone());
    repository.save(&state).expect("save");

    let written = fs::read_to_string(&path).expect("read back");
    let reloaded = repository.load().expect("reload");
    let _ = fs::remove_file(&path);

    // Bare numbers are read but always written back as tables.
    assert!(written.contains("[variables.g]"), "{written}");
    assert_eq!(reloaded.variables["g"].value, 9.81);
    assert_eq!(reloaded.history.len(), 3);
}
