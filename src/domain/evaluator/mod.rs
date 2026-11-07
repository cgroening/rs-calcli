//! The expression-evaluation port and calcli's own evaluator over `f64`.
//!
//! Keeping evaluation behind a trait (DIP) lets the service depend on the
//! abstraction, so a future unit-aware engine can replace [`F64Evaluator`]
//! without touching the calculation flow. The evaluator replaced the `meval`
//! crate (unmaintained, its `nom 1.2.4` was going to stop compiling); it
//! keeps meval's syntax, results and error texts, which the acceptance table
//! in the tests pins.

mod parse;

use std::f64::consts::{E, PI};

use parse::{Expr, Op};

use crate::domain::errors::{AppError, Result};
use crate::domain::format::AngleMode;

/// Evaluates a fully substituted expression (numbers, operators, the
/// built-in functions and constants) to a single value.
pub trait Evaluator {
    /// Evaluates `expr`, reading trigonometric arguments per `angle`.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Calculator`] when `expr` cannot be parsed or
    /// evaluated.
    fn eval(&self, expr: &str, angle: AngleMode) -> Result<f64>;
}

/// The built-in function names, in the order the completion offers them.
/// The single source of the names: highlighting and completion read them
/// from here.
pub(crate) const FUNCTION_NAMES: &[&str] = &[
    "sqrt", "exp", "ln", "abs", "sin", "cos", "tan", "asin", "acos", "atan",
    "sinh", "cosh", "tanh", "asinh", "acosh", "atanh", "floor", "ceil",
    "round", "signum", "atan2", "max", "min",
];

/// The built-in constant names.
pub(crate) const CONSTANT_NAMES: &[&str] = &["pi", "e"];

/// The default evaluator: `+ - * / % ^`, signs, parentheses, the built-in
/// functions and constants, over `f64`.
#[derive(Debug, Default, Clone, Copy)]
pub struct F64Evaluator;

impl F64Evaluator {
    /// Creates the evaluator.
    pub fn new() -> Self {
        F64Evaluator
    }
}

impl Evaluator for F64Evaluator {
    fn eval(&self, expr: &str, angle: AngleMode) -> Result<f64> {
        let tree = parse::parse(expr).map_err(AppError::Calculator)?;
        value(&tree, angle).map_err(AppError::Calculator)
    }
}

/// The value of `expr`, trigonometry read per `angle`.
fn value(expr: &Expr, angle: AngleMode) -> std::result::Result<f64, String> {
    match expr {
        Expr::Number(number) => Ok(*number),
        Expr::Name(name) => constant(name).ok_or_else(|| {
            format!("Evaluation error: unknown variable `{name}`.")
        }),
        Expr::Negate(inner) => Ok(-value(inner, angle)?),
        Expr::Binary(op, left, right) => {
            let (left, right) = (value(left, angle)?, value(right, angle)?);
            Ok(match op {
                Op::Add => left + right,
                Op::Sub => left - right,
                Op::Mul => left * right,
                Op::Div => left / right,
                Op::Rem => left % right,
                Op::Pow => left.powf(right),
            })
        }
        Expr::Call(name, args) => {
            let args = args
                .iter()
                .map(|arg| value(arg, angle))
                .collect::<std::result::Result<Vec<f64>, String>>()?;
            call(name, &args, angle)
        }
    }
}

/// The value of the constant `name`.
fn constant(name: &str) -> Option<f64> {
    match name {
        "pi" => Some(PI),
        "e" => Some(E),
        _ => None,
    }
}

/// How many arguments a function takes.
enum Arity {
    Exactly(usize),
    AtLeastOne,
}

/// Calls the built-in function `name` with `args`; the trigonometric ones
/// read and return degrees in [`AngleMode::Deg`] (`atan2` stays in radians,
/// as it always did).
fn call(
    name: &str,
    args: &[f64],
    angle: AngleMode,
) -> std::result::Result<f64, String> {
    let Some(arity) = arity(name) else {
        return Err(format!(
            "Evaluation error: function `{name}`: Unknown function"
        ));
    };
    let fits = match arity {
        Arity::Exactly(count) => args.len() == count,
        Arity::AtLeastOne => !args.is_empty(),
    };
    if !fits {
        let count = match arity {
            Arity::Exactly(count) => count,
            Arity::AtLeastOne => 1,
        };
        return Err(format!(
            "Evaluation error: function `{name}`: Expected {count} arguments"
        ));
    }
    let degrees = angle == AngleMode::Deg;
    let to_rad = |x: f64| if degrees { x.to_radians() } else { x };
    let from_rad = |x: f64| if degrees { x.to_degrees() } else { x };
    let x = args[0];
    Ok(match name {
        "sqrt" => x.sqrt(),
        "exp" => x.exp(),
        "ln" => x.ln(),
        "abs" => x.abs(),
        "sin" => to_rad(x).sin(),
        "cos" => to_rad(x).cos(),
        "tan" => to_rad(x).tan(),
        "asin" => from_rad(x.asin()),
        "acos" => from_rad(x.acos()),
        "atan" => from_rad(x.atan()),
        "sinh" => x.sinh(),
        "cosh" => x.cosh(),
        "tanh" => x.tanh(),
        "asinh" => x.asinh(),
        "acosh" => x.acosh(),
        "atanh" => x.atanh(),
        "floor" => x.floor(),
        "ceil" => x.ceil(),
        "round" => x.round(),
        "signum" => x.signum(),
        "atan2" => x.atan2(args[1]),
        "max" => args.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        "min" => args.iter().copied().fold(f64::INFINITY, f64::min),
        _ => unreachable!("arity() knows only the names matched here"),
    })
}

/// The arity of the built-in function `name`, `None` for an unknown name.
fn arity(name: &str) -> Option<Arity> {
    match name {
        "atan2" => Some(Arity::Exactly(2)),
        "max" | "min" => Some(Arity::AtLeastOne),
        _ if FUNCTION_NAMES.contains(&name) => Some(Arity::Exactly(1)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
