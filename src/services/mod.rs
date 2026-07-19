//! The service layer: stateful orchestration of the domain core, free of I/O.
//!
//! This layer is also the error funnel. Infrastructure failures
//! ([`StorageError`], [`ConfigError`]) are converted here into
//! [`AppError::Storage`], flattening their cause chain into the message so the
//! detail survives while the domain stays free of infrastructure types.

pub mod calc_service;
mod eval;

pub use calc_service::{CalcService, Preview, SubmitOutcome};

use crate::config::ConfigError;
use crate::domain::errors::AppError;
use crate::storage::StorageError;

impl From<StorageError> for AppError {
    fn from(error: StorageError) -> Self {
        AppError::Storage(flatten(&error))
    }
}

impl From<ConfigError> for AppError {
    fn from(error: ConfigError) -> Self {
        AppError::Storage(flatten(&error))
    }
}

/// Renders `error` together with its whole cause chain, so nothing an
/// infrastructure error knew is lost when it crosses into the domain.
fn flatten(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(source) = cause {
        message.push_str(": ");
        message.push_str(&source.to_string());
        cause = source.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use crate::storage::IoResultExt;

    #[test]
    fn a_storage_error_funnels_into_a_domain_error_with_its_cause_chain() {
        let failure: io::Result<()> =
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
        let storage = failure.io_context("write state.toml").unwrap_err();
        let error: AppError = storage.into();
        assert!(matches!(error, AppError::Storage(_)));
        assert_eq!(
            error.to_string(),
            "storage error: write state.toml: denied",
        );
    }

    #[test]
    fn a_config_error_funnels_into_a_domain_error() {
        let error: AppError = ConfigError::Read {
            path: "/tmp/config.toml".to_string(),
            source: io::Error::new(io::ErrorKind::NotFound, "missing"),
        }
        .into();
        assert_eq!(
            error.to_string(),
            "storage error: cannot read config file /tmp/config.toml: missing",
        );
    }
}
