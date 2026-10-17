//! Test support: the one way a test of this crate gets a temporary path.
//!
//! A `Scratch` is a fresh directory under the system temporary directory, and dropping it removes
//! that directory with everything in it, also when the test panics and unwinds.

use std::fs;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The start of every scratch directory's name.
const PREFIX: &str = "ghostlight-test-win-peer";

/// A temporary directory, removed with everything in it when dropped.
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

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::*;

    #[test]
    fn the_directory_and_what_it_holds_are_gone_after_the_drop() {
        let scratch = Scratch::named("drop");
        fs::write(scratch.join("file.txt"), b"x").unwrap();
        let root = scratch.path().to_path_buf();
        assert!(root.is_dir());
        drop(scratch);
        assert!(!root.exists());
    }

    #[test]
    fn a_panic_still_removes_the_directory() {
        let mut seen = None;
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let scratch = Scratch::named("panic");
            fs::write(scratch.join("file.txt"), b"x").unwrap();
            seen = Some(scratch.path().to_path_buf());
            panic!("the test fails while its scratch lives");
        }));
        assert!(unwound.is_err());
        assert!(!seen.expect("the scratch was made").exists());
    }
}
