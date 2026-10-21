//! The setup record: the one fact that says setup is finished.
//!
//! Setup is finished when a model client's first request reached a browser and succeeded. The
//! service writes `state/setup.json` once, at that moment, and the break migration writes it for
//! a home that was already in use. The file's existence is the answer: a file that exists but
//! does not parse still counts as complete, because setup must never start over on a person who
//! finished it. The record carries a time and nothing about the person, the agent or the page.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::record::atomic_replace;

/// The content of `state/setup.json`. A reader ignores fields it does not know.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SetupRecord {
    /// Unix milliseconds of the first successful agent request, or of the migration that adopted
    /// an installation already in use.
    pub first_request_ms: u64,
}

impl SetupRecord {
    /// A record for a first request at this moment.
    #[must_use]
    pub fn now() -> Self {
        Self {
            first_request_ms: crate::audit::unix_ms(),
        }
    }
}

/// Whether setup is complete: the record exists at `path`, whatever it holds.
#[must_use]
pub fn is_complete(path: &Path) -> bool {
    fs::metadata(path).is_ok()
}

/// The record at `path`, or `None` when it is missing or does not parse.
#[must_use]
pub fn read(path: &Path) -> Option<SetupRecord> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// Write `record` to `path` unless a record is already there, and say whether this call wrote it.
///
/// An existing record is never replaced, so the first request stays the first request.
pub fn record_if_absent(path: &Path, record: &SetupRecord) -> io::Result<bool> {
    if is_complete(path) {
        return Ok(false);
    }
    let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
    atomic_replace(path, &bytes)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn record(ms: u64) -> SetupRecord {
        SetupRecord {
            first_request_ms: ms,
        }
    }

    /// ACCEPTANCE Desktop 24: setup completes once, at the first agent request that reaches a
    /// browser. A later request leaves that first record as it is, and any file in its place,
    /// even one that does not parse, counts as complete, so setup never starts over.
    #[test]
    fn setup_is_recorded_once_and_any_file_there_counts_as_complete() {
        let scratch = Scratch::new();
        let path = scratch.path().join("state").join("setup.json");
        assert!(!is_complete(&path));
        assert!(record_if_absent(&path, &record(1)).unwrap());
        assert!(is_complete(&path));
        assert!(!record_if_absent(&path, &record(2)).unwrap());
        assert_eq!(read(&path), Some(record(1)));

        let unreadable = scratch.path().join("unreadable.json");
        fs::write(&unreadable, b"not json").unwrap();
        assert!(is_complete(&unreadable));
        assert!(!record_if_absent(&unreadable, &record(3)).unwrap());
        assert_eq!(fs::read(&unreadable).unwrap(), b"not json");
    }
}
