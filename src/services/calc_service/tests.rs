use super::*;

use crate::domain::evaluator::F64Evaluator;
use crate::domain::format::{AngleMode, Notation};

fn settings() -> FormatSettings {
    FormatSettings {
        notation: Notation::Decimal,
        decimals: 3,
        angle_mode: AngleMode::Rad,
        decimal_separator: '.',
        thousands_separator: " ".to_string(),
        trim_trailing_zeros: false,
    }
}

fn service() -> CalcService {
    CalcService::new(
        Box::new(F64Evaluator::new()),
        settings(),
        History::new(100),
        VariableStore::new(),
    )
}

fn value_at(service: &CalcService, index: usize) -> Option<f64> {
    service.history().entries()[index]
        .value
        .as_ref()
        .map(Quantity::display_value)
}

fn last(service: &CalcService) -> Option<f64> {
    service.history().last_value().map(|q| q.display_value())
}

#[test]
fn undo_takes_back_the_last_calculation_with_its_variable() {
    let mut service = service();
    service.submit("1");
    assert!(service.can_undo());
    service.submit("x = 5");
    assert!(service.variables().get("x").is_some());
    assert!(service.undo());
    assert_eq!(service.history().len(), 1);
    assert!(service.variables().get("x").is_none());
    // One level: a second undo has nothing left to take back.
    assert!(!service.undo());
}

#[test]
fn undo_restores_a_deleted_entry_and_reset_variables() {
    let mut service = service();
    service.submit("x = 2");
    service.submit("ans * 3");
    service.delete_entry(0);
    assert!(service.undo());
    assert_eq!(service.history().len(), 2);
    assert_eq!(last(&service), Some(6.0));

    service.reset_variables();
    assert!(service.variables().get("x").is_none());
    assert!(service.undo());
    assert!(service.variables().get("x").is_some());
}

fn var(service: &CalcService, name: &str) -> Option<f64> {
    service.variables().get(name).map(Quantity::display_value)
}

fn outval(outcome: &SubmitOutcome) -> Option<f64> {
    outcome.value.as_ref().map(Quantity::display_value)
}

/// A dimensionless value preview, for the preview assertions.
fn val(value: f64) -> Preview {
    Preview::Value(Quantity::dimensionless(value))
}

/// `index` and `delta` arrive from the UI, so the step must survive values
/// a plain `index as isize + delta` would panic or wrap on.
#[test]
fn stepping_an_index_clamps_instead_of_overflowing() {
    assert_eq!(step_index(0, -1, 3), 0);
    assert_eq!(step_index(2, 1, 3), 2);
    assert_eq!(step_index(1, 1, 3), 2);
    assert_eq!(step_index(1, -1, 3), 0);
    assert_eq!(step_index(0, isize::MAX, 3), 2);
    assert_eq!(step_index(2, isize::MIN, 3), 0);
    assert_eq!(step_index(usize::MAX, 1, 3), 2);
}

/// An empty history has no index to move to, and must not underflow on the
/// `len - 1` that clamping needs.
#[test]
fn moving_an_entry_in_an_empty_history_is_a_no_op() {
    let mut service = service();
    assert_eq!(service.move_entry(0, 1), 0);
    assert_eq!(service.history().len(), 0);
}

#[test]
fn submit_evaluates_and_records_history() {
    let mut service = service();
    let outcome = service.submit("2+3");
    assert_eq!(outval(&outcome), Some(5.0));
    assert_eq!(last(&service), Some(5.0));
}

#[test]
fn ans_continues_from_the_previous_line() {
    let mut service = service();
    service.submit("10");
    service.submit("+5");
    assert_eq!(value_at(&service, 1), Some(15.0));
    service.submit("ans*2");
    assert_eq!(value_at(&service, 2), Some(30.0));
}

#[test]
fn editing_a_line_recomputes_the_chain_below() {
    let mut service = service();
    service.submit("10");
    service.submit("ans+5");
    service.submit("ans*2");
    assert_eq!(value_at(&service, 2), Some(30.0));
    service.edit_entry(0, "20");
    assert_eq!(value_at(&service, 1), Some(25.0));
    assert_eq!(value_at(&service, 2), Some(50.0));
}

#[test]
fn deleting_a_line_recomputes_the_chain_below() {
    let mut service = service();
    service.submit("10");
    service.submit("ans+5");
    service.submit("ans+100");
    service.delete_entry(1);
    // The former third line now follows the first: 10 + 100.
    assert_eq!(value_at(&service, 1), Some(110.0));
}

#[test]
fn save_ans_and_assignment_define_variables() {
    let mut service = service();
    service.submit("7");
    service.submit("=x");
    assert_eq!(var(&service, "x"), Some(7.0));
    service.submit("y = x + 3");
    assert_eq!(var(&service, "y"), Some(10.0));
    service.submit("y*2");
    assert_eq!(last(&service), Some(20.0));
}

#[test]
fn reserved_and_invalid_names_are_rejected() {
    let mut service = service();
    service.submit("5");
    let outcome = service.submit("=pi");
    assert!(outcome.error.is_some());
    let outcome = service.submit("1abc = 3");
    assert!(outcome.error.is_some());
}

#[test]
fn an_errored_line_is_recorded_without_a_value() {
    let mut service = service();
    let outcome = service.submit("2+");
    assert!(outcome.error.is_some());
    assert_eq!(last(&service), None);
}

#[test]
fn toggling_angle_mode_recomputes_history() {
    let mut service = service();
    service.submit("sin(90)");
    // In radians sin(90) is not 1.
    assert!((value_at(&service, 0).unwrap() - 1.0).abs() > 0.1);
    service.toggle_angle_mode();
    assert!((value_at(&service, 0).unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn toggling_decimal_separator_reparses_history_input() {
    let mut service = service();
    // With '.' decimal, the comma is a thousands separator: 1,5 -> 15.
    service.submit("1,5");
    assert_eq!(value_at(&service, 0), Some(15.0));
    service.toggle_decimal_separator();
    // With ',' decimal, 1,5 -> 1.5.
    assert_eq!(value_at(&service, 0), Some(1.5));
}

#[test]
fn variable_used_before_assignment_recomputes_after_edit() {
    let mut service = service();
    service.submit("a = 2");
    service.submit("a * 10");
    assert_eq!(value_at(&service, 1), Some(20.0));
    service.edit_entry(0, "a = 5");
    assert_eq!(value_at(&service, 1), Some(50.0));
}

#[test]
fn preview_reports_value_incomplete_and_invalid() {
    let mut service = service();
    service.submit("10");
    assert_eq!(service.preview("2+3"), val(5.0));
    assert_eq!(service.preview("ans+5"), val(15.0));
    assert_eq!(service.preview("2+"), Preview::Incomplete);
    assert_eq!(service.preview("2+3)"), Preview::Invalid);
    assert_eq!(service.preview(""), Preview::Empty);
    assert_eq!(service.preview(":d4"), Preview::Empty);
}

#[test]
fn preview_handles_assignments_without_mutating_state() {
    let mut service = service();
    service.submit("7");
    assert_eq!(service.preview("x = ans + 3"), val(10.0));
    // Previewing must not define the variable or add to the history.
    assert!(service.variables().get("x").is_none());
    assert_eq!(service.history().len(), 1);
    // A reserved name is invalid, not a value.
    assert_eq!(service.preview("pi = 3"), Preview::Invalid);
}

#[test]
fn inline_comments_are_ignored_but_kept_in_history() {
    let mut service = service();
    let outcome = service.submit("2+3 # the sum");
    assert_eq!(outval(&outcome), Some(5.0));
    // The full input, including the comment, is stored.
    assert_eq!(service.history().entries()[0].input, "2+3 # the sum");
    // Comments work on assignments too.
    service.submit("x = 5 # a note");
    assert_eq!(var(&service, "x"), Some(5.0));
}

#[test]
fn a_comment_only_line_is_a_note_that_passes_ans_through() {
    let mut service = service();
    service.submit("5");
    let outcome = service.submit("# just a note");
    assert_eq!(outcome.value, None);
    assert_eq!(outcome.error, None);
    assert_eq!(service.history().entries()[1].input, "# just a note");
    // The note does not break the `ans` chain.
    service.submit("ans + 1");
    assert_eq!(value_at(&service, 2), Some(6.0));
}

#[test]
fn moving_an_entry_recomputes_the_ans_chain() {
    let mut service = service();
    service.submit("10");
    service.submit("ans + 5"); // 15
    service.submit("ans * 2"); // 30
    // Move the last line up one: ["10", "ans*2", "ans+5"].
    let new_index = service.move_entry(2, -1);
    assert_eq!(new_index, 1);
    assert_eq!(value_at(&service, 1), Some(20.0)); // ans*2 with ans=10
    assert_eq!(value_at(&service, 2), Some(25.0)); // ans+5 with ans=20
}

#[test]
fn inserting_a_blank_entry_shifts_and_recomputes() {
    let mut service = service();
    service.submit("10");
    service.submit("ans + 5"); // 15
    service.insert_entry(1);
    assert_eq!(service.history().len(), 3);
    assert_eq!(service.history().entries()[1].input, "");
    // The blank note passes ans through, so `ans + 5` is still 15.
    assert_eq!(value_at(&service, 2), Some(15.0));
}

#[test]
fn preview_ignores_comments() {
    let mut service = service();
    service.submit("2");
    assert_eq!(service.preview("# note"), Preview::Empty);
    assert_eq!(service.preview("ans+3 # sum"), val(5.0));
}

#[test]
fn converts_a_quantity_with_the_arrow() {
    let mut service = service();
    let outcome = service.submit("123 MPa -> bar");
    let quantity = outcome.value.unwrap();
    assert_eq!(quantity.unit_symbol(), Some("bar"));
    assert!((quantity.display_value() - 1230.0).abs() < 1e-6);
}

#[test]
fn stores_a_quantity_variable_and_converts_it() {
    let mut service = service();
    service.submit("x = 50 kN");
    assert_eq!(
        service.variables().get("x").unwrap().unit_symbol(),
        Some("kN")
    );
    let outcome = service.submit("x -> N");
    let quantity = outcome.value.unwrap();
    assert_eq!(quantity.unit_symbol(), Some("N"));
    assert!((quantity.display_value() - 50_000.0).abs() < 1e-9);
}

#[test]
fn ans_carries_its_unit_into_a_conversion() {
    let mut service = service();
    service.submit("2 bar");
    let outcome = service.submit("ans -> Pa");
    let quantity = outcome.value.unwrap();
    assert_eq!(quantity.unit_symbol(), Some("Pa"));
    assert!((quantity.display_value() - 200_000.0).abs() < 1e-6);
}

#[test]
fn compound_and_volume_conversions_work() {
    let mut service = service();
    let litre = service.submit("1 l -> dm^3");
    assert!((outval(&litre).unwrap() - 1.0).abs() < 1e-9);
    let speed = service.submit("100 km/h -> m/s");
    assert!((outval(&speed).unwrap() - 27.7777778).abs() < 1e-6);
    assert_eq!(speed.value.unwrap().unit_symbol(), Some("m/s"));
}

#[test]
fn adding_compatible_units_auto_picks_a_unit() {
    let mut service = service();
    let outcome = service.submit("1 m + 50 cm");
    assert!((outval(&outcome).unwrap() - 1.5).abs() < 1e-9);
    assert_eq!(outcome.value.unwrap().unit_symbol(), Some("m"));
}

#[test]
fn addition_with_units_picks_a_single_unit() {
    let mut service = service();
    let outcome = service.submit("20 kN + 300 N");
    let quantity = outcome.value.unwrap();
    // 20 kN + 300 N = 20.3 kN, shown with the short unit symbol.
    assert!((quantity.display_value() - 20.3).abs() < 1e-9);
    assert_eq!(quantity.unit_symbol(), Some("kN"));
}

#[test]
fn multiplying_quantities_yields_a_derived_unit() {
    let mut service = service();
    let outcome = service.submit("1 m * 2 m");
    let quantity = outcome.value.unwrap();
    assert!((quantity.display_value() - 2.0).abs() < 1e-9);
    assert_eq!(quantity.unit_symbol(), Some("m^2"));
}

#[test]
fn dividing_quantities_pins_the_conversion_target() {
    let mut service = service();
    let outcome = service.submit("2 kN / 4 m^2 -> kN/m^2");
    let quantity = outcome.value.unwrap();
    assert!((quantity.display_value() - 0.5).abs() < 1e-9);
    assert_eq!(quantity.unit_symbol(), Some("kN/m^2"));
}

#[test]
fn unit_arithmetic_with_a_unit_variable_routes_to_rink() {
    let mut service = service();
    service.submit("f = 20 kN");
    let outcome = service.submit("f + 300 N");
    let quantity = outcome.value.unwrap();
    assert!((quantity.display_value() - 20.3).abs() < 1e-9);
}

#[test]
fn an_incompatible_conversion_errors() {
    let mut service = service();
    let outcome = service.submit("5 N -> bar");
    assert!(outcome.error.is_some());
}

#[test]
fn pure_math_is_unaffected_by_the_units_router() {
    let mut service = service();
    // `e` is a constant (rink knows it too) but must stay numeric.
    assert!((outval(&service.submit("e^0")).unwrap() - 1.0).abs() < 1e-9);
    // `sin` is a function, not a unit, and respects the angle mode.
    service.toggle_angle_mode();
    let outcome = service.submit("sin(90)");
    assert!((outval(&outcome).unwrap() - 1.0).abs() < 1e-9);
}

/// The status line and the persisted history show these verbatim, so they
/// are behaviour, not implementation. `AppError` produces each of them.
#[test]
fn every_failure_mode_keeps_its_message() {
    let cases: &[(&[&str], &str)] = &[
        (&["ans"], "no previous answer"),
        (&["ans -> m"], "no previous answer"),
        (&["=x"], "no previous answer to save"),
        (&["1", "=1bad"], "invalid variable name: '1bad'"),
        (&["1", "=pi"], "'pi' is reserved and cannot be a variable"),
        (
            &["2+"],
            "cannot evaluate: Parse error: Missing argument at the end of \
             expression.",
        ),
        (
            &["5 N -> bar"],
            "Conformance error: 5 newton (force) != 100 kilopascal \
             (pressure)",
        ),
    ];
    for (lines, expected) in cases {
        let mut calc = service();
        let mut outcome = None;
        for line in *lines {
            outcome = Some(calc.submit(line));
        }
        let error = outcome.expect("a submitted line").error;
        assert_eq!(error.as_deref(), Some(*expected), "for {lines:?}");
    }
}

#[test]
fn a_rink_message_is_not_prefixed_like_a_numeric_one() {
    // rink names the offending unit itself; the evaluator a fragment.
    let rink = service().submit("1 foounit -> m").error.expect("an error");
    assert!(rink.starts_with("No such unit"), "{rink}");

    let numeric = service().submit("2+").error.expect("an error");
    assert!(numeric.starts_with("cannot evaluate: "), "{numeric}");
}
