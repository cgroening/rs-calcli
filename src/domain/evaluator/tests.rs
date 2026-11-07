//! The acceptance table: what an expression evaluates to, in radians and in
//! degrees, or the error text. It was recorded from, and checked against,
//! the `meval` crate this evaluator replaced (2026-11-07), so calcli computes
//! and reports exactly as before.

use super::*;

const EPSILON: f64 = 1e-12;

/// What an expression gives.
#[derive(Debug, Clone, Copy)]
enum Outcome {
    /// A value (compared within [`EPSILON`]; `NaN` and the infinities
    /// exactly).
    Value(f64),
    /// An error text, without the `cannot evaluate: ` prefix the service
    /// adds.
    Error(&'static str),
}

use Outcome::{Error, Value};

/// Expressions that give the same in both angle modes.
const BOTH_MODES: &[(&str, Outcome)] = &[
    ("1+2*3", Value(7.0)),
    ("(1+2)*3", Value(9.0)),
    ("2^3^2", Value(512.0)),
    ("-2^2", Value(-4.0)),
    ("2*-3", Value(-6.0)),
    ("2^-1", Value(0.5)),
    ("2^+2", Value(4.0)),
    ("--3", Value(3.0)),
    ("-(3)", Value(-3.0)),
    ("+3", Value(3.0)),
    ("1+-2", Value(-1.0)),
    ("2*+3", Value(6.0)),
    ("2--3", Value(5.0)),
    ("7%3", Value(1.0)),
    ("-7%3", Value(-1.0)),
    ("7.5%2", Value(1.5)),
    ("1e3", Value(1000.0)),
    ("1e+3", Value(1000.0)),
    ("1.5e-2", Value(0.015)),
    ("2.5E2", Value(250.0)),
    ("5.", Value(5.0)),
    ("(((1)))", Value(1.0)),
    ("2^0.5", Value(std::f64::consts::SQRT_2)),
    ("0/0", Value(f64::NAN)),
    ("1/0", Value(f64::INFINITY)),
    ("-1/0", Value(f64::NEG_INFINITY)),
    ("pi", Value(PI)),
    ("e", Value(E)),
    ("sqrt(16)", Value(4.0)),
    ("sqrt(-1)", Value(f64::NAN)),
    ("-sqrt(4)", Value(-2.0)),
    ("exp(1)", Value(E)),
    ("ln(e)", Value(1.0)),
    ("ln(0)", Value(f64::NEG_INFINITY)),
    ("abs(-3)", Value(3.0)),
    ("sinh(0)", Value(0.0)),
    ("cosh(0)", Value(1.0)),
    ("tanh(0)", Value(0.0)),
    ("asinh(0)", Value(0.0)),
    ("acosh(1)", Value(0.0)),
    ("atanh(0)", Value(0.0)),
    ("floor(2.7)", Value(2.0)),
    ("ceil(2.1)", Value(3.0)),
    ("round(2.5)", Value(3.0)),
    ("round(-2.5)", Value(-3.0)),
    ("signum(-3)", Value(-1.0)),
    ("signum(0)", Value(1.0)),
    ("atan2(1,1)", Value(PI / 4.0)),
    ("max(7)", Value(7.0)),
    ("max(1,5,3)", Value(5.0)),
    ("min(4,2)", Value(2.0)),
    ("1 + 2", Value(3.0)),
    (
        "",
        Error("Parse error: Missing argument at the end of expression."),
    ),
    (
        "2+",
        Error("Parse error: Missing argument at the end of expression."),
    ),
    ("(1", Error("Parse error: Missing 1 right parenthesis.")),
    ("1)", Error("Parse error: Unexpected token at byte 1.")),
    (".5", Error("Parse error: Unexpected token at byte 0.")),
    ("1e", Error("Parse error: Unexpected token at byte 0.")),
    ("2pi", Error("Parse error: Unexpected token at byte 1.")),
    ("2(3)", Error("Parse error: Unexpected token at byte 1.")),
    ("(2)(3)", Error("Parse error: Unexpected token at byte 3.")),
    ("sqrt 4", Error("Parse error: Unexpected token at byte 5.")),
    ("1 2", Error("Parse error: Unexpected token at byte 2.")),
    ("3!", Error("Parse error: Unexpected token at byte 1.")),
    ("1,2", Error("Parse error: Unexpected token at byte 1.")),
    ("min()", Error("Parse error: Unexpected token at byte 4.")),
    ("sin()", Error("Parse error: Unexpected token at byte 4.")),
    ("pi2", Error("Evaluation error: unknown variable `pi2`.")),
    ("x", Error("Evaluation error: unknown variable `x`.")),
    ("sin", Error("Evaluation error: unknown variable `sin`.")),
    (
        "foo(1)",
        Error("Evaluation error: function `foo`: Unknown function"),
    ),
    (
        "atan2(1)",
        Error("Evaluation error: function `atan2`: Expected 2 arguments"),
    ),
    (
        "sqrt(1,2)",
        Error("Evaluation error: function `sqrt`: Expected 1 arguments"),
    ),
];

/// Expressions in radians.
const RADIANS: &[(&str, Outcome)] = &[
    ("sin(pi/2)", Value(1.0)),
    ("cos(0)", Value(1.0)),
    ("tan(0)", Value(0.0)),
    ("asin(1)", Value(PI / 2.0)),
    ("acos(1)", Value(0.0)),
    ("atan(1)", Value(PI / 4.0)),
];

/// Expressions in degrees; `atan2` keeps radians.
const DEGREES: &[(&str, Outcome)] = &[
    ("sin(90)", Value(1.0)),
    ("cos(180)", Value(-1.0)),
    ("tan(45)", Value(1.0)),
    ("asin(1)", Value(90.0)),
    ("acos(0)", Value(90.0)),
    ("atan(1)", Value(45.0)),
    ("atan2(1,1)", Value(PI / 4.0)),
    ("sinh(1)", Value(1.175_201_193_643_801_4)),
];

/// Checks every row of `table` in `angle` against `evaluator`.
fn check(
    evaluator: &dyn Evaluator,
    table: &[(&str, Outcome)],
    angle: AngleMode,
) {
    for (expr, expected) in table {
        let got = evaluator.eval(expr, angle);
        match (expected, &got) {
            (Value(want), Ok(have)) if want.is_nan() => {
                assert!(have.is_nan(), "{expr:?}: {have} is no NaN");
            }
            (Value(want), Ok(have)) if want.is_infinite() => {
                assert_eq!(want, have, "{expr:?}");
            }
            (Value(want), Ok(have)) => {
                assert!(
                    (want - have).abs() < EPSILON,
                    "{expr:?}: {have} != {want}"
                );
            }
            (Error(want), Err(AppError::Calculator(have))) => {
                assert_eq!(want, have, "{expr:?}");
            }
            _ => panic!(
                "{expr:?} in {angle:?}: expected {expected:?}, got {got:?}"
            ),
        }
    }
}

fn check_all(evaluator: &dyn Evaluator) {
    check(evaluator, BOTH_MODES, AngleMode::Rad);
    check(evaluator, BOTH_MODES, AngleMode::Deg);
    check(evaluator, RADIANS, AngleMode::Rad);
    check(evaluator, DEGREES, AngleMode::Deg);
}

#[test]
fn the_evaluator_gives_what_meval_gave() {
    check_all(&F64Evaluator::new());
}

#[test]
fn every_function_name_is_callable() {
    for name in FUNCTION_NAMES {
        let args = match *name {
            "atan2" => "(1,1)",
            _ => "(1)",
        };
        let expr = format!("{name}{args}");
        let result = F64Evaluator::new().eval(&expr, AngleMode::Rad);
        assert!(result.is_ok(), "{expr}: {result:?}");
    }
    for name in CONSTANT_NAMES {
        assert!(F64Evaluator::new().eval(name, AngleMode::Rad).is_ok());
    }
}
