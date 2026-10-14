//! Diagnostics and recordings: buffers the extension keeps for the service.
//!
//! The extension keeps recordings without knowing workspaces. The service resolves every
//! recording a workspace names through the connection's
//! [`RecordingIndex`](super::super::link::RecordingIndex), so a workspace reaches only the
//! recordings it started.

use super::super::link::Link;
use super::super::wire;
use crate::browser::adapter::BrowserOutcome;
use crate::browser::BrowserError;

/// The protocol failure of recording facts this service cannot read.
const UNREADABLE_RECORDING: &str = "recording facts carry a value this service does not know";

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
