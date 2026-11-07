//! The port through which the app opens blocking dialogs.
//!
//! `ratada`'s modals need a live `&mut Tui`, which a test cannot build: they
//! put the terminal into raw mode. Hiding them behind [`Interaction`] keeps the
//! key path testable - a headless fake answers dialogs without a terminal -
//! while production still gets the real dimmed-backdrop modals via [`Modals`].
//!
//! Every dialog can end in [`Answer::ForcedQuit`]: `Ctrl+Q` is honoured inside
//! a modal too, and the toolkit reports it as `ModalSignal::Quit`. Dropping
//! that signal would trap the user in the dialog, so it is carried back to the
//! app rather than folded into "the user said no".

use std::io;

use ratada::quit::QuitKind;
use ratada::theme::Color;
use ratada::{
    ModalSignal, Screen, Tui, command_palette, finder, help, modal, quit,
    swatches,
};
use ratatui::Frame;

use crate::tui::App;
use crate::tui::appframe;

/// How a dialog ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The user agreed.
    Yes,
    /// The user declined, or dismissed the dialog.
    No,
    /// The user pressed `Ctrl+Q` inside the dialog: quit at once.
    ForcedQuit,
}

impl Answer {
    /// Whether the action the dialog guarded may proceed.
    pub fn is_yes(self) -> bool {
        self == Answer::Yes
    }

    /// Whether the app must quit immediately.
    pub fn is_forced_quit(self) -> bool {
        self == Answer::ForcedQuit
    }
}

/// How a fuzzy picker ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// The user chose the item at this index into the picker's items.
    Index(usize),
    /// The user dismissed the picker without choosing.
    None,
    /// The user pressed `Ctrl+Q` inside the picker: quit at once.
    ForcedQuit,
}

/// How the colour chooser ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPick {
    /// The user chose this colour.
    Color(Color),
    /// The user removed the colour (back to the theme's value).
    Cleared,
    /// The user dismissed the chooser.
    Cancelled,
    /// The user pressed `Ctrl+Q` inside the chooser: quit at once.
    ForcedQuit,
}

/// Opens the dialogs the app needs. Every method receives the app so the dialog
/// can repaint the live view behind itself as a dimmed backdrop.
pub trait Interaction {
    /// Asks `prompt`, returning how the dialog ended.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the dialog cannot be drawn.
    fn confirm(&mut self, app: &App, prompt: &str) -> io::Result<Answer>;

    /// Shows the help overlay until the user closes it, returning whether they
    /// forced a quit from inside it.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the overlay cannot be drawn.
    fn help(&mut self, app: &App) -> io::Result<Answer>;

    /// Opens a fuzzy picker over `items` titled `title`, returning the chosen
    /// index into `items`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the picker cannot be drawn.
    fn pick(
        &mut self,
        app: &App,
        title: &str,
        items: &[String],
    ) -> io::Result<Selection>;

    /// Opens the command palette over `items`, returning the chosen index.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the palette cannot be drawn.
    fn palette(
        &mut self,
        app: &App,
        items: &[command_palette::CommandItem<'_>],
    ) -> io::Result<Selection>;

    /// Opens the colour chooser titled `title` on `initial`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the chooser cannot be drawn.
    fn pick_color(
        &mut self,
        app: &App,
        title: &str,
        initial: Option<Color>,
    ) -> io::Result<ColorPick>;

    /// Whether a soft quit may proceed, asking first when configured to.
    fn may_quit(&mut self, app: &App) -> bool;

    /// `F5`: shows why the configuration cannot be reloaded (`checked` is
    /// an error) and returns `Ok`, or returns the reload signal as the error
    /// for a clean configuration.
    ///
    /// # Errors
    ///
    /// The reload signal, or an I/O error when the report cannot be drawn.
    fn reload_config(
        &mut self,
        app: &App,
        checked: Result<(), ratada::reload::ConfigProblems>,
    ) -> io::Result<()>;
}

/// The production implementation: `ratada`'s blocking modals over the live
/// view, each dimming the frame behind itself.
pub struct Modals<'a> {
    tui: &'a mut Tui,
}

impl<'a> Modals<'a> {
    /// Wraps the terminal the dialogs draw on.
    pub fn new(tui: &'a mut Tui) -> Self {
        Modals { tui }
    }
}

/// Maps a toolkit signal onto an [`Answer`].
fn answer_of(signal: ModalSignal<bool>) -> Answer {
    match signal {
        ModalSignal::Value(true) => Answer::Yes,
        ModalSignal::Value(false) | ModalSignal::Cancelled => Answer::No,
        ModalSignal::Quit => Answer::ForcedQuit,
    }
}

impl Interaction for Modals<'_> {
    fn pick_color(
        &mut self,
        app: &App,
        title: &str,
        initial: Option<Color>,
    ) -> io::Result<ColorPick> {
        let signal = swatches::color_chooser_with(
            self.tui,
            app.skin(),
            title,
            initial,
            swatches::ChooserOptions {
                clearable: true,
                picker_unless_listed: true,
            },
            |frame| app.render(frame),
        )?;
        Ok(match signal {
            ModalSignal::Value(swatches::ColorChoice::Color(color)) => {
                ColorPick::Color(color)
            }
            ModalSignal::Value(swatches::ColorChoice::Cleared) => {
                ColorPick::Cleared
            }
            ModalSignal::Cancelled => ColorPick::Cancelled,
            ModalSignal::Quit => ColorPick::ForcedQuit,
        })
    }

    fn confirm(&mut self, app: &App, prompt: &str) -> io::Result<Answer> {
        let signal = modal::confirm(self.tui, app.skin(), prompt, |frame| {
            app.render(frame);
        })?;
        Ok(answer_of(signal))
    }

    fn help(&mut self, app: &App) -> io::Result<Answer> {
        let owned = app.help_sections();
        let sections = appframe::to_help_sections(&owned);
        let signal = help::show(self.tui, app.skin(), &sections, |frame| {
            app.render(frame);
        })?;
        Ok(match signal {
            ModalSignal::Quit => Answer::ForcedQuit,
            ModalSignal::Value(()) | ModalSignal::Cancelled => Answer::No,
        })
    }

    fn pick(
        &mut self,
        app: &App,
        title: &str,
        items: &[String],
    ) -> io::Result<Selection> {
        let signal = finder::finder(self.tui, app.skin(), title, items, |f| {
            app.render(f);
        })?;
        Ok(match signal {
            ModalSignal::Value(index) => Selection::Index(index),
            ModalSignal::Cancelled => Selection::None,
            ModalSignal::Quit => Selection::ForcedQuit,
        })
    }

    fn palette(
        &mut self,
        app: &App,
        items: &[command_palette::CommandItem<'_>],
    ) -> io::Result<Selection> {
        let signal = command_palette::command_palette(
            self.tui,
            app.skin(),
            " Commands ",
            items,
            |f| {
                app.render(f);
            },
        )?;
        Ok(match signal {
            ModalSignal::Value(index) => Selection::Index(index),
            ModalSignal::Cancelled => Selection::None,
            ModalSignal::Quit => Selection::ForcedQuit,
        })
    }

    fn may_quit(&mut self, app: &App) -> bool {
        let repaint = |frame: &mut Frame| app.render(frame);
        quit::request(self.tui, QuitKind::Soft, &repaint)
    }

    fn reload_config(
        &mut self,
        app: &App,
        checked: Result<(), ratada::reload::ConfigProblems>,
    ) -> io::Result<()> {
        ratada::reload::request(self.tui, app.skin(), checked, |frame| {
            app.render(frame);
        })
    }
}

#[cfg(test)]
pub use headless::Headless;

#[cfg(test)]
mod headless {
    use super::*;

    /// A terminal-free [`Interaction`] for tests: every dialog returns
    /// `answer`, help is a no-op unless `answer` forces a quit, and a picker
    /// returns `selection`.
    pub struct Headless {
        /// What every confirmation dialog returns.
        pub answer: Answer,
        /// What a fuzzy picker returns.
        pub selection: Selection,
        /// How many dialogs were opened.
        pub asked: usize,
        /// What the colour chooser returns.
        pub color: ColorPick,
    }

    impl Headless {
        /// Answers every confirmation with yes.
        pub fn accepting() -> Self {
            Headless {
                answer: Answer::Yes,
                selection: Selection::None,
                asked: 0,
                color: ColorPick::Cancelled,
            }
        }

        /// Answers every confirmation with no.
        pub fn declining() -> Self {
            Headless {
                answer: Answer::No,
                selection: Selection::None,
                asked: 0,
                color: ColorPick::Cancelled,
            }
        }

        /// Pretends the user hit `Ctrl+Q` inside every dialog.
        pub fn force_quitting() -> Self {
            Headless {
                answer: Answer::ForcedQuit,
                selection: Selection::ForcedQuit,
                asked: 0,
                color: ColorPick::ForcedQuit,
            }
        }

        /// Makes the colour chooser return `pick`.
        pub fn choosing_color(mut self, pick: ColorPick) -> Self {
            self.color = pick;
            self
        }

        /// Makes a fuzzy picker choose the item at `index`.
        pub fn picking(mut self, index: usize) -> Self {
            self.selection = Selection::Index(index);
            self
        }
    }

    impl Interaction for Headless {
        fn confirm(&mut self, _app: &App, _prompt: &str) -> io::Result<Answer> {
            self.asked += 1;
            Ok(self.answer)
        }

        fn help(&mut self, _app: &App) -> io::Result<Answer> {
            self.asked += 1;
            Ok(self.answer)
        }

        fn pick(
            &mut self,
            _app: &App,
            _title: &str,
            _items: &[String],
        ) -> io::Result<Selection> {
            self.asked += 1;
            Ok(self.selection)
        }

        fn palette(
            &mut self,
            _app: &App,
            _items: &[command_palette::CommandItem<'_>],
        ) -> io::Result<Selection> {
            self.asked += 1;
            Ok(self.selection)
        }

        fn pick_color(
            &mut self,
            _app: &App,
            _title: &str,
            _initial: Option<Color>,
        ) -> io::Result<ColorPick> {
            self.asked += 1;
            Ok(self.color)
        }

        fn may_quit(&mut self, _app: &App) -> bool {
            true
        }

        fn reload_config(
            &mut self,
            _app: &App,
            checked: Result<(), ratada::reload::ConfigProblems>,
        ) -> io::Result<()> {
            self.asked += 1;
            match checked {
                Ok(()) => Err(ratada::reload::signal()),
                Err(_) => Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_forced_quit_is_neither_a_yes_nor_a_no() {
        assert!(!Answer::ForcedQuit.is_yes());
        assert!(Answer::ForcedQuit.is_forced_quit());
        assert!(Answer::Yes.is_yes());
        assert!(!Answer::No.is_forced_quit());
    }

    #[test]
    fn the_toolkit_quit_signal_maps_to_a_forced_quit() {
        assert_eq!(answer_of(ModalSignal::Quit), Answer::ForcedQuit);
        assert_eq!(answer_of(ModalSignal::Value(true)), Answer::Yes);
        assert_eq!(answer_of(ModalSignal::Value(false)), Answer::No);
        assert_eq!(answer_of(ModalSignal::Cancelled), Answer::No);
    }
}
