//! XDG path resolution and `~` expansion.
//!
//! Resolves the config and state directories under the application name. Home
//! resolution prefers `$HOME` (Unix), falling back to the Windows variables
//! when it is unset.

use std::env;
use std::path::PathBuf;

use crate::APP_NAME;

/// Resolves the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    if let Ok(home) = env::var("HOME")
        && !home.is_empty()
    {
        return Some(PathBuf::from(home));
    }
    if let Ok(profile) = env::var("USERPROFILE")
        && !profile.is_empty()
    {
        return Some(PathBuf::from(profile));
    }
    match (env::var("HOMEDRIVE"), env::var("HOMEPATH")) {
        (Ok(drive), Ok(path)) if !drive.is_empty() && !path.is_empty() => {
            Some(PathBuf::from(format!("{drive}{path}")))
        }
        _ => None,
    }
}

/// Expands a leading `~` (or `~/`) in `path` against the home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

/// Returns `$XDG_<var>_HOME/calcli`, else `<home>/<fallback>/calcli`.
fn xdg_dir(env_var: &str, fallback: &str) -> PathBuf {
    if let Ok(base) = env::var(env_var)
        && !base.is_empty()
    {
        return PathBuf::from(base).join(APP_NAME);
    }
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(fallback).join(APP_NAME)
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
