//! Diagnostics and recordings: buffers the extension keeps for the service.
//!
//! The extension keeps recordings without knowing workspaces. The service resolves every
//! recording a workspace names through the connection's
//! [`RecordingIndex`](super::super::link::RecordingIndex), so a workspace reaches only the
//! recordings it started.
//!
//! A diagnostics read is bound to the documents the authority admitted: the extension resets a
//! tab's retained entries when the binding changes, so entries gathered under one admitted set
//! never reach a request admitted for another. A recording started under a watching scope stops
//! when the admitted documents change.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::super::link::Link;
use super::super::locator::Locator;
use super::super::scope::{PlannedCall, Scope};
use super::super::wire;
use super::super::world;
use crate::browser::adapter::{BrowserOutcome, DiagnosticDetail, DiagnosticSource};
use crate::browser::BrowserError;

/// The protocol failure of recording facts this service cannot read.
const UNREADABLE_RECORDING: &str = "recording facts carry a value this service does not know";

/// What separates the document names a binding digests.
const ID_SEPARATOR: &str = "\n";
/// The protocol failure of a diagnostic entry this service cannot read.
const UNREADABLE_DIAGNOSTIC: &str = "a diagnostic entry carries a value this service does not know";

/// One diagnostics read as work names it.
#[derive(Clone, Debug)]
pub struct DiagnosticsRead {
    /// The tab to read.
    pub tab_id: u64,
    /// Which sources to read.
    pub source: DiagnosticSource,
    /// How much to return.
    pub detail: DiagnosticDetail,
    /// Text an entry must contain.
    pub match_text: Option<String>,
    /// Cursor after which to read.
    pub after: Option<String>,
    /// Most entries to return.
    pub limit: u16,
}

/// Read the diagnostics retained for one tab, bound to the admitted documents and guarded by
/// the scope.
///
/// # Errors
///
/// Returns the primitive's failure, and [`BrowserError::Protocol`] for an entry this service
/// cannot read.
pub fn read_diagnostics(
    l: &mut Link<'_>,
    scope: Option<&Scope>,
    read: DiagnosticsRead,
) -> Result<BrowserOutcome, BrowserError> {
    let facts = l.send(wire::DiagnosticsOp::Read {
        tab_id: read.tab_id,
        source: read.source.into(),
        detail: read.detail.into(),
        match_text: read.match_text,
        after: read.after,
        limit: read.limit,
        binding: scope.map(binding),
        guard: scope.map(Scope::guard),
    })?;
    let entries = facts
        .entries
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| BrowserError::Protocol(UNREADABLE_DIAGNOSTIC.into()))?;
    Ok(BrowserOutcome::DiagnosticsRead {
        tab_id: read.tab_id,
        entries,
        cursor: facts.cursor,
        truncated: facts.truncated,
        evicted: facts.evicted,
        capture_started: facts.capture_started,
        omitted_count: facts.omitted_count,
    })
}

/// The key of an admitted document set: the hex SHA-256 of its sorted document names, one per
/// line.
#[must_use]
pub fn binding(scope: &Scope) -> String {
    let digest = Sha256::digest(scope.allowed_ids().join(ID_SEPARATOR).as_bytes());
    let mut key = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut key, "{byte:02x}").expect("writing to a string cannot fail");
    }
    key
}

/// Start recording one tab. Under a scope that watches its documents, the recording stops when
/// they change. The workspace learns the recording's id.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn start(
    l: &mut Link<'_>,
    scope: Option<&Scope>,
    tab_id: u64,
) -> Result<BrowserOutcome, BrowserError> {
    let facts = l.send(wire::RecordingOp::Start {
        tab_id,
        watch: scope.and_then(Scope::watch),
    })?;
    recorded(l, "", facts)
}

/// Erase the diagnostics retained for some tabs.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn clear_diagnostics(
    l: &mut Link<'_>,
    tab_ids: Vec<u64>,
) -> Result<BrowserOutcome, BrowserError> {
    let facts = l.send(wire::DiagnosticsOp::Clear { tab_ids })?;
    Ok(BrowserOutcome::DiagnosticsCleared {
        cleared_count: facts.cleared_count,
    })
}

/// Report one recording's state.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn status(
    l: &mut Link<'_>,
    recording_id: Option<String>,
) -> Result<BrowserOutcome, BrowserError> {
    with_recording(l, recording_id, |recording_id| wire::RecordingOp::Status {
        recording_id,
    })
}

/// Stop capturing one recording.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn stop(
    l: &mut Link<'_>,
    recording_id: Option<String>,
) -> Result<BrowserOutcome, BrowserError> {
    with_recording(l, recording_id, |recording_id| wire::RecordingOp::Stop {
        recording_id,
    })
}

/// Encode one recording and deliver it to `destination`.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn export(
    l: &mut Link<'_>,
    recording_id: Option<String>,
    destination: wire::RecordingTarget,
    max_output_bytes: usize,
) -> Result<BrowserOutcome, BrowserError> {
    with_recording(l, recording_id, |recording_id| wire::RecordingOp::Export {
        recording_id,
        destination,
        max_output_bytes,
    })
}

/// Where an export attaches a recording in a page.
#[derive(Clone, Copy, Debug)]
pub struct PageExport<'a> {
    /// The tab that receives the GIF.
    pub tab_id: u64,
    /// The encoded locator of the file input that takes it.
    pub locator: &'a str,
    /// The name the file carries.
    pub file_name: &'a str,
}

/// Encode one recording and attach it to a file input of a page.
///
/// The extension runs the attach call with the GIF as its attachment, after checking the guard,
/// so the GIF never passes through this service. The call goes through the scope like any other
/// content call, and a document the scope does not allow is refused before anything is sent.
///
/// # Errors
///
/// Returns the plain failure of a malformed locator or a refused document, and the primitive's
/// failure.
pub fn export_to_page(
    l: &mut Link<'_>,
    scope: Option<&Scope>,
    recording_id: Option<String>,
    page: PageExport<'_>,
    max_output_bytes: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let target = Locator::decode(page.locator)?;
    let message = world::attach_file(&target.local, page.file_name);
    let plan = match scope {
        Some(scope) => scope.call(&target.doc, message)?,
        None => PlannedCall::unscoped(&target.doc, message),
    };
    export(
        l,
        recording_id,
        wire::RecordingTarget::Page {
            tab_id: page.tab_id,
            call: plan.call,
            guard: scope.map(Scope::guard),
        },
        max_output_bytes,
    )
}

/// Erase one recording.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn discard(
    l: &mut Link<'_>,
    recording_id: Option<String>,
) -> Result<BrowserOutcome, BrowserError> {
    with_recording(l, recording_id, |recording_id| wire::RecordingOp::Discard {
        recording_id,
    })
}

/// Resolve the recording a workspace names and run one operation on it.
///
/// A named recording must be one the workspace started. An omitted one is the workspace's only
/// recording. With none there is nothing to find, and with several the caller must choose; in
/// both cases nothing is sent.
fn with_recording(
    l: &mut Link<'_>,
    recording_id: Option<String>,
    op: impl FnOnce(String) -> wire::RecordingOp,
) -> Result<BrowserOutcome, BrowserError> {
    let started = l.recordings().ids(l.workspace());
    let recording_id = match recording_id {
        Some(recording_id) if started.contains(&recording_id) => recording_id,
        Some(_) => return Ok(BrowserOutcome::RecordingNotFound),
        None => match started.as_slice() {
            [] => return Ok(BrowserOutcome::RecordingNotFound),
            [only] => only.clone(),
            _ => {
                return Ok(BrowserOutcome::RecordingAmbiguous {
                    recording_ids: started,
                })
            }
        },
    };
    let facts = l.send(op(recording_id.clone()))?;
    recorded(l, &recording_id, facts)
}

/// The outcome a recording operation reported, keeping the workspace's index current: a
/// started recording is learned, and a discarded or missing one is forgotten.
///
/// # Errors
///
/// Returns [`BrowserError::Protocol`] for facts this service cannot read.
pub fn recorded(
    l: &Link<'_>,
    recording_id: &str,
    facts: wire::RecordingFacts,
) -> Result<BrowserOutcome, BrowserError> {
    let unknown = |_| BrowserError::Protocol(UNREADABLE_RECORDING.into());
    Ok(match facts {
        wire::RecordingFacts::Started { summary, existing } => {
            l.recordings().learn(l.workspace(), &summary.recording_id);
            BrowserOutcome::RecordingStarted {
                summary: summary.try_into().map_err(unknown)?,
                existing,
            }
        }
        wire::RecordingFacts::Status { summary } => BrowserOutcome::RecordingStatus {
            summary: summary.try_into().map_err(unknown)?,
        },
        wire::RecordingFacts::Stopped { summary, changed } => BrowserOutcome::RecordingStopped {
            summary: summary.try_into().map_err(unknown)?,
            changed,
        },
        wire::RecordingFacts::Exported {
            summary,
            encoded,
            delivery,
        } => BrowserOutcome::RecordingExported {
            summary: summary.try_into().map_err(unknown)?,
            encoded: encoded.into(),
            delivery: delivery.try_into().map_err(unknown)?,
        },
        wire::RecordingFacts::ExportFailed { reason } => {
            BrowserOutcome::RecordingExportFailed { reason }
        }
        wire::RecordingFacts::Discarded {
            recording_id: discarded,
            released_bytes,
        } => {
            l.recordings().forget(l.workspace(), &discarded);
            BrowserOutcome::RecordingDiscarded {
                recording_id: discarded,
                released_bytes,
            }
        }
        wire::RecordingFacts::NotFound => {
            l.recordings().forget(l.workspace(), recording_id);
            BrowserOutcome::RecordingNotFound
        }
        wire::RecordingFacts::Unknown => {
            return Err(BrowserError::Protocol(UNREADABLE_RECORDING.into()))
        }
    })
}
