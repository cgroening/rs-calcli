# calcli – TODO

Deferred features, in rough priority order.

## Unit-aware arithmetic
Calculating *with* units (the conversion-only support already exists via `->`):
- Same dimension `+` / `-` with prefix scaling, e.g. `20 kN + 300 N` → `20.3 kN`
  (result in the first operand's unit).
- Scalar `*` / `/`, e.g. `2 * 20 kN`, `5 kN / 2`.
- Full dimensional analysis afterwards: `5 m * 3 m` → `15 m²`,
  `force / area` → pressure, derived units.

## Unit-selection form (input variant 2)
An overlay to pick value / from-unit / to-unit (grouped by category) that
inserts a `value FROM -> TO` line into the history.

## Compound / derived units
Units written with `/` such as `km/h`, `m/s`, `N/mm²`.

## Config-extensible units
Let users add custom units (symbol, factor, dimension) via `config.toml`.
