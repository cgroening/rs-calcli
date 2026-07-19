//! The per-view shortcut-hint groups.
//!
//! Each view names its groups and the actions in them; the keys and
//! descriptions come from [`crate::keymap`], so a rebinding shows up in the
//! footer and the help overlay without an edit here.
//!
//! Two groups are appended by [`App`](crate::tui::App) rather than repeated in
//! every table: [`VIEW_GROUP`] (the tab switches) and the closing `Global`
//! group. That is why no table below names `?`, `q`, `F1` or `Ctrl+Q`.

use crate::keymap::Action;

/// One named group of actions.
pub type Group = (&'static str, &'static [Action]);

/// The tab switches, appended to every view that allows them.
pub const VIEW_GROUP: Group = (
    "Views",
    &[
        Action::ViewCalc,
        Action::ViewVariables,
        Action::ViewSettings,
    ],
);

/// The Calc view while typing an expression.
pub const INPUT_HINT_GROUPS: &[Group] = &[(
    "Calculate",
    &[
        Action::Submit,
        Action::EnterHistory,
        Action::ClearInput,
        Action::CopyLast,
        Action::SearchHistory,
        Action::OpenPalette,
    ],
)];

/// The Calc view while editing a history line in place. Tab switching is locked
/// here, so [`VIEW_GROUP`] is deliberately not appended to it.
pub const EDIT_HINT_GROUPS: &[Group] =
    &[("Editing", &[Action::ApplyEdit, Action::CancelEdit])];

/// The Calc view while browsing the history.
pub const HISTORY_HINT_GROUPS: &[Group] = &[
    (
        "Navigation",
        &[
            Action::Up,
            Action::Down,
            Action::PageUp,
            Action::PageDown,
            Action::Top,
            Action::Bottom,
        ],
    ),
    (
        "Actions",
        &[
            Action::EditEntry,
            Action::InsertBelow,
            Action::InsertAbove,
            Action::MoveUp,
            Action::MoveDown,
            Action::DeleteEntry,
            Action::ClearHistory,
            Action::CopyPlain,
            Action::CopyDisplay,
            Action::Back,
        ],
    ),
];

/// The Variables view.
pub const VARIABLES_HINT_GROUPS: &[Group] = &[
    (
        "Navigation",
        &[
            Action::Up,
            Action::Down,
            Action::PageUp,
            Action::PageDown,
            Action::Top,
            Action::Bottom,
            Action::Back,
        ],
    ),
    (
        "Actions",
        &[
            Action::InsertVariable,
            Action::CopyPlain,
            Action::CopyDisplay,
            Action::DeleteVariable,
            Action::ResetVariables,
        ],
    ),
];

/// The Settings view.
pub const SETTINGS_HINT_GROUPS: &[Group] = &[
    (
        "Navigation",
        &[
            Action::Up,
            Action::Down,
            Action::PageUp,
            Action::PageDown,
            Action::Top,
            Action::Bottom,
            Action::Back,
        ],
    ),
    ("Change", &[Action::PreviousValue, Action::NextValue]),
];

/// The settings toggles that stay reachable from every view.
pub const SETTINGS_CHORDS: Group = (
    "Settings",
    &[
        Action::CycleNotation,
        Action::ToggleAngle,
        Action::ToggleDecimalSeparator,
        Action::ToggleTrim,
    ],
);

/// The palette category for `action`: the label of the first hint group that
/// lists it, so the command palette groups commands exactly as the footer and
/// help do. The app-wide chords that live only in the closing `Global` group
/// fall back to `"General"`.
pub fn category_of(action: Action) -> &'static str {
    let tables: [&[Group]; 5] = [
        INPUT_HINT_GROUPS,
        EDIT_HINT_GROUPS,
        HISTORY_HINT_GROUPS,
        VARIABLES_HINT_GROUPS,
        SETTINGS_HINT_GROUPS,
    ];
    let singles = [VIEW_GROUP, SETTINGS_CHORDS];
    let groups = tables.iter().flat_map(|table| table.iter());
    for &(label, actions) in groups.chain(singles.iter()) {
        if actions.contains(&action) {
            return label;
        }
    }
    "General"
}

/// Every view's groups, titled for the help overlay.
pub const HELP_TABLES: &[(&str, &[Group])] = &[
    ("Input", INPUT_HINT_GROUPS),
    ("Editing a line", EDIT_HINT_GROUPS),
    ("History", HISTORY_HINT_GROUPS),
    ("Variables", VARIABLES_HINT_GROUPS),
    ("Settings", SETTINGS_HINT_GROUPS),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Context, Keymap, Scope};

    /// Every hint table in the app.
    fn all_tables() -> [&'static [Group]; 5] {
        [
            INPUT_HINT_GROUPS,
            EDIT_HINT_GROUPS,
            HISTORY_HINT_GROUPS,
            VARIABLES_HINT_GROUPS,
            SETTINGS_HINT_GROUPS,
        ]
    }

    #[test]
    fn no_table_names_help_or_quit_since_the_global_group_owns_them() {
        for table in all_tables() {
            for (_, actions) in table {
                assert!(!actions.contains(&Action::OpenHelp));
                assert!(!actions.contains(&Action::Quit));
            }
        }
    }

    #[test]
    fn no_table_repeats_the_view_switches() {
        for table in all_tables() {
            for (_, actions) in table {
                assert!(
                    !actions.contains(&Action::ViewCalc),
                    "the Views group is appended once, not per table",
                );
            }
        }
    }

    #[test]
    fn every_hinted_action_has_a_key() {
        let keymap = Keymap::default();
        let extra = [VIEW_GROUP, SETTINGS_CHORDS];
        let tables = all_tables();
        let groups = tables.iter().flat_map(|t| t.iter()).chain(extra.iter());
        for (label, actions) in groups {
            for action in *actions {
                assert!(
                    !keymap.keys_for(*action).is_empty(),
                    "{action:?} in group {label:?} has no key",
                );
            }
        }
    }

    #[test]
    fn each_table_only_hints_actions_reachable_in_its_own_context() {
        let expected = [
            (INPUT_HINT_GROUPS, Context::Input),
            (EDIT_HINT_GROUPS, Context::Edit),
            (HISTORY_HINT_GROUPS, Context::History),
            (VARIABLES_HINT_GROUPS, Context::Variables),
            (SETTINGS_HINT_GROUPS, Context::Settings),
        ];
        for (table, context) in expected {
            for (label, actions) in table {
                for action in *actions {
                    let scope = action.scope();
                    assert!(
                        scope == Scope::Global || scope_matches(scope, context),
                        "{action:?} in {label:?} is unreachable in {context:?}",
                    );
                }
            }
        }
    }

    /// Whether a non-global scope belongs to `context`.
    fn scope_matches(scope: Scope, context: Context) -> bool {
        matches!(
            (scope, context),
            (Scope::Input, Context::Input)
                | (Scope::Edit, Context::Edit)
                | (Scope::History, Context::History)
                | (Scope::Variables, Context::Variables)
                | (Scope::Settings, Context::Settings)
                | (Scope::List, Context::History)
                | (Scope::List, Context::Variables)
                | (Scope::List, Context::Settings)
        )
    }

    #[test]
    fn the_help_tables_cover_every_view() {
        assert_eq!(HELP_TABLES.len(), all_tables().len());
        for (title, _) in HELP_TABLES {
            assert!(!title.is_empty());
        }
    }

    #[test]
    fn group_labels_are_non_empty() {
        for table in all_tables() {
            for (label, _) in table {
                assert!(!label.is_empty());
            }
        }
    }

    #[test]
    fn category_of_reads_the_hint_tables_and_falls_back_to_general() {
        assert_eq!(category_of(Action::Submit), "Calculate");
        assert_eq!(category_of(Action::EditEntry), "Actions");
        assert_eq!(category_of(Action::ViewCalc), "Views");
        assert_eq!(category_of(Action::CycleNotation), "Settings");
        // A chord that lives only in the closing Global group.
        assert_eq!(category_of(Action::OpenHelp), "General");
    }

    #[test]
    fn every_action_has_a_palette_category() {
        for action in Action::all() {
            assert!(!category_of(action).is_empty());
        }
    }
}
