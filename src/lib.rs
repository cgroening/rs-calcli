//! calcli — a fast terminal calculator (TUI) with history, variables and
//! engineering helpers.
//!
//! The crate is organized into layers: [`domain`] holds the pure calculator
//! core, [`service`] orchestrates it, [`storage`] and [`config`] handle
//! persistence and settings, [`tui`] is the Ratatui front-end and [`util`]
//! gathers stateless helpers. The binary wires them together in `main.rs`.

pub mod config;
pub mod demo;
pub mod domain;
pub mod service;
pub mod storage;
pub mod tui;
pub mod util;
