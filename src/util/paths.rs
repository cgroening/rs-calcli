//! XDG path resolution.
//!
//! Resolves the config and state directories under the application name, so
//! the rest of the crate never spells a path convention out for itself. Home
//! resolution prefers `$HOME` (Unix), falling back to the Windows variables
//! when it is unset.

use std::env;
use std::path::PathBuf;

use crate::APP_NAME;

/// Resolves the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    resolve_home(
        non_empty("HOME"),
        non_empty("USERPROFILE"),
        non_empty("HOMEDRIVE").zip(non_empty("HOMEPATH")),
    )
}

/// The value of `name`, treating an unset and an empty variable alike.
fn non_empty(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

/// Picks the home directory from the candidates, in order of preference.
///
/// Split from [`home_dir`] so the precedence can be tested without touching
/// the process-wide environment.
fn resolve_home(
    home: Option<String>,
    profile: Option<String>,
    drive_and_path: Option<(String, String)>,
) -> Option<PathBuf> {
    if let Some(home) = home {
        return Some(PathBuf::from(home));
    }
    if let Some(profile) = profile {
        return Some(PathBuf::from(profile));
    }
    drive_and_path.map(|(drive, path)| PathBuf::from(format!("{drive}{path}")))
}

/// Returns `$XDG_<var>_HOME/calcli`, else `<home>/<fallback>/calcli`.
fn xdg_dir(env_var: &str, fallback: &str) -> PathBuf {
    app_dir(non_empty(env_var), home_dir(), fallback)
}

/// Joins the application name onto `base`, or onto `<home>/<fallback>`.
///
/// An empty `$XDG_*_HOME` reaches this as `None`: the XDG specification calls
/// an empty or relative value invalid, so it is ignored rather than honoured,
/// which would otherwise scatter state into the working directory. Without a
/// home directory the last resort is `.`, so calcli still starts.
fn app_dir(
    base: Option<String>,
    home: Option<PathBuf>,
    fallback: &str,
) -> PathBuf {
    match base {
        Some(base) => PathBuf::from(base).join(APP_NAME),
        None => home
            .unwrap_or_else(|| PathBuf::from("."))
            .join(fallback)
            .join(APP_NAME),
    }
}

/// Config directory (`$XDG_CONFIG_HOME/calcli` or `~/.config/calcli`).
pub fn config_dir() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

/// State directory (`$XDG_STATE_HOME/calcli` or `~/.local/state/calcli`).
pub fn state_dir() -> PathBuf {
    xdg_dir("XDG_STATE_HOME", ".local/state")
}

/// Config file path (`<config_dir>/config.toml`).
pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// State file path (`<state_dir>/state.toml`).
pub fn state_file() -> PathBuf {
    state_dir().join("state.toml")
}

/// Log file path (`<state_dir>/calcli.log`).
pub fn log_file() -> PathBuf {
    state_dir().join("calcli.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some(value: &str) -> Option<String> {
        Some(value.to_string())
    }

    #[test]
    fn home_wins_over_the_windows_variables() {
        let home = resolve_home(
            some("/home/ada"),
            some("C:\\Users\\Ada"),
            Some(("C:".into(), "\\Users\\Ada".into())),
        );
        assert_eq!(home, Some(PathBuf::from("/home/ada")));
    }

    #[test]
    fn user_profile_stands_in_when_home_is_unset() {
        let home = resolve_home(None, some("C:\\Users\\Ada"), None);
        assert_eq!(home, Some(PathBuf::from("C:\\Users\\Ada")));
    }

    #[test]
    fn the_drive_and_path_pair_is_the_last_fallback() {
        let home = resolve_home(
            None,
            None,
            Some(("C:".into(), "\\Users\\Ada".into())),
        );
        assert_eq!(home, Some(PathBuf::from("C:\\Users\\Ada")));
    }

    #[test]
    fn no_variable_at_all_yields_no_home() {
        assert_eq!(resolve_home(None, None, None), None);
    }

    #[test]
    fn an_xdg_base_is_used_verbatim_under_the_app_name() {
        let dir = app_dir(some("/xdg/config"), None, ".config");
        assert_eq!(dir, PathBuf::from("/xdg/config").join(APP_NAME));
    }

    /// An empty `$XDG_*_HOME` arrives as `None` and must not win over the
    /// home-relative default, or state would land in the working directory.
    #[test]
    fn an_absent_xdg_base_falls_back_to_the_home_relative_default() {
        let dir = app_dir(None, Some(PathBuf::from("/home/ada")), ".config");
        assert_eq!(dir, PathBuf::from("/home/ada/.config").join(APP_NAME));
    }

    #[test]
    fn without_a_home_the_default_is_rooted_at_the_current_directory() {
        let dir = app_dir(None, None, ".local/state");
        assert_eq!(dir, PathBuf::from("./.local/state").join(APP_NAME));
    }

    /// `non_empty` is the one place that decides "unset" and "empty" are the
    /// same thing, so it is pinned against the real environment.
    ///
    /// This is the only test that touches `CALCLI_PATHS_TEST_VAR`; it covers
    /// every case in one function so no second test can race it.
    #[test]
    fn an_empty_variable_reads_as_unset() {
        const VAR: &str = "CALCLI_PATHS_TEST_VAR";

        // SAFETY: this test is the sole user of `CALCLI_PATHS_TEST_VAR` in the
        // crate, and it restores the variable before returning.
        unsafe {
            env::set_var(VAR, "");
        }
        assert_eq!(non_empty(VAR), None);

        // SAFETY: see above - same variable, same exclusive owner.
        unsafe {
            env::set_var(VAR, "/somewhere");
        }
        assert_eq!(non_empty(VAR), some("/somewhere"));

        // SAFETY: see above - same variable, same exclusive owner.
        unsafe {
            env::remove_var(VAR);
        }
        assert_eq!(non_empty(VAR), None);
    }

    #[test]
    fn the_file_paths_sit_inside_their_directories() {
        assert_eq!(config_file(), config_dir().join("config.toml"));
        assert_eq!(state_file(), state_dir().join("state.toml"));
        assert_eq!(log_file(), state_dir().join("calcli.log"));
    }
}
