//! The expression-evaluation port and its meval-backed implementation.
//!
//! Keeping evaluation behind a trait (DIP) lets the service depend on the
//! abstraction, so a future unit-aware engine can replace [`MevalEvaluator`]
//! without touching the calculation flow.

use std::f64::consts::PI;

// https://crates.io/crates/meval
use meval::{Context, Expr};

use crate::domain::error::{Error, Result};
use crate::domain::format::AngleMode;

/// Evaluates a fully substituted, meval-ready expression to a single value.
pub trait Evaluator {
    /// Evaluates `expr`, reading trigonometric arguments per `angle`.
    ///
    /// # Errors
    /// Returns [`Error::Calculator`] when `expr` cannot be parsed or evaluated.
    fn eval(&self, expr: &str, angle: AngleMode) -> Result<f64>;
}

/// The default evaluator, delegating to the `meval` crate over `f64`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MevalEvaluator;

impl MevalEvaluator {
    /// Creates the evaluator.
    pub fn new() -> Self {
        MevalEvaluator
    }
}

impl Evaluator for MevalEvaluator {
    fn eval(&self, expr: &str, angle: AngleMode) -> Result<f64> {
        let parsed: Expr = expr
            .parse()
            .map_err(|e| Error::Calculator(format!("{e}")))?;
        let result = match angle {
            AngleMode::Rad => parsed.eval(),
            AngleMode::Deg => parsed.eval_with_context(degree_context()),
        };
        result.map_err(|e| Error::Calculator(format!("{e}")))
    }
}

/// A meval context whose trigonometric functions read and return degrees,
/// overriding the radian builtins while keeping every other builtin intact.
fn degree_context() -> Context<'static> {
    let mut ctx = Context::new();
    ctx.func("sin", |x| (x * PI / 180.0).sin());
    ctx.func("cos", |x| (x * PI / 180.0).cos());
    ctx.func("tan", |x| (x * PI / 180.0).tan());
    ctx.func("asin", |x| x.asin() * 180.0 / PI);
    ctx.func("acos", |x| x.acos() * 180.0 / PI);
    ctx.func("atan", |x| x.atan() * 180.0 / PI);
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-9;

    #[test]
    fn evaluates_basic_arithmetic_and_power() {
        let evaluator = MevalEvaluator::new();
        let value = evaluator.eval("2^8+1", AngleMode::Rad).unwrap();
        assert!((value - 257.0).abs() < EPSILON);
    }

    #[test]
    fn radian_mode_matches_the_mathematical_definition() {
        let evaluator = MevalEvaluator::new();
        let value = evaluator.eval("sin(pi/2)", AngleMode::Rad).unwrap();
        assert!((value - 1.0).abs() < EPSILON);
    }

    #[test]
    fn degree_mode_reads_arguments_as_degrees() {
        let evaluator = MevalEvaluator::new();
        let value = evaluator.eval("sin(90)", AngleMode::Deg).unwrap();
        assert!((value - 1.0).abs() < EPSILON);
        let inverse = evaluator.eval("asin(1)", AngleMode::Deg).unwrap();
        assert!((inverse - 90.0).abs() < EPSILON);
    }

    #[test]
    fn non_trigonometric_builtins_survive_degree_mode() {
        let evaluator = MevalEvaluator::new();
        let value = evaluator.eval("sqrt(16)", AngleMode::Deg).unwrap();
        assert!((value - 4.0).abs() < EPSILON);
    }

    #[test]
    fn an_invalid_expression_is_a_calculator_error() {
        let evaluator = MevalEvaluator::new();
        let error = evaluator.eval("2+", AngleMode::Rad).unwrap_err();
        assert!(matches!(error, Error::Calculator(_)));
    }
}
