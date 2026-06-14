# calcli – Projektregeln für Claude Code

`calcli` ist ein schneller Terminal-Taschenrechner (Ratatui-TUI) mit Verlauf,
Zwischenspeicher (Variablen) und Ingenieur-Funktionen. Zielnutzer: Entwickler,
Ingenieure. Sprache des Codes durchgehend **Englisch** (Bezeichner, Kommentare,
sichtbare Texte).

## Architektur

Schichtentrennung mit Composition Root in `main.rs`, DI über Traits (DIP).
Referenzprojekte für Stil und Aufbau: `mdtask`, `numcli`, `hop`.

- `domain/` – reine Kernlogik: `error`, `evaluator` (Trait + `MevalEvaluator`),
  `expression` (Preprocessing), `history`, `variables`, `format`. Keine I/O.
- `service/` – `calc_service`: Orchestrierung (submit/edit/delete + Recompute,
  Variablen, Settings). Keine I/O.
- `storage/` – `StateRepository`-Trait + TOML-Implementierung (`state.toml`).
- `config/` – `Config` (+ Defaults), `loader` (RawConfig-Merge + env `CALCLI_*`).
- `tui/` – Ratatui-Oberfläche (App, render, Event-Dispatch, Widgets).
- `util/` – `paths`, `logging`, `clipboard`, `app_info`.

Die Berechnungs-Engine liegt hinter dem `Evaluator`-Trait, damit später eine
Einheitenumrechnung ergänzt werden kann, ohne den Service zu ändern.

## Verbindliche Regeln (aus dem Style Guide, Rust §7)

- **Edition 2024**; `rustfmt` (Default + `group_imports=StdExternalCrate`,
  `imports_granularity=Module`); `cargo clippy -- -D warnings` muss sauber sein.
- **Fehlerbehandlung:** `Result<T, E>` + `?`. `thiserror` für Domänenfehler
  (ein Error-Typ pro Domäne), `anyhow` nur am Binary-Rand (`main`). **Kein
  `unwrap()`**; `expect()` nur an beweisbar unfehlbaren Stellen (mit Begründung).
  Kein `panic!` im Normalfluss. Eingabefehler → Statuszeile, nie Crash.
- **Typen:** `enum`s statt magischer Strings, `struct`s statt loser Tupel.
  Sinnvolle `derive`s; `Option<T>` statt Sentinel-Werten.
- **Funktionen:** klein (SRP), eine Abstraktionsebene (SLAP), max. zwei
  Verschachtelungen mit frühem Return, ≤ 3 Parameter (sonst in Struct bündeln),
  keine Flag-Argumente. Reine Funktionen bevorzugen.
- **Namen:** Prädikate `is_`/`has_`/`can_`; Methoden = Verben, Typen =
  Substantive; keine `Manager`/`Helper`/`Data`.
- **Doku:** `///` über jedem öffentlichen Item (erste Zeile = Ein-Satz-
  Zusammenfassung, Prosa statt `# Arguments`-Listen); `//!` Modul-Doc oben.
- **Zeilenlänge ≤ 80** in `.rs`; 4 Spaces; Trailing Commas in mehrzeiligen
  Listen; LF; Datei endet mit genau einem Newline.
- **Gedankenstrich:** niemals den Geviertstrich `—` (em dash) verwenden. In
  Code-Dateien `-` (Bindestrich), in `.md`-Dateien `–` (en dash).
- **Logging** über `log` (Datei-Sink in `util/logging`), nie `println!`/
  `eprintln!` für Diagnose. Sichtbare TUI-Ausgabe ist kein Logging.
- **Tests** werden immer mitgeliefert (`#[cfg(test)] mod tests` je Datei,
  Integrationstests in `tests/`); Testnamen beschreiben das erwartete Verhalten;
  Fakes vor Mocks. **Nach jeder Änderung `cargo test` laufen lassen.**
- **Dependencies** minimieren; neue Crates vorher abstimmen. Etablierte Crates
  mit `// https://crates.io/crates/<name>` über dem `use` dokumentieren.
- **Changelog pflegen:** Jede nutzersichtbare Änderung in `CHANGELOG.md` (Format
  „Keep a Changelog") unter `## [Unreleased]` eintragen; bei einem Release den
  Abschnitt versionieren/datieren und die `version` in `Cargo.toml` nach SemVer
  anheben.

## TUI-Konventionen (Style Guide §7.10)

Ein gedämpfter Akzentton (zentral in `tui/colors`), `BorderType::Rounded`,
Footer-Hint-Line, `?`-Hilfe-Overlay, transiente Statuszeile (Fehler crashen
nie), `…`-Truncation, Scrollbar bei Überlauf, zyklische Listennavigation,
geteilte `text_edit`-Logik für Eingabefelder, Glyphen Unicode/ASCII per Config.
`Ctrl+Q` beendet hart (Session speichern); `Ctrl+C` bleibt Zwischenablage.

## Domänenspezifika

- **Präzision:** intern immer der volle `f64`; Rundung nur in der Anzeige
  (`format`). Verlauf speichert Eingabe-Strings + berechnete `f64`-Werte.
- **`ans`** einer Verlaufszeile = Wert der vorhergehenden Zeile. Editieren/
  Löschen rechnet alle Zeilen darunter neu (`recompute`).
- **Trennzeichen:** Eingabe tolerant (Leerzeichen, `_`, sowie das jeweils
  andere von `.`/`,` als Tausendertrenner); Anzeige nutzt die konfigurierten
  Trenner. `y` kopiert den reinen Wert (volle Präzision, ohne Tausendertrenner),
  `Y` kopiert wie angezeigt (gerundet, mit Trennern, später Einheit).

## Spätere Erweiterungen (Architektur offen halten, jetzt nicht umsetzen)

Einheitenumrechnung (hinter `Evaluator`/Service), GUI.
