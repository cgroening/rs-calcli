# calcli – Projektregeln für Claude Code

`calcli` ist ein schneller Terminal-Taschenrechner (Ratatui-TUI) mit Verlauf,
Zwischenspeicher (Variablen) und Ingenieur-Funktionen. Zielnutzer: Entwickler,
Ingenieure. Sprache des Codes durchgehend **Englisch** (Bezeichner, Kommentare,
sichtbare Texte).

## Architektur

Schichtentrennung mit Composition Root in `main.rs`, DI über Traits (DIP).
calcli folgt der `clibase`-Vorlage; Referenzprojekte für Stil und Aufbau:
`clibase`, `mdtask`, `numcli`, `hop`.

- `domain/` – reine Kernlogik: `errors` (`AppError`), `evaluator` (Trait +
  `MevalEvaluator`), `units` (rink), `expression`, `format`, `highlight`,
  `completion` (Vorschlagsnamen + Wort unter dem Cursor, teilt die Namenslisten
  mit `highlight`), `history`, `quantity`, `variables`. Keine I/O.
- `services/` – `calc_service`: Orchestrierung (submit/edit/delete + Recompute,
  Variablen, Settings). Keine I/O. Hier liegt auch der **Fehler-Trichter**:
  `StorageError`/`ConfigError` werden zu `AppError::Storage` (Cause-Chain wird
  in die Meldung geflacht).
- `storage/` – `StateRepository`-Trait + TOML-Implementierung (`state.toml`,
  atomar geschrieben) + `errors` (`StorageError`, verlässt die Schicht nie).
- `config/` – `Config` (+ Defaults), `appearance`, `highlight`, `loader`
  (Default → TOML → env `CALCLI_*`), inkl. Kompatibilitäts-Shim für 0.2.
- `keymap.rs` – Action-Katalog: **SSOT** für Tasten-Dispatch, Footer-Hints,
  Hilfe-Overlay und die `[keys]`-Namen der Config.
- `tui/` – Ratatui-Oberfläche: `appframe` (gemeinsamer App-Rahmen), `app`
  (`ratada::Screen`), `bindings`, `interaction` (Port für blockierende Dialoge),
  `colors`, `text_edit`, `views/{calc,variables,settings}`.
- `util/` – `fs` (atomare Writes), `paths`, `logging`.

Die Berechnungs-Engine liegt hinter dem `Evaluator`-Trait; Einheiten laufen über
`domain::units` (rink-core).

## Toolkit: `ratada`

Widgets, Theming, Modals, Hilfe-Overlay, Terminal-Guard, Event-Loop, Shortcut-
Hints, Quit-Logik und Zwischenablage kommen aus `ratada` (Pfad-Dependency).
`crate::theme` ist ein Re-Export von `ratada::theme`. **Kein Widget nachbauen,
das es dort schon gibt** – fehlt eines, die Lib erweitern (vorher abstimmen),
keine App-lokale Kopie.

Einzige bewusste Ausnahme: `tui/text_edit.rs`. calcli hebt seine Eingabe
zeichenweise farblich hervor; `ratada::input::InputField` und
`ratada::textarea::TextArea` rendern nur schmucklosen Text.

## Verbindliche Regeln (aus dem Style Guide, Rust §8)

- **Edition 2024**, MSRV 1.88 (`ratada` nutzt let-chains); `rustfmt` (Default +
  `group_imports=StdExternalCrate`, `imports_granularity=Module`);
  `cargo clippy --all-targets -- -D warnings` muss sauber sein.
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
- **Gedankenstrich:** niemals den Geviertstrich (em dash) verwenden. In
  Code-Dateien `-` (Bindestrich), in `.md`-Dateien `–` (en dash).
- **Logging** über `log` (Datei-Sink in `util/logging`), nie `println!`/
  `eprintln!` für Diagnose. Sichtbare TUI-Ausgabe ist kein Logging.
- **Tests** werden immer mitgeliefert (`#[cfg(test)] mod tests` je Datei,
  Integrationstests in `tests/`); Testnamen beschreiben das erwartete Verhalten;
  Fakes vor Mocks. **Nach jeder Änderung `cargo test` laufen lassen.**
- **Dependencies** minimieren; neue Crates vorher abstimmen. Etablierte Crates
  mit `// https://crates.io/crates/<name>` über dem `use` dokumentieren.
- **Changelog pflegen:** Jede nutzersichtbare Änderung in `CHANGELOG.md` (Format
  "Keep a Changelog") unter `## [Unreleased]` eintragen; bei einem Release den
  Abschnitt versionieren/datieren und die `version` in `Cargo.toml` nach SemVer
  anheben.
- **Shortcut-Änderungen:** `keymap.rs` anpassen, dann die Tastentabellen in
  `README.md` und den `[keys]`-Block in `examples/config.toml` nachziehen.
  Footer und Hilfe folgen automatisch.
- **On-Disk-Formate rückwärtskompatibel:** neue Felder `#[serde(default)]`.
  `tests/legacy_data.rs` sichert beide Dateien gegen echte Fixtures. Sie
  scheitern unterschiedlich, also gelten unterschiedliche Regeln:
  - `state.toml` muss **jede je geschriebene Form** weiter *lesen*, auch wenn
    sie nicht mehr *geschrieben* wird. `main` behandelt eine unlesbare
    `state.toml` als leere Session – eine abgelehnte Datei verwirft
    stillschweigend Einstellungen, Variablen und die ganze Historie.
  - `config.toml` **darf einen Schlüssel verlieren**: `deny_unknown_fields`
    lehnt die Datei ab, `main` druckt die Cause-Chain und bricht ab. Nichts geht
    verloren, die Meldung nennt die Zeile. Das ist eine nutzersichtbare,
    brechende Änderung und gehört in den `CHANGELOG.md` – kein Refactoring.

## TUI-Konventionen (Style Guide §8.10)

Panel-Layout aus `clibase`: getöntes Header-Panel mit **nur** Brand + Tab-Bar,
erhabene Content-Fläche, getöntes Status-Band (links die aktiven Settings,
rechts die transiente Statusmeldung), darunter eine Leerzeile und die
hintergrundlosen Shortcut-Hints. Der App-Rahmen zeichnet **keine** Rahmenlinien;
`BorderType::Rounded` gilt nur für einzelne Widgets (Eingabefeld, Modals).
Alles wird über `tui::appframe::render_frame` gezeichnet (SSOT).

- **Footer-Hints** über `ratada::shortcut_hints`, pro Ansicht in benannte
  `HintGroup`s gruppiert. Die **letzte Gruppe heißt `Global`** und stammt aus
  `App::global_group` – derselbe Helfer speist Footer *und* Hilfe-Overlay.
- **Hint-Bandhöhe nie als Konstante:** `shortcut_hints::height(...)`, zusätzlich
  gedeckelt, damit der Content `MIN_CONTENT_HEIGHT` Zeilen behält.
- **`Ctrl+Q` nirgends hartkodieren** – weder im Footer noch in der Hilfe noch
  als `[keys]`-Bindung. Es gehört dem Toolkit, ebenso `F1` (Hints umschalten).
- **Modals blockieren** (`ratada::modal`) und dimmen die *lebende* Ansicht.
  Ein `Ctrl+Q` im Modal beendet die App (`Answer::ForcedQuit`).
- **`Ctrl+C` bleibt die Zwischenablage** und wird nie als Quit belegt.
- Transiente Statuszeile (Fehler crashen nie), `…`-Truncation, Scrollbar bei
  Überlauf, zyklische Listennavigation, Glyphen Unicode/ASCII per Config.

## Modale Tasten: Scopes

calcli ist modal (`Enter` heißt in jedem Kontext etwas anderes). Deshalb trägt
jede `Action` in `keymap.rs` einen `Scope`; ein Chord wird nur in den Scopes
aufgelöst, die im aktuellen `Context` aktiv sind. Zwei Bindungen kollidieren nur,
wenn ihre Scopes gleichzeitig aktiv sein können.

Ein **nacktes druckbares Zeichen löst nie eine Action aus**, solange ein
Textfeld die Tastatur hat – nur so bleiben `q`, `?`, `y`, `d` tippbar. Aus
demselben Grund sind die Tab-Tasten `Alt+1..3` statt blanker Ziffern.

## Domänenspezifika

- **Präzision:** intern immer der volle `f64`; Rundung nur in der Anzeige
  (`format`). Verlauf speichert Eingabe-Strings + berechnete `f64`-Werte.
- **`ans`** einer Verlaufszeile = Wert der vorhergehenden Zeile. Editieren/
  Löschen rechnet alle Zeilen darunter neu (`recompute`).
- **Trennzeichen:** Eingabe tolerant (Leerzeichen, `_`, sowie das jeweils
  andere von `.`/`,` als Tausendertrenner); Anzeige nutzt die konfigurierten
  Trenner. `y` kopiert den reinen Wert (volle Präzision, ohne Tausendertrenner),
  `Y` kopiert wie angezeigt (gerundet, mit Trennern, später Einheit).
- **Dialoge über den `Interaction`-Port** (`tui/interaction.rs`): produktiv
  `Modals`, im Test `Headless`. Nur so ist der Tastenpfad ohne Terminal testbar.

## Spätere Erweiterungen (Architektur offen halten, jetzt nicht umsetzen)

CLI-Schicht (`cli/` + `sparcli`), GUI. calcli ist bewusst TUI-only: es gibt
keine Subcommands und keine `println!`-Ausgabe – `sparcli` wird daher nicht
eingebunden.

## Fehler-Meldungen sind Verhalten

`AppError` (`domain/errors.rs`) ist der einzige Fehlertyp der Domäne; das
`Display` jeder Variante ist genau das, was der Nutzer in der Statuszeile liest
und was die History für eine fehlgeschlagene Zeile speichert. Eine Meldung zu
ändern ist eine nutzersichtbare Änderung, kein Refactoring – `AppError::Units`
gibt rinks Wortlaut deshalb unverändert weiter (rink formuliert ganze Sätze),
während `AppError::Calculator` das Fragment von meval präfixiert.
`every_failure_mode_keeps_its_message` in `calc_service.rs` friert alle
Meldungen ein.
