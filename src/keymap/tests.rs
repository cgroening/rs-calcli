use crossterm::event::{KeyCode, KeyModifiers};

use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn chord(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn every_action_has_exactly_one_catalog_row() {
    for action in Action::all() {
        let rows = ACTIONS.iter().filter(|s| s.action == action).count();
        assert_eq!(rows, 1, "{action:?} must have exactly one row");
    }
}

#[test]
fn config_names_are_unique() {
    let mut names: Vec<&str> =
        ACTIONS.iter().map(|spec| spec.config_name).collect();
    names.sort_unstable();
    let total = names.len();
    names.dedup();
    assert_eq!(names.len(), total, "duplicate config names in ACTIONS");
}

#[test]
fn every_default_key_parses() {
    for action in Action::all() {
        for key in action.default_keys() {
            assert!(
                KeyChord::parse(key).is_some(),
                "{action:?} has an unparseable default key {key:?}",
            );
        }
    }
}

#[test]
fn the_defaults_bind_without_any_conflict() {
    assert!(Keymap::default().conflicts().is_empty());
}

#[test]
fn ctrl_q_is_never_bound_since_the_toolkit_owns_the_hard_quit() {
    let keymap = Keymap::default();
    let ctrl_q = chord(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert_eq!(keymap.action_for(&ctrl_q, Context::Input), None);
    for context in Context::all() {
        assert_eq!(keymap.action_for(&ctrl_q, context), None);
    }
}

#[test]
fn enter_means_something_different_in_every_context() {
    let keymap = Keymap::default();
    let enter = key(KeyCode::Enter);
    let expected = [
        (Context::Input, Action::Submit),
        (Context::Edit, Action::ApplyEdit),
        (Context::History, Action::EditEntry),
        (Context::Variables, Action::InsertVariable),
        (Context::Settings, Action::NextValue),
    ];
    for (context, action) in expected {
        assert_eq!(
            keymap.action_for(&enter, context),
            Some(action),
            "enter in {context:?}",
        );
    }
}

#[test]
fn esc_means_something_different_in_every_context() {
    let keymap = Keymap::default();
    let esc = key(KeyCode::Esc);
    let expected = [
        (Context::Input, Action::ClearInput),
        (Context::Edit, Action::CancelEdit),
        (Context::History, Action::Back),
        (Context::Variables, Action::Back),
        (Context::Settings, Action::Back),
    ];
    for (context, action) in expected {
        assert_eq!(
            keymap.action_for(&esc, context),
            Some(action),
            "esc in {context:?}",
        );
    }
}

#[test]
fn d_deletes_an_entry_in_history_and_a_variable_in_variables() {
    let keymap = Keymap::default();
    let delete = key(KeyCode::Char('d'));
    assert_eq!(
        keymap.action_for(&delete, Context::History),
        Some(Action::DeleteEntry),
    );
    assert_eq!(
        keymap.action_for(&delete, Context::Variables),
        Some(Action::DeleteVariable),
    );
    // Nothing claims it in the settings list.
    assert_eq!(keymap.action_for(&delete, Context::Settings), None);
}

#[test]
fn list_actions_work_in_every_list_context() {
    let keymap = Keymap::default();
    let lists = [Context::History, Context::Variables, Context::Settings];
    for context in lists {
        assert_eq!(
            keymap.action_for(&key(KeyCode::Up), context),
            Some(Action::Up),
        );
    }
    // ... but never while a text field owns the keyboard.
    assert_eq!(
        keymap.action_for(&key(KeyCode::Up), Context::Input),
        Some(Action::EnterHistory),
        "up leaves the input rather than moving a list cursor",
    );
    assert_eq!(keymap.action_for(&key(KeyCode::Up), Context::Edit), None);
}

#[test]
fn a_text_field_swallows_bare_characters_but_not_chords() {
    let keymap = Keymap::default();
    // `y` copies a selection in a list, but must reach the text editor.
    assert_eq!(
        keymap.action_for(&key(KeyCode::Char('y')), Context::Input),
        None,
    );
    assert_eq!(
        keymap.action_for(&key(KeyCode::Char('q')), Context::Edit),
        None,
        "q types a character while editing a line",
    );
    // The function keys and the tab chords stay reachable while typing.
    assert_eq!(
        keymap.action_for(&key(KeyCode::F(2)), Context::Input),
        Some(Action::CycleNotation),
    );
    assert_eq!(
        keymap.action_for(
            &chord(KeyCode::Char('1'), KeyModifiers::ALT),
            Context::Input,
        ),
        Some(Action::ViewCalc),
    );
}

#[test]
fn the_tab_chords_are_alt_digits_not_bare_digits() {
    let keymap = Keymap::default();
    // A bare digit must remain typeable.
    assert_eq!(
        keymap.action_for(&key(KeyCode::Char('1')), Context::Input),
        None
    );
    assert_eq!(keymap.keys_for(Action::ViewCalc), vec!["alt+1"]);
    assert_eq!(keymap.keys_for(Action::ViewSettings), vec!["alt+3"]);
}

#[test]
fn alt_gr_never_triggers_a_tab_switch() {
    // On a German layout AltGr reports as Control+Alt and must type a
    // character, so `alt+1` must not match it.
    let keymap = Keymap::default();
    let alt_gr = KeyModifiers::ALT | KeyModifiers::CONTROL;
    assert_eq!(
        keymap.action_for(&chord(KeyCode::Char('1'), alt_gr), Context::Input),
        None,
    );
}

#[test]
fn f4_is_a_second_key_for_the_variables_view() {
    let keymap = Keymap::default();
    assert_eq!(
        keymap.action_for(&key(KeyCode::F(4)), Context::Input),
        Some(Action::ViewVariables),
    );
    let keys = keymap.keys_for(Action::ViewVariables);
    assert_eq!(keys, vec!["alt+2", "f4"]);
}

#[test]
fn help_answers_to_f12_and_question_mark() {
    let keymap = Keymap::default();
    assert_eq!(
        keymap.action_for(&key(KeyCode::F(12)), Context::History),
        Some(Action::OpenHelp),
    );
    assert_eq!(
        keymap.action_for(&key(KeyCode::Char('?')), Context::History),
        Some(Action::OpenHelp),
    );
}

#[test]
fn shift_is_carried_by_the_characters_case() {
    let keymap = Keymap::default();
    assert_eq!(
        keymap.action_for(&key(KeyCode::Char('d')), Context::History),
        Some(Action::DeleteEntry),
    );
    assert_eq!(
        keymap.action_for(&key(KeyCode::Char('D')), Context::History),
        Some(Action::ClearHistory),
    );
}

#[test]
fn alt_arrows_move_an_entry_without_shadowing_plain_arrows() {
    let keymap = Keymap::default();
    assert_eq!(
        keymap.action_for(&key(KeyCode::Up), Context::History),
        Some(Action::Up),
    );
    assert_eq!(
        keymap.action_for(
            &chord(KeyCode::Up, KeyModifiers::ALT),
            Context::History,
        ),
        Some(Action::MoveUp),
    );
}

#[test]
fn an_override_replaces_the_default_keys_of_one_action() {
    let overrides =
        BTreeMap::from([("quit".to_string(), vec!["x".to_string()])]);
    let keymap = Keymap::from_overrides(&overrides);
    assert_eq!(
        keymap.action_for(&key(KeyCode::Char('x')), Context::History),
        Some(Action::Quit),
    );
    assert_eq!(
        keymap.action_for(&key(KeyCode::Char('q')), Context::History),
        None
    );
    // Other actions keep their defaults.
    assert_eq!(
        keymap.action_for(&key(KeyCode::F(2)), Context::Input),
        Some(Action::CycleNotation),
    );
}

#[test]
fn an_overlapping_override_is_reported_as_a_conflict() {
    // `notation` (Global) is bound before `edit` (History), so it wins and
    // `edit`'s binding is dropped: a global chord shadows every context.
    let overrides =
        BTreeMap::from([("edit".to_string(), vec!["f2".to_string()])]);
    let keymap = Keymap::from_overrides(&overrides);
    assert_eq!(keymap.conflicts().len(), 1);
    let conflict = &keymap.conflicts()[0];
    assert_eq!(conflict.action, Action::EditEntry);
    assert_eq!(conflict.claimed_by, Action::CycleNotation);
    assert_eq!(
        keymap.action_for(&key(KeyCode::F(2)), Context::History),
        Some(Action::CycleNotation),
    );
}

#[test]
fn a_key_bound_in_two_disjoint_scopes_is_not_a_conflict() {
    // `delete` lives in History, `delete_variable` in Variables: both may
    // claim `d` because the two contexts are never active together.
    assert!(Keymap::default().conflicts().is_empty());
    assert!(!Scope::History.overlaps(Scope::Variables));
    assert!(Scope::Global.overlaps(Scope::History));
    assert!(Scope::List.overlaps(Scope::Variables));
}

#[test]
fn back_steps_out_of_every_list_context() {
    let keymap = Keymap::default();
    let lists = [Context::History, Context::Variables, Context::Settings];
    for context in lists {
        assert_eq!(
            keymap.action_for(&key(KeyCode::Esc), context),
            Some(Action::Back),
        );
    }
}

#[test]
fn the_input_and_edit_scopes_never_overlap_the_list_scopes() {
    assert!(!Scope::Input.overlaps(Scope::List));
    assert!(!Scope::Input.overlaps(Scope::Edit));
    assert!(!Scope::Edit.overlaps(Scope::History));
    assert!(Scope::Global.overlaps(Scope::Input));
}

#[test]
fn an_unparseable_override_key_is_skipped_not_bound() {
    let overrides = BTreeMap::from([(
        "quit".to_string(),
        vec!["nonsense".to_string(), "x".to_string()],
    )]);
    let keymap = Keymap::from_overrides(&overrides);
    assert_eq!(keymap.keys_for(Action::Quit), vec!["x"]);
}

#[test]
fn hints_join_multiple_keys_and_skip_unbound_actions() {
    let overrides =
        BTreeMap::from([("quit".to_string(), Vec::<String>::new())]);
    let keymap = Keymap::from_overrides(&overrides);
    let hints = keymap.hints(&[Action::OpenHelp, Action::Quit]);
    assert_eq!(
        hints,
        vec![("F12/?".to_string(), "help".to_string())],
        "an unbound action leaves no hint behind",
    );
}
