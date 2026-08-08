// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Workspace-owned, memory-only GIF recording state (ADR-0073, amended by ADR-0096).
//!
//! The extension supplies Chrome mechanics and compressed frames. This module owns recording
//! identity, state transitions, bounds, action tagging, deadlines, and erasure. Captured bytes
//! never touch a filesystem here.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::b64;
use crate::gif::{take_action_for_frame, ActionMeta, RecordedFrame};

/// Maximum kept frames in one recording. Byte bounds remain authoritative when frames are large.
pub(crate) const MAX_FRAMES: usize = 100;
/// Maximum compressed bytes held by one recording before ordinary frames are thinned.
pub(crate) const MAX_RECORDING_BYTES: usize = 16 * 1024 * 1024;
/// Maximum accepted compressed bytes for one frame.
pub(crate) const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;
/// Process-wide ceiling for all retained compressed recording bytes.
pub(crate) const MAX_GLOBAL_RECORDING_BYTES: usize = 64 * 1024 * 1024;
/// Bound on actions awaiting the first kept frame painted at or after their timestamp.
const PENDING_ACTION_BOUND: usize = 20;

/// Default inactivity window. Relevant same-surface browser activity refreshes this deadline.
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Absolute recording lifetime. This deadline never refreshes.
pub(crate) const HARD_TIMEOUT: Duration = Duration::from_secs(120);
/// Frozen/interrupted content lifetime. Status and export do not refresh it.
pub(crate) const RETENTION_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Extension-side health lease. Renewal is a backstop; a native-port disconnect stops capture
/// immediately without waiting for this interval.
pub(crate) const HEALTH_LEASE: Duration = Duration::from_secs(15);
/// Service renewal cadence for the extension-side health lease.
pub(crate) const HEALTH_RENEW_INTERVAL: Duration = Duration::from_secs(5);

/// The browser surface whose pixels a recording contains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SurfaceId {
    pub(crate) slot: u32,
    pub(crate) native_tab: i64,
}

/// Opaque identity returned to callers and carried by every capture frame.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RecordingId(String);

impl RecordingId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Explicit recording lifecycle. No hidden `active` boolean exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecordingState {
    Starting,
    Recording,
    Finalizing,
    Frozen,
    Interrupted,
    Erased,
    Expired,
}

impl RecordingState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Recording => "recording",
            Self::Finalizing => "finalizing",
            Self::Frozen => "frozen",
            Self::Interrupted => "interrupted",
            Self::Erased => "erased",
            Self::Expired => "expired",
        }
    }

    fn accepts_frames(self) -> bool {
        matches!(self, Self::Starting | Self::Recording | Self::Finalizing)
    }
}

/// Why capture stopped or content disappeared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StopReason {
    Explicit,
    IdleTimeout,
    HardTimeout,
    LeaseExpired,
    BrowserDisconnected,
    MemoryLimit,
    InvalidFrame,
    SessionEnded,
    Panic,
    PolicyChanged,
    UserHold,
    Cleared,
    RetentionExpired,
    FinalizeFailed,
}

impl StopReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::IdleTimeout => "idle_timeout",
            Self::HardTimeout => "hard_timeout",
            Self::LeaseExpired => "lease_expired",
            Self::BrowserDisconnected => "browser_disconnected",
            Self::MemoryLimit => "memory_limit",
            Self::InvalidFrame => "invalid_frame",
            Self::SessionEnded => "session_ended",
            Self::Panic => "panic",
            Self::PolicyChanged => "policy_changed",
            Self::UserHold => "user_hold",
            Self::Cleared => "cleared",
            Self::RetentionExpired => "retention_expired",
            Self::FinalizeFailed => "finalize_failed",
        }
    }
}

/// Token proving which staging/finalizing generation an asynchronous browser reply belongs to.
#[derive(Clone, Debug)]
pub(crate) struct RecordingTicket {
    pub(crate) id: RecordingId,
    pub(crate) generation: u64,
    pub(crate) surface: SurfaceId,
}

/// A recording whose browser-side final-frame barrier is due now.
#[derive(Clone, Debug)]
pub(crate) struct DueFinalization {
    pub(crate) owner: String,
    pub(crate) ticket: RecordingTicket,
    pub(crate) reason: StopReason,
}

/// An active extension-side capture lease that should be renewed.
#[derive(Clone, Debug)]
pub(crate) struct LeaseTarget {
    pub(crate) ticket: RecordingTicket,
    pub(crate) owner: String,
}

/// Content-free recording status safe to return or log.
#[derive(Clone, Debug)]
pub(crate) struct RecordingSummary {
    pub(crate) id: RecordingId,
    pub(crate) state: RecordingState,
    pub(crate) surface: SurfaceId,
    pub(crate) frame_count: usize,
    pub(crate) bytes_held: usize,
    pub(crate) duration_ms: u64,
    pub(crate) idle_remaining_ms: Option<u64>,
    pub(crate) hard_remaining_ms: Option<u64>,
    pub(crate) expires_at_ms: Option<u64>,
    pub(crate) stop_reason: Option<StopReason>,
}

/// Why an acknowledged browser-side recording start could not become the active local capture.
#[derive(Clone, Debug)]
pub(crate) enum CommitStartError {
    /// A hold, disconnect, or other terminal event interrupted the staged generation first.
    Interrupted(RecordingSummary),
    /// The generation was removed or replaced before the acknowledgement arrived.
    Stale,
}

/// Coordinator decision for one extension recording frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrameAdmission {
    /// The frame was accepted, including an already-accepted duplicate sequence.
    Accepted,
    /// This exact live generation was rejected and transitioned to Interrupted. Queue one cancel.
    RejectAndCancel,
    /// The identity was stale, unknown, or already stopped. Do not enqueue another cancel.
    Ignored,
}

struct StoredFrame {
    frame: RecordedFrame,
    protected: bool,
}

struct Recording {
    id: RecordingId,
    owner: String,
    surface: SurfaceId,
    generation: u64,
    state: RecordingState,
    started_at: Instant,
    last_activity: Instant,
    hard_deadline: Instant,
    retention_deadline: Option<Instant>,
    retention_wall_ms: Option<u64>,
    frames: Vec<StoredFrame>,
    bytes_held: usize,
    next_seq: u64,
    vp_w: Option<f64>,
    pending: Vec<ActionMeta>,
    in_flight: usize,
    stop_reason: Option<StopReason>,
}

impl Recording {
    fn summary(&self, now: Instant) -> RecordingSummary {
        RecordingSummary {
            id: self.id.clone(),
            state: self.state,
            surface: self.surface,
            frame_count: self.frames.len(),
            bytes_held: self.bytes_held,
            duration_ms: now
                .saturating_duration_since(self.started_at)
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            idle_remaining_ms: self.state.accepts_frames().then(|| {
                self.last_activity
                    .checked_add(IDLE_TIMEOUT)
                    .unwrap_or(now)
                    .saturating_duration_since(now)
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX)
            }),
            hard_remaining_ms: self.state.accepts_frames().then(|| {
                self.hard_deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX)
            }),
            expires_at_ms: self.retention_wall_ms,
            stop_reason: self.stop_reason,
        }
    }
}

#[derive(Default)]
struct Inner {
    next_generation: u64,
    records: HashMap<RecordingId, Recording>,
    current: HashMap<SurfaceId, RecordingId>,
    staging: HashMap<SurfaceId, RecordingId>,
    tombstones: HashMap<(String, SurfaceId), RecordingSummary>,
}

/// All workspace-owned recordings in this service process.
pub(crate) struct RecordingCoordinator {
    inner: Mutex<Inner>,
}

impl RecordingCoordinator {
    /// Create an empty memory-only coordinator.
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Stage a transactional start. Existing active capture is reported; frozen content stays
    /// intact until [`Self::commit_start`] succeeds.
    pub(crate) fn begin_start(
        &self,
        owner: &str,
        surface: SurfaceId,
    ) -> Result<RecordingTicket, RecordingSummary> {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        if let Some(id) = inner.current.get(&surface) {
            let existing = inner.records.get(id).expect("current recording exists");
            if existing.state.accepts_frames() {
                return Err(existing.summary(now));
            }
        }
        if let Some(id) = inner.staging.get(&surface) {
            let existing = inner.records.get(id).expect("staging recording exists");
            return Err(existing.summary(now));
        }

        inner.next_generation = inner.next_generation.wrapping_add(1).max(1);
        let generation = inner.next_generation;
        let id = RecordingId(format!("rec_{}", uuid::Uuid::new_v4().simple()));
        inner.records.insert(
            id.clone(),
            Recording {
                id: id.clone(),
                owner: owner.to_string(),
                surface,
                generation,
                state: RecordingState::Starting,
                started_at: now,
                last_activity: now,
                hard_deadline: now + HARD_TIMEOUT,
                retention_deadline: None,
                retention_wall_ms: None,
                frames: Vec::new(),
                bytes_held: 0,
                next_seq: 0,
                vp_w: None,
                pending: Vec::new(),
                in_flight: 0,
                stop_reason: None,
            },
        );
        inner.staging.insert(surface, id.clone());
        Ok(RecordingTicket {
            id,
            generation,
            surface,
        })
    }

    /// Commit a successful extension start and atomically replace any prior frozen recording.
    pub(crate) fn commit_start(
        &self,
        ticket: &RecordingTicket,
        vp_w: Option<f64>,
    ) -> Result<RecordingSummary, CommitStartError> {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        if inner.staging.get(&ticket.surface) != Some(&ticket.id) {
            return Err(CommitStartError::Stale);
        }
        let replaces_current = inner.current.get(&ticket.surface).cloned();
        let Some(record) = inner.records.get_mut(&ticket.id) else {
            return Err(CommitStartError::Stale);
        };
        if record.generation != ticket.generation {
            return Err(CommitStartError::Stale);
        }
        if record.state == RecordingState::Interrupted {
            if replaces_current.is_some() {
                record.frames.clear();
                record.bytes_held = 0;
                record.pending.clear();
            }
            let summary = record.summary(now);
            inner.staging.remove(&ticket.surface);
            if replaces_current.is_some() {
                inner.records.remove(&ticket.id);
            } else {
                inner.current.insert(ticket.surface, ticket.id.clone());
            }
            return Err(CommitStartError::Interrupted(summary));
        }
        if record.state != RecordingState::Starting {
            return Err(CommitStartError::Stale);
        }
        record.state = RecordingState::Recording;
        record.vp_w = vp_w;
        inner.staging.remove(&ticket.surface);
        if let Some(old) = inner.current.insert(ticket.surface, ticket.id.clone()) {
            if old != ticket.id {
                inner.records.remove(&old);
            }
        }
        inner
            .records
            .get(&ticket.id)
            .map(|record| record.summary(now))
            .ok_or(CommitStartError::Stale)
    }

    /// Roll back a staging start without touching the prior committed recording.
    pub(crate) fn fail_start(&self, ticket: &RecordingTicket) {
        let mut inner = self.inner.lock().unwrap();
        if inner.staging.get(&ticket.surface) == Some(&ticket.id) {
            inner.staging.remove(&ticket.surface);
            inner.records.remove(&ticket.id);
        }
    }

    /// Whether this owner currently records the surface.
    pub(crate) fn is_active(&self, owner: &str, surface: SurfaceId) -> bool {
        let inner = self.inner.lock().unwrap();
        let Some(id) = inner.current.get(&surface) else {
            return false;
        };
        inner
            .records
            .get(id)
            .is_some_and(|r| r.owner == owner && r.state == RecordingState::Recording)
    }

    /// Note an action for the first subsequently painted kept frame.
    pub(crate) fn note_action(&self, owner: &str, surface: SurfaceId, meta: ActionMeta) {
        let mut inner = self.inner.lock().unwrap();
        let Some(id) = inner.current.get(&surface).cloned() else {
            return;
        };
        let Some(record) = inner
            .records
            .get_mut(&id)
            .filter(|r| r.owner == owner && r.state == RecordingState::Recording)
        else {
            return;
        };
        record.pending.push(meta);
        if record.pending.len() > PENDING_ACTION_BOUND {
            record.pending.remove(0);
        }
    }

    /// Admit a relevant same-surface browser operation and refresh the idle window. Returns true
    /// only when a matching recording was active, so completion can balance the in-flight count.
    pub(crate) fn begin_activity(&self, owner: &str, surface: SurfaceId) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let Some(id) = inner.current.get(&surface).cloned() else {
            return false;
        };
        let Some(record) = inner
            .records
            .get_mut(&id)
            .filter(|r| r.owner == owner && r.state == RecordingState::Recording)
        else {
            return false;
        };
        record.last_activity = Instant::now();
        record.in_flight = record.in_flight.saturating_add(1);
        true
    }

    /// Complete a previously admitted browser operation and refresh the idle window once more.
    pub(crate) fn finish_activity(&self, owner: &str, surface: SurfaceId) {
        let mut inner = self.inner.lock().unwrap();
        let Some(id) = inner.current.get(&surface).cloned() else {
            return;
        };
        let Some(record) = inner.records.get_mut(&id).filter(|r| r.owner == owner) else {
            return;
        };
        record.in_flight = record.in_flight.saturating_sub(1);
        if record.state == RecordingState::Recording {
            record.last_activity = Instant::now();
        }
    }

    /// Accept one base64 JPEG only when its complete identity matches the current generation.
    /// A rejected live generation asks the caller for exactly one cancel; repeats are ignored.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn on_frame(
        &self,
        surface: SurfaceId,
        recording_id: &str,
        generation: u64,
        sequence: u64,
        data_b64: &str,
        ts_ms: i64,
        device_width: Option<f64>,
        final_frame: bool,
    ) -> FrameAdmission {
        if data_b64.len() > MAX_FRAME_BYTES.saturating_mul(4).saturating_div(3) + 8 {
            return self.reject_frame(surface, recording_id, generation, StopReason::InvalidFrame);
        }
        let Some(bytes) = b64::decode(data_b64) else {
            return self.reject_frame(surface, recording_id, generation, StopReason::InvalidFrame);
        };
        if bytes.len() > MAX_FRAME_BYTES {
            return self.reject_frame(surface, recording_id, generation, StopReason::InvalidFrame);
        }

        let mut inner = self.inner.lock().unwrap();
        let global_bytes = inner.records.values().fold(0usize, |total, record| {
            total.saturating_add(record.bytes_held)
        });
        let Some(record) = inner
            .records
            .get_mut(&RecordingId(recording_id.to_string()))
        else {
            return FrameAdmission::Ignored;
        };
        if record.surface != surface
            || record.generation != generation
            || !record.state.accepts_frames()
        {
            return FrameAdmission::Ignored;
        }
        if sequence < record.next_seq {
            return FrameAdmission::Accepted;
        }
        if global_bytes.saturating_add(bytes.len()) > MAX_GLOBAL_RECORDING_BYTES {
            let now = Instant::now();
            record.state = RecordingState::Interrupted;
            record.stop_reason = Some(StopReason::MemoryLimit);
            record.retention_deadline = Some(now + RETENTION_TIMEOUT);
            record.retention_wall_ms =
                Some(wall_ms().saturating_add(RETENTION_TIMEOUT.as_millis() as u64));
            return FrameAdmission::RejectAndCancel;
        }
        record.next_seq = sequence.saturating_add(1);
        let action = take_action_for_frame(&mut record.pending, ts_ms);
        let protected = record.frames.is_empty() || action.is_some() || final_frame;
        let byte_len = bytes.len();
        record.frames.push(StoredFrame {
            frame: RecordedFrame::new(bytes, ts_ms, device_width.or(record.vp_w), action),
            protected,
        });
        record.bytes_held = record.bytes_held.saturating_add(byte_len);
        thin_to_bounds(record);
        if record.state == RecordingState::Interrupted {
            FrameAdmission::RejectAndCancel
        } else {
            FrameAdmission::Accepted
        }
    }

    fn reject_frame(
        &self,
        surface: SurfaceId,
        recording_id: &str,
        generation: u64,
        reason: StopReason,
    ) -> FrameAdmission {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        let Some(record) = inner
            .records
            .get_mut(&RecordingId(recording_id.to_string()))
            .filter(|record| {
                record.surface == surface
                    && record.generation == generation
                    && record.state.accepts_frames()
            })
        else {
            return FrameAdmission::Ignored;
        };
        Self::interrupt_record(record, reason, now);
        FrameAdmission::RejectAndCancel
    }

    /// Move an active recording into finalization and return its generation token.
    pub(crate) fn begin_finalizing(
        &self,
        owner: &str,
        surface: SurfaceId,
    ) -> Result<RecordingTicket, Option<RecordingSummary>> {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        let Some(id) = inner.current.get(&surface).cloned() else {
            return Err(None);
        };
        let Some(record) = inner.records.get_mut(&id).filter(|r| r.owner == owner) else {
            return Err(None);
        };
        match record.state {
            RecordingState::Recording => {
                record.state = RecordingState::Finalizing;
                Ok(RecordingTicket {
                    id: record.id.clone(),
                    generation: record.generation,
                    surface,
                })
            }
            _ => Err(Some(record.summary(now))),
        }
    }

    /// Complete the finalization barrier as frozen or interrupted.
    pub(crate) fn finish_finalizing(
        &self,
        ticket: &RecordingTicket,
        success: bool,
        reason: StopReason,
    ) -> Option<RecordingSummary> {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        let record = inner.records.get_mut(&ticket.id)?;
        if record.generation != ticket.generation || record.state != RecordingState::Finalizing {
            return None;
        }
        record.state = if success {
            RecordingState::Frozen
        } else {
            RecordingState::Interrupted
        };
        record.stop_reason = Some(reason);
        record.retention_deadline = Some(now + RETENTION_TIMEOUT);
        record.retention_wall_ms =
            Some(wall_ms().saturating_add(RETENTION_TIMEOUT.as_millis() as u64));
        Some(record.summary(now))
    }

    /// Freeze an active recording without a browser barrier. Used only for a proven interruption.
    pub(crate) fn interrupt_surface(&self, surface: SurfaceId, reason: StopReason) {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        let ids = [
            inner.current.get(&surface).cloned(),
            inner.staging.get(&surface).cloned(),
        ];
        for id in ids.into_iter().flatten() {
            if let Some(record) = inner.records.get_mut(&id) {
                Self::interrupt_record(record, reason, now);
            }
        }
    }

    /// Interrupt one exact extension generation. Stale lease-expiry events cannot affect a newer
    /// recording on the same tab.
    pub(crate) fn interrupt_identity(
        &self,
        surface: SurfaceId,
        recording_id: &str,
        generation: u64,
        reason: StopReason,
    ) {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        let id = inner
            .staging
            .get(&surface)
            .filter(|id| id.as_str() == recording_id)
            .or_else(|| {
                inner
                    .current
                    .get(&surface)
                    .filter(|id| id.as_str() == recording_id)
            })
            .cloned();
        if let Some(record) = id.and_then(|id| inner.records.get_mut(&id)) {
            if record.generation == generation {
                Self::interrupt_record(record, reason, now);
            }
        }
    }

    fn interrupt_record(record: &mut Recording, reason: StopReason, now: Instant) {
        if record.state.accepts_frames() {
            record.state = RecordingState::Interrupted;
            record.stop_reason = Some(reason);
            record.retention_deadline = Some(now + RETENTION_TIMEOUT);
            record.retention_wall_ms =
                Some(wall_ms().saturating_add(RETENTION_TIMEOUT.as_millis() as u64));
        }
    }

    /// Interrupt every active recording routed through one disconnected browser slot.
    pub(crate) fn interrupt_slot(&self, slot: u32, reason: StopReason) {
        let surfaces: Vec<SurfaceId> = {
            let inner = self.inner.lock().unwrap();
            inner
                .records
                .values()
                .filter(|record| record.surface.slot == slot && record.state.accepts_frames())
                .map(|record| record.surface)
                .collect()
        };
        for surface in surfaces {
            self.interrupt_surface(surface, reason);
        }
    }

    /// Interrupt all active captures, used for the global take-the-wheel and panic paths.
    pub(crate) fn interrupt_all(&self, reason: StopReason) -> Vec<RecordingTicket> {
        let tickets: Vec<RecordingTicket> = {
            let inner = self.inner.lock().unwrap();
            inner
                .records
                .values()
                .filter(|record| record.state.accepts_frames())
                .map(|record| RecordingTicket {
                    id: record.id.clone(),
                    generation: record.generation,
                    surface: record.surface,
                })
                .collect()
        };
        for ticket in &tickets {
            self.interrupt_surface(ticket.surface, reason);
        }
        tickets
    }

    /// Move due active recordings into finalization and expire retained content. The caller owns
    /// the browser-side stop barrier for each returned item.
    pub(crate) fn poll_deadlines(&self) -> Vec<DueFinalization> {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        let mut due = Vec::new();
        for record in inner.records.values_mut() {
            if !matches!(
                record.state,
                RecordingState::Starting | RecordingState::Recording
            ) {
                continue;
            }
            let reason = if now >= record.hard_deadline {
                Some(StopReason::HardTimeout)
            } else if record.in_flight == 0
                && now.saturating_duration_since(record.last_activity) >= IDLE_TIMEOUT
            {
                Some(StopReason::IdleTimeout)
            } else {
                None
            };
            if let Some(reason) = reason {
                record.state = RecordingState::Finalizing;
                due.push(DueFinalization {
                    owner: record.owner.clone(),
                    ticket: RecordingTicket {
                        id: record.id.clone(),
                        generation: record.generation,
                        surface: record.surface,
                    },
                    reason,
                });
            }
        }

        let expired: Vec<RecordingId> = inner
            .records
            .values()
            .filter(|record| {
                record
                    .retention_deadline
                    .is_some_and(|deadline| now >= deadline)
            })
            .map(|record| record.id.clone())
            .collect();
        for id in expired {
            let Some(mut record) = inner.records.remove(&id) else {
                continue;
            };
            if inner.current.get(&record.surface) == Some(&id) {
                inner.current.remove(&record.surface);
            }
            if inner.staging.get(&record.surface) == Some(&id) {
                inner.staging.remove(&record.surface);
            }
            record.frames.clear();
            record.bytes_held = 0;
            record.state = RecordingState::Expired;
            record.stop_reason = Some(StopReason::RetentionExpired);
            record.retention_deadline = None;
            record.retention_wall_ms = None;
            inner
                .tombstones
                .insert((record.owner.clone(), record.surface), record.summary(now));
        }
        due
    }

    /// Snapshot active generations for extension health-lease renewal.
    pub(crate) fn lease_targets(&self) -> Vec<LeaseTarget> {
        let inner = self.inner.lock().unwrap();
        inner
            .records
            .values()
            .filter(|record| record.state.accepts_frames())
            .map(|record| LeaseTarget {
                ticket: RecordingTicket {
                    id: record.id.clone(),
                    generation: record.generation,
                    surface: record.surface,
                },
                owner: record.owner.clone(),
            })
            .collect()
    }

    /// Return immutable frame handles for one owner's current frozen/interrupted recording.
    pub(crate) fn frames(&self, owner: &str, surface: SurfaceId) -> Vec<RecordedFrame> {
        let inner = self.inner.lock().unwrap();
        let Some(id) = inner.current.get(&surface) else {
            return Vec::new();
        };
        let Some(record) = inner.records.get(id).filter(|r| {
            r.owner == owner
                && matches!(
                    r.state,
                    RecordingState::Frozen | RecordingState::Interrupted
                )
        }) else {
            return Vec::new();
        };
        record.frames.iter().map(|f| f.frame.clone()).collect()
    }

    /// Whether an already encoded snapshot may still cross its explicit export boundary. Clear,
    /// expiry, workspace retirement, panic, and policy changes all revoke delivery.
    pub(crate) fn delivery_allowed(
        &self,
        owner: &str,
        surface: SurfaceId,
        recording_id: &RecordingId,
    ) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.current.get(&surface) == Some(recording_id)
            && inner.records.get(recording_id).is_some_and(|record| {
                record.owner == owner
                    && matches!(
                        record.state,
                        RecordingState::Frozen | RecordingState::Interrupted
                    )
            })
    }

    /// Current content-free status for an owner/surface, including a recent erasure tombstone.
    pub(crate) fn status(&self, owner: &str, surface: SurfaceId) -> Option<RecordingSummary> {
        let now = Instant::now();
        let inner = self.inner.lock().unwrap();
        if let Some(id) = inner
            .staging
            .get(&surface)
            .or_else(|| inner.current.get(&surface))
        {
            if let Some(record) = inner.records.get(id).filter(|r| r.owner == owner) {
                return Some(record.summary(now));
            }
        }
        inner.tombstones.get(&(owner.to_string(), surface)).cloned()
    }

    /// Current generation token for best-effort stop/renew mechanics.
    pub(crate) fn ticket(&self, owner: &str, surface: SurfaceId) -> Option<RecordingTicket> {
        let inner = self.inner.lock().unwrap();
        let id = inner
            .staging
            .get(&surface)
            .or_else(|| inner.current.get(&surface))?;
        let record = inner
            .records
            .get(id)
            .filter(|record| record.owner == owner)?;
        Some(RecordingTicket {
            id: record.id.clone(),
            generation: record.generation,
            surface,
        })
    }

    /// Erase one owner's recording immediately and retain only a content-free tombstone.
    ///
    /// Returns whether an owned current or staging recording was actually erased. A prior
    /// tombstone is not a new clear effect.
    pub(crate) fn clear(&self, owner: &str, surface: SurfaceId, reason: StopReason) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let ids = [
            inner.current.get(&surface).cloned(),
            inner.staging.get(&surface).cloned(),
        ];
        let mut changed = false;
        for id in ids.into_iter().flatten() {
            changed |= Self::erase_exact(&mut inner, owner, surface, &id, None, reason);
        }
        changed
    }

    /// Erase only the exact recording generation named by a previously captured ticket.
    ///
    /// A replacement that wins after the ticket snapshot is never erased or unmapped. Returns
    /// whether the exact owned generation was present and erased.
    pub(crate) fn clear_ticket(
        &self,
        owner: &str,
        ticket: &RecordingTicket,
        reason: StopReason,
    ) -> bool {
        let mut inner = self.inner.lock().unwrap();
        Self::erase_exact(
            &mut inner,
            owner,
            ticket.surface,
            &ticket.id,
            Some(ticket.generation),
            reason,
        )
    }

    fn erase_exact(
        inner: &mut Inner,
        owner: &str,
        surface: SurfaceId,
        id: &RecordingId,
        generation: Option<u64>,
        reason: StopReason,
    ) -> bool {
        let matches = inner.records.get(id).is_some_and(|record| {
            record.owner == owner
                && record.surface == surface
                && generation.is_none_or(|expected| record.generation == expected)
        });
        if !matches {
            return false;
        }
        if inner.current.get(&surface) == Some(id) {
            inner.current.remove(&surface);
        }
        if inner.staging.get(&surface) == Some(id) {
            inner.staging.remove(&surface);
        }
        let Some(mut record) = inner.records.remove(id) else {
            return false;
        };
        record.frames.clear();
        record.bytes_held = 0;
        record.state = RecordingState::Erased;
        record.stop_reason = Some(reason);
        record.retention_deadline = None;
        record.retention_wall_ms = None;
        inner
            .tombstones
            .insert((owner.to_string(), surface), record.summary(Instant::now()));
        true
    }

    /// Erase all content owned by a retiring workspace and return generations whose relays should
    /// stop immediately.
    pub(crate) fn end_session(&self, owner: &str, reason: StopReason) -> Vec<RecordingTicket> {
        let tickets: Vec<RecordingTicket> = {
            let inner = self.inner.lock().unwrap();
            inner
                .records
                .values()
                .filter(|r| r.owner == owner)
                .map(|r| RecordingTicket {
                    id: r.id.clone(),
                    generation: r.generation,
                    surface: r.surface,
                })
                .collect()
        };
        for ticket in &tickets {
            self.clear(owner, ticket.surface, reason);
        }
        tickets
    }

    /// Erase every recording in process memory and return generations whose relays should stop.
    pub(crate) fn end_all(&self, reason: StopReason) -> Vec<RecordingTicket> {
        let owned_surfaces: Vec<(String, SurfaceId)> = {
            let inner = self.inner.lock().unwrap();
            inner
                .records
                .values()
                .map(|record| (record.owner.clone(), record.surface))
                .collect()
        };
        let mut tickets = Vec::new();
        for (owner, surface) in owned_surfaces {
            if let Some(ticket) = self.ticket(&owner, surface) {
                tickets.push(ticket);
            }
            self.clear(&owner, surface, reason);
        }
        tickets
    }
}

fn wall_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn thin_to_bounds(record: &mut Recording) {
    while record.frames.len() > MAX_FRAMES || record.bytes_held > MAX_RECORDING_BYTES {
        let index = record
            .frames
            .iter()
            .position(|frame| !frame.protected)
            .unwrap_or(0);
        if record.frames[index].protected {
            record.state = RecordingState::Interrupted;
            record.stop_reason = Some(StopReason::MemoryLimit);
            let now = Instant::now();
            record.retention_deadline = Some(now + RETENTION_TIMEOUT);
            record.retention_wall_ms =
                Some(wall_ms().saturating_add(RETENTION_TIMEOUT.as_millis() as u64));
        }
        let removed = record.frames.remove(index);
        record.bytes_held = record.bytes_held.saturating_sub(removed.frame.jpeg.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(slot: u32, tab: i64) -> SurfaceId {
        SurfaceId {
            slot,
            native_tab: tab,
        }
    }

    fn frame() -> String {
        b64::encode(&[1, 2, 3])
    }

    #[test]
    fn failed_replacement_preserves_the_committed_recording() {
        let coordinator = RecordingCoordinator::new();
        let first = coordinator.begin_start("g1", surface(1, 7)).unwrap();
        coordinator.commit_start(&first, Some(800.0)).unwrap();
        let finishing = coordinator.begin_finalizing("g1", surface(1, 7)).unwrap();
        coordinator.finish_finalizing(&finishing, true, StopReason::Explicit);

        let replacement = coordinator.begin_start("g1", surface(1, 7)).unwrap();
        coordinator.fail_start(&replacement);
        let status = coordinator.status("g1", surface(1, 7)).unwrap();
        assert_eq!(status.id, first.id);
        assert_eq!(status.state, RecordingState::Frozen);
    }

    #[test]
    fn slot_and_generation_prevent_cross_recording_frames() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(2, 9)).unwrap();
        assert_eq!(
            coordinator.on_frame(
                surface(1, 9),
                ticket.id.as_str(),
                ticket.generation,
                0,
                &frame(),
                100,
                None,
                false,
            ),
            FrameAdmission::Ignored
        );
        assert_eq!(
            coordinator.on_frame(
                surface(2, 9),
                ticket.id.as_str(),
                ticket.generation + 1,
                0,
                &frame(),
                100,
                None,
                false,
            ),
            FrameAdmission::Ignored
        );
        assert_eq!(
            coordinator.on_frame(
                surface(2, 9),
                ticket.id.as_str(),
                ticket.generation,
                0,
                &frame(),
                100,
                None,
                false,
            ),
            FrameAdmission::Accepted
        );
    }

    #[test]
    fn clear_erases_frames_and_leaves_only_a_tombstone() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 3)).unwrap();
        coordinator.commit_start(&ticket, None).unwrap();
        assert_eq!(
            coordinator.on_frame(
                surface(1, 3),
                ticket.id.as_str(),
                ticket.generation,
                0,
                &frame(),
                100,
                None,
                false,
            ),
            FrameAdmission::Accepted
        );
        assert!(coordinator.clear("g1", surface(1, 3), StopReason::Cleared));
        assert!(!coordinator.clear("g1", surface(1, 3), StopReason::Cleared));
        let status = coordinator.status("g1", surface(1, 3)).unwrap();
        assert_eq!(status.state, RecordingState::Erased);
        assert_eq!(status.bytes_held, 0);
        assert_eq!(status.frame_count, 0);
        assert!(coordinator.frames("g1", surface(1, 3)).is_empty());
        assert!(!coordinator.delivery_allowed("g1", surface(1, 3), &ticket.id));
    }

    #[test]
    fn idle_waits_for_in_flight_work_but_hard_deadline_does_not() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 5)).unwrap();
        coordinator.commit_start(&ticket, None).unwrap();
        assert!(coordinator.begin_activity("g1", surface(1, 5)));
        {
            let mut inner = coordinator.inner.lock().unwrap();
            let record = inner.records.get_mut(&ticket.id).unwrap();
            record.last_activity = Instant::now() - IDLE_TIMEOUT - Duration::from_secs(1);
        }
        assert!(coordinator.poll_deadlines().is_empty());
        {
            let mut inner = coordinator.inner.lock().unwrap();
            inner.records.get_mut(&ticket.id).unwrap().hard_deadline = Instant::now();
        }
        let due = coordinator.poll_deadlines();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].reason, StopReason::HardTimeout);
    }

    #[test]
    fn retention_expiry_erases_content_and_leaves_expired_status() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 6)).unwrap();
        coordinator.commit_start(&ticket, None).unwrap();
        let finishing = coordinator.begin_finalizing("g1", surface(1, 6)).unwrap();
        coordinator.finish_finalizing(&finishing, true, StopReason::Explicit);
        {
            let mut inner = coordinator.inner.lock().unwrap();
            inner
                .records
                .get_mut(&ticket.id)
                .unwrap()
                .retention_deadline = Some(Instant::now());
        }
        coordinator.poll_deadlines();
        let status = coordinator.status("g1", surface(1, 6)).unwrap();
        assert_eq!(status.state, RecordingState::Expired);
        assert_eq!(status.frame_count, 0);
    }

    #[test]
    fn wrong_owner_cannot_clear_or_unmap_a_recording() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 8)).unwrap();
        coordinator.commit_start(&ticket, None).unwrap();
        assert!(!coordinator.clear("g2", surface(1, 8), StopReason::Cleared));
        assert!(coordinator.status("g1", surface(1, 8)).is_some());
        assert!(coordinator.is_active("g1", surface(1, 8)));
    }

    #[test]
    fn clear_ticket_is_generation_exact_and_reports_no_op() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 9)).unwrap();
        coordinator.commit_start(&ticket, None).unwrap();

        let mut stale = ticket.clone();
        stale.generation = stale.generation.saturating_add(1);
        assert!(!coordinator.clear_ticket("g1", &stale, StopReason::Cleared));
        assert!(coordinator.is_active("g1", surface(1, 9)));

        assert!(coordinator.clear_ticket("g1", &ticket, StopReason::Cleared));
        assert!(!coordinator.clear_ticket("g1", &ticket, StopReason::Cleared));
        assert_eq!(
            coordinator.status("g1", surface(1, 9)).unwrap().state,
            RecordingState::Erased
        );
    }

    #[test]
    fn acknowledged_stop_can_lose_its_local_generation_before_finish() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 10)).unwrap();
        coordinator.commit_start(&ticket, None).unwrap();
        let finishing = coordinator.begin_finalizing("g1", surface(1, 10)).unwrap();

        assert!(coordinator.clear_ticket("g1", &finishing, StopReason::Cleared));
        assert!(coordinator
            .finish_finalizing(&finishing, true, StopReason::Explicit)
            .is_none());
    }

    #[test]
    fn disconnect_interrupts_a_staged_start_before_acknowledgement() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(4, 12)).unwrap();

        coordinator.interrupt_slot(4, StopReason::BrowserDisconnected);
        let CommitStartError::Interrupted(summary) =
            coordinator.commit_start(&ticket, None).unwrap_err()
        else {
            panic!("staged disconnect must retain an interrupted acknowledgement outcome");
        };
        assert_eq!(summary.state, RecordingState::Interrupted);
        assert_eq!(summary.stop_reason, Some(StopReason::BrowserDisconnected));
        assert!(!coordinator.is_active("g1", surface(4, 12)));
        assert_eq!(
            coordinator.status("g1", surface(4, 12)).unwrap().state,
            RecordingState::Interrupted
        );
    }

    #[test]
    fn hold_interrupts_current_and_staging_generations_atomically() {
        let coordinator = RecordingCoordinator::new();
        let current = coordinator.begin_start("g1", surface(5, 13)).unwrap();
        coordinator.commit_start(&current, None).unwrap();
        let finishing = coordinator.begin_finalizing("g1", surface(5, 13)).unwrap();
        coordinator.finish_finalizing(&finishing, true, StopReason::Explicit);
        let staged = coordinator.begin_start("g1", surface(5, 13)).unwrap();

        let interrupted = coordinator.interrupt_all(StopReason::UserHold);
        assert!(interrupted.iter().any(|ticket| ticket.id == staged.id));
        let CommitStartError::Interrupted(summary) =
            coordinator.commit_start(&staged, None).unwrap_err()
        else {
            panic!("held staged generation must not commit as recording");
        };
        assert_eq!(summary.state, RecordingState::Interrupted);
        assert_eq!(summary.stop_reason, Some(StopReason::UserHold));
        assert_eq!(
            coordinator.status("g1", surface(5, 13)).unwrap().id,
            current.id
        );
    }

    #[test]
    fn invalid_current_frame_interrupts_capture() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 11)).unwrap();
        coordinator.commit_start(&ticket, None).unwrap();
        assert_eq!(
            coordinator.on_frame(
                surface(1, 11),
                ticket.id.as_str(),
                ticket.generation,
                0,
                "not base64!",
                100,
                None,
                false,
            ),
            FrameAdmission::RejectAndCancel
        );
        assert_eq!(
            coordinator.on_frame(
                surface(1, 11),
                ticket.id.as_str(),
                ticket.generation,
                1,
                "still not base64!",
                101,
                None,
                false,
            ),
            FrameAdmission::Ignored,
            "an already-interrupted generation must not request another critical cancel"
        );
        let status = coordinator.status("g1", surface(1, 11)).unwrap();
        assert_eq!(status.state, RecordingState::Interrupted);
        assert_eq!(status.stop_reason, Some(StopReason::InvalidFrame));
    }

    #[test]
    fn oversized_staged_frame_interrupts_before_start_acknowledgement() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 14)).unwrap();
        let oversized = "A".repeat(MAX_FRAME_BYTES.saturating_mul(4).saturating_div(3) + 9);
        assert_eq!(
            coordinator.on_frame(
                surface(1, 14),
                ticket.id.as_str(),
                ticket.generation,
                0,
                &oversized,
                100,
                None,
                false,
            ),
            FrameAdmission::RejectAndCancel
        );
        let CommitStartError::Interrupted(summary) =
            coordinator.commit_start(&ticket, None).unwrap_err()
        else {
            panic!("oversized staged frame must prevent Recording state");
        };
        assert_eq!(summary.state, RecordingState::Interrupted);
        assert_eq!(summary.stop_reason, Some(StopReason::InvalidFrame));
        assert!(!coordinator.is_active("g1", surface(1, 14)));
    }
}
