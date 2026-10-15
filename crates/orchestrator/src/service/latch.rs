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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::scratch::Scratch;

    #[test]
    fn setting_creates_the_latch_and_clearing_removes_it() {
        let scratch = Scratch::new();
        let path = scratch.path().join("state").join("quit");
        assert!(!is_set(&path));
        set(&path).unwrap();
        assert!(is_set(&path));
        set(&path).unwrap();
        clear(&path).unwrap();
        assert!(!is_set(&path));
    }

    #[test]
    fn clearing_a_latch_that_is_not_set_is_not_an_error() {
        let scratch = Scratch::new();
        clear(&scratch.path().join("state").join("quit")).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_latch_is_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new();
        let path = scratch.path().join("state").join("quit");
        set(&path).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
