//! Minimal `log` backend writing to an optional file.
//!
//! A TUI owns the terminal, so diagnostics must not go to stderr (it would
//! corrupt the alternate screen). When a log file is configured, records are
//! appended there; otherwise logging is a no-op.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Why installing the logger failed.
///
/// Logging is non-fatal by design, so the caller is expected to report this
/// and carry on rather than abort.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    /// The log file could not be created or opened for appending.
    #[error("cannot open log file {path}")]
    OpenFile {
        /// The file that could not be opened.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// A global logger was already installed by someone else.
    #[error("cannot install logger")]
    InstallLogger {
        /// The failure reported by the `log` crate.
        #[source]
        source: log::SetLoggerError,
    },
}

/// A file logger; silent when no file is configured.
struct FileLogger {
    file: Option<Mutex<File>>,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        let Some(file) = &self.file else {
            return;
        };
        if let Ok(mut handle) = file.lock() {
            let _ = writeln!(
                handle,
                "[{:<5}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Some(file) = &self.file
            && let Ok(mut handle) = file.lock()
        {
            let _ = handle.flush();
        }
    }
}

/// Installs the logger at `level`, appending to `file` when given.
///
/// Best-effort: a failure is reported through the returned result, but the app
/// should treat logging as non-fatal and keep running without it.
///
/// # Errors
///
/// Returns [`LogError::OpenFile`] when the log file cannot be created or
/// opened, and [`LogError::InstallLogger`] when a global logger is already
/// installed.
pub fn init(level: LevelFilter, file: Option<&Path>) -> Result<(), LogError> {
    let sink = match file {
        Some(path) => Some(Mutex::new(open_append(path)?)),
        None => None,
    };
    let logger = FileLogger { file: sink };
    log::set_boxed_logger(Box::new(logger))
        .map_err(|source| LogError::InstallLogger { source })?;
    log::set_max_level(level);
    Ok(())
}

/// Opens `path` for appending, creating it and its parents as needed.
fn open_append(path: &Path) -> Result<File, LogError> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| LogError::OpenFile {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The message must name the offending file: a logger that fails silently
    /// leaves no way to find out why diagnostics never appear.
    #[test]
    fn an_unopenable_log_file_is_reported_with_its_path() {
        // A regular file where a directory would have to be: `create_dir_all`
        // cannot make it, so the open below fails.
        let blocker = std::env::temp_dir()
            .join(format!("calcli-log-test-{}", std::process::id()));
        std::fs::write(&blocker, b"not a directory")
            .expect("the temp directory is writable");
        let path = blocker.join("nested").join("calcli.log");

        let error = init(LevelFilter::Info, Some(&path))
            .expect_err("a file cannot act as a parent directory");

        assert!(matches!(error, LogError::OpenFile { .. }));
        assert!(error.to_string().contains("calcli.log"));

        let _ = std::fs::remove_file(&blocker);
    }

    /// Without a file the logger is a no-op, and building one must not touch
    /// the filesystem at all.
    #[test]
    fn a_logger_without_a_file_swallows_records() {
        let logger = FileLogger { file: None };
        logger.log(
            &Record::builder()
                .args(format_args!("dropped"))
                .level(Level::Info)
                .build(),
        );
        logger.flush();
    }
}
