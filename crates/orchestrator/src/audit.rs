//! One serialized audit writer, bounded recovery, and truthful live history projection.

use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::governance::AuditRecord;
use crate::language::audit_health::{AuditHealth, Storage, StorageFailure};
use crate::workbench::WorkbenchProjection;

/// A failed destination receives at most one recovery attempt per interval.
pub const RECOVERY_INTERVAL: Duration = Duration::from_secs(5);
/// The parser never allocates an unbounded buffer for a damaged historical line.
const MAX_ENTRY_BYTES: u64 = 1024 * 1024;

/// Content-free record of unconfirmed storage, never a replacement receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditGap {
    pub id: String,
    pub started_at_ms: u64,
    pub recovered_at_ms: u64,
    pub unconfirmed_receipts: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GapEntry {
    audit_gap: AuditGap,
}

/// Separate content-minimized persistence port. Success acknowledges storage, not a browser effect.
pub trait AuditSink: Send + Sync {
    /// Append and synchronize one terminal receipt.
    fn record(&self, record: &AuditRecord) -> io::Result<()>;
    /// Prove that new writes can be synchronized, persisting an optional recovery marker.
    /// In-memory fixtures need no physical probe.
    fn probe(&self, _gap: Option<&AuditGap>) -> io::Result<()> {
        Ok(())
    }
}

/// A path-bound JSONL destination, reopened on every attempt so replacement and repair take effect.
#[derive(Debug)]
pub struct JsonlAuditSink {
    path: PathBuf,
    writer: Mutex<bool>,
}

impl JsonlAuditSink {
    /// Construct a destination even when storage is temporarily unavailable.
    #[must_use]
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.into(),
            writer: Mutex::new(true),
        }
    }

    /// Open and verify a local destination for callers that explicitly require immediate success.
    pub fn open(path: &Path) -> io::Result<Self> {
        let sink = Self::new(path);
        sink.probe(None)?;
        Ok(sink)
    }

    fn append(&self, value: Option<&impl Serialize>) -> io::Result<()> {
        let mut needs_separator = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let mut bytes = Vec::new();
        if let Some(value) = value {
            // Serialize before touching the file.
            serde_json::to_writer(&mut bytes, value).map_err(io::Error::other)?;
            bytes.push(b'\n');
        } else {
            bytes.push(b'\n');
        }
        // A successful probe or append establishes a complete line boundary. After any failed
        // attempt, a separator isolates potentially torn bytes. This never requires read access
        // to a destination provisioned for append-only writing.
        if *needs_separator {
            bytes.insert(0, b'\n');
        }
        let result = (|| {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            file.write_all(&bytes)?;
            file.sync_all()
        })();
        *needs_separator = result.is_err();
        result
    }
}

impl AuditSink for JsonlAuditSink {
    fn record(&self, record: &AuditRecord) -> io::Result<()> {
        self.append(Some(record))
    }
    fn probe(&self, gap: Option<&AuditGap>) -> io::Result<()> {
        self.append(
            gap.map(|gap| GapEntry {
                audit_gap: gap.clone(),
            })
            .as_ref(),
        )
    }
}

struct RecorderState {
    health: AuditHealth,
    gap: Option<AuditGap>,
    next_probe: Instant,
}

/// The shared recorder used by completions and asynchronous browser events.
/// No failed receipt queue is kept; only bounded counters survive until recovery.
pub struct AuditRecorder {
    sink: Arc<dyn AuditSink>,
    projection: WorkbenchProjection,
    state: Mutex<RecorderState>,
}

impl AuditRecorder {
    /// Establish initial health without making history-storage failure a service-startup failure.
    #[must_use]
    pub fn new(sink: Arc<dyn AuditSink>, projection: WorkbenchProjection) -> Self {
        let mut state = RecorderState {
            health: projection.audit_health(),
            gap: None,
            next_probe: Instant::now() + RECOVERY_INTERVAL,
        };
        if let Err(error) = sink.probe(None) {
            fail(&mut state, &error);
        }
        projection.audit_health_changed(state.health.clone());
        Self {
            sink,
            projection,
            state: Mutex::new(state),
        }
    }

    /// Append once, retain the actual outcome in the live view, and report storage independently.
    pub fn record(&self, record: &AuditRecord) -> Storage {
        self.record_with_provenance(record, None)
    }

    /// Save only bounded audit evidence; keep the original claimed name in the live view alone.
    pub fn record_with_provenance(
        &self,
        record: &AuditRecord,
        provenance: Option<&crate::provenance::ConnectionEvidence>,
    ) -> Storage {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let storage = if state.health.unavailable() {
            Storage::Unconfirmed
        } else if let Err(error) = self.sink.record(record) {
            fail(&mut state, &error);
            Storage::Unconfirmed
        } else {
            Storage::Saved
        };
        if storage == Storage::Unconfirmed {
            state.health.unconfirmed_receipts = state.health.unconfirmed_receipts.saturating_add(1);
            if let Some(gap) = &mut state.gap {
                gap.unconfirmed_receipts = gap.unconfirmed_receipts.saturating_add(1);
            }
        }
        // Publish under the writer lock so concurrent completions cannot reorder health or receipts.
        self.projection.audit_health_changed(state.health.clone());
        self.projection.record_with_provenance(
            record,
            storage,
            provenance.map(|value| value.details()),
        );
        storage
    }

    /// Read the same current health used for strict dispatch admission.
    #[must_use]
    pub fn health(&self) -> AuditHealth {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .health
            .clone()
    }

    /// Make one due recovery attempt. No caller operation is retained or replayed.
    pub fn recover_if_due(&self) {
        self.recover_at(Instant::now());
    }

    fn recover_at(&self, now: Instant) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.health.unavailable() || now < state.next_probe {
            return;
        }
        state.next_probe = now + RECOVERY_INTERVAL;
        if let Some(gap) = &mut state.gap {
            gap.recovered_at_ms = unix_ms();
        }
        match self.sink.probe(state.gap.as_ref()) {
            Ok(()) => {
                state.health.failure = None;
                state.gap = None;
            }
            Err(error) => {
                state.health.failure = Some(category(&error));
            }
        }
        self.projection.audit_health_changed(state.health.clone());
    }
}

fn fail(state: &mut RecorderState, error: &io::Error) {
    state.health.failure = Some(category(error));
    state.next_probe = Instant::now() + RECOVERY_INTERVAL;
    state.gap.get_or_insert_with(|| AuditGap {
        id: format!("gap_{}", uuid::Uuid::new_v4().simple()),
        started_at_ms: unix_ms(),
        recovered_at_ms: 0,
        unconfirmed_receipts: 0,
    });
}

fn category(error: &io::Error) -> StorageFailure {
    match error.kind() {
        io::ErrorKind::PermissionDenied => StorageFailure::Access,
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory | io::ErrorKind::IsADirectory => {
            StorageFailure::Unavailable
        }
        _ => StorageFailure::Write,
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

/// One bounded historical line; corruption does not erase readable adjacent receipts.
pub enum AuditLine {
    Receipt(Box<AuditRecord>),
    Gap(AuditGap),
    Unreadable,
    Empty,
}

/// Read at most one entry into memory, draining oversized lines without retaining their payload.
pub fn read_line(reader: &mut impl BufRead) -> io::Result<Option<AuditLine>> {
    let mut bytes = Vec::new();
    let read = reader
        .take(MAX_ENTRY_BYTES + 1)
        .read_until(b'\n', &mut bytes)?;
    if read == 0 {
        return Ok(None);
    }
    if read as u64 > MAX_ENTRY_BYTES {
        if bytes.last() != Some(&b'\n') {
            loop {
                let buffer = reader.fill_buf()?;
                if buffer.is_empty() {
                    break;
                }
                let newline = buffer.iter().position(|b| *b == b'\n');
                let consumed = newline.map_or(buffer.len(), |n| n + 1);
                reader.consume(consumed);
                if newline.is_some() {
                    break;
                }
            }
        }
        return Ok(Some(AuditLine::Unreadable));
    }
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Some(AuditLine::Empty));
    }
    Ok(Some(
        if let Ok(record) = serde_json::from_slice::<AuditRecord>(&bytes) {
            AuditLine::Receipt(Box::new(record))
        } else if let Ok(entry) = serde_json::from_slice::<GapEntry>(&bytes) {
            AuditLine::Gap(entry.audit_gap)
        } else {
            AuditLine::Unreadable
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{CapabilitySet, Decision};
    use crate::language::outcome::Outcome;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Default)]
    struct SwitchSink {
        fails: AtomicBool,
        writes: AtomicUsize,
        probes: AtomicUsize,
        records: Mutex<Vec<AuditRecord>>,
        gaps: Mutex<Vec<AuditGap>>,
    }
    impl AuditSink for SwitchSink {
        fn record(&self, record: &AuditRecord) -> io::Result<()> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            if self.fails.load(Ordering::SeqCst) {
                return Err(io::Error::other("PRIVATE_DISK_DETAIL"));
            }
            self.records.lock().unwrap().push(record.clone());
            Ok(())
        }
        fn probe(&self, gap: Option<&AuditGap>) -> io::Result<()> {
            self.probes.fetch_add(1, Ordering::SeqCst);
            if self.fails.load(Ordering::SeqCst) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "PRIVATE_PATH",
                ));
            }
            if let Some(gap) = gap {
                self.gaps.lock().unwrap().push(gap.clone());
            }
            Ok(())
        }
    }
    fn record(invocation: &str) -> AuditRecord {
        AuditRecord::now(
            invocation,
            "workspace",
            "browser_read",
            CapabilitySet::READ,
            "authority",
            Decision::permitted(),
            "succeeded",
            "none",
            &Outcome::TextRead {
                words: 3,
                host: Some("example.com".into()),
            }
            .audit(),
            1,
        )
    }

    #[test]
    fn failed_receipts_stay_unconfirmed_after_bounded_recovery_without_backfill() {
        let sink = Arc::new(SwitchSink::default());
        let projection = WorkbenchProjection::default();
        let recorder = AuditRecorder::new(sink.clone(), projection.clone());
        assert_eq!(recorder.record(&record("saved")), Storage::Saved);
        sink.fails.store(true, Ordering::SeqCst);
        assert_eq!(recorder.record(&record("missing")), Storage::Unconfirmed);
        for n in 0..100 {
            recorder.record(&record(&format!("missing_{n}")));
            recorder.recover_if_due();
        }
        assert_eq!(
            sink.writes.load(Ordering::SeqCst),
            2,
            "no hot retry loop while storage is down"
        );
        assert_eq!(sink.probes.load(Ordering::SeqCst), 1);
        assert_eq!(recorder.health().unconfirmed_receipts, 101);
        sink.fails.store(false, Ordering::SeqCst);
        recorder.recover_at(Instant::now() + RECOVERY_INTERVAL);
        assert!(!recorder.health().unavailable());
        assert_eq!(
            sink.records.lock().unwrap().len(),
            1,
            "failed records are never replayed"
        );
        assert_eq!(sink.gaps.lock().unwrap()[0].unconfirmed_receipts, 101);
        assert_eq!(recorder.record(&record("after")), Storage::Saved);
        assert_eq!(projection.audit_health().unconfirmed_receipts, 101);
        assert!(!serde_json::to_string(&recorder.health())
            .unwrap()
            .contains("PRIVATE"));
    }

    #[test]
    fn cold_failure_and_concurrent_receipts_share_one_recovery_state() {
        let sink = Arc::new(SwitchSink::default());
        sink.fails.store(true, Ordering::SeqCst);
        let recorder = Arc::new(AuditRecorder::new(
            sink.clone(),
            WorkbenchProjection::default(),
        ));
        assert!(recorder.health().unavailable());
        let threads: Vec<_> = (0..20)
            .map(|n| {
                let recorder = recorder.clone();
                std::thread::spawn(move || recorder.record(&record(&format!("invocation_{n}"))))
            })
            .collect();
        for thread in threads {
            assert_eq!(thread.join().unwrap(), Storage::Unconfirmed);
        }
        assert_eq!(recorder.health().unconfirmed_receipts, 20);
        assert_eq!(sink.writes.load(Ordering::SeqCst), 0);
        recorder.recover_at(Instant::now() + RECOVERY_INTERVAL);
        assert!(recorder.health().unavailable());
        sink.fails.store(false, Ordering::SeqCst);
        recorder.recover_at(Instant::now() + 2 * RECOVERY_INTERVAL);
        assert!(!recorder.health().unavailable());
        assert_eq!(sink.gaps.lock().unwrap().len(), 1);
    }

    #[test]
    fn recovered_composition_parent_preserves_child_storage_gaps_live_and_after_restart() {
        use crate::language::composition::{CompositionProgress, EffectCounts, StepCounts};
        use crate::language::history::{CompositionKind, StepReceipt};
        use crate::workbench::{StepHistoryState, WorkbenchFacade};

        let history = |projection| {
            WorkbenchFacade::new(
                projection,
                crate::workspace::WorkspaceStore::default(),
                crate::governance::GovernanceFacade::new(None, None),
                Arc::new(crate::browser::RelayBrowserPort::new(
                    "audit-recovery-test".into(),
                )),
                crate::diagnostics::DiagnosticsHub::for_tests(),
            )
            .snapshot()
            .history
        };

        for kind in [CompositionKind::Flow, CompositionKind::Sequence] {
            let sink = Arc::new(SwitchSink::default());
            let projection = WorkbenchProjection::default();
            let recorder = AuditRecorder::new(sink.clone(), projection.clone());
            let mut child = AuditRecord::now(
                "composition",
                "workspace",
                "browser_fill_form",
                CapabilitySet::READ.union(CapabilitySet::WRITE),
                "authority",
                Decision::permitted(),
                "succeeded",
                "applied",
                &Outcome::FormFilled {
                    fields: 1,
                    submitted: false,
                    host: Some("example.com".into()),
                }
                .audit(),
                1,
            );
            child.step = Some(StepReceipt {
                parent: kind,
                position: 1,
                total: 2,
                preparation_failed: false,
            });
            sink.fails.store(true, Ordering::SeqCst);
            assert_eq!(recorder.record(&child), Storage::Unconfirmed);
            sink.fails.store(false, Ordering::SeqCst);
            recorder.recover_at(Instant::now() + RECOVERY_INTERVAL);
            assert!(!recorder.health().unavailable());

            let mut second = record("composition");
            second.step = Some(StepReceipt {
                parent: kind,
                position: 2,
                total: 2,
                preparation_failed: false,
            });
            assert_eq!(recorder.record(&second), Storage::Saved);
            let progress = CompositionProgress {
                counts: StepCounts {
                    total: 2,
                    succeeded: 2,
                    ..Default::default()
                },
                effects: EffectCounts {
                    applied: 1,
                    none: 1,
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = AuditRecord::now(
                "composition",
                "workspace",
                kind.tool(),
                CapabilitySet::EMPTY,
                "authority",
                Decision::permitted(),
                "succeeded",
                "applied",
                &Outcome::CompositionRan(progress)
                    .audit()
                    .with_unconfirmed_history(1),
                2,
            );
            assert_eq!(recorder.record(&parent), Storage::Saved);
            let live = history(projection.clone());
            assert_eq!(live.len(), 1);
            assert_eq!(live[0].status, "succeeded");
            assert_eq!(live[0].effect, "applied");
            assert_eq!(live[0].storage, Storage::Saved);
            assert!(live[0].storage_detail.contains("Some step history"));
            let first = live[0].steps[0].record.as_ref().unwrap();
            assert_eq!(first.storage, Storage::Unconfirmed);
            assert_eq!(first.status, "succeeded");
            assert_eq!(first.effect, "applied");
            assert_eq!(
                sink.writes.load(Ordering::SeqCst),
                3,
                "recovery does not retry the failed receipt"
            );

            // Reload only what the destination confirmed, including its explicit gap marker.
            let path = std::env::temp_dir().join(format!(
                "ghostlight-composition-recovery-{}.jsonl",
                uuid::Uuid::new_v4()
            ));
            let mut persisted = Vec::new();
            for gap in sink.gaps.lock().unwrap().iter() {
                assert_eq!(gap.unconfirmed_receipts, 1);
                serde_json::to_writer(
                    &mut persisted,
                    &GapEntry {
                        audit_gap: gap.clone(),
                    },
                )
                .unwrap();
                persisted.push(b'\n');
            }
            for record in sink.records.lock().unwrap().iter() {
                serde_json::to_writer(&mut persisted, record).unwrap();
                persisted.push(b'\n');
            }
            assert!(!String::from_utf8_lossy(&persisted).contains("PRIVATE_"));
            fs::write(&path, persisted).unwrap();
            let restored = WorkbenchProjection::default();
            restored.load_history(&path).unwrap();
            fs::remove_file(path).unwrap();
            let history = history(restored.clone());
            assert_eq!(history.len(), 1);
            assert_eq!(history[0].status, "succeeded");
            assert_eq!(history[0].effect, "applied");
            assert_eq!(history[0].storage, Storage::Saved);
            assert_eq!(history[0].steps[0].state, StepHistoryState::Unconfirmed);
            assert!(
                history[0].steps[0].record.is_none(),
                "restart must not fabricate the missing child's receipt"
            );
            assert_eq!(
                history[0].steps[1].record.as_ref().unwrap().storage,
                Storage::Saved
            );
            assert_eq!(restored.audit_health().unconfirmed_receipts, 1);
        }
    }

    #[test]
    fn actual_file_repair_keeps_good_history_and_reports_torn_and_missing_entries() {
        let root = std::env::temp_dir().join(format!("ghostlight-h7-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("audit.jsonl");
        fs::write(
            &path,
            format!(
                "{}\n{{\"torn\":",
                serde_json::to_string(&record("old")).unwrap()
            ),
        )
        .unwrap();
        let projection = WorkbenchProjection::default();
        projection.load_history(&path).unwrap();
        assert_eq!(projection.audit_health().unreadable_entries, 1);
        let recorder = AuditRecorder::new(Arc::new(JsonlAuditSink::new(&path)), projection);
        assert_eq!(recorder.record(&record("before")), Storage::Saved);
        fs::rename(&path, root.join("old.jsonl")).unwrap();
        fs::create_dir(&path).unwrap();
        assert_eq!(recorder.record(&record("missing")), Storage::Unconfirmed);
        fs::remove_dir(&path).unwrap();
        fs::rename(root.join("old.jsonl"), &path).unwrap();
        recorder.recover_at(Instant::now() + RECOVERY_INTERVAL);
        assert_eq!(recorder.record(&record("after")), Storage::Saved);
        let restored = WorkbenchProjection::default();
        restored.load_history(&path).unwrap();
        assert_eq!(restored.audit_health().unreadable_entries, 1);
        assert_eq!(restored.audit_health().unconfirmed_receipts, 1);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("before") && text.contains("after"));
        assert!(!text.contains("\"missing\""));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn oversized_history_does_not_hide_the_following_receipt() {
        let mut bytes = vec![b'x'; MAX_ENTRY_BYTES as usize + 100];
        bytes.extend_from_slice(b"\n");
        serde_json::to_writer(&mut bytes, &record("after")).unwrap();
        let mut reader = io::Cursor::new(bytes);
        assert!(matches!(
            read_line(&mut reader).unwrap(),
            Some(AuditLine::Unreadable)
        ));
        let Some(AuditLine::Receipt(receipt)) = read_line(&mut reader).unwrap() else {
            panic!("receipt lost")
        };
        assert_eq!(receipt.invocation, "after");
        assert!(read_line(&mut reader).unwrap().is_none());
    }

    #[test]
    fn recovery_markers_and_health_round_trip_without_payloads() {
        let gap = AuditGap {
            id: "gap_1".into(),
            started_at_ms: 1,
            recovered_at_ms: 2,
            unconfirmed_receipts: 4,
        };
        let mut encoded = Vec::new();
        serde_json::to_writer(
            &mut encoded,
            &GapEntry {
                audit_gap: gap.clone(),
            },
        )
        .unwrap();
        let Some(AuditLine::Gap(decoded)) = read_line(&mut io::Cursor::new(encoded)).unwrap()
        else {
            panic!("gap lost")
        };
        assert_eq!(decoded, gap);
        let health = AuditHealth {
            failure: Some(StorageFailure::Write),
            unconfirmed_receipts: 4,
            ..Default::default()
        };
        assert_eq!(
            serde_json::from_str::<AuditHealth>(&serde_json::to_string(&health).unwrap()).unwrap(),
            health
        );
    }
}
