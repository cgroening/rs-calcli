# Changelog

All notable changes to calcli are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Autocomplete in the input field: typing an identifier opens a dropdown of matching variables, built-in functions and constants. `↑`/`↓` pick, `Enter`/`Tab`/`→` accept, `Esc` closes. While it is open `↑` steers it, so `PgUp` browses the history from there.
- History search on `Ctrl+R`: a fuzzy finder over past inputs that recalls the chosen line into the input, ready to edit or resubmit.
- Command palette on `Ctrl+P`: run any action by name, filtered fuzzily. Commands unavailable in the current context are shown dimmed. The `[keys]` names and descriptions come from the same action catalog as the footer and help.
- The colour of the input box's focused frame is now configurable, as
  `border_focus` under `[appearance.colors]` or in a `[themes.<name>]`. Left out
  it follows `border`, and overriding `border` alone drags it along, so the
  focused frame never sinks into its own border.
- Three views with a tab bar – Calc, Variables and Settings – switched with `Alt+1` / `Alt+2` / `Alt+3` (`F4` still opens Variables). The Calc view is a text field, so bare digits stay typeable. The active view is restored on the next start.
- Settings view: every display setting on one editable list (`←`/`→` steps the focused value, applied live), including the colour theme and the glyph set. The function keys keep working from every view.
- Grouped shortcut hints in the footer, closing with a `Global` group. `F1` hides the whole band and the state survives a restart; the band gives way rather than squeezing the input box off a small terminal.
- Scrollable, fuzzy-filtered help overlay on `F12` or `?`, sharing its `Global` section with the footer.
- Configurable key bindings under `[keys]`, resolved against a single action catalog (`src/keymap.rs`) that also feeds the footer and the help.
- Custom colour themes under `[themes.<name>]`, palette overrides under `[appearance.colors]`, and the `confirm_delete` / `confirm_quit` switches.
- Soft quit: `q` outside the input, or the `:q` command anywhere. With `confirm_quit = true` it asks first; `Ctrl+Q` still exits at once, from anywhere.
- `docs/DEVELOPMENT.md` describing the layout, the action catalog and the on-disk compatibility rules.

### Changed

- The interface now follows the shared `clibase` panel layout: a tinted header panel carrying only the brand and the tab bar, a raised content surface, a tinted status band and backgroundless shortcut hints. The settings bar and the transient status moved into the status band. The app frame draws no border lines; rounded borders are reserved for individual widgets.
- Widgets, theming, modals, the help overlay, the terminal guard, the event loop and the clipboard now come from the shared `ratada` toolkit instead of being kept in-tree. The syntax-highlighting input editor stays app-local, because the toolkit's text widgets render plain text only.
- Modals (delete, clear, reset, quit) dim the live view behind them instead of a blank backdrop, and `Ctrl+Q` inside a modal now exits the app.
- Variables moved from an overlay to a view of their own.
- Config: the flat `[theme]` table is superseded by `[appearance]`, `[appearance.colors]` and `[highlight]`. A 0.2 `config.toml` still loads, its colours mapped onto the new sections – except for the removed `history_zebra` key (see below).
- `state.toml` is written atomically, and gained a `[ui]` section (active view, theme, glyphs, hint visibility). Older state files load unchanged.
- The domain error type no longer names files or TOML; infrastructure errors are funnelled into it at the service boundary. The service itself no longer passes error messages around as bare `String`s: `AppError` now carries every failure, and `domain::units` returns it too. The wording of every message is unchanged and pinned by a test.
- `ratatui` 0.29 → 0.30, `crossterm` 0.28 → 0.29; the `arboard` dependency was dropped in favour of the toolkit's clipboard. Minimum supported Rust version is now 1.88.

### Fixed

- A `[themes.<name>]` table naming a colour the toolkit derives (`cursor`,
  `selection`, the input fills, …) had the value silently discarded: the table
  was validated against every palette colour, but only the theme's own base
  colours were read. Such a colour is now reported and belongs in
  `[appearance.colors]`.
- A `state.toml` written before units existed – where a dimensionless variable was a bare number (`g = 9.81`) rather than a table – failed to parse, and the failure was swallowed by starting from an empty session. Upgrading silently discarded the user's settings, variables and history. Both shapes are now read; the table is written.
- Restoring the persisted theme dropped the configured colour overrides, so on every restart the block cursor turned accent-coloured and the input field turned bright. The overrides are now re-applied whenever the theme changes, at startup and when cycling it at runtime.
- The `--demo` banner told the user to press `F1` for help, a leftover from the old binding. Help is `F12` or `?`; `F1` toggles the shortcut hints.

### Removed

- **Breaking:** the `history_zebra` option, which tinted every second history entry. The separator rules between entries (`history_separator`, on by default) do the same job better. An unknown key is refused rather than ignored, so a `config.toml` that still sets `history_zebra` stops calcli from starting: delete the line. The 0.2 `[theme].history_alt_bg` key is still accepted, but no longer colours anything.
- `F1` no longer opens the help (it toggles the shortcut hints, as everywhere else in the toolkit). Help moved to `F12`, and to `?` wherever the keyboard is not in a text field.

## [0.2.1] - 2026-06-14

### Added

- Unit conversion with `->` / `to` and arithmetic with units, powered by [`rink-core`](https://crates.io/crates/rink-core): e.g. `123 MPa -> bar`, `1 l -> dm^3`, `100 km/h -> m/s`, `20 kN + 300 N`, `1 m * 2 m`, `2 kN / 4 m^2 -> kN/m^2`. `ans` and variables carry their unit.
- Short unit symbols for derived results (`kN`, `m^2`, `m/s`) instead of rink's spelled-out names; conversions and plain `value unit` literals keep the typed symbol.
- `--demo` launch flag that fills the session with sample data and leaves the saved session (`state.toml`) untouched.
- Trailing-zero trimming: drop superfluous fractional zeros on display (`12` instead of `12.000`), toggled at runtime with `F6` and via the `trim_trailing_zeros` config option (default on).
- crates.io package metadata (repository, homepage, keywords, categories, `rust-version`).

### Changed

- The units layer now uses `rink-core` in place of the small built-in unit table, adding exponent units (`cm^3`), compound units (`N/mm^2`) and dimensional arithmetic.

### Fixed

- Footer shortcut hints wrap across multiple lines instead of being cut off on narrow terminals.

## [0.2.0]

### Added

- Rewrite as a [Ratatui](https://ratatui.rs) terminal UI: editable history with recompute, stored variables, `ans` chaining, and full-`f64` precision with display-only rounding.
- Notation (decimal / scientific / SI-prefixed), angle mode (deg / rad) and decimal-separator toggles; configurable theme, colours and glyph set.
- Syntax highlighting, a live result preview / validity warning, a growing soft-wrapping multi-line input field, a full-width settings bar, and history editing (reorder, insert, delete, clear – destructive actions confirmed).
- Inline `#` comments and comment-only note lines that pass `ans` through; session persistence of settings, variables and history.
