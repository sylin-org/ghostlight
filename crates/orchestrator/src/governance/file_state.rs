// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Change detection for the files governance watches, without reading them.
//!
//! The refresher asks this once a second for each policy file and the managed bootstrap. A file is
//! read again only when its state differs from the one last seen, so an idle machine does no
//! file reads at all.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// What a watched file looks like from its metadata alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FileState {
    exists: bool,
    modified: Option<SystemTime>,
    length: u64,
}

impl FileState {
    /// The state of the file at `path` right now. A path that cannot be examined reads as absent.
    pub(super) fn of(path: &Path) -> Self {
        match fs::metadata(path) {
            Ok(metadata) => Self {
                exists: true,
                modified: metadata.modified().ok(),
                length: metadata.len(),
            },
            Err(_) => Self {
                exists: false,
                modified: None,
                length: 0,
            },
        }
    }
}
