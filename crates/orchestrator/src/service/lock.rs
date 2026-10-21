//! The service lock: held by the one running service for its whole life.
//!
//! The lock is an exclusive lock on `run/service.lock`. The operating system releases it when the
//! holder exits, however it exits, so a crashed service never strands it. It is taken before any
//! state is opened, so a second service can neither bind the endpoint nor touch the home
//! (ADR-0199 Decision 9).

use std::fs::{File, TryLockError};
use std::io;
use std::path::Path;

use ghostlight_relay::private;

/// The lock, held until it is dropped.
#[derive(Debug)]
pub struct ServiceLock {
    _file: File,
}

impl ServiceLock {
    /// Try to take the lock at `path`, creating the file and its directory when missing.
    ///
    /// The file and its directory are owner-private on Linux. Returns `None` when another process
    /// holds the lock.
    pub fn try_acquire(path: &Path) -> io::Result<Option<Self>> {
        if let Some(parent) = path.parent() {
            private::create_dir_all(parent)?;
        }
        let file = private::file_options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    /// Whether nothing holds the lock at `path`. Taking it for a moment is the test; it is
    /// released at once.
    pub fn is_free(path: &Path) -> bool {
        matches!(Self::try_acquire(path), Ok(Some(_)))
    }
}
