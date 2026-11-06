//! Comment-preserving edits of `config.toml`, used by the settings view.
//!
//! Each public editor wraps a pure `*_in_toml` core that takes and returns the
//! file's text, so the edit itself is tested without a file. The result is
//! parsed as a config before it is written, so an edit never leaves a file the
//! loader would reject.

use std::path::Path;

use toml_edit::{DocumentMut, Item, Table, value};

use crate::config::{ConfigError, loader};
use crate::util::fs::write_atomic;

/// Sets the `[appearance.colors]` override `name` to `color`, or removes it
/// when `color` is `None` (the theme's colour applies again).
///
/// # Errors
///
/// Returns [`ConfigError::Write`] when the file cannot be read, edited,
/// validated or written.
pub fn set_color(
    path: &Path,
    name: &str,
    color: Option<&str>,
) -> Result<(), ConfigError> {
    edit(path, |text| set_color_in_toml(text, name, color))
}

/// Sets the `[appearance.colors.<group>]` color `key` to `color`, or removes
/// it when `color` is `None` (the theme's or built-in value applies again).
///
/// # Errors
///
/// Returns [`ConfigError::Write`] when the file cannot be read, edited,
/// validated or written.
pub fn set_group_color(
    path: &Path,
    group: &str,
    key: &str,
    color: Option<&str>,
) -> Result<(), ConfigError> {
    edit(path, |text| {
        set_group_color_in_toml(text, group, key, color)
    })
}

/// Sets `[appearance] dim_amount`.
///
/// # Errors
///
/// Returns [`ConfigError::Write`] when the file cannot be read, edited,
/// validated or written.
pub fn set_dim_amount(path: &Path, amount: u8) -> Result<(), ConfigError> {
    edit(path, |text| set_dim_amount_in_toml(text, amount))
}

/// `text` with the `[appearance.colors]` override `name` set to `color`, or
/// removed when `color` is `None`.
///
/// # Errors
///
/// Returns a message when `text` is not valid TOML.
pub fn set_color_in_toml(
    text: &str,
    name: &str,
    color: Option<&str>,
) -> Result<String, String> {
    let mut doc = parse(text)?;
    let colors = table(table(doc.as_table_mut(), "appearance")?, "colors")?;
    match color {
        Some(color) => colors[name] = value(color),
        None => {
            colors.remove(name);
        }
    }
    Ok(doc.to_string())
}

/// `text` with the `[appearance.colors.<group>]` color `key` set to `color`,
/// or removed when `color` is `None`.
///
/// # Errors
///
/// Returns a message when `text` is not valid TOML.
pub fn set_group_color_in_toml(
    text: &str,
    group: &str,
    key: &str,
    color: Option<&str>,
) -> Result<String, String> {
    let mut doc = parse(text)?;
    let colors = table(table(doc.as_table_mut(), "appearance")?, "colors")?;
    let group = table(colors, group)?;
    match color {
        Some(color) => group[key] = value(color),
        None => {
            group.remove(key);
        }
    }
    Ok(doc.to_string())
}

/// `text` with `[appearance] dim_amount` set to `amount`.
///
/// # Errors
///
/// Returns a message when `text` is not valid TOML.
pub fn set_dim_amount_in_toml(
    text: &str,
    amount: u8,
) -> Result<String, String> {
    let mut doc = parse(text)?;
    table(doc.as_table_mut(), "appearance")?["dim_amount"] =
        value(i64::from(amount));
    Ok(doc.to_string())
}

/// Reads `path` (a missing file is empty), applies `change`, validates the
/// result as a config and writes it atomically.
fn edit(
    path: &Path,
    change: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(), ConfigError> {
    let failure = |reason: String| ConfigError::Write {
        path: path.display().to_string(),
        reason,
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            String::new()
        }
        Err(error) => return Err(failure(error.to_string())),
    };
    let edited = change(&text).map_err(failure)?;
    loader::config_from_str(&edited)
        .map_err(|error| failure(error.to_string()))?;
    write_atomic(path, edited.as_bytes())
        .map_err(|error| failure(error.to_string()))
}

/// Parses `text` as an editable TOML document.
fn parse(text: &str) -> Result<DocumentMut, String> {
    text.parse::<DocumentMut>()
        .map_err(|error| error.to_string())
}

/// The sub-table `key` of `parent`, created when missing.
fn table<'a>(
    parent: &'a mut Table,
    key: &str,
) -> Result<&'a mut Table, String> {
    let item = parent
        .entry(key)
        .or_insert_with(|| Item::Table(Table::new()));
    item.as_table_mut()
        .ok_or_else(|| format!("`{key}` is not a table"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_added_under_appearance_colors() {
        let text = set_color_in_toml("", "dim", Some("#6e6a78")).unwrap();
        assert!(text.contains("[appearance.colors]"), "{text}");
        assert!(text.contains("dim = \"#6e6a78\""), "{text}");
    }

    #[test]
    fn removing_a_colour_keeps_comments_and_other_keys() {
        let base = "# mine\n[appearance]\ntheme = \"default\"\n\n\
                    [appearance.colors]\naccent = \"#010203\" # keep\n\
                    dim = \"#040506\"\n";
        let text = set_color_in_toml(base, "dim", None).unwrap();
        assert!(text.contains("# mine"), "{text}");
        assert!(text.contains("accent = \"#010203\" # keep"), "{text}");
        assert!(!text.contains("dim ="), "{text}");
    }

    #[test]
    fn a_group_color_goes_into_its_own_table() {
        let text = set_group_color_in_toml("", "badge", "bg", Some("#010203"))
            .unwrap();
        assert!(text.contains("[appearance.colors.badge]"), "{text}");
        assert!(text.contains("bg = \"#010203\""), "{text}");
        let text = set_group_color_in_toml(&text, "badge", "bg", None).unwrap();
        assert!(!text.contains("bg ="), "{text}");
    }

    #[test]
    fn dim_amount_is_written_as_a_number() {
        let text = set_dim_amount_in_toml("", 55).unwrap();
        assert!(text.contains("dim_amount = 55"), "{text}");
        let config = loader::config_from_str(&text).unwrap();
        assert_eq!(config.appearance.dim_amount, 55);
    }

    #[test]
    fn an_invalid_file_is_refused() {
        assert!(set_color_in_toml("[appearance", "dim", None).is_err());
    }

    #[test]
    fn a_written_file_reads_back() {
        let dir = std::env::temp_dir()
            .join(format!("calcli-writer-{}", std::process::id()));
        let path = dir.join("config.toml");
        set_color(&path, "accent", Some("ansi:green")).unwrap();
        let config = loader::load_from(&path).unwrap();
        assert_eq!(
            config.appearance.colors.get("accent").map(String::as_str),
            Some("ansi:green")
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
