# Development

How calcli is put together, and what to touch when you change something.

## Gates

All three must pass before a change lands:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Then drive the real thing – tests do not catch a broken layout:

```sh
cargo run -- --demo
```

## Layout

Dependencies point inward, toward `domain`. Nothing below a layer knows about
the layer above it.

```text
src/
  main.rs        composition root: config, state, service, TUI, save on exit
  lib.rs         APP_NAME/APP_VERSION, and `pub use ratada::theme`
  keymap.rs      the action catalog (see below)
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
    calc_service.rs   submit / edit / delete + recompute, settings
    mod.rs            the error funnel: StorageError -> AppError::Storage

  storage/       persistence
    repository.rs   the StateRepository port and the on-disk shape
    toml_state.rs   the TOML adapter, writing atomically
    errors.rs       StorageError, which never leaves this layer

  config/        defaults -> config.toml -> CALCLI_*
    mod.rs          Config, the calcli theme, and the palette/keymap helpers
    appearance.rs   theme name, glyphs, palette overrides
    highlight.rs    the eight token colours
    loader.rs       the merge, including the 0.2 compatibility shim

  tui/           the Ratatui front-end
    app.rs         App: the Screen implementation and the key dispatch
    appframe.rs    the shared frame every view draws through
    bindings.rs    which actions each view hints at
    interaction.rs the port the blocking dialogs open through
    colors.rs      the syntax-highlight styles and the caret colours
    text_edit.rs   the highlighted, soft-wrapping editor (see below)
    views/         calc, variables, settings

  util/          fs (atomic writes), paths, logging
```

## The toolkit

The terminal toolkit comes from [`ratada`](../../../libs/ratada): the terminal
guard and event loop (`Tui`, `Screen`, `run`), the widgets (`list`, `tabs`,
`statusbar`, `scroll`, `chrome`), the blocking modals and the help overlay, the
theming (`Palette`, `Skin`, `Glyphs`, `ThemeRegistry`), the shortcut hints, the
quit machinery and the clipboard. `crate::theme` is a re-export of
`ratada::theme`, so nothing reaches for a second path.

Before writing a widget, look for it in `ratada`. If it is missing, extend the
library rather than keeping a copy here.

**The one deliberate exception** is `tui/text_edit.rs`. calcli colours its input
per character (functions, constants, operators, numbers, variables, units,
`ans`, comments), and `ratada::input::InputField` / `ratada::textarea::TextArea`
render plain text with no per-token style hook. Everything else the old calcli
had – its own colours, modals, help overlay, terminal guard and clipboard – is
gone in favour of the toolkit.

## The action catalog

`src/keymap.rs` is the single source of truth for shortcuts. Each `Action` row
carries its `[keys]` config name, its human description, its **scope** and its
default keys. From that one table come the key dispatch, the footer hints, the
help overlay and the `config` documentation.

calcli is modal, which a flat chord-to-action map cannot express: `Enter`
submits an expression in the input, applies an in-place edit, edits a line in
the history, inserts a name in the variables list and steps a value in the
settings list. So each action names a `Scope`, a key is resolved within the
scopes active in the current `Context`, and two bindings only *conflict* when
their scopes can be active at the same time. A conflict is logged, and the
earlier action in the catalog keeps the key.

Two rules the catalog enforces:

- A bare printable character never triggers an action while a text field has the
  keyboard. That is what keeps `q`, `?`, `y` and `d` typeable inside an
  expression, and why the view tabs are `Alt+N` rather than bare digits.
- `Ctrl+Q` is never bound. It is the toolkit's unconditional escape hatch, and
  it is reported by `shortcut_hints::global_bindings()` together with `F1`. Both
  appear in the closing `Global` group, which `App::global_group` builds once and
  hands to *both* the footer and the help overlay – they cannot drift apart.

When you add or rebind a shortcut, update `src/keymap.rs`, then the key tables
in `README.md` and the `[keys]` block in `examples/config.toml`. The footer and
help follow on their own.

## The app frame

`tui/appframe.rs` draws the frame every view sits in, so the app looks the same
wherever you are: a tinted header panel carrying **only** the brand and the tab
bar, the raised content surface, a tinted status band (the active settings on
the left, the transient status on the right) and the backgroundless shortcut
hints. The frame itself draws no border lines; rounded borders belong to
individual widgets (the input box, the modals).

The hint band's height comes from `shortcut_hints::height`, never a constant, so
`F1` reclaims the hint rows *and* the blank separator line above them. It is then
capped so the content always keeps `MIN_CONTENT_HEIGHT` rows: on a narrow
terminal the grouped hints would otherwise wrap far enough to squeeze the input
box off the screen.

## Dialogs and testability

`ratada`'s modals need a live `&mut Tui`, which a test cannot build – they put
the terminal into raw mode. `tui/interaction.rs` hides them behind the
`Interaction` port: production uses `Modals` (real, dimming the live view behind
them), tests use `Headless`, which answers every dialog without a terminal. That
is what lets `App::dispatch` be driven key-by-key in `tui/app.rs`'s tests.

A dialog can end in `Answer::ForcedQuit`: `Ctrl+Q` is honoured inside a modal
too, and the toolkit reports it as `ModalSignal::Quit`. Dropping that signal
would trap the user in the dialog, so it is carried back to the app rather than
folded into "the user said no".

## Colours

Two sections, two key sets, and they are not the same:

- `[appearance.colors]` overrides any **palette** colour (`ThemeColors::KEYS`
  plus the derived ones: `selection`, `cursor`, `input_bg`, …). Validated
  against `Palette::KEYS`.
- `[themes.<name>]` defines a **theme**, and a theme contributes only the base
  colours a palette is derived *from*. Validated against `ThemeColors::KEYS`.
  Naming a derived colour here is reported, not silently dropped.

The input box's focused frame is `palette.border_focus`, a toolkit colour that
follows `border` unless it is set. `tui/views/calc.rs::focus_skin` swaps it into
`border` on a `Skin` copy rather than only styling `border_style`, because
`chrome::border_title` reads the title's leading stroke from `palette.border` –
that one stroke would otherwise stay dark.

## Error messages

`AppError` (`domain/errors.rs`) is the domain's only error type, and each
variant's `Display` is exactly what the user reads in the status line and what
the history stores for a failed line. Changing the wording is a user-visible
change, not a refactor; `every_failure_mode_keeps_its_message` in
`services/calc_service.rs` pins all of them.

Two variants differ on purpose: `Calculator` prefixes meval's fragment with
"cannot evaluate: ", while `Units` renders rink's message verbatim, because rink
already phrases whole sentences that name the offending unit.

## On-disk compatibility

Every shape calcli has ever written must keep loading; `tests/legacy_data.rs`
guards this against verbatim fixtures in `tests/data/`. Two rules:

- New fields are `#[serde(default)]`, so an older file loads.
- A shape that has changed is still *read*, even if it is no longer *written*.
  A dimensionless variable used to be a bare number (`g = 9.81`) and is now a
  table; `PersistedValue` accepts both and writes the table.

This matters more than it looks: `main` treats an unreadable `state.toml` as an
empty session, so a rejected file silently discards the user's whole history.

## Tests

Unit tests live beside their code in `#[cfg(test)] mod tests`; integration tests
in `tests/`. Rendering is checked through ratatui's `TestBackend`, and the key
path through `Headless`. Names describe the expected behaviour.
