//! A thin, best-effort wrapper over the system clipboard.
//!
//! Copying is a convenience, never load-bearing, so a failure (no clipboard
//! available, a headless session) is logged and swallowed rather than surfaced
//! as an error.

// https://crates.io/crates/arboard
use arboard::Clipboard;

/// Copies `text` to the system clipboard, returning whether it succeeded.
///
/// Failures are logged at warn level and reported via the return value so a
/// caller can show a hint, but they never propagate as an error.
pub fn copy(text: &str) -> bool {
    match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text)) {
        Ok(()) => true,
        Err(error) => {
            log::warn!("clipboard copy failed: {error}");
            false
        }
    }
}

/// Reads UTF-8 text from the system clipboard, or `None` when none is available
/// (no clipboard, a headless session, or non-text contents).
pub fn paste() -> Option<String> {
    match Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) => Some(text),
        Err(error) => {
            log::warn!("clipboard paste failed: {error}");
            None
        }
    }
}
