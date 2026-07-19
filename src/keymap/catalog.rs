//! The action catalog: every shortcut calcli owns, as one table.
//!
//! This is the single source the key dispatch, the footer hints, the help
//! overlay, the command palette and the `[keys]` config names all read from.
//! It is pure data, which is why it sits apart from the lookup logic in
//! [`super`] despite its length.
//!
//! Adding a row here is enough for the footer and the help to pick the action
//! up; the `README.md` key table and the `[keys]` block in
//! `examples/config.toml` are held to it by tests in [`super`].

use crate::keymap::{Action, Scope};

/// One catalog row: an [`Action`] with its config name, description, scope and
/// default keys.
pub(super) struct ActionSpec {
    pub(super) action: Action,
    pub(super) config_name: &'static str,
    pub(super) description: &'static str,
    pub(super) scope: Scope,
    pub(super) default_keys: &'static [&'static str],
}

/// Every action with its config name, description, scope and default keys, in a
/// stable order. The order decides conflict precedence: an earlier action
/// claims a contested key first.
///
/// The tab keys are `alt+1`/`alt+2`/`alt+3` rather than the bare digits used by
/// other apps built on this layout, because the Calc view is a text field and a
/// bare digit has to reach it as a character.
pub(super) const ACTIONS: &[ActionSpec] = &[
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
