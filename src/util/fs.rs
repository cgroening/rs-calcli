//! Atomic file writes.
//!
//! A crash or a full disk halfway through a `write` would leave a truncated
//! `state.toml` behind and lose the session. Writing to a temporary file in the
//! same directory, flushing it to disk and renaming it over the target makes
//! the replacement atomic: a reader sees either the old file or the new one.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Writes `contents` to `path`, replacing it atomically.
///
/// Missing parent directories are created. On failure the temporary file is
/// removed and `path` is left untouched.
///
/// # Errors
///
/// Returns the underlying I/O error when the directory cannot be created, the
/// temporary file cannot be written or flushed, or the rename fails.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let temp = temp_path(path);
    match write_then_rename(&temp, path, contents) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error)
        }
    }
}

/// Writes `contents` to `temp`, flushes it to disk and renames it over `path`.
fn write_then_rename(
    temp: &Path,
    path: &Path,
    contents: &[u8],
) -> io::Result<()> {
    let mut file = File::create(temp)?;
    file.write_all(contents)?;
    // Without this the rename may land before the data does, leaving an empty
    // file behind after a power loss.
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)
}

/// A sibling temporary path, unique per process so two calcli instances writing
/// at once cannot clobber each other's temporary file.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".tmp.{}", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique scratch directory for one test, removed by the caller.
    fn scratch_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "calcli-fs-{}-{}",
            label,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn writes_a_new_file_and_creates_missing_parents() {
        let dir = scratch_dir("new");
        let path = dir.join("nested").join("state.toml");
        write_atomic(&path, b"hello").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn replaces_an_existing_file_leaving_no_temporary_behind() {
        let dir = scratch_dir("replace");
        let path = dir.join("state.toml");
        write_atomic(&path, b"old").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");

        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .filter(|name| name != "state.toml")
            .collect();
        assert!(leftovers.is_empty(), "temporary files left: {leftovers:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_write_leaves_the_previous_contents_intact() {
        let dir = scratch_dir("failed");
        let path = dir.join("state.toml");
        write_atomic(&path, b"good").unwrap();

        // A directory where the temporary file wants to live makes File::create
        // fail without touching the target.
        fs::create_dir_all(temp_path(&path)).unwrap();
        assert!(write_atomic(&path, b"bad").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "good");
        let _ = fs::remove_dir_all(&dir);
    }
}
