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

use crate::sync::lock;

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
        let _changing = lock(&self.changing);
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

    /// Protects docs/trust/data-flows.md (`state/watch.off` exists exactly while watching is off)
    /// and ADR-0199's amendment of 2026-10-15, item 8: watching is off exactly while the home holds
    /// the marker, through a load, a change, a repeated change, a restart and a marker that cannot
    /// be written, and a change is reported only when there is one.
    #[test]
    fn watching_is_off_exactly_while_the_home_holds_the_marker() {
        let directory = Scratch::named("watching");
        let marker = directory.join("state").join("watch.off");
        let watching = Watching::load(marker.clone());
        assert!(
            watching.is_on(),
            "a fresh home, with no state folder yet, is on"
        );
        assert!(!watching.set(true).unwrap(), "a fresh home is already on");
        assert!(watching.set(false).unwrap());
        assert!(!watching.is_on());
        assert!(marker.exists());
        assert!(!watching.set(false).unwrap(), "a repeat changes nothing");
        assert!(
            !Watching::load(marker.clone()).is_on(),
            "a restart keeps it off"
        );
        assert!(watching.set(true).unwrap());
        assert!(watching.is_on());
        assert!(!marker.exists());

        // A file where the state folder should be makes the marker impossible to create, and
        // watching stays on rather than claim an off a restart would forget.
        let blocked = Scratch::named("watching");
        let state = blocked.join("state");
        std::fs::write(&state, b"").unwrap();
        let marker = state.join("watch.off");
        let watching = Watching::load(marker.clone());
        assert!(watching.set(false).is_err());
        assert!(watching.is_on());
        assert!(!marker.exists());
    }
}
