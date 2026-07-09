//! The domain error type and its `Result` alias.
//!
//! The domain names only domain failures. Infrastructure errors
//! ([`StorageError`](crate::storage::errors::StorageError),
//! [`ConfigError`](crate::config::ConfigError)) are converted at the service
//! boundary into [`AppError::Storage`], so nothing below this layer has to know
//! that a file or a TOML parser exists.

use thiserror::Error;

/// Anything that can go wrong while calculating or persisting.
#[derive(Debug, Error)]
pub enum AppError {
    /// An expression could not be parsed or evaluated.
    #[error("cannot evaluate: {0}")]
    Calculator(String),

    /// A variable name contained characters other than `[A-Za-z0-9_]`.
    #[error("invalid variable name: '{0}'")]
    InvalidVariableName(String),

    /// `=name` was used without a previous answer to store.
    #[error("no previous answer to save")]
    NoPreviousAnswer,

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

    #[test]
    fn a_calculator_error_renders_its_cause() {
        let error = AppError::Calculator("unexpected ')'".to_string());
        assert_eq!(error.to_string(), "cannot evaluate: unexpected ')'");
    }

    #[test]
    fn a_missing_answer_renders_a_standalone_message() {
        assert_eq!(
            AppError::NoPreviousAnswer.to_string(),
            "no previous answer to save",
        );
    }
}
