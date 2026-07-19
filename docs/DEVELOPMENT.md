# Development

How calcli is put together, and what to touch when you change something.

## Gates

All three must pass before a change lands:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo deny check` runs the advisory and licence audit against `deny.toml`. It is not installed by default (`cargo install cargo-deny`) and there is no CI workflow in this repository, so it is a local step.

Then drive the real thing – tests do not catch a broken layout:

```sh
cargo run -- --demo
```

## Layout

Dependencies point inward, toward `domain`. Nothing below a layer knows about the layer above it.

```text
src/
  main.rs        composition root: config, state, service, TUI, save on exit
  lib.rs         APP_NAME/APP_VERSION, and `pub use ratada::theme`
  keymap/        mod (Context, Scope, Keymap) + catalog (the action table)
  demo.rs        the --demo session

  domain/        pure, no I/O
    errors.rs      AppError: only domain failures
    evaluator.rs   the Evaluator trait + the meval implementation
    units.rs       the rink-core wrapper (conversion, unit arithmetic)
    expression.rs  preprocessing, `ans`, assignments, comments
    format.rs      how a value is rendered
    highlight.rs   per-character token classification
    history.rs     the entries and their replay
    quantity.rs    a value with an optional unit
    variables.rs   the store

  services/      orchestration, no I/O
    calc_service.rs   the façade: submit / edit / delete + recompute, settings
    eval.rs           the evaluation router: meval vs. rink, unit literals
    mod.rs            the error funnel: StorageError -> AppError::Storage

  storage/       persistence
    repository.rs   the StateRepository port and the on-disk shape
    toml_state.rs   the TOML adapter, writing atomically
    errors.rs       StorageError, which reaches no layer above the service

  config/        defaults -> config.toml -> CALCLI_*
    mod.rs          Config, the calcli theme, and the palette/keymap helpers
    appearance.rs   theme name, glyphs, palette overrides
    highlight.rs    the eight token colours
    loader/         mod (entry points + merge), raw (the on-disk shape),
                    legacy (the 0.2 shim), validate (unknown colours), env

  tui/           the Ratatui front-end
    app/           App: the state in mod.rs, the handlers in child modules
                   (dispatch, render, hints, calc_input, calc_history,
                   variables, settings, clipboard) - see below
    appframe.rs    the shared frame every view draws through
    bindings.rs    which actions each view hints at
    interaction.rs the port the blocking dialogs open through
    colors.rs      the syntax-highlight styles and the caret colours
    text_edit.rs   the per-character colouring of a wrapped value (see below)
    views/         calc/ (mod, history, input, completion), variables, settings

  util/          fs (atomic writes), paths, logging
```

## The toolkit

The terminal toolkit comes from [`ratada`](../../../libs/ratada): the terminal guard and event loop (`Tui`, `Screen`, `run`), the widgets (`list`, `tabs`, `statusbar`, `scroll`, `chrome`), the blocking modals and the help overlay, the theming (`Palette`, `Skin`, `Glyphs`, `ThemeRegistry`), the shortcut hints, the chord grammar (`keymap`), the modifier rules (`input::is_command`, `input::is_bare_character`), the quit machinery and the clipboard. `crate::theme` is a re-export of `ratada::theme`, so nothing reaches for a second path.

Before writing a widget, look for it in `ratada`. If it is missing, extend the library rather than keeping a copy here.

**The one deliberate exception** is `tui/text_edit.rs`, and it covers the *rendering* only. calcli colours its input per character (functions, constants, operators, numbers, variables, units, `ans`, comments), and `ratada::input::InputField` / `ratada::textarea::TextArea` render plain text with no per-token style hook. Everything else the old calcli had – its own colours, modals, help overlay, terminal guard and clipboard – is gone in favour of the toolkit.

The module used to be a fork of the whole edit core: its own `TextCursor`, `EditMode`, `apply_edit_key`, `handle_clipboard`, `replace_selection` and soft-wrap arithmetic, roughly 400 of its 685 lines duplicating `ratada::input` and `ratada::textarea`. That fork is gone; what those names refer to now are re-exports, and the file is down to the span builders that actually need to exist here.

Removing it needed one addition to the toolkit. The input holds an **expression**, which is one logical line by definition while being too long to sit on one row. `EditMode::SingleLine` navigates wrongly there (`Home`/`End` would jump the whole value rather than the display line), and `EditMode::Multiline` lets `Enter` and a pasted newline put a `\n` into a buffer that must never contain one. `ratada::input::EditMode::Wrapped` is the mode that sits between them: it navigates like `Multiline` and keeps `SingleLine`'s newline rule.

The key rules in particular are never reimplemented here: `ratada::input::is_command` decides whether a key is a chord, rather than a hand-rolled `CONTROL` test. A terminal reports `AltGr` as Control+Alt, so a plain Control check swallows the characters it produces (`@`, `\`, `[`, `~` on a German layout) instead of typing them.

## The action catalog

`src/keymap.rs` is the single source of truth for shortcuts. Each `Action` row carries its `[keys]` config name, its human description, its **scope** and its default keys. From that one table come the key dispatch, the footer hints, the help overlay and the `config` documentation.

The chord grammar underneath is not calcli's: parsing a `[keys]` string, rendering a key, merging the defaults with the overrides and detecting a shadowed binding all live in `ratada::keymap`, which every app on the toolkit shares. calcli implements `ratada::keymap::Action` for its own enum and keeps what is genuinely its own – the catalog, the scopes and the lookup.

calcli is modal, which a flat chord-to-action map cannot express: `Enter` submits an expression in the input, applies an in-place edit, edits a line in the history, inserts a name in the variables list and steps a value in the settings list. So each action names a `Scope`, and a key is resolved within the scopes active in the current `Context`. `Keymap` is therefore a newtype around `ratada::keymap::Keymap<Action>` rather than a bare alias: the toolkit holds the bindings, while the context-aware lookup stays here, where `Scope` lives.

The same split governs conflicts. Two bindings only *conflict* when their scopes can be active at the same time – a rule the toolkit cannot know, so `Action::overlaps` hands it down and the toolkit's check applies it. A conflict is logged, and the earlier action in the catalog keeps the key.

Two rules the catalog enforces:

- A bare printable character never triggers an action while a text field has the keyboard. That is what keeps `q`, `?`, `y` and `d` typeable inside an expression, and why the view tabs are `Alt+N` rather than bare digits. The test is `ratada::input::is_bare_character`, re-exported here; it is stricter than "not a chord", because an `AltGr` character must type rather than dispatch.
- `Ctrl+Q` is never bound. It is the toolkit's unconditional escape hatch, and it is reported by `shortcut_hints::global_bindings()` together with `F1`. Both appear in the closing `Global` group, which `App::global_group` builds once and hands to *both* the footer and the help overlay – they cannot drift apart.

When you add or rebind a shortcut, update `src/keymap/catalog.rs`, then the key tables in `README.md` and the `[keys]` block in `examples/config.toml`. The footer and help follow on their own – and `tests/docs_in_sync.rs` fails if the two documents do not, so this is a checked requirement rather than a request.

## The app frame

`tui/appframe.rs` draws the frame every view sits in, so the app looks the same wherever you are: a tinted header panel carrying **only** the brand and the tab bar, the raised content surface, a tinted status band (the active settings on the left, the transient status on the right) and the backgroundless shortcut hints. The frame itself draws no border lines; rounded borders belong to individual widgets (the input box, the modals).

The hint band's height comes from `shortcut_hints::height`, never a constant, so `F1` reclaims the hint rows *and* the blank separator line above them. It is then capped so the content always keeps `MIN_CONTENT_HEIGHT` rows: on a narrow terminal the grouped hints would otherwise wrap far enough to squeeze the input box off the screen.

## Dialogs and testability

`ratada`'s modals need a live `&mut Tui`, which a test cannot build – they put the terminal into raw mode. `tui/interaction.rs` hides them behind the `Interaction` port: production uses `Modals` (real, dimming the live view behind them), tests use `Headless`, which answers every dialog without a terminal. That is what lets `App::dispatch` be driven key-by-key in `tui/app.rs`'s tests.

A dialog can end in `Answer::ForcedQuit`: `Ctrl+Q` is honoured inside a modal too, and the toolkit reports it as `ModalSignal::Quit`. Dropping that signal would trap the user in the dialog, so it is carried back to the app rather than folded into "the user said no".

The history finder (`Ctrl+R`) and the command palette (`Ctrl+P`) go through the same port: `Interaction::pick` and `Interaction::palette` return a `Selection` (`Index`, `None` or `ForcedQuit`), the picker analogue of `Answer`. `Headless` returns a configurable `Selection`, so both flows are driven terminal-free in the tests. The palette's items are built from the action catalog itself: the label is each action's description, the category comes from `bindings::category_of`, and an action unavailable in the current context is passed as `enabled: false`, so the toolkit dims it and refuses to select it.

## Input completion

The suggestion dropdown under the input field is `ratada::autocomplete`, driven from pure helpers in `domain/completion.rs`: `candidates` lists the current variables plus the built-in names (the lists shared with the highlighter), and `identifier_before` finds the word under the caret to match against and replace. Its keys are widget-internal, so they stay out of the keymap; `App::dispatch` lets an open dropdown consume `↑`/`↓`/`Enter`/`Esc` before the normal key path, and rebuilds it after every edit. `PgUp` still enters the history while the dropdown owns `↑`.

## Colours

Two sections, two key sets, and they are not the same:

- `[appearance.colors]` overrides any **palette** colour (`ThemeColors::KEYS` plus the derived ones: `selection`, `cursor`, `input_bg`, …). Validated against `Palette::KEYS`.
- `[themes.<name>]` defines a **theme**, and a theme contributes only the base colours a palette is derived *from*. Validated against `ThemeColors::KEYS`. Naming a derived colour here is reported, not silently dropped.

The input box's focused frame is `palette.border_focus`, a toolkit colour that follows `border` unless it is set. `tui/views/calc.rs::focus_skin` swaps it into `border` on a `Skin` copy rather than only styling `border_style`, because `chrome::border_title` reads the title's leading stroke from `palette.border` – that one stroke would otherwise stay dark.

## Configuration

`config/loader.rs` builds the `Config` in three layers, each overriding the one before it: the built-in defaults, then `config.toml` if it exists (a missing file is not an error), then the `CALCLI_*` environment variables (`CALCLI_THEME`, `CALCLI_DECIMALS`, `CALCLI_ACCENT`, …). `examples/config.toml` is the fully commented reference for every key. It is held to the defaults by `the_shipped_example_config_parses_and_states_the_defaults` in `config/loader/mod.rs`, and to the action catalog by `tests/docs_in_sync.rs`; neither can rot unnoticed.

Two things the loader is strict about live in their own sections: the colour tables (see `Colours`) and the refusal of unknown keys (see `On-disk compatibility`).

## Error messages

`AppError` (`domain/errors.rs`) is the domain's only error type, and each variant's `Display` is exactly what the user reads in the status line and what the history stores for a failed line. Changing the wording is a user-visible change, not a refactor; `every_failure_mode_keeps_its_message` in `services/calc_service.rs` pins all of them.

Two variants differ on purpose: `Calculator` prefixes meval's fragment with "cannot evaluate: ", while `Units` renders rink's message verbatim, because rink already phrases whole sentences that name the offending unit.

## On-disk compatibility

`tests/legacy_data.rs` guards the two on-disk files against verbatim fixtures in `tests/data/`. New fields are `#[serde(default)]` in both, so an older file loads. Beyond that the two files answer to different rules, because they fail differently.

**`state.toml` must read every shape calcli has ever written.** A shape that has changed is still *read*, even if it is no longer *written*: a dimensionless variable used to be a bare number (`g = 9.81`) and is now a table, and `PersistedValue` accepts both. This is not politeness. `main` treats an unreadable `state.toml` as an empty session, so a rejected file silently discards the user's settings, variables and whole history.

**`config.toml` may lose a key.** `RawConfig` carries `deny_unknown_fields`, so a key that no longer exists refuses the file, `main` prints the cause chain and exits non-zero. Nothing is lost and the message names the offending line, which the user deletes. `history_zebra` went this way, and the two `a_0_2_config_file_*` tests pin both halves: the old file is rejected *by name*, and the same file minus that line still loads with its colours mapped.

Removing a config key is therefore a breaking change to be announced in `CHANGELOG.md`, not a silent one to be papered over.

## Logging

Diagnostics go through the `log` crate, never `println!`/`eprintln!`: a TUI owns the terminal, and writing to stderr would corrupt the alternate screen. The backend is `util/logging.rs`, a minimal `Log` implementation that appends to a file. `main` installs it once at `Info` level, pointed at `paths::log_file()`; when no file can be opened the logger stays a silent no-op, so logging is never fatal. Visible TUI output (the status band, the history) is not logging.

## Tests

Unit tests live beside their code in `#[cfg(test)] mod tests`; integration tests in `tests/`. Rendering is checked through ratatui's `TestBackend`, and the key path through `Headless`. Names describe the expected behaviour.
