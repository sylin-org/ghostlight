//! Whether "Others can watch" is on.
//!
//! Watching is on unless the home holds the marker `state/watch.off`, so a fresh home is on. The
//! facade is the only thing that changes it: it writes or removes the marker and flips the state
//! every request asks, so a request that arrives after the change sees it at once (ADR-0199,
//! amendment of 2026-10-15).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use ghostlight_relay::private;

/// The state of "Others can watch", shared by the server and the facade.
#[derive(Debug)]
pub struct Watching {
    marker: PathBuf,
    on: AtomicBool,
    /// Held across a change, so two changes cannot interleave their marker and their state.
    changing: Mutex<()>,
}

impl Watching {
    /// Read the state from the marker at `marker`: off exactly while the file exists.
    #[must_use]
    pub fn load(marker: PathBuf) -> Self {
        let on = !marker.exists();
        Self {
            marker,
            on: AtomicBool::new(on),
            changing: Mutex::new(()),
        }
    }

    /// Whether other screens on this computer may watch.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.on.load(Ordering::Acquire)
    }

    /// The marker file that records "off".
    #[must_use]
    pub fn marker(&self) -> &Path {
        &self.marker
    }

    /// Turn watching on or off, and say whether the state changed.
    ///
    /// Off writes the marker first and flips the state after, so no request is answered as
    /// watching once the marker could be lost. On removes the marker and then flips. A failed
    /// write leaves both as they were.
    pub fn set(&self, on: bool) -> io::Result<bool> {
        let _changing = self
            .changing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.is_on() == on {
            return Ok(false);
        }
        if on {
            match std::fs::remove_file(&self.marker) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        } else {
            if let Some(directory) = self.marker.parent() {
                private::create_dir_all(directory)?;
            }
            private::file_options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&self.marker)?;
        }
        self.on.store(on, Ordering::Release);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn watching_is_off_exactly_while_the_marker_exists() {
        let directory = Scratch::named("watching");
        let marker = directory.join("watch.off");
        assert!(Watching::load(marker.clone()).is_on());
        std::fs::write(&marker, b"").unwrap();
        assert!(!Watching::load(marker).is_on());
    }

    #[test]
    fn setting_writes_and_removes_the_marker_and_reports_a_change_only_when_there_is_one() {
        let directory = Scratch::named("watching");
        let marker = directory.join("state").join("watch.off");
        let watching = Watching::load(marker.clone());
        assert!(!watching.set(true).unwrap(), "a fresh home is already on");
        assert!(watching.set(false).unwrap());
        assert!(!watching.is_on());
        assert!(marker.exists());
        assert!(!watching.set(false).unwrap());
        assert!(
            !Watching::load(marker.clone()).is_on(),
            "a restart keeps it off"
        );
        assert!(watching.set(true).unwrap());
        assert!(watching.is_on());
        assert!(!marker.exists());
    }

    #[test]
    fn a_marker_that_cannot_be_written_leaves_watching_on() {
        let directory = Scratch::named("watching");
        // A file where the state directory should be makes the marker impossible to create.
        let blocker = directory.join("state");
        std::fs::write(&blocker, b"").unwrap();
        let watching = Watching::load(blocker.join("watch.off"));
        assert!(watching.set(false).is_err());
        assert!(watching.is_on());
    }
}
