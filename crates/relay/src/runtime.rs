//! The runtime file: how a connector finds the running service.
//!
//! The service publishes `{relay, port, token, service_version}` once its listener accepts.
//! Readers ignore unknown fields and fields are never removed, so a newer service stays readable
//! by an older connector. A file from another relay revision, including the runtime file earlier
//! releases wrote, reads as "no service here".

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{file, private, RELAY};

/// Largest runtime file a reader accepts, in bytes.
const MAX_RUNTIME_FILE_BYTES: u64 = 64 * 1024;

/// Suffix of the sibling file a write goes through before it replaces the runtime file.
const TEMPORARY_SUFFIX: &str = ".tmp";

/// What the running service publishes for connectors and the CLI.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeFile {
    /// Relay revision the service speaks.
    pub relay: u16,
    /// Loopback TCP port of the service's one listener.
    pub port: u16,
    /// Per-service secret every opening presents.
    pub token: String,
    /// Product version of the running service.
    pub service_version: String,
}

impl RuntimeFile {
    /// Describe a service speaking this crate's relay revision.
    #[must_use]
    pub fn new(port: u16, token: String, service_version: String) -> Self {
        Self {
            relay: RELAY,
            port,
            token,
            service_version,
        }
    }

    /// Read the runtime file at `path`.
    ///
    /// Returns `None` when the file is missing, malformed, too large, or names another relay
    /// revision. Unknown fields are ignored. Only an unexpected I/O failure is an error.
    pub fn read(path: &Path) -> io::Result<Option<Self>> {
        let Some(document) = file::read_json(path, MAX_RUNTIME_FILE_BYTES)? else {
            return Ok(None);
        };
        if document.get("relay").and_then(Value::as_u64) != Some(u64::from(RELAY)) {
            return Ok(None);
        }
        Ok(serde_json::from_value(document).ok())
    }

    /// Publish this runtime file at `path`, atomically and readable only by the current user.
    ///
    /// The document is written to a sibling file first and renamed over `path`, so a reader sees
    /// the old file or the new one, never a partial one. Missing parent directories are created.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            private::create_dir_all(parent)?;
        }
        let mut temporary = OsString::from(path.as_os_str());
        temporary.push(TEMPORARY_SUFFIX);
        let temporary = Path::new(&temporary);
        match fs::remove_file(temporary) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        let bytes = serde_json::to_vec(self).map_err(io::Error::other)?;
        let mut file = create_private(temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temporary, path).inspect_err(|_| {
            let _ = fs::remove_file(temporary);
        })
    }
}

/// Create a new file only the current user can read.
#[cfg(windows)]
fn create_private(path: &Path) -> io::Result<File> {
    ghostlight_win_peer::create_private_file(path)
}

/// Create a new file only the current user can read.
#[cfg(not(windows))]
fn create_private(path: &Path) -> io::Result<File> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// A scratch directory removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "ghostlight-relay-runtime-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn file(&self) -> PathBuf {
            self.0.join("ghostlight-runtime.json")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample() -> RuntimeFile {
        RuntimeFile::new(49152, "secret".to_owned(), "2.0.0".to_owned())
    }

    #[test]
    fn a_written_file_reads_back() {
        let scratch = Scratch::new();
        sample().write(&scratch.file()).unwrap();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), Some(sample()));
    }

    #[test]
    fn a_write_replaces_the_previous_file_and_leaves_no_temporary() {
        let scratch = Scratch::new();
        sample().write(&scratch.file()).unwrap();
        let next = RuntimeFile::new(49153, "other".to_owned(), "2.0.1".to_owned());
        next.write(&scratch.file()).unwrap();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), Some(next));
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    #[test]
    fn a_write_creates_missing_parent_directories() {
        let scratch = Scratch::new();
        let nested = scratch.0.join("home").join("ghostlight-runtime.json");
        sample().write(&nested).unwrap();
        assert_eq!(RuntimeFile::read(&nested).unwrap(), Some(sample()));
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_private() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new();
        sample().write(&scratch.file()).unwrap();
        let mode = fs::metadata(scratch.file()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_missing_file_reads_as_none() {
        let scratch = Scratch::new();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), None);
    }

    #[test]
    fn a_malformed_file_reads_as_none() {
        let scratch = Scratch::new();
        fs::write(scratch.file(), b"{ not json").unwrap();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), None);
        fs::write(scratch.file(), br#"{"relay":3,"port":"x"}"#).unwrap();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), None);
    }

    #[test]
    fn another_relay_value_reads_as_none() {
        let scratch = Scratch::new();
        fs::write(
            scratch.file(),
            br#"{"relay":4,"port":1,"token":"t","service_version":"9"}"#,
        )
        .unwrap();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), None);
    }

    #[test]
    fn a_1_3_runtime_file_reads_as_none() {
        let scratch = Scratch::new();
        fs::write(
            scratch.file(),
            br#"{"service_port":1,"browser_port":2,"token":"t","service_bridge_major":1,"browser_relay_major":1,"service_version":"1.3.15"}"#,
        )
        .unwrap();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), None);
    }

    #[test]
    fn an_oversize_file_reads_as_none() {
        let scratch = Scratch::new();
        let padding = " ".repeat(MAX_RUNTIME_FILE_BYTES as usize);
        fs::write(
            scratch.file(),
            format!(r#"{{"relay":3,"port":1,"token":"t","service_version":"2"}}{padding}"#),
        )
        .unwrap();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), None);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let scratch = Scratch::new();
        fs::write(
            scratch.file(),
            br#"{"relay":3,"port":49152,"token":"secret","service_version":"2.0.0","later":{"a":1}}"#,
        )
        .unwrap();
        assert_eq!(RuntimeFile::read(&scratch.file()).unwrap(), Some(sample()));
    }
}
