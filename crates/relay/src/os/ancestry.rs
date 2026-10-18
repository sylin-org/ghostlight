//! The executables a process descends from: its parent, that one's parent, and so on.
//!
//! [`ancestor_images`] walks up from one process, reading each process's parent id, start time and
//! executable path from the operating system: on Windows one process snapshot for the parent ids,
//! then each process opened for query only; on Linux `/proc`. The walk ends at the limit, at a
//! parent that has exited or cannot be read, and at a parent that started after its child, whose
//! id therefore names a later process. Nothing here is logged, and the paths are returned whole:
//! the caller decides what of them it keeps.

use std::path::PathBuf;

/// What the walk reads about one process.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ProcessFacts {
    /// The id of the process that started it, when the system names one.
    pub(super) parent: Option<u32>,
    /// When it started, in the system's own unit. Only the order of two start times is read.
    pub(super) started: u64,
    /// The full path of its executable, when it can be read.
    pub(super) image: Option<PathBuf>,
}

/// The executable paths of the ancestors of `process_id`, nearest parent first, at most `limit`.
///
/// Empty when the process itself cannot be read. A read that fails partway returns the ancestors
/// read so far.
#[must_use]
pub fn ancestor_images(process_id: u32, limit: usize) -> Vec<PathBuf> {
    walk(process_id, limit, super::platform_facts())
}

/// [`ancestor_images`] over the process facts `facts` reports.
pub(super) fn walk(
    process_id: u32,
    limit: usize,
    facts: impl Fn(u32) -> Option<ProcessFacts>,
) -> Vec<PathBuf> {
    let mut images = Vec::new();
    let Some(mut child) = facts(process_id) else {
        return images;
    };
    while images.len() < limit {
        let Some(parent) = child.parent.and_then(&facts) else {
            break;
        };
        if parent.started > child.started {
            break;
        }
        let Some(image) = parent.image.clone() else {
            break;
        };
        images.push(image);
        child = parent;
    }
    images
}
