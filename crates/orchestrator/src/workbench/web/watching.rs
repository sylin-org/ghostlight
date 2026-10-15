//! Whether "Others can watch" is on.
//!
//! Watching is on unless the home holds the marker `state/watch.off`. The state is read once, when
//! the server starts, and held here for every request to ask (ADR-0199, amendment of 2026-10-15).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The state of "Others can watch", shared by the server and the facade.
#[derive(Debug)]
pub struct Watching {
    marker: PathBuf,
    on: AtomicBool,
}

impl Watching {
    /// Read the state from the marker at `marker`: off exactly while the file exists.
    #[must_use]
    pub fn load(marker: PathBuf) -> Self {
        let on = !marker.exists();
        Self {
            marker,
            on: AtomicBool::new(on),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watching_is_off_exactly_while_the_marker_exists() {
        let directory = std::env::temp_dir().join(format!(
            "ghostlight-watching-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let marker = directory.join("watch.off");
        assert!(Watching::load(marker.clone()).is_on());
        std::fs::write(&marker, b"").unwrap();
        assert!(!Watching::load(marker).is_on());
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
