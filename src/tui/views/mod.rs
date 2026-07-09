//! The three top-level views and the tab bar that selects them.

pub mod calc;
pub mod settings;
pub mod variables;

/// A top-level view, one per tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewId {
    /// The calculator: history plus the input field.
    #[default]
    Calc,
    /// The saved variables.
    Variables,
    /// The display settings.
    Settings,
}

/// The tab entries in display order: `(key hint, label)`.
///
/// The keys are `alt+N` rather than bare digits, because the Calc view is a
/// text field and a bare digit has to reach it as a character. Keep in step
/// with the `view_*` actions in [`crate::keymap`].
const TABS: &[(&str, &str)] = &[
    ("alt+1", "Calc"),
    ("alt+2", "Variables"),
    ("alt+3", "Settings"),
];

impl ViewId {
    /// Every view, in tab order.
    pub fn all() -> [ViewId; 3] {
        [ViewId::Calc, ViewId::Variables, ViewId::Settings]
    }

    /// The tab entries for the header bar.
    pub fn tabs() -> &'static [(&'static str, &'static str)] {
        TABS
    }

    /// The tab index of this view.
    pub fn index(self) -> usize {
        match self {
            ViewId::Calc => 0,
            ViewId::Variables => 1,
            ViewId::Settings => 2,
        }
    }

    /// The view at `index`, falling back to [`ViewId::Calc`] when out of range
    /// (an old or hand-edited state file).
    pub fn from_index(index: usize) -> ViewId {
        ViewId::all().get(index).copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_round_trip_through_from_index() {
        for view in ViewId::all() {
            assert_eq!(ViewId::from_index(view.index()), view);
        }
    }

    #[test]
    fn an_out_of_range_index_falls_back_to_the_calc_view() {
        assert_eq!(ViewId::from_index(99), ViewId::Calc);
    }

    #[test]
    fn there_is_one_tab_entry_per_view() {
        assert_eq!(ViewId::tabs().len(), ViewId::all().len());
    }

    #[test]
    fn the_tab_keys_are_alt_digits_so_bare_digits_stay_typeable() {
        let keys: Vec<&str> =
            ViewId::tabs().iter().map(|(key, _)| *key).collect();
        assert_eq!(keys, vec!["alt+1", "alt+2", "alt+3"]);
    }
}
