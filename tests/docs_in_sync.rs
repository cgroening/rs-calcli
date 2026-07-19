//! The documentation is held to the action catalog by test, not by promise.
//!
//! `src/keymap` is the single source for every shortcut, but two files repeat
//! it for the reader: the key tables in `README.md` and the commented `[keys]`
//! block in `examples/config.toml`. A note asking the next author to keep them
//! in step is not a safeguard - it fails silently. These tests fail loudly.

use calcli::keymap::Action;

/// The shipped example config, which documents every `[keys]` name.
const EXAMPLE_CONFIG: &str = include_str!("../examples/config.toml");

/// The README, which documents every shortcut for the reader.
const README: &str = include_str!("../README.md");

/// The `[keys]` names the example config documents.
///
/// The whole block ships commented out, so the names are read off the comment
/// lines rather than by parsing the file as TOML.
fn documented_config_names() -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in EXAMPLE_CONFIG.lines() {
        let line = line.trim();
        let Some(body) = line.strip_prefix('#') else {
            // A non-comment line ends the commented block.
            if !line.is_empty() {
                inside = false;
            }
            continue;
        };
        let body = body.trim();
        if body == "[keys]" {
            inside = true;
            continue;
        }
        if body.starts_with('[') {
            inside = false;
            continue;
        }
        if inside && let Some((name, _)) = body.split_once('=') {
            let name = name.trim();
            if !name.is_empty() && !name.starts_with('#') {
                names.push(name.to_string());
            }
        }
    }
    names
}

#[test]
fn the_example_config_documents_every_action_exactly_once() {
    let mut documented = documented_config_names();
    documented.sort();

    let mut deduped = documented.clone();
    deduped.dedup();
    assert_eq!(documented, deduped, "a [keys] name is listed twice");

    let mut catalog: Vec<String> =
        Action::all().map(|a| a.config_name().to_string()).collect();
    catalog.sort();

    assert_eq!(
        documented, catalog,
        "examples/config.toml and the action catalog have drifted apart",
    );
}

/// How the README spells a key the catalog names differently.
///
/// The catalog stores the chord as the config parses it (`alt+up`, `pgup`);
/// the README shows what is printed on the key (`Alt+↑`, `PgUp`). The table is
/// deliberately explicit: a silent fallback would let a genuinely undocumented
/// key pass as "spelled differently".
const KEY_SPELLINGS: &[(&str, &str)] = &[
    ("up", "↑"),
    ("down", "↓"),
    ("left", "←"),
    ("right", "→"),
    ("pgup", "PgUp"),
    ("pgdn", "PgDn"),
    ("del", "Del"),
    ("home", "Home"),
    ("end", "End"),
    ("esc", "Esc"),
    ("enter", "Enter"),
];

/// `key` as the README would print it: each part of the chord spelled out,
/// function keys upper-cased, a plain character left as it is.
///
/// One subtlety the table cannot carry: a letter held with a modifier is
/// printed in capitals (`ctrl+y` reads `Ctrl+Y`), while a bare letter keeps
/// its case, because `y` and `Y` are two different actions.
fn as_readme_spells_it(key: &str) -> String {
    let modified = key.contains('+');
    key.split('+')
        .map(|part| {
            if modified && part.chars().count() == 1 {
                return part.to_uppercase();
            }
            if let Some((_, shown)) =
                KEY_SPELLINGS.iter().find(|(raw, _)| *raw == part)
            {
                return (*shown).to_string();
            }
            // A function key is written out in capitals (`f4` -> `F4`); a
            // modifier is merely capitalised (`alt` -> `Alt`).
            let is_function_key = part.starts_with('f')
                && part.len() > 1
                && part[1..].chars().all(|c| c.is_ascii_digit());
            if is_function_key {
                return part.to_uppercase();
            }
            let mut chars = part.chars();
            match chars.next() {
                Some(first) if part.len() > 1 => {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                }
                _ => part.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Every action must be findable in the README by at least one of its keys.
///
/// One key is enough on purpose: several actions answer to two (`edit` is
/// `Enter` *and* `e`), and the README is free to show whichever reads better
/// in its table. What it may not do is omit the action altogether.
#[test]
fn the_readme_documents_every_action() {
    let missing: Vec<String> = Action::all()
        .filter(|action| {
            !action
                .default_keys()
                .iter()
                .any(|key| README.contains(&as_readme_spells_it(key)))
        })
        .map(|action| {
            format!(
                "{} ({})",
                action.config_name(),
                action.default_keys().join(", "),
            )
        })
        .collect();

    assert!(
        missing.is_empty(),
        "the README does not mention these actions: {missing:?}",
    );
}

/// The hard quit belongs to the toolkit and is never bound in the catalog, but
/// the README still has to tell the user about it - it is the one key that
/// always works.
#[test]
fn the_readme_documents_the_toolkit_owned_keys() {
    for key in ["Ctrl+Q", "F1"] {
        assert!(README.contains(key), "the README omits {key}");
    }
}

/// `Ctrl+C` stays the clipboard. A README that ever advertises it as quit is
/// documenting a rule the app deliberately does not follow.
#[test]
fn the_readme_never_advertises_ctrl_c_as_quit() {
    for line in README.lines() {
        if line.contains("Ctrl+C") {
            assert!(
                !line.to_lowercase().contains("quit"),
                "Ctrl+C is the clipboard, never quit: {line}",
            );
        }
    }
}

/// A guard for the two tests above: they are only worth anything if the
/// spelling function and the lookup actually discriminate. A key the README
/// does not contain must not be found, or "documented" would mean nothing.
#[test]
fn the_readme_check_can_actually_fail() {
    assert_eq!(as_readme_spells_it("ctrl+y"), "Ctrl+Y");
    assert_eq!(as_readme_spells_it("alt+up"), "Alt+↑");
    assert_eq!(as_readme_spells_it("f12"), "F12");
    assert_eq!(
        as_readme_spells_it("Y"),
        "Y",
        "a bare letter keeps its case"
    );

    assert!(
        !README.contains(&as_readme_spells_it("ctrl+alt+f9")),
        "a key nobody documented must not be found",
    );
}

/// Likewise for the config check: an invented name must not appear.
#[test]
fn the_config_check_reads_real_names() {
    let names = documented_config_names();
    assert!(names.contains(&"view_calc".to_string()), "{names:?}");
    assert!(!names.contains(&"definitely_not_an_action".to_string()));
    assert!(names.len() > 30, "only {} names parsed", names.len());
}
