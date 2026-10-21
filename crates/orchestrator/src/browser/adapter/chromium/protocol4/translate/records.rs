//! Diagnostics and recordings: buffers the extension keeps for the service.
//!
//! The extension keeps recordings without knowing workspaces, and every recording command names its
//! recording. Which recording a workspace holds, and whether a save has delivered it, is the
//! workspace's own fact (ADR-0203 Decision 6.7), so a relay restart does not lose it.
//!
//! A diagnostics read under a watching scope is bound to the documents the authority admitted:
//! the extension resets a tab's retained entries when the binding changes, so entries gathered
//! under one admitted set never reach a request admitted for another. A read the authority did
//! not ask to watch sends no binding: its policy admits every document, so there is no other set
//! to keep entries from, and an iframe that reloads leaves the tab's history in place. A
//! recording started under a watching scope stops when the admitted documents change.

use super::super::link::Link;
use super::super::locator::Locator;
use super::super::scope::Scope;
use super::super::wire;
use super::super::world;
use crate::browser::adapter::{BrowserOutcome, PhysicalDiagnosticsRead};
use crate::browser::BrowserError;
use crate::install::icons::sha256_hex;

/// The protocol failure of recording facts this service cannot read.
const UNREADABLE_RECORDING: &str = "recording facts carry a value this service does not know";

/// What separates the document names a binding digests.
const ID_SEPARATOR: &str = "\n";
/// The protocol failure of a diagnostic entry this service cannot read.
const UNREADABLE_DIAGNOSTIC: &str = "a diagnostic entry carries a value this service does not know";

/// Read the diagnostics retained for one tab, guarded by the scope, and bound to the admitted
/// documents when the scope watches them.
///
/// # Errors
///
/// Returns the primitive's failure, and [`BrowserError::Protocol`] for an entry this service
/// cannot read.
pub fn read_diagnostics(
    l: &mut Link<'_>,
    scope: &Scope,
    read: PhysicalDiagnosticsRead,
) -> Result<BrowserOutcome, BrowserError> {
    let facts = l.send(wire::DiagnosticsOp::Read {
        tab_id: read.tab_id,
        source: read.source.into(),
        detail: read.detail.into(),
        match_text: read.match_text,
        after: read.after,
        limit: read.limit,
        binding: scope.watches().then(|| binding(scope)),
        guard: Some(scope.guard()),
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
    sha256_hex(scope.allowed_ids().join(ID_SEPARATOR).as_bytes())
}

/// Start a recording of one tab. Under a scope that watches its documents, the recording stops when
/// they change. Asking again for the tab a recording is still capturing answers with that
/// recording.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn start(l: &mut Link<'_>, scope: &Scope, tab_id: u64) -> Result<BrowserOutcome, BrowserError> {
    let facts = l.send(wire::RecordingOp::Start {
        tab_id,
        watch: scope.watch(),
    })?;
    recorded(facts)
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

/// Report the state of the recording `recording_id` names.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn status(l: &mut Link<'_>, recording_id: String) -> Result<BrowserOutcome, BrowserError> {
    recorded(l.send(wire::RecordingOp::Status { recording_id })?)
}

/// Stop capturing the recording `recording_id` names.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn stop(l: &mut Link<'_>, recording_id: String) -> Result<BrowserOutcome, BrowserError> {
    recorded(l.send(wire::RecordingOp::Stop { recording_id })?)
}

/// Encode the recording `recording_id` names and deliver it to `destination`.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn export(
    l: &mut Link<'_>,
    recording_id: String,
    destination: wire::RecordingTarget,
    max_output_bytes: usize,
) -> Result<BrowserOutcome, BrowserError> {
    recorded(l.send(wire::RecordingOp::Export {
        recording_id,
        destination,
        max_output_bytes,
    })?)
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

/// Encode the recording `recording_id` names and attach it to a file input of a page.
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
    scope: &Scope,
    recording_id: String,
    page: PageExport<'_>,
    max_output_bytes: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let target = Locator::decode(page.locator)?;
    let plan = scope.call(
        &target.doc,
        world::attach_file(&target.local, page.file_name),
    )?;
    export(
        l,
        recording_id,
        wire::RecordingTarget::Page {
            tab_id: page.tab_id,
            call: plan.call,
            guard: Some(scope.guard()),
        },
        max_output_bytes,
    )
}

/// Erase the recording `recording_id` names.
///
/// # Errors
///
/// Returns the primitive's failure.
pub fn discard(l: &mut Link<'_>, recording_id: String) -> Result<BrowserOutcome, BrowserError> {
    recorded(l.send(wire::RecordingOp::Discard { recording_id })?)
}

/// The outcome a recording operation reported.
///
/// # Errors
///
/// Returns [`BrowserError::Protocol`] for facts this service cannot read.
pub fn recorded(facts: wire::RecordingFacts) -> Result<BrowserOutcome, BrowserError> {
    let unknown = |_| BrowserError::Protocol(UNREADABLE_RECORDING.into());
    Ok(match facts {
        wire::RecordingFacts::Started { summary, existing } => BrowserOutcome::RecordingStarted {
            recording_id: summary.recording_id.clone(),
            summary: summary.try_into().map_err(unknown)?,
            existing,
        },
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
        wire::RecordingFacts::ExportFailed { reason } => BrowserOutcome::RecordingExportFailed {
            page_refused: reason == wire::PAGE_REFUSED_REPLAY,
            reason,
        },
        wire::RecordingFacts::Discarded { released_bytes, .. } => {
            BrowserOutcome::RecordingDiscarded { released_bytes }
        }
        wire::RecordingFacts::NotFound => BrowserOutcome::RecordingNotFound,
        wire::RecordingFacts::Unknown => {
            return Err(BrowserError::Protocol(UNREADABLE_RECORDING.into()))
        }
    })
}
