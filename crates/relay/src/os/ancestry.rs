//! The processes a process descends from: its parent, that one's parent, and so on.
//!
//! [`ancestors`] walks up from one process, reading each process's parent id, start time and
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
    /// When it started, in the system's own unit.
    pub(super) started: u64,
    /// The full path of its executable, when it can be read.
    pub(super) image: Option<PathBuf>,
}

/// One ancestor of a process, as the walk read it.
///
/// Its id and start time together name one process: an id the system hands out again belongs to a
/// process that started later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ancestor {
    /// Its process id.
    pub process_id: u32,
    /// When it started, in the system's own unit: the creation FILETIME on Windows, clock ticks
    /// since boot on Linux. Only equality and order mean anything.
    pub started: u64,
    /// The full path of its executable.
    pub image: PathBuf,
}

/// The ancestors of `process_id`, nearest parent first, at most `limit`.
///
/// Empty when the process itself cannot be read. A read that fails partway returns the ancestors
/// read so far.
#[must_use]
pub fn ancestors(process_id: u32, limit: usize) -> Vec<Ancestor> {
    walk(process_id, limit, super::platform_facts())
}

/// [`ancestors`] over the process facts `facts` reports.
pub(super) fn walk(
    process_id: u32,
    limit: usize,
    facts: impl Fn(u32) -> Option<ProcessFacts>,
) -> Vec<Ancestor> {
    let mut found = Vec::new();
    let Some(mut child) = facts(process_id) else {
        return found;
    };
    while found.len() < limit {
        let Some((parent_id, parent)) = child
            .parent
            .and_then(|parent_id| Some((parent_id, facts(parent_id)?)))
        else {
            break;
        };
        if parent.started > child.started {
            break;
        }
        let Some(image) = parent.image.clone() else {
            break;
        };
        found.push(Ancestor {
            process_id: parent_id,
            started: parent.started,
            image,
        });
        child = parent;
    }
    found
}
