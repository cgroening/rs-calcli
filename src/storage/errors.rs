//! Infrastructure errors raised by the persistence adapters.
//!
//! These name files, I/O and TOML. They reach no layer *above* the service:
//! the service boundary converts them into
//! [`AppError::Storage`](crate::domain::errors::AppError::Storage), flattening
//! the cause chain into the message so no detail is lost.
//!
//! The one place that sees them unwrapped is the composition root, which loads
//! the state before any service exists and logs an unreadable file rather than
//! failing the startup.

use std::io;

use thiserror::Error;

/// Anything that can go wrong while reading or writing a persisted file.
#[derive(Debug, Error)]
pub enum StorageError {
    /// A filesystem operation failed. `context` says what was being attempted.
    #[error("{context}")]
    Io {
        /// What the adapter was doing, e.g. `read state file`.
        context: String,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// The persisted file could not be parsed as TOML.
    #[error("cannot parse the state file")]
    Deserialization(#[from] toml::de::Error),

    /// The state could not be rendered as TOML.
    #[error("cannot serialize the state")]
    Serialization(#[from] toml::ser::Error),
}

/// The storage result type.
pub type StorageResult<T> = std::result::Result<T, StorageError>;

/// Attaches a human-readable context to an [`io::Error`].
pub trait IoResultExt<T> {
    /// Converts an I/O failure into a [`StorageError::Io`] carrying `context`.
    fn io_context(self, context: impl Into<String>) -> StorageResult<T>;
}

impl<T> IoResultExt<T> for io::Result<T> {
    fn io_context(self, context: impl Into<String>) -> StorageResult<T> {
        self.map_err(|source| StorageError::Io {
            context: context.into(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    #[test]
    fn io_context_wraps_the_error_and_keeps_it_as_the_source() {
        let failure: io::Result<()> =
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
        let error = failure.io_context("write state file").unwrap_err();
        assert_eq!(error.to_string(), "write state file");
        assert_eq!(error.source().unwrap().to_string(), "denied");
    }

    #[test]
    fn a_parse_failure_converts_from_a_toml_error() {
        let parsed = toml::from_str::<toml::Value>("= nonsense");
        let error: StorageError = parsed.unwrap_err().into();
        assert!(matches!(error, StorageError::Deserialization(_)));
        assert_eq!(error.to_string(), "cannot parse the state file");
    }
}
