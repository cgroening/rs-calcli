# calcli – TODO

Deferred features, in rough priority order.

## Refresh the README screenshot
`README.md` embeds `images/screenshot.png`, taken before the clibase migration. It shows an interface that no longer exists – no tab bar, no panel bands, no status band, a flat footer instead of the grouped shortcut hints. It is the first thing a reader sees.

Take a new one from `cargo run -- --demo` (the demo session fills the history with unit conversions and variables, and never touches the saved state), then replace `images/screenshot.png`.

## Config-extensible units
Let users add custom units on top of rink's database (e.g. an extra `definitions.units` snippet loaded into the context).
