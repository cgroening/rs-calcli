# calcli – TODO

Deferred features, in rough priority order.

## Short unit symbols for derived results
Arithmetic results are shown with rink's spelled-out unit names (`meter^2`,
`kilonewton`) rather than symbols (`m²`, `kN`); conversions and plain quantity
literals already keep the typed symbol. Map common rink names back to symbols
for display (the value is correct either way, and any unit can be pinned with
`->`).

## Unit-selection form (input variant 2)
An overlay to pick value / from-unit / to-unit (grouped by category) that
inserts a `value FROM -> TO` line into the history.

## Config-extensible units
Let users add custom units on top of rink's database (e.g. an extra
`definitions.units` snippet loaded into the context).
