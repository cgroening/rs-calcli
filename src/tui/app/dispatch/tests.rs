use crossterm::event::{KeyCode, KeyModifiers};
use ratada::shortcut_hints;

use super::*;
use crate::domain::format::AngleMode;
use crate::tui::app::testing::{
    chord, key, press, render_to_string, submit, test_app, type_text, value_at,
};
use crate::tui::interaction::Headless;

/// Points `app` at a config file holding `content`.
fn with_config(app: &mut App, name: &str, content: &str) {
    let path = std::env::temp_dir()
        .join(format!("calcli-test-{name}-{}.toml", std::process::id()));
    std::fs::write(&path, content).expect("the temp config is written");
    app.config_path = Some(path);
}

#[test]
fn f5_on_a_clean_config_ends_the_tui_with_the_reload_signal() {
    let mut app = test_app();
    with_config(&mut app, "clean", "");
    let mut io = Headless::accepting();
    let Err(error) = app.dispatch(key(KeyCode::F(5)), &mut io) else {
        panic!("F5 on a clean config must end the TUI");
    };
    let io_error = error.downcast_ref::<std::io::Error>().expect("io");
    assert!(ratada::reload::is_reload(io_error));
}

#[test]
fn f5_on_a_faulty_config_only_reports_it() {
    let mut app = test_app();
    with_config(&mut app, "faulty", "decimal_separator = \";\"\n");
    let mut io = Headless::accepting();
    let flow = app.dispatch(key(KeyCode::F(5)), &mut io);
    assert!(matches!(flow, Ok(Flow::Continue)));
    assert_eq!(io.asked, 1, "the problems were shown");
}

#[test]
fn u_takes_back_the_last_calculation_from_the_history() {
    let mut app = test_app();
    submit(&mut app, "1");
    submit(&mut app, "2");
    press(&mut app, key(KeyCode::Up));
    press(&mut app, key(KeyCode::Char('u')));
    assert_eq!(app.service().history().len(), 1);
    assert_eq!(app.status.as_deref(), Some("undone"));
}

// --- Autocomplete ---

#[test]
fn typing_an_identifier_prefix_opens_the_suggestion_dropdown() {
    let mut app = test_app();
    type_text(&mut app, "si");
    assert!(app.autocomplete.is_open());
    // Hide the hints so the history area has room for the dropdown box;
    // otherwise the grouped hints squeeze it out (content wins).
    shortcut_hints::set_visible(false);
    let screen = render_to_string(&app, 40, 24);
    shortcut_hints::set_visible(true);
    assert!(screen.contains("sin"));
}

#[test]
fn a_caret_after_an_operator_offers_no_suggestions() {
    let mut app = test_app();
    type_text(&mut app, "2+");
    assert!(!app.autocomplete.is_open());
}

#[test]
fn navigating_and_confirming_replaces_the_typed_prefix() {
    let mut app = test_app();
    type_text(&mut app, "si");
    press(&mut app, key(KeyCode::Down));
    press(&mut app, key(KeyCode::Enter));
    assert_eq!(app.input, "sin");
    assert!(!app.autocomplete.is_open());
}

#[test]
fn a_suggestion_replaces_only_the_word_under_the_caret() {
    let mut app = test_app();
    type_text(&mut app, "2*co");
    press(&mut app, key(KeyCode::Down));
    press(&mut app, key(KeyCode::Enter));
    assert_eq!(app.input, "2*cos");
}

#[test]
fn accepting_mid_word_rewrites_the_whole_identifier() {
    let mut app = test_app();
    render_to_string(&app, 80, 24);
    type_text(&mut app, "sinh");
    // Move the caret into the middle of the word: `si|nh`.
    press(&mut app, key(KeyCode::Left));
    press(&mut app, key(KeyCode::Left));
    // The first match for the `si` prefix is `sin`; accepting it must
    // replace the whole `sinh`, not just its head (which left `sinnh`).
    press(&mut app, key(KeyCode::Down));
    press(&mut app, key(KeyCode::Enter));
    assert_eq!(app.input, "sin");
}

#[test]
fn a_bare_number_offers_no_suggestions() {
    let mut app = test_app();
    type_text(&mut app, "42");
    assert!(!app.autocomplete.is_open());
}

#[test]
fn the_dropdown_never_overwrites_the_header_on_a_short_terminal() {
    let mut app = test_app();
    type_text(&mut app, "si");
    assert!(app.autocomplete.is_open());
    // Too short for the box above the input: it is dropped rather than
    // spilling up over the tab bar.
    let screen = render_to_string(&app, 40, 14);
    assert!(screen.contains("Calc"), "the tab bar must survive intact");
}

#[test]
fn escape_closes_the_dropdown_before_clearing_the_input() {
    let mut app = test_app();
    type_text(&mut app, "si");
    press(&mut app, key(KeyCode::Esc));
    assert!(!app.autocomplete.is_open());
    assert_eq!(app.input, "si", "the first Esc only closes the dropdown");
    press(&mut app, key(KeyCode::Esc));
    assert_eq!(app.input, "", "a second Esc clears the input");
}

#[test]
fn enter_submits_when_no_suggestion_is_highlighted() {
    let mut app = test_app();
    type_text(&mut app, "1+2");
    press(&mut app, key(KeyCode::Enter));
    assert_eq!(value_at(&app, 0), Some(3.0));
    assert!(!app.autocomplete.is_open());
}

#[test]
fn a_defined_variable_is_offered_as_a_suggestion() {
    let mut app = test_app();
    submit(&mut app, "radius = 5");
    type_text(&mut app, "rad");
    assert!(app.autocomplete.is_open());
    press(&mut app, key(KeyCode::Down));
    press(&mut app, key(KeyCode::Enter));
    assert_eq!(app.input, "radius");
}

#[test]
fn switching_views_closes_the_dropdown() {
    let mut app = test_app();
    type_text(&mut app, "si");
    assert!(app.autocomplete.is_open());
    // The Variables view must own `Up`/`Down`, not a lingering dropdown.
    press(&mut app, chord(KeyCode::Char('2'), KeyModifiers::ALT));
    assert_eq!(app.active, ViewId::Variables);
    assert!(!app.autocomplete.is_open());
}

#[test]
fn page_up_still_enters_history_while_the_dropdown_is_open() {
    let mut app = test_app();
    submit(&mut app, "7");
    type_text(&mut app, "si");
    // A wide render puts the caret on the first line, so `Up` would qualify
    // too; `PageUp` is the guaranteed escape while the dropdown owns `Up`.
    render_to_string(&app, 80, 24);
    assert!(app.autocomplete.is_open());
    press(&mut app, key(KeyCode::PageUp));
    assert_eq!(app.mode, Mode::History);
}

// --- History search ---

#[test]
fn searching_recalls_the_chosen_expression_into_the_input() {
    let mut app = test_app();
    submit(&mut app, "1+1");
    submit(&mut app, "2*3");
    // Items are newest first, so index 1 is the older "1+1".
    let mut io = Headless::accepting().picking(1);
    app.dispatch(chord(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut io)
        .expect("dispatch");
    assert_eq!(app.input, "1+1");
    assert_eq!(app.mode, Mode::Input);
    assert_eq!(app.cursor.pos, 3);
}

#[test]
fn dismissing_the_search_leaves_the_input_untouched() {
    let mut app = test_app();
    submit(&mut app, "9-4");
    type_text(&mut app, "5+");
    let mut io = Headless::declining();
    app.dispatch(chord(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut io)
        .expect("dispatch");
    assert_eq!(app.input, "5+");
}

#[test]
fn searching_an_empty_history_reports_and_opens_nothing() {
    let mut app = test_app();
    let mut io = Headless::accepting().picking(0);
    app.dispatch(chord(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut io)
        .expect("dispatch");
    assert_eq!(io.asked, 0, "no picker opens without history");
    assert_eq!(app.status.as_deref(), Some("no history yet"));
}

#[test]
fn a_forced_quit_inside_the_search_quits_the_app() {
    let mut app = test_app();
    submit(&mut app, "1");
    let mut io = Headless::force_quitting();
    app.dispatch(chord(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut io)
        .expect("dispatch");
    assert!(app.quit);
}

// --- Command palette ---

#[test]
fn the_palette_runs_the_chosen_action() {
    let mut app = test_app();
    // The catalog lists ToggleAngle before the palette entry, so it keeps
    // its index once the palette itself is filtered out.
    let index = Action::all()
        .filter(|a| *a != Action::OpenPalette)
        .position(|a| a == Action::ToggleAngle)
        .expect("angle action is in the catalog");
    assert_eq!(app.service().settings().angle_mode, AngleMode::Rad);
    let mut io = Headless::accepting().picking(index);
    app.dispatch(chord(KeyCode::Char('p'), KeyModifiers::CONTROL), &mut io)
        .expect("dispatch");
    assert_eq!(app.service().settings().angle_mode, AngleMode::Deg);
}

#[test]
fn a_forced_quit_inside_the_palette_quits_the_app() {
    let mut app = test_app();
    let mut io = Headless::force_quitting();
    app.dispatch(chord(KeyCode::Char('p'), KeyModifiers::CONTROL), &mut io)
        .expect("dispatch");
    assert!(app.quit);
}

// --- Quitting ---

#[test]
fn ctrl_q_inside_a_confirm_dialog_quits_and_abandons_the_action() {
    let mut app = test_app();
    submit(&mut app, "10");
    render_to_string(&app, 80, 24);
    press(&mut app, key(KeyCode::Up));

    let mut forced = Headless::force_quitting();
    let flow = app.dispatch(key(KeyCode::Char('d')), &mut forced).unwrap();
    assert!(matches!(flow, Flow::Quit));
    assert!(app.quit);
    assert_eq!(
        app.service().history().len(),
        1,
        "a forced quit is not a yes",
    );
}

#[test]
fn ctrl_q_inside_the_help_overlay_quits() {
    let mut app = test_app();
    let mut forced = Headless::force_quitting();
    let flow = app.dispatch(key(KeyCode::F(12)), &mut forced).unwrap();
    assert!(matches!(flow, Flow::Quit));
    assert!(app.quit);
}

#[test]
fn closing_the_help_overlay_normally_keeps_the_app_running() {
    let mut app = test_app();
    let mut declining = Headless::declining();
    let flow = app.dispatch(key(KeyCode::F(12)), &mut declining).unwrap();
    assert!(matches!(flow, Flow::Continue));
    assert!(!app.quit);
    assert_eq!(declining.asked, 1);
}

#[test]
fn the_colon_q_command_quits() {
    let mut app = test_app();
    submit(&mut app, ":q");
    assert!(app.quit);
}

#[test]
fn q_quits_from_a_list_view_but_not_from_the_input() {
    let mut app = test_app();
    press(&mut app, key(KeyCode::Char('q')));
    assert!(!app.quit, "q is a character in the input");

    press(&mut app, key(KeyCode::Esc));
    press(&mut app, key(KeyCode::F(4)));
    press(&mut app, key(KeyCode::Char('q')));
    assert!(app.quit);
}
