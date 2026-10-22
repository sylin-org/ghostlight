//! The ports the workbench reaches the operating system and its screens through, and their
//! failures.
//!
//! The service attaches each port; the workbench owns what to show, open, end or tell, and the
//! adapter behind a port owns how this computer does it. Every port is best-effort: a failure here
//! is a presentation failure and never reaches domain authority, governance, or completion truth.

use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::install::HarnessError;
use crate::intake::desktop::TrayModel;
use crate::language::workbench as words;

use super::{PageAddress, WorkbenchChange};

/// Best-effort operating-system presentation port.
pub trait WorkbenchPresentationPort: Send + Sync {
    /// Reveal and focus the existing native workbench surface, at `address` when one is given.
    fn reveal(&self, address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError>;

    /// Deliver one high-signal, content-free notification.
    fn notify(&self, notification: WorkbenchNotification)
        -> Result<(), WorkbenchPresentationError>;
}

/// What only the operating system can do for the workbench: open a link, open a folder, ask for
/// a file.
///
/// The service owns what to open and when; the adapter that implements this owns how this
/// computer does it. It is kept apart from [`WorkbenchPresentationPort`] so a presentation
/// adapter that only shows and notifies needs none of it.
pub trait WorkbenchNativeActs: Send + Sync {
    /// Open an address in the person's browser. The facade passes only `https://` addresses.
    fn open_url(&self, url: &str) -> Result<(), WorkbenchPresentationError>;

    /// Show a folder in the file manager.
    fn open_folder(&self, path: &Path) -> Result<(), WorkbenchPresentationError>;

    /// Ask the person to choose a file, under `title`. Answers `None` when they close the dialog.
    /// The dialog blocks the calling thread until the person answers.
    fn pick_file(&self, title: &str) -> Result<Option<PathBuf>, WorkbenchPresentationError>;
}

/// What ends the service when the person asks for it.
///
/// The service attaches it, so the workbench asks for a Quit without depending on how the service
/// ends.
pub trait WorkbenchQuitPort: Send + Sync {
    /// Begin a person's Quit. Returns at once; the service finishes what is in flight first.
    fn quit(&self);
}

/// What draws the tray when the service describes it differently.
///
/// The service attaches it, so the workbench says what the tray is without depending on how it
/// reaches the shell that draws it.
pub trait WorkbenchTrayPort: Send + Sync {
    /// The tray changed. This returns at once: it is called from inside the refresh of the live
    /// parts, so it must not wait on anything slow.
    fn tray_changed(&self, model: TrayModel);
}

/// Why a native act did not happen.
#[derive(Debug, Error)]
pub enum NativeActError {
    /// No adapter is attached, or this computer refused.
    #[error(transparent)]
    Workbench(#[from] WorkbenchError),
    /// The client roster refused: an unknown product or a path that is not a client.
    #[error(transparent)]
    Harness(#[from] HarnessError),
    /// Process diagnostics are off, so there is no folder to show.
    #[error("{}", words::DIAGNOSTICS_OFF)]
    DiagnosticsOff,
    /// The address is not a secure one.
    #[error("{}", words::LINK_NOT_SECURE)]
    LinkNotSecure,
}

/// A refusal of the adapter, as the failure of the act.
pub(super) fn native(error: WorkbenchPresentationError) -> NativeActError {
    WorkbenchError::from(error).into()
}

/// Best-effort outbound port for sequenced workbench changes.
///
/// Delivery failures are presentation failures. The adapter contains them, and they never reach
/// domain authority, governance, or completion truth.
pub trait WorkbenchEventSink: Send + Sync {
    /// Deliver one sequenced change to the attached presentation surface.
    fn publish(&self, event: WorkbenchEvent);
}

/// One sequenced workbench change fact for a disposable presentation surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkbenchEvent {
    /// Monotonic publication sequence.
    ///
    /// A surface that receives a sequence other than its last plus one has missed a change and
    /// must resynchronize from a fresh snapshot rather than trust its local cache.
    pub seq: u64,
    /// What changed.
    pub change: WorkbenchChange,
}

/// One content-free notification decision made by the orchestrator.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkbenchNotification {
    /// Semantic reason for the notification.
    pub kind: NotificationKind,
    /// Fixed Ghostlight-authored title.
    pub title: String,
    /// Fixed Ghostlight-authored body.
    pub body: String,
}

/// Closed notification reasons.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// Governance blocked work.
    Blocked,
    /// The user explicitly tested notifications.
    Checkup,
}

/// Failure of the replaceable operating-system presentation adapter.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WorkbenchPresentationError {
    /// Native notification delivery failed.
    #[error("native presentation failed: {0}")]
    Native(String),
    /// The computer refused an open or a dialog. The text is the computer's own.
    #[error("{0}")]
    ActFailed(String),
    /// There is no window program to carry the request out, or it went away before it answered.
    #[error("{}", words::PRESENTATION_UNAVAILABLE)]
    Unavailable,
}

/// Failure at the typed workbench application boundary.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WorkbenchError {
    /// No native presentation adapter is currently attached.
    #[error("{}", words::PRESENTATION_UNAVAILABLE)]
    PresentationUnavailable,
    /// Native presentation rejected the request.
    #[error(transparent)]
    Presentation(WorkbenchPresentationError),
    /// No service is attached to end.
    #[error("{}", words::QUIT_UNAVAILABLE)]
    QuitUnavailable,
    /// A setup step names a browser or profile this computer no longer offers, or is not a step
    /// the service performs.
    #[error("{}", words::SETUP_STEP_NOT_OFFERED)]
    SetupStepNotOffered,
    /// A setup step would start a process while the session is ending.
    #[error("{}", words::SETUP_SESSION_ENDING)]
    SetupSessionEnding,
    /// The computer would not start the browser a setup step names.
    #[error("{}", words::SETUP_BROWSER_NOT_STARTED)]
    SetupBrowserNotStarted,
}

impl From<WorkbenchPresentationError> for WorkbenchError {
    /// A presentation adapter with nothing to talk to is the facade's own unavailable failure, so
    /// a person reads one failure however the adapter came to be missing.
    fn from(error: WorkbenchPresentationError) -> Self {
        match error {
            WorkbenchPresentationError::Unavailable => Self::PresentationUnavailable,
            other => Self::Presentation(other),
        }
    }
}
