//! calcli - a fast terminal calculator (TUI) with history, variables and
//! engineering helpers.
//!
//! The crate is organized into layers: [`domain`] holds the pure calculator
//! core, [`services`] orchestrates it, [`storage`] and [`config`] handle
//! persistence and settings, [`tui`] is the Ratatui front-end and [`util`]
//! gathers stateless helpers. [`keymap`] is the single source for key bindings,
//! shared by the TUI dispatch and the shortcut hints. The binary wires them
//! together in `main.rs`.

pub mod config;
pub mod demo;
pub mod domain;
pub mod keymap;
pub mod services;
pub mod storage;
pub mod tui;
pub mod util;

/// The application name, used for paths, the brand and the env prefix.
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");

/// The application version, shown in the help overlay.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A one-line description of the application.
pub const APP_ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

/// Colours, glyphs, palettes and skins come from the shared TUI toolkit, so
/// `crate::theme` stays the one path the rest of the crate reaches for.
pub use ratada::theme;
