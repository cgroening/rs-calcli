//! The calculator core: errors, evaluation, expression preprocessing, number
//! formatting, variables and history. This layer is pure (no I/O) and holds the
//! single source of truth for how calcli computes and renders values.

pub mod completion;
pub mod errors;
pub mod evaluator;
pub mod expression;
pub mod format;
pub mod highlight;
pub mod history;
pub mod quantity;
pub mod units;
pub mod variables;
