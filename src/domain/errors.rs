//! The domain error type and its `Result` alias.
//!
//! The domain names only domain failures. Infrastructure errors
//! ([`StorageError`](crate::storage::errors::StorageError),
//! [`ConfigError`](crate::config::ConfigError)) are converted at the service
//! boundary into [`AppError::Storage`], so nothing below this layer has to know
//! that a file or a TOML parser exists.
//!
//! Every variant's `Display` is what the user reads in the status line and what
//! the history stores for a failed entry, so the wording is part of the
//! behaviour and pinned by tests.

use thiserror::Error;

/// Anything that can go wrong while calculating or persisting.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppError {
    /// An expression could not be parsed or evaluated by the numeric engine.
    #[error("cannot evaluate: {0}")]
    Calculator(String),

    /// The units engine rejected a query: an unknown unit, a conformance
    /// mismatch, a non-numeric result.
    ///
    /// Rendered verbatim. rink already phrases its failures as whole sentences
    /// ("No such unit foounit, did you mean bloodunit?"), and burying one under
    /// a second prefix would only make it harder to read.
    #[error("{0}")]
    Units(String),

    /// A variable name contained characters other than `[A-Za-z0-9_]`.
    #[error("invalid variable name: '{0}'")]
    InvalidVariableName(String),

    /// A variable name collided with `ans`, `pi` or `e`.
    #[error("'{0}' is reserved and cannot be a variable")]
    ReservedName(String),

    /// An expression referenced `ans` before anything had been computed.
    #[error("no previous answer")]
    NoPreviousAnswer,

    /// `=name` was used without a previous answer to store.
    #[error("no previous answer to save")]
    NoAnswerToSave,

    /// Persisting or restoring the session failed. The message carries the
    /// flattened cause chain of the underlying infrastructure error.
    #[error("storage error: {0}")]
    Storage(String),
}

/// The domain result type.
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    /// These strings reach the status line and the persisted history. Changing
    /// one is a user-visible change, not a refactor.
    #[test]
    fn every_variant_renders_the_message_the_user_reads() {
        let cases = [
            (
                AppError::Calculator("unexpected ')'".to_string()),
                "cannot evaluate: unexpected ')'",
            ),
            (
                AppError::Units("No such unit foounit".to_string()),
                "No such unit foounit",
            ),
            (
                AppError::InvalidVariableName("1bad".to_string()),
                "invalid variable name: '1bad'",
            ),
            (
                AppError::ReservedName("pi".to_string()),
                "'pi' is reserved and cannot be a variable",
            ),
            (AppError::NoPreviousAnswer, "no previous answer"),
            (AppError::NoAnswerToSave, "no previous answer to save"),
            (
                AppError::Storage("write failed".to_string()),
                "storage error: write failed",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn a_units_error_is_not_buried_under_a_second_prefix() {
        // rink's own wording is already a sentence; the calculator prefix is
        // for the numeric engine, whose messages are fragments.
        let units = AppError::Units("Conformance error: N != Pa".to_string());
        assert!(!units.to_string().starts_with("cannot evaluate"));
    }
}
