//! Test support: the one way a test of this crate gets a temporary path.
//!
//! Every `Scratch` is a fresh directory under the system temporary directory, and dropping it
//! removes that directory with everything in it, also when the test panics and unwinds. A test
//! that needs a file asks for one with [`Scratch::file`]; the file lives alone in its own
//! directory, so any sibling a write leaves beside it (an atomic-replace partner, a renamed copy)
//! goes with it.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The start of every scratch directory's name.
const PREFIX: &str = "ghostlight-test-service";

/// A temporary directory, or a file path inside one, removed with its directory when dropped.
pub(crate) struct Scratch {
    /// The directory this value owns and removes.
    root: PathBuf,
    /// The path the test uses: the directory itself, or a file inside it.
    path: PathBuf,
}

impl Scratch {
    /// A fresh empty directory.
    pub(crate) fn new() -> Self {
        Self::named("dir")
    }

    /// A fresh empty directory whose name carries `label`, so a person can tell which test made
    /// it.
    pub(crate) fn named(label: &str) -> Self {
        let root = fresh_directory(label);
        Self {
            path: root.clone(),
            root,
        }
    }

    /// The path of a file called `name` in a fresh empty directory. The file is not created.
    pub(crate) fn file(name: &str) -> Self {
        let label = Path::new(name)
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("file");
        let root = fresh_directory(label);
        Self {
            path: root.join(name),
            root,
        }
    }

    /// The path the test uses.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Create a directory no other test, process or earlier run owns.
fn fresh_directory(label: &str) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let base = std::env::temp_dir();
    loop {
        let candidate = base.join(format!(
            "{PREFIX}-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => return candidate,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("cannot create {}: {error}", candidate.display()),
        }
    }
}

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::*;

    #[test]
    fn a_directory_and_what_it_holds_are_gone_after_the_drop() {
        let scratch = Scratch::named("drop");
        fs::create_dir(scratch.join("inner")).unwrap();
        fs::write(scratch.join("inner").join("file.txt"), b"x").unwrap();
        let root = scratch.path().to_path_buf();
        assert!(root.is_dir());
        drop(scratch);
        assert!(!root.exists());
    }

    #[test]
    fn a_file_and_its_directory_are_gone_after_the_drop() {
        let scratch = Scratch::file("policy.json");
        assert_eq!(scratch.file_name(), Some(OsStr::new("policy.json")));
        assert!(!scratch.exists(), "the file is not created");
        fs::write(&scratch, b"{}").unwrap();
        fs::write(scratch.with_extension("tmp"), b"{}").unwrap();
        let root = scratch.parent().unwrap().to_path_buf();
        drop(scratch);
        assert!(!root.exists());
    }

    #[test]
    fn a_panic_still_removes_the_directory() {
        let mut seen = None;
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let scratch = Scratch::file("panic.json");
            fs::write(&scratch, b"{}").unwrap();
            seen = Some(scratch.parent().unwrap().to_path_buf());
            panic!("the test fails while its scratch lives");
        }));
        assert!(unwound.is_err());
        let root = seen.expect("the scratch was made");
        assert!(!root.exists());
    }

    #[test]
    fn two_scratches_never_share_a_directory() {
        let first = Scratch::new();
        let second = Scratch::new();
        assert_ne!(first.path(), second.path());
    }
}
