//! The crate-wide error type and `Result` alias.
//!
//! One error enum spans the layers (evaluation, variables, config, storage I/O)
//! so the binary edge can match on a single type. Variants carry enough context
//! (what, where) to render a clear message in the status line.

use std::path::Path;

/// Anything that can go wrong while calculating, configuring or persisting.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An expression could not be parsed or evaluated.
    #[error("cannot evaluate: {0}")]
    Calculator(String),

    /// A variable name contained characters other than `[A-Za-z0-9_]`.
    #[error("invalid variable name: '{0}'")]
    InvalidVariableName(String),

    /// `=name` was used without a previous answer to store.
    #[error("no previous answer to save")]
    NoPreviousAnswer,

    /// A config file could not be read or parsed.
    #[error("error in config {path}: {message}")]
    Config { path: String, message: String },

    /// A state file could not be read, parsed or written.
    #[error("error in state {path}: {message}")]
    State { path: String, message: String },

    /// A filesystem operation failed.
    #[error("{context}: {source}")]
    Io {
        context: String,
        source: std::io::Error,
    },
}

impl Error {
    /// Builds a [`Error::Config`] from a path and message.
    pub fn config(path: &Path, message: impl Into<String>) -> Self {
        Error::Config {
            path: path.display().to_string(),
            message: message.into(),
        }
    }

    /// Builds a [`Error::State`] from a path and message.
    pub fn state(path: &Path, message: impl Into<String>) -> Self {
        Error::State {
            path: path.display().to_string(),
            message: message.into(),
        }
    }

    /// Builds a [`Error::Io`] with a human-readable context.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io {
            context: context.into(),
            source,
        }
    }
}

/// The crate-wide result type.
pub type Result<T> = std::result::Result<T, Error>;
