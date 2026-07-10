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

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

/// One catalog row: an [`Action`] with its config name, description, scope and
/// default keys.
struct ActionSpec {
    action: Action,
    config_name: &'static str,
    description: &'static str,
    scope: Scope,
    default_keys: &'static [&'static str],
}

/// Every action with its config name, description, scope and default keys, in a
/// stable order. The order decides conflict precedence: an earlier action
/// claims a contested key first.
///
/// The tab keys are `alt+1`/`alt+2`/`alt+3` rather than the bare digits used by
/// other apps built on this layout, because the Calc view is a text field and a
/// bare digit has to reach it as a character.
const ACTIONS: &[ActionSpec] = &[
    ActionSpec {
        action: Action::ViewCalc,
        config_name: "view_calc",
        description: "calc",
        scope: Scope::Global,
        default_keys: &["alt+1"],
    },
    ActionSpec {
        action: Action::ViewVariables,
        config_name: "view_variables",
        description: "variables",
        scope: Scope::Global,
        default_keys: &["alt+2", "f4"],
    },
    ActionSpec {
        action: Action::ViewSettings,
        config_name: "view_settings",
        description: "settings",
        scope: Scope::Global,
        default_keys: &["alt+3"],
    },
    ActionSpec {
        action: Action::CycleNotation,
        config_name: "notation",
        description: "notation",
        scope: Scope::Global,
        default_keys: &["f2"],
    },
    ActionSpec {
        action: Action::ToggleAngle,
        config_name: "angle_mode",
        description: "deg/rad",
        scope: Scope::Global,
        default_keys: &["f3"],
    },
    ActionSpec {
        action: Action::ToggleDecimalSeparator,
        config_name: "decimal_separator",
        description: "decimal mark",
        scope: Scope::Global,
        default_keys: &["f5"],
    },
    ActionSpec {
        action: Action::ToggleTrim,
        config_name: "trim_zeros",
        description: "trailing zeros",
        scope: Scope::Global,
        default_keys: &["f6"],
    },
    ActionSpec {
        action: Action::CopyLast,
        config_name: "copy_result",
        description: "copy result",
        scope: Scope::Global,
        default_keys: &["ctrl+y"],
    },
    ActionSpec {
        action: Action::SearchHistory,
        config_name: "search_history",
        description: "search history",
        scope: Scope::Global,
        default_keys: &["ctrl+r"],
    },
    ActionSpec {
        action: Action::OpenPalette,
        config_name: "palette",
        description: "command palette",
        scope: Scope::Global,
        default_keys: &["ctrl+p"],
    },
    ActionSpec {
        action: Action::OpenHelp,
        config_name: "help",
        description: "help",
        scope: Scope::Global,
        default_keys: &["f12", "?"],
    },
    ActionSpec {
        action: Action::Quit,
        config_name: "quit",
        description: "quit",
        scope: Scope::Global,
        default_keys: &["q"],
    },
    ActionSpec {
        action: Action::Submit,
        config_name: "submit",
        description: "calc",
        scope: Scope::Input,
        default_keys: &["enter"],
    },
    ActionSpec {
        action: Action::EnterHistory,
        config_name: "enter_history",
        description: "history",
        scope: Scope::Input,
        default_keys: &["up", "pgup"],
    },
    ActionSpec {
        action: Action::ClearInput,
        config_name: "clear_input",
        description: "clear",
        scope: Scope::Input,
        default_keys: &["esc"],
    },
    ActionSpec {
        action: Action::ApplyEdit,
        config_name: "apply_edit",
        description: "apply",
        scope: Scope::Edit,
        default_keys: &["enter"],
    },
    ActionSpec {
        action: Action::CancelEdit,
        config_name: "cancel_edit",
        description: "cancel",
        scope: Scope::Edit,
        default_keys: &["esc"],
    },
    ActionSpec {
        action: Action::Up,
        config_name: "up",
        description: "up",
        scope: Scope::List,
        default_keys: &["up"],
    },
    ActionSpec {
        action: Action::Down,
        config_name: "down",
        description: "down",
        scope: Scope::List,
        default_keys: &["down"],
    },
    ActionSpec {
        action: Action::PageUp,
        config_name: "page_up",
        description: "page up",
        scope: Scope::List,
        default_keys: &["pgup"],
    },
    ActionSpec {
        action: Action::PageDown,
        config_name: "page_down",
        description: "page down",
        scope: Scope::List,
        default_keys: &["pgdn"],
    },
    ActionSpec {
        action: Action::Top,
        config_name: "top",
        description: "first",
        scope: Scope::List,
        default_keys: &["home"],
    },
    ActionSpec {
        action: Action::Bottom,
        config_name: "bottom",
        description: "last",
        scope: Scope::List,
        default_keys: &["end"],
    },
    ActionSpec {
        action: Action::CopyPlain,
        config_name: "copy",
        description: "copy",
        scope: Scope::List,
        default_keys: &["y"],
    },
    ActionSpec {
        action: Action::CopyDisplay,
        config_name: "copy_displayed",
        description: "copy shown",
        scope: Scope::List,
        default_keys: &["Y"],
    },
    ActionSpec {
        action: Action::MoveUp,
        config_name: "move_up",
        description: "move up",
        scope: Scope::History,
        default_keys: &["alt+up"],
    },
    ActionSpec {
        action: Action::MoveDown,
        config_name: "move_down",
        description: "move down",
        scope: Scope::History,
        default_keys: &["alt+down"],
    },
    ActionSpec {
        action: Action::EditEntry,
        config_name: "edit",
        description: "edit",
        scope: Scope::History,
        default_keys: &["enter", "e"],
    },
    ActionSpec {
        action: Action::InsertBelow,
        config_name: "insert_below",
        description: "insert below",
        scope: Scope::History,
        default_keys: &["o"],
    },
    ActionSpec {
        action: Action::InsertAbove,
        config_name: "insert_above",
        description: "insert above",
        scope: Scope::History,
        default_keys: &["O"],
    },
    ActionSpec {
        action: Action::DeleteEntry,
        config_name: "delete",
        description: "delete",
        scope: Scope::History,
        default_keys: &["d", "del"],
    },
    ActionSpec {
        action: Action::ClearHistory,
        config_name: "clear",
        description: "clear",
        scope: Scope::History,
        default_keys: &["D"],
    },
    ActionSpec {
        action: Action::Back,
        config_name: "back",
        description: "back",
        scope: Scope::List,
        default_keys: &["esc"],
    },
    ActionSpec {
        action: Action::InsertVariable,
        config_name: "insert_variable",
        description: "insert",
        scope: Scope::Variables,
        default_keys: &["enter"],
    },
    ActionSpec {
        action: Action::DeleteVariable,
        config_name: "delete_variable",
        description: "delete",
        scope: Scope::Variables,
        default_keys: &["d", "del"],
    },
    ActionSpec {
        action: Action::ResetVariables,
        config_name: "reset_variables",
        description: "reset",
        scope: Scope::Variables,
        default_keys: &["R"],
    },
    ActionSpec {
        action: Action::PreviousValue,
        config_name: "prev_value",
        description: "previous",
        scope: Scope::Settings,
        default_keys: &["left"],
    },
    ActionSpec {
        action: Action::NextValue,
        config_name: "next_value",
        description: "next",
        scope: Scope::Settings,
        default_keys: &["right", "enter"],
    },
];

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

    fn from_config_name(name: &str) -> Option<Action> {
        Action::all().find(|action| action.config_name() == name)
    }
}

/// A parsed key chord: a key plus the `ctrl`/`alt` modifiers. `shift` is
/// encoded in the character's case and otherwise ignored when matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    code: KeyCode,
    ctrl: bool,
    alt: bool,
}

impl KeyChord {
    /// Parses a chord like `"a"`, `"D"`, `"alt+1"`, `"f2"`, `"pgup"` or
    /// `"enter"`. Returns `None` for an unrecognised string.
    pub fn parse(text: &str) -> Option<KeyChord> {
        let parts: Vec<&str> = text
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();
        // The final token is the key; anything before it is a modifier.
        let (code_token, modifiers) = parts.split_last()?;
        let mut ctrl = false;
        let mut alt = false;
        for modifier in modifiers {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "alt" | "option" => alt = true,
                "shift" => {}
                _ => return None,
            }
        }
        let code = code_from_token(code_token)?;
        Some(KeyChord { code, ctrl, alt })
    }

    /// Whether `key` triggers this chord (code plus ctrl/alt; shift ignored).
    pub fn matches(&self, key: &KeyEvent) -> bool {
        self.code == key.code
            && self.ctrl == key.modifiers.contains(KeyModifiers::CONTROL)
            && self.alt == key.modifiers.contains(KeyModifiers::ALT)
    }

    /// A display string for the hints, e.g. `alt+1`, `f2`, `pgup`, `D`.
    pub fn display(&self) -> String {
        let mut text = String::new();
        if self.ctrl {
            text.push_str("ctrl+");
        }
        if self.alt {
            text.push_str("alt+");
        }
        text.push_str(&token_for_code(self.code));
        text
    }
}

/// Whether `key` is a bare printable character, i.e. one that has to reach the
/// text editor rather than trigger an action while the input has focus.
pub fn is_bare_character(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char(_))
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && !key.modifiers.contains(KeyModifiers::ALT)
}

/// Parses a single key token (no modifiers) into a [`KeyCode`].
fn code_from_token(token: &str) -> Option<KeyCode> {
    let lower = token.to_ascii_lowercase();
    let code = match lower.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "space" => KeyCode::Char(' '),
        "backspace" => KeyCode::Backspace,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "pgup" | "pageup" => KeyCode::PageUp,
        "pgdn" | "pgdown" | "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "del" | "delete" => KeyCode::Delete,
        _ => return function_or_char(token, &lower),
    };
    Some(code)
}

/// Resolves an `fN` function key or a single character (preserving case).
fn function_or_char(token: &str, lower: &str) -> Option<KeyCode> {
    if let Some(digits) = lower.strip_prefix('f')
        && let Ok(number) = digits.parse::<u8>()
        && (1..=12).contains(&number)
    {
        return Some(KeyCode::F(number));
    }
    let mut chars = token.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(KeyCode::Char(first))
}

/// The display token for a key code (inverse of [`code_from_token`]).
fn token_for_code(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(character) => character.to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::PageUp => "pgup".to_string(),
        KeyCode::PageDown => "pgdn".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::Delete => "del".to_string(),
        KeyCode::F(number) => format!("f{number}"),
        _ => "?".to_string(),
    }
}

/// A configured key that was dropped because an earlier action in an
/// overlapping scope already claimed it.
#[derive(Debug, Clone)]
pub struct Conflict {
    /// The contested key, as a display string (e.g. `"d"`).
    pub key: String,
    /// The action whose binding was dropped.
    pub action: Action,
    /// The action that already owns the key.
    pub claimed_by: Action,
}

/// The resolved key map: chords paired with the action they trigger.
#[derive(Debug, Clone)]
pub struct Keymap {
    entries: Vec<(KeyChord, Action)>,
    conflicts: Vec<Conflict>,
}

impl Default for Keymap {
    /// The compiled-in default key map.
    fn default() -> Self {
        Self::from_overrides(&BTreeMap::new())
    }
}

impl Keymap {
    /// Builds the map from the defaults, replacing an action's keys if
    /// `overrides` names it. Unknown action names and unparseable keys are
    /// logged and skipped; a key already bound to an earlier action in an
    /// overlapping scope keeps that binding.
    pub fn from_overrides(overrides: &BTreeMap<String, Vec<String>>) -> Self {
        for name in overrides.keys() {
            if Action::from_config_name(name).is_none() {
                log::warn!("unknown key action '{name}' in config, ignoring");
            }
        }
        let mut entries: Vec<(KeyChord, Action)> = Vec::new();
        let mut conflicts: Vec<Conflict> = Vec::new();
        for action in Action::all() {
            for key in override_keys(overrides, action) {
                bind_key(&mut entries, &mut conflicts, action, &key);
            }
        }
        Keymap { entries, conflicts }
    }

    /// The bindings dropped because an earlier action owned the key.
    pub fn conflicts(&self) -> &[Conflict] {
        &self.conflicts
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
        if context.is_text_editing() && is_bare_character(key) {
            return None;
        }
        self.entries
            .iter()
            .find(|(chord, action)| {
                action.scope().is_active_in(context) && chord.matches(key)
            })
            .map(|(_, action)| *action)
    }

    /// The display strings of the keys bound to `action`.
    pub fn keys_for(&self, action: Action) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, bound)| *bound == action)
            .map(|(chord, _)| chord.display())
            .collect()
    }

    /// Builds `(keys, description)` hint pairs for `actions`, skipping any with
    /// no bound key. The single source for the footer and the help overlay.
    pub fn hints(&self, actions: &[Action]) -> Vec<(String, String)> {
        actions
            .iter()
            .filter_map(|&action| {
                let keys = self.keys_for(action).join("/");
                if keys.is_empty() {
                    None
                } else {
                    Some((keys, action.description().to_string()))
                }
            })
            .collect()
    }
}

/// The configured keys for `action`, or its defaults when unconfigured.
fn override_keys(
    overrides: &BTreeMap<String, Vec<String>>,
    action: Action,
) -> Vec<String> {
    overrides
        .get(action.config_name())
        .cloned()
        .unwrap_or_else(|| {
            action
                .default_keys()
                .iter()
                .map(|key| key.to_string())
                .collect()
        })
}

/// Binds `key` to `action`, or records why it was dropped: an unparseable key
/// is logged, and a key already claimed by an earlier action in an overlapping
/// scope is logged and pushed to `conflicts` (the earlier binding wins).
fn bind_key(
    entries: &mut Vec<(KeyChord, Action)>,
    conflicts: &mut Vec<Conflict>,
    action: Action,
    key: &str,
) {
    let Some(chord) = KeyChord::parse(key) else {
        log::warn!("invalid key '{key}' for '{}'", action.config_name());
        return;
    };
    let owner = entries.iter().find(|(existing, owner)| {
        *existing == chord && owner.scope().overlaps(action.scope())
    });
    if let Some((_, owner)) = owner {
        log::warn!(
            "key '{key}' already bound to '{}', ignoring for '{}'",
            owner.config_name(),
            action.config_name(),
        );
        conflicts.push(Conflict {
            key: chord.display(),
            action,
            claimed_by: *owner,
        });
        return;
    }
    entries.push((chord, action));
}

#[cfg(test)]
mod tests {
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
    fn bare_characters_are_recognised_so_the_input_keeps_them() {
        assert!(is_bare_character(&key(KeyCode::Char('q'))));
        assert!(is_bare_character(&key(KeyCode::Char('?'))));
        assert!(!is_bare_character(&chord(
            KeyCode::Char('y'),
            KeyModifiers::CONTROL,
        )));
        assert!(!is_bare_character(&chord(
            KeyCode::Char('1'),
            KeyModifiers::ALT,
        )));
        assert!(!is_bare_character(&key(KeyCode::F(2))));
        assert!(!is_bare_character(&key(KeyCode::Enter)));
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

    #[test]
    fn chords_round_trip_through_parse_and_display() {
        for text in ["a", "D", "alt+1", "ctrl+y", "f12", "pgup", "enter", "esc"]
        {
            let chord = KeyChord::parse(text).expect("parses");
            assert_eq!(chord.display(), text);
        }
    }

    #[test]
    fn parse_rejects_an_unknown_modifier_or_a_multi_character_token() {
        assert!(KeyChord::parse("hyper+a").is_none());
        assert!(KeyChord::parse("abc").is_none());
        assert!(KeyChord::parse("f13").is_none());
    }
}
