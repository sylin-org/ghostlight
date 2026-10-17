//! Tests of the edge's own surface: its version and the private diagnostics it writes, and the
//! one way a test of this crate gets a temporary path.
//!
//! The support lives in this test file, beside the crate root, so it stays outside the source the
//! published relay revision locks.

use std::fs;
use std::io;
use std::ops::Deref;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The start of every scratch directory's name.
const PREFIX: &str = "ghostlight-test-edge";

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

#[test]
fn a_scratch_and_what_it_holds_are_gone_after_the_drop() {
    let scratch = Scratch::named("drop");
    fs::write(scratch.join("file.txt"), b"x").unwrap();
    let root = scratch.path().to_path_buf();
    assert!(root.is_dir());
    drop(scratch);
    assert!(!root.exists());
}

#[test]
fn a_panic_still_removes_the_scratch() {
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

/// The crate version major is the relay revision, so a published edge names its revision.
#[test]
fn crate_version_major_equals_relay_revision() {
    let major: u16 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    assert_eq!(major, ghostlight_relay::RELAY);
}

#[cfg(unix)]
mod diagnostics {
    //! The diagnostics files an edge role writes are private to the user.

    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use ghostlight_relay::home::{
        DIAGNOSTICS_DIRECTORY_NAME, DIAGNOSTICS_MARKER_NAME, TEST_HOME_VARIABLE,
    };
    use ghostlight_relay::log::Event as LogEvent;

    use super::Scratch;
    use crate::record_mcp;

    /// The permission bits of `path`.
    fn mode(path: &std::path::Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_mcp_relay_writes_a_private_log_in_a_private_directory() {
        let home = Scratch::named("log");
        fs::write(home.join(DIAGNOSTICS_MARKER_NAME), "").unwrap();
        std::env::set_var(TEST_HOME_VARIABLE, home.path());
        record_mcp(LogEvent::ProcessStarted, "");
        std::env::remove_var(TEST_HOME_VARIABLE);
        let directory = home.join(DIAGNOSTICS_DIRECTORY_NAME);
        let log = directory.join("edge-mcp.jsonl");
        assert!(log.is_file());
        assert_eq!(mode(&log), 0o600);
        assert_eq!(mode(&directory), 0o700);
    }
}
