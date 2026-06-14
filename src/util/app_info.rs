//! Static application metadata.

/// Application name; also the config/state directory name.
pub const APP_NAME: &str = "calcli";

/// Crate version, from Cargo at build time.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One-line description.
pub const APP_ABOUT: &str =
    "Fast terminal calculator with history, variables and engineering helpers";
