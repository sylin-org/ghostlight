// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Session-owned, memory-only GIF recording state (ADR-0073).
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

/// Stable gif_creator action vocabulary shared by schema, policy classification, and handler.
pub(crate) mod action {
    pub(crate) const START: &str = "start_recording";
    pub(crate) const STOP: &str = "stop_recording";
    pub(crate) const STATUS: &str = "status";
    pub(crate) const CLEAR: &str = "clear";
    pub(crate) const EXPORT: &str = "export";
}

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

/// All recording sessions in this service process.
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
    ) -> Option<RecordingSummary> {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        if inner.staging.get(&ticket.surface) != Some(&ticket.id) {
            return None;
        }
        let record = inner.records.get_mut(&ticket.id)?;
        if record.generation != ticket.generation || record.state != RecordingState::Starting {
            return None;
        }
        record.state = RecordingState::Recording;
        record.vp_w = vp_w;
        inner.staging.remove(&ticket.surface);
        if let Some(old) = inner.current.insert(ticket.surface, ticket.id.clone()) {
            if old != ticket.id {
                inner.records.remove(&old);
            }
        }
        inner.records.get(&ticket.id).map(|r| r.summary(now))
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
    ) -> bool {
        if data_b64.len() > MAX_FRAME_BYTES.saturating_mul(4).saturating_div(3) + 8 {
            self.interrupt_identity(surface, recording_id, generation, StopReason::InvalidFrame);
            return false;
        }
        let Some(bytes) = b64::decode(data_b64) else {
            self.interrupt_identity(surface, recording_id, generation, StopReason::InvalidFrame);
            return false;
        };
        if bytes.len() > MAX_FRAME_BYTES {
            self.interrupt_identity(surface, recording_id, generation, StopReason::InvalidFrame);
            return false;
        }

        let mut inner = self.inner.lock().unwrap();
        let global_bytes = inner.records.values().fold(0usize, |total, record| {
            total.saturating_add(record.bytes_held)
        });
        let Some(record) = inner
            .records
            .get_mut(&RecordingId(recording_id.to_string()))
        else {
            return false;
        };
        if record.surface != surface
            || record.generation != generation
            || !record.state.accepts_frames()
        {
            return false;
        }
        if sequence < record.next_seq {
            return true;
        }
        if global_bytes.saturating_add(bytes.len()) > MAX_GLOBAL_RECORDING_BYTES {
            let now = Instant::now();
            record.state = RecordingState::Interrupted;
            record.stop_reason = Some(StopReason::MemoryLimit);
            record.retention_deadline = Some(now + RETENTION_TIMEOUT);
            record.retention_wall_ms =
                Some(wall_ms().saturating_add(RETENTION_TIMEOUT.as_millis() as u64));
            return false;
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
        record.state.accepts_frames()
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
        let Some(id) = inner.current.get(&surface).cloned() else {
            return;
        };
        let Some(record) = inner.records.get_mut(&id) else {
            return;
        };
        if record.state.accepts_frames() {
            record.state = RecordingState::Interrupted;
            record.stop_reason = Some(reason);
            record.retention_deadline = Some(now + RETENTION_TIMEOUT);
            record.retention_wall_ms =
                Some(wall_ms().saturating_add(RETENTION_TIMEOUT.as_millis() as u64));
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
        let matches = {
            let inner = self.inner.lock().unwrap();
            inner.current.get(&surface).is_some_and(|id| {
                id.as_str() == recording_id
                    && inner
                        .records
                        .get(id)
                        .is_some_and(|record| record.generation == generation)
            })
        };
        if matches {
            self.interrupt_surface(surface, reason);
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
    pub(crate) fn interrupt_all(&self, reason: StopReason) {
        let surfaces: Vec<SurfaceId> = {
            let inner = self.inner.lock().unwrap();
            inner
                .records
                .values()
                .filter(|record| record.state.accepts_frames())
                .map(|record| record.surface)
                .collect()
        };
        for surface in surfaces {
            self.interrupt_surface(surface, reason);
        }
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
    /// expiry, session teardown, panic, and policy changes all revoke delivery.
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
    pub(crate) fn clear(&self, owner: &str, surface: SurfaceId, reason: StopReason) {
        let mut inner = self.inner.lock().unwrap();
        let ids = [
            inner.current.get(&surface).cloned(),
            inner.staging.get(&surface).cloned(),
        ];
        for id in ids.into_iter().flatten() {
            if inner
                .records
                .get(&id)
                .is_some_and(|record| record.owner == owner)
            {
                if inner.current.get(&surface) == Some(&id) {
                    inner.current.remove(&surface);
                }
                if inner.staging.get(&surface) == Some(&id) {
                    inner.staging.remove(&surface);
                }
                let Some(mut record) = inner.records.remove(&id) else {
                    continue;
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
            }
        }
    }

    /// Erase all content owned by an ending session and return generations whose relays should
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
        assert!(!coordinator.on_frame(
            surface(1, 9),
            ticket.id.as_str(),
            ticket.generation,
            0,
            &frame(),
            100,
            None,
            false,
        ));
        assert!(!coordinator.on_frame(
            surface(2, 9),
            ticket.id.as_str(),
            ticket.generation + 1,
            0,
            &frame(),
            100,
            None,
            false,
        ));
        assert!(coordinator.on_frame(
            surface(2, 9),
            ticket.id.as_str(),
            ticket.generation,
            0,
            &frame(),
            100,
            None,
            false,
        ));
    }

    #[test]
    fn clear_erases_frames_and_leaves_only_a_tombstone() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 3)).unwrap();
        coordinator.commit_start(&ticket, None);
        assert!(coordinator.on_frame(
            surface(1, 3),
            ticket.id.as_str(),
            ticket.generation,
            0,
            &frame(),
            100,
            None,
            false,
        ));
        coordinator.clear("g1", surface(1, 3), StopReason::Cleared);
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
        coordinator.commit_start(&ticket, None);
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
        coordinator.commit_start(&ticket, None);
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
        coordinator.commit_start(&ticket, None);
        coordinator.clear("g2", surface(1, 8), StopReason::Cleared);
        assert!(coordinator.status("g1", surface(1, 8)).is_some());
        assert!(coordinator.is_active("g1", surface(1, 8)));
    }

    #[test]
    fn invalid_current_frame_interrupts_capture() {
        let coordinator = RecordingCoordinator::new();
        let ticket = coordinator.begin_start("g1", surface(1, 11)).unwrap();
        coordinator.commit_start(&ticket, None);
        assert!(!coordinator.on_frame(
            surface(1, 11),
            ticket.id.as_str(),
            ticket.generation,
            0,
            "not base64!",
            100,
            None,
            false,
        ));
        let status = coordinator.status("g1", surface(1, 11)).unwrap();
        assert_eq!(status.state, RecordingState::Interrupted);
        assert_eq!(status.stop_reason, Some(StopReason::InvalidFrame));
    }
}
