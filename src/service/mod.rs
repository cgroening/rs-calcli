//! The service layer: stateful orchestration of the domain core, free of I/O.

pub mod calc_service;

pub use calc_service::{CalcService, SubmitOutcome};
