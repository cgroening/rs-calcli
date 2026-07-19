//! The footer hints and the help overlay, built from one shared source.
//!
//! Both read the same [`crate::tui::bindings`] tables through the same
//! [`App::groups_of`], so a key added to a table shows up in the footer and in
//! the help without a second edit - and neither can quietly fall behind the
//! other.

use ratada::shortcut_hints;

use crate::keymap::Action;
use crate::tui::app::{App, GLOBAL_GROUP};
use crate::tui::appframe::HintGroups;
use crate::tui::bindings;
use crate::tui::views::ViewId;
use crate::tui::views::calc::Mode;

impl App {
    /// The closing `Global` group: the app's own chords first, then the
    /// toolkit's (`f1 toggle hints`, `ctrl+q force quit`). One helper feeds
    /// both the footer and the help overlay, so they cannot drift apart.
    fn global_group(&self) -> (String, Vec<(String, String)>) {
        let mut hints = self.keymap.hints(&[
            Action::OpenPalette,
            Action::OpenHelp,
            Action::Quit,
        ]);
        hints.extend(shortcut_hints::global_bindings());
        (GLOBAL_GROUP.to_string(), hints)
    }

    /// Renders one hint table into owned groups, dropping any that ended up
    /// empty because every action in it was unbound.
    fn groups_of(&self, table: &[bindings::Group]) -> HintGroups {
        table
            .iter()
            .filter_map(|(label, actions)| {
                let hints = self.keymap.hints(actions);
                (!hints.is_empty()).then(|| ((*label).to_string(), hints))
            })
            .collect()
    }

    /// The hint groups of the current view: its own groups, then the settings
    /// chords and the tab switches, closed by the `Global` group.
    ///
    /// An in-place edit locks the tab bar, so the `Views` group is left out
    /// there rather than promising a switch that will not happen.
    pub(super) fn hint_groups(&self) -> HintGroups {
        let table = match (self.active, self.mode) {
            (ViewId::Calc, Mode::Input) => bindings::INPUT_HINT_GROUPS,
            (ViewId::Calc, Mode::Edit(_)) => bindings::EDIT_HINT_GROUPS,
            (ViewId::Calc, Mode::History) => bindings::HISTORY_HINT_GROUPS,
            (ViewId::Variables, _) => bindings::VARIABLES_HINT_GROUPS,
            (ViewId::Settings, _) => bindings::SETTINGS_HINT_GROUPS,
        };
        let mut groups = self.groups_of(table);
        groups.extend(self.groups_of(&[bindings::SETTINGS_CHORDS]));
        if !matches!(self.mode, Mode::Edit(_)) {
            groups.extend(self.groups_of(&[bindings::VIEW_GROUP]));
        }
        groups.push(self.global_group());
        groups
    }

    /// The help overlay's sections: every view's groups, the shared chords,
    /// then the same `Global` group the footer closes with.
    pub(crate) fn help_sections(&self) -> HintGroups {
        let mut sections: HintGroups = Vec::new();
        for (view, table) in bindings::HELP_TABLES {
            for (label, hints) in self.groups_of(table) {
                sections.push((format!("{view} \u{b7} {label}"), hints));
            }
        }
        sections.extend(self.groups_of(&[bindings::SETTINGS_CHORDS]));
        sections.extend(self.groups_of(&[bindings::VIEW_GROUP]));
        sections.push(self.global_group());
        sections
    }
}
