//! The persistence layer: a [`StateRepository`] port and its TOML
//! implementation, decoupling the rest of the app from the on-disk format.

pub mod repository;
pub mod toml_state;

pub use repository::{
    PersistedEntry, PersistedSettings, PersistedState, PersistedValue,
    StateRepository,
};
pub use toml_state::TomlStateRepository;
