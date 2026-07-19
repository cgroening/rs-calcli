//! The TUI state and its [`Screen`] implementation.
//!
//! [`App`] owns the calculator service and the view-only state, translates keys
//! into service calls through [`crate::keymap`] and renders through the shared
//! [`appframe`](crate::tui::appframe). Modals are blocking (`ratada::modal`),
//! so they dim the live view behind them instead of a black screen.
//!
//! The type is not split, only its `impl` is: this module holds the state, the
//! construction and the small state helpers every handler leans on, while the
//! handlers themselves live in sibling child modules by responsibility. Private
//! fields stay reachable there because the children sit *below* this module.

mod calc_history;
mod calc_input;
mod clipboard;
mod dispatch;
mod hints;
mod render;
mod settings;
#[cfg(test)]
mod testing;
mod variables;

use std::cell::Cell;
use std::collections::BTreeMap;

use anyhow::Result;
use crossterm::event::KeyEvent;
use ratada::autocomplete::Autocomplete;
use ratada::nav;
use ratada::quit::QuitConfirm;
use ratada::theme::{
    ColorOverrides, DEFAULT_THEME, GlyphVariant, Glyphs, Palette, Skin,
    ThemeRegistry,
};
use ratada::{Flow, Screen, Tui, modal, quit, shortcut_hints, style};
use ratatui::Frame;

use crate::config::Config;
use crate::domain::completion;
use crate::domain::quantity::Quantity;
use crate::keymap::{Context, Keymap};
use crate::services::CalcService;
use crate::storage::{
    PersistedEntry, PersistedSettings, PersistedState, PersistedValue, UiState,
};
use crate::tui::colors::Highlight;
use crate::tui::interaction::{Answer, Interaction, Modals};
use crate::tui::text_edit::{self, TextCursor};
use crate::tui::views::ViewId;
use crate::tui::views::calc::{HistoryStyle, Metrics, Mode};

/// The label of the closing hint group, shown in the footer and the help.
const GLOBAL_GROUP: &str = "Global";

/// The whole TUI state: the calculator service plus view-only fields.
pub struct App {
    service: CalcService,
    skin: Skin,
    registry: ThemeRegistry,
    theme: String,
    /// The `[appearance.colors]` overrides, re-applied whenever the theme
    /// changes so a runtime theme switch never loses them.
    overrides: BTreeMap<String, String>,
    keymap: Keymap,
    highlight: Highlight,
    confirm_delete: bool,

    history: HistoryStyle,
    input_max_lines: usize,
    live_feedback: bool,

    active: ViewId,
    input: String,
    cursor: TextCursor,
    /// The suggestion dropdown for the input field, rebuilt from the current
    /// variables plus the built-in names on every edit.
    autocomplete: Autocomplete,
    mode: Mode,
    selected: Option<usize>,
    var_selected: usize,
    setting_selected: usize,

    status: Option<String>,
    quit: bool,
    editing_inserted: bool,

    metrics: Cell<Metrics>,
    var_offset: Cell<usize>,
    setting_offset: Cell<usize>,
    /// Rows visible in the active list view, recorded while drawing it. The
    /// page-wise keys need it and only the renderer knows the real height.
    list_height: Cell<usize>,
}

impl App {
    /// Builds the app from a configured service, the loaded config and the
    /// restored UI state.
    pub fn new(service: CalcService, config: &Config, ui: &UiState) -> Self {
        let registry = config.theme_registry();
        let skin =
            Skin::new(config.palette(), Glyphs::new(config.appearance.glyphs));
        let autocomplete =
            Autocomplete::new(completion::candidates(service.variables()));
        let mut app = App {
            service,
            skin,
            registry,
            theme: config.appearance.theme.clone(),
            overrides: config.appearance.colors.clone(),
            keymap: config.keymap(),
            highlight: Highlight::new(&config.highlight),
            confirm_delete: config.confirm_delete,
            history: HistoryStyle {
                spacing: config.history_spacing,
                separator: config
                    .history_separator
                    .then(|| style::to_ratatui(config.palette().border)),
            },
            input_max_lines: config.input_max_lines,
            live_feedback: config.live_feedback,
            active: ViewId::Calc,
            input: String::new(),
            cursor: TextCursor::at(0),
            autocomplete,
            mode: Mode::Input,
            selected: None,
            var_selected: 0,
            setting_selected: 0,
            status: None,
            quit: false,
            editing_inserted: false,
            metrics: Cell::new(Metrics::default()),
            var_offset: Cell::new(0),
            setting_offset: Cell::new(0),
            list_height: Cell::new(0),
        };
        for conflict in app.keymap.conflicts() {
            log::warn!(
                "key '{}' for '{}' is shadowed by '{}'",
                conflict.key,
                conflict.action.config_name(),
                conflict.claimed_by.config_name(),
            );
        }
        app.restore(ui);
        // Only the soft quit is questioned; `Ctrl+Q` stays the unconditional
        // escape hatch either way.
        quit::set_confirm(if config.confirm_quit {
            QuitConfirm::Soft
        } else {
            QuitConfirm::Never
        });
        app.install_quit_guard();
        app
    }

    /// Registers how the quit confirmation is drawn, with the current skin.
    /// Re-run from [`Self::set_skin`] so the dialog follows the active theme.
    fn install_quit_guard(&self) {
        let skin = self.skin;
        quit::set_guard(move |tui, _kind, background| {
            modal::confirm(tui, &skin, " Quit? ", background)
        });
    }

    /// The single seam for changing the skin: the quit guard holds a copy of it
    /// and has to be re-registered whenever the theme or the glyphs change.
    fn set_skin(&mut self, skin: Skin) {
        self.skin = skin;
        self.install_quit_guard();
    }

    /// Switches the active theme by name, falling back to the default when the
    /// name is unknown.
    ///
    /// The configured colour overrides are re-applied on top, exactly as at
    /// startup. Dropping them here would undo calcli's own defaults (the red
    /// block cursor, the input fills) on every restart, because restoring the
    /// persisted theme name comes through this path.
    pub(super) fn set_theme(&mut self, name: &str) {
        let name = if self.registry.contains(name) {
            name.to_string()
        } else {
            log::warn!("unknown theme '{name}', using '{DEFAULT_THEME}'");
            DEFAULT_THEME.to_string()
        };
        let base = self.registry.resolve(&name);
        let mut skin = self.skin;
        skin.palette = Palette::resolve(base, &self.color_overrides());
        self.set_skin(skin);
        self.theme = name;
    }

    /// The configured colour overrides, layered over whichever theme is active.
    fn color_overrides(&self) -> ColorOverrides<'_> {
        ColorOverrides::from_lookup(|name| {
            self.overrides.get(name).map_or("", String::as_str)
        })
    }

    /// Switches the glyph variant.
    pub(super) fn set_glyphs(&mut self, variant: GlyphVariant) {
        let mut skin = self.skin;
        skin.glyphs = Glyphs::new(variant);
        self.set_skin(skin);
    }

    /// Applies the persisted UI state.
    fn restore(&mut self, ui: &UiState) {
        self.active = ViewId::from_index(ui.active_view);
        if let Some(theme) = &ui.theme {
            self.set_theme(theme);
        }
        if let Some(glyphs) = ui.glyphs {
            self.set_glyphs(glyphs);
        }
        if let Some(visible) = ui.hints_visible {
            shortcut_hints::set_visible(visible);
        }
    }

    /// Snapshots the session for persistence: settings, UI, variables, history.
    pub fn persisted_state(&self) -> PersistedState {
        PersistedState {
            settings: Some(self.persisted_settings()),
            ui: self.persisted_ui(),
            variables: self.persisted_variables(),
            history: self.persisted_history(),
        }
    }

    /// The display settings, as written to `state.toml`.
    fn persisted_settings(&self) -> PersistedSettings {
        let settings = self.service.settings();
        PersistedSettings {
            notation: settings.notation,
            decimals: settings.decimals,
            angle_mode: settings.angle_mode,
            decimal_separator: settings.decimal_separator.to_string(),
            trim_trailing_zeros: settings.trim_trailing_zeros,
        }
    }

    /// The view-only state that survives a restart.
    fn persisted_ui(&self) -> UiState {
        UiState {
            active_view: self.active.index(),
            theme: Some(self.theme.clone()),
            glyphs: Some(self.skin.glyphs.variant),
            hints_visible: Some(shortcut_hints::visible()),
        }
    }

    /// The variables, as name/value pairs carrying their unit.
    fn persisted_variables(&self) -> BTreeMap<String, PersistedValue> {
        self.service
            .variables()
            .iter()
            .map(|(name, value)| (name.clone(), persisted_value(value)))
            .collect()
    }

    /// The history lines, keeping the full precision of each result.
    fn persisted_history(&self) -> Vec<PersistedEntry> {
        self.service
            .history()
            .entries()
            .iter()
            .map(|entry| PersistedEntry {
                input: entry.input.clone(),
                value: entry.value.as_ref().map(Quantity::display_value),
                unit: entry
                    .value
                    .as_ref()
                    .and_then(|value| value.unit_symbol().map(str::to_string)),
            })
            .collect()
    }

    /// The calculator service, for the integration tests.
    pub fn service(&self) -> &CalcService {
        &self.service
    }

    /// The warning marker for the current glyph set.
    pub(super) fn warn(&self) -> &'static str {
        match self.skin.glyphs.variant {
            GlyphVariant::Unicode => "\u{26a0}",
            GlyphVariant::Ascii => "!",
        }
    }

    /// Which keymap context the current view and mode select.
    pub(super) fn context(&self) -> Context {
        match (self.active, self.mode) {
            (ViewId::Calc, Mode::Input) => Context::Input,
            (ViewId::Calc, Mode::Edit(_)) => Context::Edit,
            (ViewId::Calc, Mode::History) => Context::History,
            (ViewId::Variables, _) => Context::Variables,
            (ViewId::Settings, _) => Context::Settings,
        }
    }

    /// The active skin, for the dialogs drawn over this view.
    pub(crate) fn skin(&self) -> &Skin {
        &self.skin
    }

    /// Asks before a destructive action, unless `confirm_delete` is off.
    ///
    /// A `Ctrl+Q` inside the dialog quits at once and the action is abandoned:
    /// the toolkit honours the hard quit everywhere, modals included.
    ///
    /// # Errors
    ///
    /// Propagates whatever the dialog reports while drawing or reading input.
    pub(super) fn confirm(
        &mut self,
        io: &mut dyn Interaction,
        prompt: &str,
    ) -> Result<bool> {
        if !self.confirm_delete {
            return Ok(true);
        }
        let answer = io.confirm(self, prompt)?;
        self.absorb(answer);
        Ok(answer.is_yes())
    }

    /// Records a forced quit raised from inside a dialog.
    pub(super) fn absorb(&mut self, answer: Answer) {
        if answer.is_forced_quit() {
            self.quit = true;
        }
    }

    /// Records a transient status message, cleared by the next key press.
    pub(super) fn report(&mut self, message: String) {
        self.status = Some(message);
    }

    /// Empties the input buffer and parks the caret at its start.
    pub(super) fn reset_input(&mut self) {
        self.input.clear();
        self.cursor = TextCursor::at(0);
    }

    /// Replaces the input's selection (or inserts at the caret) with `text`.
    pub(super) fn replace_selection(&mut self, text: &str) {
        text_edit::replace_selection(&mut self.input, &mut self.cursor, text);
    }

    /// How far a page-wise key moves in a list: one screenful, at least one
    /// row even before the first draw has recorded a height.
    fn list_page(&self) -> usize {
        self.list_height.get().max(1)
    }

    /// `cursor` moved by one page in `direction` within `total` entries.
    ///
    /// Page jumps clamp at both ends rather than wrapping: the arrow keys are
    /// the cyclic ones, and a page jump that wrapped would skip past the end
    /// the user was aiming for.
    pub(super) fn page_step(
        &self,
        cursor: usize,
        total: usize,
        direction: isize,
    ) -> usize {
        let page = isize::try_from(self.list_page()).unwrap_or(isize::MAX);
        nav::step_clamped(cursor, total, page.saturating_mul(direction))
    }

    /// Whether the loop should keep running.
    pub(super) fn flow(&self) -> Flow {
        if self.quit {
            Flow::Quit
        } else {
            Flow::Continue
        }
    }
}

/// One variable as written to `state.toml`, value and unit apart.
fn persisted_value(value: &Quantity) -> PersistedValue {
    PersistedValue {
        value: value.display_value(),
        unit: value.unit_symbol().map(str::to_string),
    }
}

impl Screen for App {
    type Error = anyhow::Error;

    fn render(&self, frame: &mut Frame) {
        self.render_body(frame);
    }

    fn handle_key(&mut self, key: KeyEvent, tui: &mut Tui) -> Result<Flow> {
        let mut modals = Modals::new(tui);
        self.dispatch(key, &mut modals)
    }
}
