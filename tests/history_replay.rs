//! End-to-end check of the `ans` chain and history recomputation through the
//! public service API.

use calcli::domain::evaluator::MevalEvaluator;
use calcli::domain::format::{AngleMode, FormatSettings, Notation};
use calcli::domain::history::History;
use calcli::domain::quantity::Quantity;
use calcli::domain::variables::VariableStore;
use calcli::services::CalcService;

fn service() -> CalcService {
    let settings = FormatSettings {
        notation: Notation::Decimal,
        decimals: 3,
        angle_mode: AngleMode::Rad,
        decimal_separator: '.',
        thousands_separator: " ".to_string(),
        trim_trailing_zeros: false,
    };
    CalcService::new(
        Box::new(MevalEvaluator::new()),
        settings,
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

#[test]
fn editing_a_line_propagates_through_ans_and_variables() {
    let mut service = service();
    service.submit("100");
    service.submit("=base");
    service.submit("base + 50");
    service.submit("ans * 2");

    assert_eq!(value_at(&service, 2), Some(150.0));
    assert_eq!(value_at(&service, 3), Some(300.0));

    // Editing the first line re-runs everything below, including the `=base`
    // save and the lines that read `base` and `ans`.
    service.edit_entry(0, "200");
    assert_eq!(
        service.variables().get("base").map(Quantity::display_value),
        Some(200.0)
    );
    assert_eq!(value_at(&service, 2), Some(250.0));
    assert_eq!(value_at(&service, 3), Some(500.0));
}

#[test]
fn rounded_display_keeps_full_precision_for_further_math() {
    let mut service = service();
    service.submit("1/3");
    // Displayed rounded to 3 dp, but the stored value is the full f64.
    let first = service.history().entries()[0].value.clone().unwrap();
    assert_eq!(service.format_display(&first), "0.333");
    service.submit("ans * 3");
    // If only the rounded 0.333 were used this would be 0.999.
    assert_eq!(value_at(&service, 1), Some(1.0));
}
