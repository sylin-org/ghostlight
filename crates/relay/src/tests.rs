//! Crate-wide test support: the one way a test of this crate gets a temporary path.
//!
//! The support lives in this test file, beside the crate root, so it stays outside the source the
//! published relay revision locks.

use std::fs;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The start of every scratch directory's name.
const PREFIX: &str = "ghostlight-test-relay";

/// A fresh directory under the system temporary directory, removed with everything in it when
/// dropped, also when the test panics and unwinds.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    /// A fresh empty directory whose name carries `label`, so a person can tell which test made
    /// it.
    pub(crate) fn named(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir();
        loop {
            let candidate = base.join(format!(
                "{PREFIX}-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => return Self(candidate),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create {}: {error}", candidate.display()),
            }
        }
    }

    /// The directory.
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
