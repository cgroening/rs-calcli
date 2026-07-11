# calcli – project rules for Claude Code

`calcli` is a fast terminal calculator (Ratatui TUI) with history, scratch storage (variables) and engineering functions. Target users: developers, engineers. Code language throughout **English** (identifiers, comments, visible text).

## Architecture

Layer separation with the composition root in `main.rs`, DI via traits (DIP). calcli follows the `clibase` template; reference projects for style and structure: `clibase`, `mdtask`, `numcli`, `hop`.

- `domain/` – pure core logic: `errors` (`AppError`), `evaluator` (trait + `MevalEvaluator`), `units` (rink), `expression`, `format`, `highlight`, `completion` (suggestion names + word under the cursor, shares the name lists with `highlight`), `history`, `quantity`, `variables`. No I/O.
- `services/` – `calc_service`: orchestration (submit/edit/delete + recompute, variables, settings). No I/O. The **error funnel** also lives here: `StorageError`/`ConfigError` are turned into `AppError::Storage` (the cause chain is flattened into the message).
- `storage/` – `StateRepository` trait + TOML implementation (`state.toml`, written atomically) + `errors` (`StorageError`, never leaves the layer).
- `config/` – `Config` (+ defaults), `appearance`, `highlight`, `loader` (default → TOML → env `CALCLI_*`), incl. compatibility shim for 0.2.
- `keymap.rs` – action catalog: **SSOT** for key dispatch, footer hints, help overlay and the `[keys]` names in the config.
- `tui/` – Ratatui surface: `appframe` (shared app frame), `app` (`ratada::Screen`), `bindings`, `interaction` (port for blocking dialogs), `colors`, `text_edit`, `views/{calc,variables,settings}`.
- `util/` – `fs` (atomic writes), `paths`, `logging`.

The computation engine sits behind the `Evaluator` trait; units run through `domain::units` (rink-core).

## Toolkit: `ratada`

Widgets, theming, modals, help overlay, terminal guard, event loop, shortcut hints, quit logic and clipboard come from `ratada` (path dependency). `crate::theme` is a re-export of `ratada::theme`. **Do not rebuild a widget that already exists there** – if one is missing, extend the lib (agree on it beforehand), no app-local copy.

The only deliberate exception: `tui/text_edit.rs`. calcli highlights its input character by character with color; `ratada::input::InputField` and `ratada::textarea::TextArea` render only plain text.

## Mandatory rules (from the Style Guide, Rust §8)

- **Edition 2024**, MSRV 1.88 (`ratada` uses let-chains); `rustfmt` (default + `group_imports=StdExternalCrate`, `imports_granularity=Module`); `cargo clippy --all-targets -- -D warnings` must be clean.
- **Error handling:** `Result<T, E>` + `?`. `thiserror` for domain errors (one error type per domain), `anyhow` only at the binary edge (`main`). **No `unwrap()`**; `expect()` only in provably infallible places (with a rationale). No `panic!` in the normal flow. Input errors → status line, never a crash.
- **Types:** `enum`s instead of magic strings, `struct`s instead of loose tuples. Sensible `derive`s; `Option<T>` instead of sentinel values.
- **Functions:** small (SRP), one level of abstraction (SLAP), at most two levels of nesting with early return, ≤ 3 parameters (otherwise bundle in a struct), no flag arguments. Prefer pure functions.
- **Names:** predicates `is_`/`has_`/`can_`; methods = verbs, types = nouns; no `Manager`/`Helper`/`Data`.
- **Docs:** `///` above every public item (first line = one-sentence summary, prose instead of `# Arguments` lists); `//!` module doc at the top.
- **Line length ≤ 80** in `.rs`; 4 spaces; trailing commas in multi-line lists; LF; file ends with exactly one newline.
- **Dash:** never use the em dash. In code files `-` (hyphen), in `.md` files `–` (en dash).
- **Logging** via `log` (file sink in `util/logging`), never `println!`/ `eprintln!` for diagnostics. Visible TUI output is not logging.
- **Tests** are always shipped along (`#[cfg(test)] mod tests` per file, integration tests in `tests/`); test names describe the expected behavior; fakes over mocks. **Run `cargo test` after every change.**
- **Minimize dependencies**; agree on new crates beforehand. Document established crates with `// https://crates.io/crates/<name>` above the `use`.
- **Maintain the changelog:** enter every user-visible change in `CHANGELOG.md` (format "Keep a Changelog") under `## [Unreleased]`; on a release, version/date the section and bump the `version` in `Cargo.toml` per SemVer.
- **Shortcut changes:** adjust `keymap.rs`, then update the key tables in `README.md` and the `[keys]` block in `examples/config.toml` accordingly. Footer and help follow automatically.
- **On-disk formats backward compatible:** new fields `#[serde(default)]`. `tests/legacy_data.rs` guards both files against real fixtures. They fail differently, so different rules apply:
  - `state.toml` must keep *reading* **every form ever written**, even if it is no longer *written*. `main` treats an unreadable `state.toml` as an empty session – a rejected file silently discards settings, variables and the entire history.
  - `config.toml` **may lose a key**: `deny_unknown_fields` rejects the file, `main` prints the cause chain and aborts. Nothing is lost, the message names the line. This is a user-visible, breaking change and belongs in the `CHANGELOG.md` – not a refactoring.

## TUI conventions (Style Guide §8.10)

Panel layout from `clibase`: tinted header panel with **only** brand + tab bar, raised content area, tinted status band (the active settings on the left, the transient status message on the right), below it a blank line and the background-less shortcut hints. The app frame draws **no** border lines; `BorderType::Rounded` applies only to individual widgets (input field, modals). Everything is drawn via `tui::appframe::render_frame` (SSOT).

- **Footer hints** via `ratada::shortcut_hints`, grouped per view into named `HintGroup`s. The **last group is called `Global`** and comes from `App::global_group` – the same helper feeds the footer *and* the help overlay.
- **Hint band height never as a constant:** `shortcut_hints::height(...)`, additionally capped so the content keeps `MIN_CONTENT_HEIGHT` lines.
- **Never hardcode `Ctrl+Q` anywhere** – neither in the footer nor in the help nor as a `[keys]` binding. It belongs to the toolkit, as does `F1` (toggle hints).
- **Modals block** (`ratada::modal`) and dim the *live* view. A `Ctrl+Q` in a modal quits the app (`Answer::ForcedQuit`).
- **`Ctrl+C` stays the clipboard** and is never assigned as quit.
- Transient status line (errors never crash), `…` truncation, scrollbar on overflow, cyclic list navigation, Unicode/ASCII glyphs per config.

## Modal keys: scopes

calcli is modal (`Enter` means something different in every context). That is why every `Action` in `keymap.rs` carries a `Scope`; a chord is resolved only in the scopes that are active in the current `Context`. Two bindings collide only when their scopes can be active at the same time.

A **bare printable character never triggers an action** as long as a text field has the keyboard – only that way do `q`, `?`, `y`, `d` stay typable. For the same reason the tab keys are `Alt+1..3` instead of bare digits.

## Domain specifics

- **Precision:** internally always the full `f64`; rounding only in the display (`format`). History stores input strings + computed `f64` values.
- **`ans`** of a history line = value of the preceding line. Editing/ deleting recomputes all lines below it (`recompute`).
- **Separators:** input is tolerant (spaces, `_`, as well as whichever of `.`/`,` is the other one as thousands separator); display uses the configured separators. `y` copies the raw value (full precision, without thousands separator), `Y` copies as displayed (rounded, with separators, later unit).
- **Dialogs via the `Interaction` port** (`tui/interaction.rs`): in production `Modals`, in tests `Headless`. Only that way is the key path testable without a terminal.

## Later extensions (keep the architecture open, do not implement now)

CLI layer (`cli/` + `sparcli`), GUI. calcli is deliberately TUI-only: there are no subcommands and no `println!` output – `sparcli` is therefore not included.

## Error messages are behavior

`AppError` (`domain/errors.rs`) is the domain's only error type; the `Display` of each variant is exactly what the user reads in the status line and what the history stores for a failed line. Changing a message is a user-visible change, not a refactoring – `AppError::Units` therefore passes rink's wording through unchanged (rink phrases whole sentences), while `AppError::Calculator` prefixes the fragment from meval. `every_failure_mode_keeps_its_message` in `calc_service.rs` freezes all messages.
