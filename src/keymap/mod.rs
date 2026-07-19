//! Configurable key bindings for the app's own shortcuts.
//!
//! Maps key chords to [`Action`]s, built from compiled-in defaults plus
//! per-action overrides from `[keys]`. Kept free of UI widgets so the TUI uses
//! it for both dispatch and the footer/help hints. Widget-internal shortcuts
//! (modals, the help overlay, the text editor) are not covered here.
//!
//! # Scopes
//!
//! calcli is modal: `Enter` submits an expression in the input, edits a line in
//! the history and inserts a name in the variables list. A flat chord-to-action
//! map cannot express that, so every action carries a [`Scope`] naming where it
//! applies. A key is looked up within the scopes active in the current
//! [`Context`], and two bindings only conflict when their scopes can be active
//! at the same time.
//!
//! `Ctrl+Q` is deliberately absent: the hard quit belongs to `ratada` and is
//! reported by [`ratada::shortcut_hints::global_bindings`], never bound here.
//!
//! The chord grammar, the defaults-vs-overrides merge and the display form live
//! in [`ratada::keymap`]; this module owns the action catalog, the scopes, and
//! the context-aware lookup they need.

mod catalog;

use std::collections::BTreeMap;

pub use ratada::input::is_bare_character;
pub use ratada::keymap::{Conflict as ChordConflict, KeyChord};

/// A configured key dropped because an earlier action in an overlapping scope
/// already claimed it.
pub type Conflict = ChordConflict<Action>;

use crossterm::event::KeyEvent;

use crate::keymap::catalog::{ACTIONS, ActionSpec};

/// Where a key is being pressed, selecting the active [`Scope`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// Typing a new expression in the input field.
    Input,
    /// Editing a history line in place.
    Edit,
    /// Browsing the history list in the Calc view.
    History,
    /// The variables list.
    Variables,
    /// The settings list.
    Settings,
}

impl Context {
    /// Every context, used to decide whether two scopes overlap.
    pub fn all() -> impl Iterator<Item = Context> {
        [
            Context::Input,
            Context::Edit,
            Context::History,
            Context::Variables,
            Context::Settings,
        ]
        .into_iter()
    }

    /// Whether a text field owns the keyboard here, so a bare character must
    /// reach it rather than trigger an action.
    pub fn is_text_editing(self) -> bool {
        matches!(self, Context::Input | Context::Edit)
    }
}

/// Which contexts an action belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Available in every context, and (for non-character chords) while typing.
    Global,
    /// Only while typing a new expression.
    Input,
    /// Only while editing a history line in place.
    Edit,
    /// Shared list navigation, available in every list context.
    List,
    /// Only while browsing the history.
    History,
    /// Only in the variables list.
    Variables,
    /// Only in the settings list.
    Settings,
}

impl Scope {
    /// Whether this scope is active in `context`.
    pub fn is_active_in(self, context: Context) -> bool {
        match self {
            Scope::Global => true,
            Scope::Input => context == Context::Input,
            Scope::Edit => context == Context::Edit,
            Scope::List => !context.is_text_editing(),
            Scope::History => context == Context::History,
            Scope::Variables => context == Context::Variables,
            Scope::Settings => context == Context::Settings,
        }
    }

    /// Whether two scopes can be active at the same time, so that binding the
    /// same chord in both would shadow one of them.
    fn overlaps(self, other: Scope) -> bool {
        Context::all().any(|context| {
            self.is_active_in(context) && other.is_active_in(context)
        })
    }
}

/// An app-level action a key can trigger. The catalog below is the single
/// source of truth for each action's config name, description, scope and keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Switch to the Calc view.
    ViewCalc,
    /// Switch to the Variables view.
    ViewVariables,
    /// Switch to the Settings view.
    ViewSettings,
    /// Cycle to the next notation.
    CycleNotation,
    /// Toggle between degrees and radians.
    ToggleAngle,
    /// Toggle the decimal mark between `.` and `,`.
    ToggleDecimalSeparator,
    /// Toggle trimming of trailing fractional zeros.
    ToggleTrim,
    /// Copy the most recent result, at full precision.
    CopyLast,
    /// Search the history with a fuzzy finder and recall the chosen line.
    SearchHistory,
    /// Open the command palette to run any action by name.
    OpenPalette,
    /// Open the help overlay.
    OpenHelp,
    /// Quit the application.
    Quit,
    /// Evaluate the typed expression.
    Submit,
    /// Leave the input field and browse the history.
    EnterHistory,
    /// Clear the input buffer.
    ClearInput,
    /// Apply the in-place edit of a history line.
    ApplyEdit,
    /// Abandon the in-place edit of a history line.
    CancelEdit,
    /// Move the selection up one row.
    Up,
    /// Move the selection down one row.
    Down,
    /// Move the selection up one page.
    PageUp,
    /// Move the selection down one page.
    PageDown,
    /// Jump to the first row.
    Top,
    /// Jump to the last row.
    Bottom,
    /// Copy the selected value at full precision.
    CopyPlain,
    /// Copy the selected value exactly as displayed.
    CopyDisplay,
    /// Move the selected history entry up one row.
    MoveUp,
    /// Move the selected history entry down one row.
    MoveDown,
    /// Edit the selected history entry in place.
    EditEntry,
    /// Insert a blank entry below the selection.
    InsertBelow,
    /// Insert a blank entry above the selection.
    InsertAbove,
    /// Delete the selected history entry.
    DeleteEntry,
    /// Clear the entire history.
    ClearHistory,
    /// Step back: out of the history to the input, or out of a list view to
    /// the Calc view.
    Back,
    /// Insert the selected variable's name into the input.
    InsertVariable,
    /// Delete the selected variable.
    DeleteVariable,
    /// Remove every variable.
    ResetVariables,
    /// Move the focused setting to its previous value.
    PreviousValue,
    /// Move the focused setting to its next value.
    NextValue,
}

impl Action {
    /// Every action, in catalog order.
    pub fn all() -> impl Iterator<Item = Action> + Clone {
        ACTIONS.iter().map(|spec| spec.action)
    }

    /// The catalog row for this action.
    fn spec(self) -> &'static ActionSpec {
        // Every variant has exactly one `ACTIONS` row (guarded by the
        // `every_action_has_exactly_one_catalog_row` test), so this never
        // fails.
        ACTIONS
            .iter()
            .find(|spec| spec.action == self)
            .expect("every action has an ACTIONS entry")
    }

    /// The `[keys]` config key for this action.
    pub fn config_name(self) -> &'static str {
        self.spec().config_name
    }

    /// A short human description for the footer and help hints.
    pub fn description(self) -> &'static str {
        self.spec().description
    }

    /// The default key strings for this action.
    pub fn default_keys(self) -> &'static [&'static str] {
        self.spec().default_keys
    }

    /// Where this action applies.
    pub fn scope(self) -> Scope {
        self.spec().scope
    }
}

/// Hands the catalog to the toolkit, which owns the chords.
///
/// `overlaps` carries calcli's scope rule into the conflict check, so two
/// actions may share a chord as long as their scopes are never active at the
/// same time. The rest delegate to the inherent methods above, so the catalog
/// stays the single source and call sites need no `use ratada::keymap::Action`.
impl ratada::keymap::Action for Action {
    fn all() -> impl Iterator<Item = Self> + Clone {
        Action::all()
    }

    fn config_name(&self) -> &'static str {
        (*self).config_name()
    }

    fn description(&self) -> &'static str {
        (*self).description()
    }

    fn default_keys(&self) -> &'static [&'static str] {
        (*self).default_keys()
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.scope().overlaps(other.scope())
    }
}

/// The resolved key map: every binding, looked up per [`Context`].
///
/// A newtype over [`ratada::keymap::Keymap`] rather than a bare alias, because
/// calcli's lookup is context-aware: the toolkit holds the bindings and the
/// chord logic, while the scope rule stays here, where [`Scope`] lives.
#[derive(Debug, Clone, Default)]
pub struct Keymap {
    inner: ratada::keymap::Keymap<Action>,
}

impl Keymap {
    /// Builds the map from the defaults, replacing an action's keys if
    /// `overrides` names it. Unknown action names and unparseable keys are
    /// logged and skipped; a key already bound to an earlier action in an
    /// overlapping scope keeps that binding.
    pub fn from_overrides(overrides: &BTreeMap<String, Vec<String>>) -> Self {
        ratada::keymap::warn_unknown::<Action>(overrides);
        Self {
            inner: ratada::keymap::Keymap::from_overrides(overrides),
        }
    }

    /// The bindings dropped because an earlier action owned the key.
    pub fn conflicts(&self) -> &[Conflict] {
        self.inner.conflicts()
    }

    /// The action bound to `key` among the scopes active in `context`.
    ///
    /// A bare printable character never triggers an action while a text field
    /// has the keyboard: it has to reach the editor instead. That is what keeps
    /// `q`, `?`, `y` and `d` typeable inside an expression.
    pub fn action_for(
        &self,
        key: &KeyEvent,
        context: Context,
    ) -> Option<Action> {
        if context.is_text_editing() && is_bare_character(*key) {
            return None;
        }
        self.inner.action_for_where(key, |action| {
            action.scope().is_active_in(context)
        })
    }

    /// The display strings of the keys bound to `action`.
    pub fn keys_for(&self, action: Action) -> Vec<String> {
        self.inner.keys_for(action)
    }

    /// Builds `(keys, description)` hint pairs for `actions`, skipping any with
    /// no bound key. The single source for the footer and the help overlay.
    pub fn hints(&self, actions: &[Action]) -> Vec<(String, String)> {
        self.inner.hints(actions)
    }
}

#[cfg(test)]
mod tests {
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
            keymap
                .action_for(&chord(KeyCode::Char('1'), alt_gr), Context::Input),
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
            vec![("f12/?".to_string(), "help".to_string())],
            "an unbound action leaves no hint behind",
        );
    }
}
