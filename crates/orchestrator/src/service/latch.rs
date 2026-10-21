//! The quit latch: a file that makes a person's Quit hold.
//!
//! A person's Quit writes `state/quit` before the service exits (ADR-0199 Decision 10). While it
//! exists, a start for a browser or a recovering shell is refused, so a browser that restarts
//! does not bring Ghostlight back. A start for any other reason deletes it and serves: an agent
//! that needs a browser, a command that needs the service, a person who opens Ghostlight, or an
//! installer that has just replaced it. Existence is the whole signal; the file holds nothing.

use std::fs;
use std::io;
use std::path::Path;

use crate::install::record;

/// Whether the latch at `path` is set.
#[must_use]
pub fn is_set(path: &Path) -> bool {
    path.exists()
}

/// Set the latch at `path`, creating its directory. The file is private to the user.
pub fn set(path: &Path) -> io::Result<()> {
    record::atomic_replace(path, b"")
}

/// Clear the latch at `path`. A latch that is not set is already clear.
pub fn clear(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
