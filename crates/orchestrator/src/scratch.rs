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
