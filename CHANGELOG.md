# Changelog

All notable changes to calcli are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- Syntax highlighting, a live result preview / validity warning, a growing soft-wrapping multi-line input field, a full-width settings bar, and history editing (reorder, insert, delete, clear — destructive actions confirmed).
- Inline `#` comments and comment-only note lines that pass `ans` through; session persistence of settings, variables and history.
