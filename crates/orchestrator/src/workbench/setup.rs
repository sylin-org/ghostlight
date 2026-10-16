//! Setup progress: whether a model client has yet had a request answered through a browser.
//!
//! The projection sees every audit record. The first one that shows an agent doing real work in
//! a browser completes setup: it writes the setup record once ([`crate::install::setup_state`]),
//! remembers in memory which workspace did it, and calls the callback a surface attached so it
//! can tell the person. The browser, the policy and the page played no part in the decision, and
//! the record carries nothing about them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::audit::AuditRecord;
use crate::install::setup_state::{self, SetupRecord};
use crate::intake::session::IntakeChannel;
use crate::work::result::Status;

/// What completed setup during this run of the service. It is not stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finished {
    /// The workspace whose request completed setup.
    pub workspace: String,
}

/// What runs once, when setup completes.
type Callback = Arc<dyn Fn() + Send + Sync>;

/// Whether setup is complete, and the one moment it becomes so.
pub struct SetupProgress {
    path: PathBuf,
    complete: AtomicBool,
    finished: Mutex<Option<Finished>>,
    callback: Mutex<Option<Callback>>,
}

impl SetupProgress {
    /// Progress for the home whose setup record is at `path`: complete already when the record
    /// exists.
    #[must_use]
    pub fn load(path: &Path) -> Arc<Self> {
        Arc::new(Self {
            path: path.to_owned(),
            complete: AtomicBool::new(setup_state::is_complete(path)),
            finished: Mutex::new(None),
            callback: Mutex::new(None),
        })
    }

    /// Whether setup is complete, from the record or from this run.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    /// What completed setup, when it was completed during this run.
    #[must_use]
    pub fn finished(&self) -> Option<Finished> {
        lock(&self.finished).clone()
    }

    /// Run `callback` when setup completes during this run. It runs at most once, and not at all
    /// if setup is already complete. A second callback replaces the first.
    pub fn on_complete(&self, callback: impl Fn() + Send + Sync + 'static) {
        *lock(&self.callback) = Some(Arc::new(callback));
    }

    /// Consider one audit record, and say whether this call completed setup.
    ///
    /// A record qualifies when it is a direct request from a model client, was allowed, succeeded
    /// and reached a browser. The first qualifying record writes the setup record, and a failed
    /// write still completes setup for this run.
    pub fn observe(&self, record: &AuditRecord) -> bool {
        if self.is_complete() || !qualifies(record) {
            return false;
        }
        if self.complete.swap(true, Ordering::AcqRel) {
            return false;
        }
        let _ = setup_state::record_if_absent(
            &self.path,
            &SetupRecord {
                first_request_ms: record.timestamp_ms,
            },
        );
        *lock(&self.finished) = Some(Finished {
            workspace: record.workspace.clone(),
        });
        let callback = lock(&self.callback).clone();
        if let Some(callback) = callback {
            callback();
        }
        true
    }
}

/// Whether `record` shows an agent's request answered by a browser.
fn qualifies(record: &AuditRecord) -> bool {
    record.step.is_none()
        && record.channel == Some(IntakeChannel::Mcp)
        && record.allowed
        && record.status == Status::Succeeded.stored_name()
        && record.browser.is_some()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::AtomicUsize;

    use super::*;
    use crate::governance::{CapabilitySet, Decision};
    use crate::install::scratch::Scratch;
    use crate::language::history::{CompositionKind, StepReceipt};
    use crate::language::outcome::Outcome;

    fn request(status: &str) -> AuditRecord {
        let mut record = AuditRecord::now(
            "invocation_1",
            "workspace_1",
            "browser_read",
            CapabilitySet::EMPTY,
            "authority_1",
            Decision::permitted(),
            status,
            "none",
            &Outcome::StepNotStarted.audit(),
            5,
        )
        .from_channel(Some(IntakeChannel::Mcp))
        .with_browser(Some("Chrome".into()));
        record.timestamp_ms = 1234;
        record
    }

    fn succeeded() -> AuditRecord {
        request(&Status::Succeeded.stored_name())
    }

    fn progress(scratch: &Scratch) -> (Arc<SetupProgress>, PathBuf) {
        let path = scratch.path().join("state").join("setup.json");
        (SetupProgress::load(&path), path)
    }

    #[test]
    fn a_qualifying_record_writes_its_timestamp_once() {
        let scratch = Scratch::new();
        let (progress, path) = progress(&scratch);
        assert!(!progress.is_complete());
        assert!(progress.observe(&succeeded()));
        assert!(progress.is_complete());
        assert_eq!(
            setup_state::read(&path),
            Some(SetupRecord {
                first_request_ms: 1234
            })
        );
        let mut later = succeeded();
        later.timestamp_ms = 9999;
        assert!(!progress.observe(&later));
        assert_eq!(setup_state::read(&path).unwrap().first_request_ms, 1234);
    }

    #[test]
    fn the_run_remembers_which_workspace_finished_setup() {
        let scratch = Scratch::new();
        let (progress, _) = progress(&scratch);
        assert_eq!(progress.finished(), None);
        progress.observe(&succeeded());
        assert_eq!(
            progress.finished(),
            Some(Finished {
                workspace: "workspace_1".into()
            })
        );
    }

    #[test]
    fn records_that_are_not_an_agent_reaching_a_browser_write_nothing() {
        let scratch = Scratch::new();
        let (progress, path) = progress(&scratch);
        let mut child = succeeded();
        child.step = Some(StepReceipt {
            parent: CompositionKind::Flow,
            position: 0,
            total: 1,
            preparation_failed: false,
        });
        let mut command_line = succeeded();
        command_line.channel = Some(IntakeChannel::Cli);
        let mut no_channel = succeeded();
        no_channel.channel = None;
        let mut refused = succeeded();
        refused.allowed = false;
        let mut no_browser = succeeded();
        no_browser.browser = None;
        for record in [
            child,
            command_line,
            no_channel,
            refused,
            no_browser,
            request(&Status::Failed.stored_name()),
            request(&Status::Blocked.stored_name()),
            request(&Status::Cancelled.stored_name()),
            request(&Status::Unknown.stored_name()),
        ] {
            assert!(!progress.observe(&record));
        }
        assert!(!progress.is_complete());
        assert!(!path.exists());
        assert_eq!(progress.finished(), None);
    }

    #[test]
    fn an_existing_record_means_complete_and_is_not_rewritten() {
        let scratch = Scratch::new();
        let path = scratch.path().join("setup.json");
        fs::write(&path, b"junk").unwrap();
        let progress = SetupProgress::load(&path);
        assert!(progress.is_complete());
        assert!(!progress.observe(&succeeded()));
        assert_eq!(fs::read(&path).unwrap(), b"junk");
        assert_eq!(progress.finished(), None);
    }

    #[test]
    fn a_failed_write_still_completes_this_run() {
        let scratch = Scratch::new();
        // The parent of the record is a file, so the record cannot be written.
        fs::write(scratch.path().join("state"), b"not a directory").unwrap();
        let progress = SetupProgress::load(&scratch.path().join("state").join("setup.json"));
        assert!(!progress.is_complete());
        assert!(progress.observe(&succeeded()));
        assert!(progress.is_complete());
        assert!(progress.finished().is_some());
    }

    #[test]
    fn the_callback_fires_exactly_once() {
        let scratch = Scratch::new();
        let (progress, _) = progress(&scratch);
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        progress.on_complete(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });
        assert!(progress.observe(&succeeded()));
        assert!(!progress.observe(&succeeded()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_callback_attached_after_completion_never_fires() {
        let scratch = Scratch::new();
        let (progress, _) = progress(&scratch);
        assert!(progress.observe(&succeeded()));
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        progress.on_complete(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });
        assert!(!progress.observe(&succeeded()));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_projection_completes_setup_on_a_settled_request_and_works_without_it() {
        use crate::language::audit_health::Storage;
        use crate::workbench::WorkbenchProjection;

        let bare = WorkbenchProjection::default();
        bare.record(&succeeded(), Storage::Saved);

        let scratch = Scratch::new();
        let (progress, path) = progress(&scratch);
        let projection = WorkbenchProjection::default();
        projection.attach_setup(Arc::clone(&progress));
        let mut child = succeeded();
        child.step = Some(StepReceipt {
            parent: CompositionKind::Flow,
            position: 0,
            total: 1,
            preparation_failed: false,
        });
        projection.record(&child, Storage::Saved);
        assert!(!progress.is_complete());
        projection.record(&succeeded(), Storage::Saved);
        assert!(progress.is_complete());
        assert!(path.is_file());
    }
}
