# calcli

A fast terminal calculator (TUI) with an editable history, stored variables and
a few helpers engineers reach for. Built with [Ratatui](https://ratatui.rs).

Type an expression, press Enter, keep going. The previous result is available as
`ans`, every line stays in an editable history (editing a line recomputes
everything below it), and results keep full `f64` precision internally even when
the display is rounded.

## Features

- **History with recompute** — each entry shows the input with its result on
  the line below (both soft-wrap long content). Edit any earlier line and every
  line below is re-evaluated, so `ans` chains stay correct.
- **Full precision** — the rounded value you see is only for display; further
  math always uses the exact internal value.
- **Variables** — save with `=name` (stores the last answer) or `name = expr`.
  Manage them in an overlay: insert, copy, delete, reset all. Persisted.
- **Notation** — cycle decimal / scientific / SI-prefixed (`F2`); trailing
  fractional zeros are dropped by default (`12`, not `12.000`), toggle with `F6`
  or the `trim_trailing_zeros` config key.
- **Angle mode** — toggle degrees / radians for trig (`F3`).
- **Lenient input** — spaces, `_` and the non-decimal one of `.`/`,` are
  accepted as thousands separators; SI prefixes like `3.3k`, `100u` are
  expanded. Function arguments use `;` (e.g. `max(1;2)`).
- **Clipboard** — `y` copies the plain, full-precision value (no grouping); `Y`
  copies it as shown (rounded, grouped).
- **Growing input** — long expressions soft-wrap and the input field grows
  (up to `input_max_lines`); `↑/↓` move the caret across the wrapped lines,
  `↑` on the first line enters the history.
- **Syntax highlighting** — functions, constants, operators (bold), numbers,
  defined variables, parentheses and `ans` are coloured in the input, the
  history and while editing. Colours are configurable in the `[theme]` table;
  unknown identifiers stay neutral.
- **Live feedback** — as you type, the input border shows a dim `= value`
  preview while the expression is valid and a subtle warning marker when it
  looks complete but won't parse (silent while still mid-typing). Toggle with
  `live_feedback` in the config.
- **Persistence** — settings, variables and history are saved on exit and
  restored next time (settings restore is configurable).

## Install

```sh
cargo install --path .
# or run from the repo
cargo run --release
```

Start with `--demo` to fill the session with sample data (history, variables,
unit conversions) for a quick tour — demo mode never writes to your saved
session:

```sh
calcli --demo
```

## Keys

### Input
| Key | Action |
| --- | --- |
| `Enter` | evaluate the expression |
| `↑` | enter the history |
| `Ctrl+Y` | copy the last result (plain) |
| `Ctrl+C` / `X` / `V` | copy / cut / paste in the input |
| `Esc` | clear the input |

### History (after `↑`)
| Key | Action |
| --- | --- |
| `↑` `↓` / `PgUp` `PgDn` / `Home` `End` | move the selection |
| `Alt+↑` / `Alt+↓` | move the selected line up / down (recomputes) |
| `o` / `O` | insert a line below / above and edit it |
| `Enter` / `e` | edit the selected line (recomputes below) |
| `d` / `Del` | delete the selected line (asks first) |
| `Shift+D` | clear the whole history (asks first) |
| `y` | copy the value (plain, full precision) |
| `Y` | copy the value (as shown, grouped) |
| `Esc` | back to the input |

Reordering, inserting and deleting all re-evaluate the affected lines, so `ans`
and variable assignments stay consistent with the new order.

### Variables (`F4`)
| Key | Action |
| --- | --- |
| `↑` `↓` | select |
| `Enter` | insert the name into the input |
| `y` / `Y` | copy the value |
| `d` | delete the variable |
| `R` | reset all variables |
| `Esc` | close |

### Global
| Key | Action |
| --- | --- |
| `F1` | toggle help |
| `F2` | cycle notation (dec / sci / SI) |
| `F3` | toggle angle mode (deg / rad) |
| `F4` | variables overlay |
| `F5` | toggle decimal separator (`.` / `,`) |
| `F6` | toggle trailing-zero trimming |
| `Ctrl+Q` | quit (saving the session) |

### Typed commands (in the input)
`:d[n]`, `:s[n]`, `:si[n]` set notation (and optional decimals); `:deg` / `:rad`
set the angle mode; `:clear` clears the history.

## Expressions

Backed by [`meval`](https://crates.io/crates/meval): `+ - * / ^` (and `**` for
power), parentheses, constants `pi`, `e`, and functions such as `sqrt`, `exp`,
`ln`, `abs`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`/`cosh`/`tanh`,
`floor`, `ceil`, `round`, `min`, `max`, `atan2`. Trig respects the angle mode.

- `ans` — the previous result. A line starting with an operator (e.g. `+5`)
  continues from it automatically.
- `=name` — save the previous answer to `name`.
- `name = expr` — evaluate and store. `ans`, `pi` and `e` are reserved.
- SI prefixes on numbers: `k M G T m µ u n p` (e.g. `4.7k` → `4700`).
- Units, conversion & arithmetic: write a quantity as `value unit`
  (space-separated, e.g. `123 MPa`) and convert with `->` (or `to`):
  `123 MPa -> bar`, `1 l -> dm^3`, `100 km/h -> m/s`, `ans -> psi`, `x = 50 kN`
  then `x -> N`. You can also calculate *with* units — `20 kN + 300 N` →
  `20.3 kilonewton`, `1 m + 50 cm` → `1.5 meter`, `1 m * 2 m` → `2 meter^2`,
  `2 kN / 4 m^2 -> kN/m^2` → `0.5 kN/m^2`. `ans` and variables keep their unit.
  Units are powered by [`rink-core`](https://crates.io/crates/rink-core); a
  conversion is shown in the unit you typed, while a derived result uses rink's
  unit name (pin any unit with `->`). Exponents use `^` (`cm^3`, `m^2`).
- Comments: everything after `#` is ignored by the calculation but kept in the
  history (e.g. `2*pi*r  # circumference`). A line that is only a comment is
  kept as a note (no result); notes don't break the `ans` chain.

## Configuration

`calcli` reads `config.toml` from `$XDG_CONFIG_HOME/calcli/` (or
`~/.config/calcli/`). Every key is optional. See
[`examples/config.toml`](examples/config.toml) for the full list with defaults
(notation, decimals, angle mode, separators, max history, glyph set, the
`restore_last_settings` and `live_feedback` switches, and the accent and
syntax-highlight colours).

The session (settings, variables, history) is stored in `state.toml` under
`$XDG_STATE_HOME/calcli/` (or `~/.local/state/calcli/`); see
[`examples/state.toml`](examples/state.toml). History values are recomputed on
load so the `ans` chain stays consistent with the active settings.

> The `glyphs = "ascii"` option currently switches the warning marker to ASCII;
> borders and a few separators use broadly-compatible Unicode box-drawing and
> middle-dot glyphs.

## Architecture

Layered, with the composition root in `main.rs`:

- `domain/` — pure core: evaluation (`Evaluator` trait + meval), the `units`
  engine (rink), expression preprocessing, number formatting, variables, history
  (and its replay).
- `service/` — `CalcService`: orchestration (submit / edit / delete + recompute,
  variables, settings).
- `storage/` — `StateRepository` port + TOML implementation (`state.toml`).
- `config/` — `Config` and its loader.
- `tui/` — the Ratatui front-end.
- `util/` — paths, logging, clipboard, app metadata.

Dimensionless math runs through the `Evaluator` trait (meval), keeping full
`f64` precision and the angle mode; anything involving units is routed to the
`domain::units` wrapper around `rink-core`, which owns conversion and unit
arithmetic.

## Development

```sh
cargo test                  # unit + integration tests
cargo clippy --all-targets  # must be warning-free
```

See `CLAUDE.md` for the coding standards this project follows.
