# Contributing to calcli

Thanks for taking the time to contribute. calcli is a small, focused terminal
calculator, and the bar for a change is simple: it keeps the code clean, it is
tested, and it does not regress the interface.

## Getting started

```sh
git clone https://github.com/cgroening/rs-calcli
cd rs-calcli
cargo run -- --demo
```

`--demo` fills the session with sample history and variables and never touches
your saved state, so it is the quickest way to see the interface.

## Gates

Every change must pass all three before it lands:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests do not catch a broken layout, so also drive the real thing once:

```sh
cargo run -- --demo
```

## Style

The coding standards this project follows are described in `CLAUDE.md`, and the
architecture – the layers, the action catalog and the on-disk compatibility
rules – in [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md). A few load-bearing
conventions:

- Code is English throughout: identifiers, comments and visible text.
- Errors flow through `Result` and `?`; no `unwrap()`, and `expect()` only where
  a failure is provably impossible, with a reason.
- Every public item carries a `///` doc comment (`missing_docs` is denied), and
  every module opens with a `//!` summary.
- Lines wrap at 80 columns in `.rs` files.

## Keeping things in sync

Some changes ripple into more than one file:

- A shortcut change starts in `src/keymap.rs`, then the key tables in
  `README.md` and the `[keys]` block in `examples/config.toml` are updated by
  hand. The footer and the help overlay follow the catalog on their own.
- Any user-visible change is recorded in `CHANGELOG.md` under `## [Unreleased]`,
  following [Keep a Changelog](https://keepachangelog.com/).
- New on-disk fields are `#[serde(default)]` so older files keep loading;
  `tests/legacy_data.rs` guards both files against real fixtures.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)
and are written in the imperative (`add …`, `fix …`, `refactor …`). Keep a pull
request focused on one change, describe what it does and why, and make sure the
gates are green.

## Code of conduct

Participation in this project is governed by the
[Code of Conduct](CODE_OF_CONDUCT.md).
