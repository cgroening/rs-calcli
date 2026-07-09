//! The persistence layer: a [`StateRepository`] port and its TOML
//! implementation, decoupling the rest of the app from the on-disk format.
//!
//! Failures surface as [`StorageError`], which the service boundary funnels
//! into a domain error so no layer above here names files or TOML.

pub mod errors;
pub mod repository;
pub mod toml_state;

pub use errors::{IoResultExt, StorageError, StorageResult};
pub use repository::{
    PersistedEntry, PersistedSettings, PersistedState, PersistedValue,
    StateRepository, UiState,
};
pub use toml_state::TomlStateRepository;
