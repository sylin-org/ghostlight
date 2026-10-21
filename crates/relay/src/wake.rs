//! Waking the edges that wait for a service (ADR-0199 amendment of 2026-10-20).
//!
//! An edge that finds no service, or loses the one it had, registers a wake endpoint of its own
//! with [`Waiting::register`] and keeps it until it exits. A service whose endpoint accepts
//! openings calls [`wake_all`]: it connects to every endpoint registered in `run/wake/` and waits
//! up to [`WAKE_DEADLINE`] for the edge to close the connection. Nothing is written either way;
//! the connection means only "look now". The edge then looks for the service as it always does,
//! at once instead of at its next scheduled look. The schedule stays, so a wake that never comes
//! costs what it always cost.
//!
//! A registration is a file named by a random id ([`ID_LENGTH`] lowercase hexadecimal digits) in
//! `run/wake/`: on Linux the wake socket itself, on Windows an empty marker for the pipe the id
//! names. A registration whose edge has gone answers no connection, and [`wake_all`] removes it.
//! Both ends check each other as they do at the service's endpoint ([`crate::endpoint`]).

use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::endpoint::{self, ConnectError, Listener};

/// How long a ready service waits for one woken edge to close the wake connection.
pub const WAKE_DEADLINE: Duration = Duration::from_millis(1_000);

/// Length of a wake id: sixteen lowercase hexadecimal digits.
pub const ID_LENGTH: usize = 16;

/// One edge's registration, served until the process exits.
#[derive(Debug)]
pub struct Waiting {
    listener: Listener,
    /// The registration's file in `run/wake/`, removed when this is dropped.
    file: PathBuf,
}

impl Waiting {
    /// Register a wake endpoint for this process under `home`.
    ///
    /// Fails when the endpoint cannot be bound or, on Windows, its marker cannot be written. The
    /// edge then keeps its scheduled looks alone.
    pub fn register(home: &Path) -> io::Result<Self> {
        let id = fresh_id();
        let listener = Listener::bind_wake(home, &id)?;
        let directory = endpoint::wake_directory(home);
        let file = directory.join(&id);
        #[cfg(windows)]
        {
            crate::private::create_dir_all(&directory)?;
            crate::private::file_options()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&file)?;
        }
        Ok(Self { listener, file })
    }

    /// Wait until a service wakes this edge.
    ///
    /// Fails only when the endpoint itself fails, and with [`io::ErrorKind::ConnectionAborted`]
    /// after [`Waiting::shutdown`].
    pub fn next(&self) -> io::Result<()> {
        // Dropping the connection closes it, which tells the service the wake arrived.
        self.listener.accept().map(drop)
    }

    /// Wake a [`Waiting::next`] blocked on another thread, and fail every later one.
    pub fn shutdown(&self) {
        self.listener.shutdown();
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.file);
    }
}

/// What [`wake_all`] did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Wakes {
    /// Edges that took the wake.
    pub woken: usize,
    /// Registrations whose edge had gone, now removed.
    pub removed: usize,
}

/// Wake every edge registered under `home`, and remove the registrations whose edge has gone.
///
/// Each edge has [`WAKE_DEADLINE`] to close the connection. A file in `run/wake/` whose name is
/// not a wake id is left alone, and so is an endpoint served by another user.
#[must_use]
pub fn wake_all(home: &Path) -> Wakes {
    let mut wakes = Wakes::default();
    let Ok(entries) = fs::read_dir(endpoint::wake_directory(home)) else {
        return wakes;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(id) = name.to_str().filter(|name| is_id(name)) else {
            continue;
        };
        match endpoint::connect_wake(home, id, Instant::now() + WAKE_DEADLINE) {
            Ok(mut connection) => {
                // The edge closes its end once it has taken the wake. Until then this end stays
                // open, because a Windows edge skips a client that left before it was checked.
                let _ = connection.set_read_timeout(Some(WAKE_DEADLINE));
                let _ = connection.read(&mut [0; 1]);
                wakes.woken += 1;
            }
            Err(ConnectError::NoService) => {
                if fs::remove_file(entry.path()).is_ok() {
                    wakes.removed += 1;
                }
            }
            Err(_) => {}
        }
    }
    wakes
}

/// A fresh random wake id.
fn fresh_id() -> String {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
        hasher.write_u128(now.as_nanos());
    }
    format!("{:0width$x}", hasher.finish(), width = ID_LENGTH)
}

/// Whether `name` is a wake id.
fn is_id(name: &str) -> bool {
    name.len() == ID_LENGTH
        && name
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests;
