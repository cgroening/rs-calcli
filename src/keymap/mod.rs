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
    /// Take back the last change to the history or the variables.
    Undo,
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
    /// Reset the focused colour to the theme's value.
    ResetValue,
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

    /// The keys bound to `action` in config syntax (`alt+1`).
    pub fn keys_for(&self, action: Action) -> Vec<String> {
        self.inner.keys_for(action)
    }

    /// The keys bound to `action` as a reader sees them (`Alt+1`, `Enter`).
    pub fn labels_for(&self, action: Action) -> Vec<String> {
        self.inner.labels_for(action)
    }

    /// Builds `(keys, description)` hint pairs for `actions`, skipping any with
    /// no bound key. The single source for the footer and the help overlay.
    pub fn hints(&self, actions: &[Action]) -> Vec<(String, String)> {
        self.inner.hints(actions)
    }
}

#[cfg(test)]
mod tests;
