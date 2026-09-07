//! Bounded history groups reconstructed from safe receipts, with explicit missing evidence.

use super::{push_bounded, AuditRecord, HistoryItem, VecDeque};
use crate::governance::CapabilitySet;
use crate::language::history::{CompositionKind, COMPOSITION_STEP_LIMIT};
use crate::language::outcome::{Observed, Outcome};
use serde::Serialize;

/// What history can establish about one planned step.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepHistoryState {
    Recorded,
    NotStarted,
    NotRun,
    Pending,
    Unconfirmed,
}

/// One level of child detail. Unexecuted and missing rows have no fabricated receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HistoryStep {
    pub position: usize,
    pub tool: Option<String>,
    pub state: StepHistoryState,
    pub record: Option<Box<HistoryItem>>,
}

/// Merge one receipt into its group, preserving children when the parent later completes.
pub(super) fn merge(
    history: &mut VecDeque<HistoryItem>,
    record: &AuditRecord,
    live: bool,
) -> HistoryItem {
    merge_stored(
        history,
        record,
        live,
        crate::language::audit_health::Storage::Saved,
    )
}

/// Preserve each child's storage confirmation when the parent later completes or saving recovers.
pub(super) fn merge_stored(
    history: &mut VecDeque<HistoryItem>,
    record: &AuditRecord,
    live: bool,
    storage: crate::language::audit_health::Storage,
) -> HistoryItem {
    let stored_item = || {
        let mut item = HistoryItem::from(record.clone());
        item.storage = storage;
        if storage == crate::language::audit_health::Storage::Unconfirmed {
            item.storage_detail = storage.detail().into();
        }
        item
    };
    let index = history
        .iter()
        .position(|item| item.invocation == record.invocation);
    let old = index.map(|index| history[index].clone());
    let mut item = if let Some(step) = record.step {
        let mut item = old.unwrap_or_else(|| {
            let mut parent = HistoryItem::from(record.clone());
            parent.tool = step.parent.tool().into();
            parent.capability = CapabilitySet::EMPTY.label();
            parent.requirements = CapabilitySet::EMPTY;
            parent.status = "unknown".into();
            parent.effect = "unknown".into();
            parent.allowed = true;
            parent.reason.clear();
            parent.policy_tier = None;
            parent.grant_id = None;
            parent.denial_id = None;
            parent.observed = Observed::default();
            parent.duration_ms = 0;
            parent.permission_explanations.clear();
            parent.permissions = Default::default();
            parent.complete = false;
            parent
        });
        ensure_steps(&mut item, step.total, live);
        if let Some(row) = step
            .position
            .checked_sub(1)
            .and_then(|index| item.steps.get_mut(index))
        {
            row.tool = Some(record.tool.clone());
            row.state = if step.preparation_failed {
                StepHistoryState::NotStarted
            } else {
                StepHistoryState::Recorded
            };
            row.record = Some(Box::new(stored_item()));
        }
        if !item.complete {
            let completed = item
                .steps
                .iter()
                .filter(|step| {
                    step.record
                        .as_ref()
                        .is_some_and(|record| record.status == "succeeded")
                })
                .count();
            item.summary = if live {
                Outcome::CompositionProgress {
                    completed,
                    total: item.steps.len(),
                }
                .summary()
            } else {
                Outcome::CompositionUnrecorded.summary()
            };
        }
        item
    } else {
        let mut item = stored_item();
        if let Some(old) = old {
            item.steps = old.steps;
        }
        if let Some(progress) = item.composition {
            ensure_steps(&mut item, progress.counts.total, false);
            for row in &mut item.steps {
                if row.record.is_none() {
                    row.state = if row.position
                        > progress
                            .counts
                            .total
                            .saturating_sub(progress.counts.not_run)
                    {
                        StepHistoryState::NotRun
                    } else {
                        StepHistoryState::Unconfirmed
                    };
                }
                if let Some(tool) = record.composition_tools.get(row.position - 1) {
                    row.tool = Some(tool.clone());
                }
            }
        }
        item
    };
    item.timestamp_ms = record.timestamp_ms;
    if let Some(index) = index {
        history[index] = item.clone();
    } else {
        push_bounded(history, item.clone());
    }
    item
}

fn ensure_steps(item: &mut HistoryItem, total: usize, live: bool) {
    for index in item.steps.len()..total.min(COMPOSITION_STEP_LIMIT) {
        item.steps.push(HistoryStep {
            position: index + 1,
            tool: None,
            state: if live {
                StepHistoryState::Pending
            } else {
                StepHistoryState::Unconfirmed
            },
            record: None,
        });
    }
}

/// Flatten only recorded operations for policy previews; aggregate wrappers carry no action.
pub(super) fn operations(items: &[HistoryItem]) -> Vec<&HistoryItem> {
    items
        .iter()
        .flat_map(|item| {
            if item.steps.is_empty()
                && item.composition.is_none()
                && ![
                    CompositionKind::Flow.tool(),
                    CompositionKind::Sequence.tool(),
                ]
                .contains(&item.tool.as_str())
            {
                vec![item]
            } else {
                item.steps
                    .iter()
                    .filter(|step| step.state == StepHistoryState::Recorded)
                    .filter_map(|step| step.record.as_deref())
                    .collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{AuditSink, Decision};
    use crate::language::composition::{CompositionProgress, StepCounts};
    use crate::language::history::{CompositionKind, StepReceipt};

    fn child(position: usize) -> AuditRecord {
        let mut record = AuditRecord::now(
            "parent",
            "workspace",
            "browser_read",
            CapabilitySet::READ,
            "snapshot",
            Decision::permitted(),
            "succeeded",
            "none",
            &Outcome::TextRead {
                words: 5,
                host: Some("example.com".into()),
            }
            .audit(),
            12,
        );
        record.step = Some(StepReceipt {
            parent: CompositionKind::Flow,
            position,
            total: 4,
            preparation_failed: false,
        });
        record
    }

    #[test]
    fn groups_preserve_incremental_evidence_and_never_guess_missing_execution() {
        let mut history = VecDeque::new();
        let record = child(1);
        let item = merge(&mut history, &record, true);
        assert!(!item.complete);
        assert_eq!(item.summary, "Completed 1 of 4 steps.");
        assert_eq!(item.steps[1].state, StepHistoryState::Pending);
        merge(&mut history, &record, true);
        assert_eq!(history.len(), 1);
        assert_eq!(
            operations(&history.iter().cloned().collect::<Vec<_>>()).len(),
            1
        );
        // Dry-run and historical wrappers can have no child/progress metadata at all.
        let mut dry_run = item.clone();
        dry_run.steps.clear();
        dry_run.complete = true;
        assert!(operations(&[dry_run]).is_empty());
        let mut restored = VecDeque::new();
        let item = merge(&mut restored, &record, false);
        assert_eq!(item.summary, "Completion was not recorded.");
        assert_eq!(item.steps[1].state, StepHistoryState::Unconfirmed);
        let mut preparation = child(2);
        preparation.step.as_mut().unwrap().preparation_failed = true;
        preparation.status = "not_started".into();
        merge(&mut restored, &preparation, false);
        let progress = CompositionProgress {
            counts: StepCounts {
                total: 4,
                succeeded: 1,
                not_started: 1,
                failed: 1,
                not_run: 1,
                ..StepCounts::default()
            },
            stopped: true,
            ..CompositionProgress::default()
        };
        let parent = AuditRecord::now(
            "parent",
            "workspace",
            "browser_flow",
            CapabilitySet::EMPTY,
            "snapshot",
            Decision::permitted(),
            "failed",
            "none",
            &Outcome::CompositionRan(progress)
                .audit()
                .with_tools(["browser_read"; 4]),
            30,
        );
        let item = merge(&mut restored, &parent, false);
        assert!(item.complete);
        assert_eq!(item.steps[1].state, StepHistoryState::NotStarted);
        assert_eq!(
            item.steps[2].state,
            StepHistoryState::Unconfirmed,
            "missing failed receipt is not an unrun operation"
        );
        assert_eq!(item.steps[3].state, StepHistoryState::NotRun);
        assert!(item.steps[2].record.is_none());
        assert_eq!(
            operations(&[item]).len(),
            1,
            "preview excludes wrapper, preparation, and missing rows"
        );
    }

    #[test]
    fn child_receipt_updates_do_not_settle_parent_or_notify_and_reload_equally() {
        let projection = super::super::WorkbenchProjection::default();
        projection.react(&crate::events::DomainEvent::WorkStarted {
            invocation: "parent".into(),
            workspace: "workspace".into(),
            tool: "browser_flow".into(),
            activity: ghostlight_bridge::browser::PresentationActivity::Quiet,
            capabilities: CapabilitySet::EMPTY,
        });
        let receipt = child(1);
        projection.record(&receipt, crate::language::audit_health::Storage::Saved);
        assert_eq!(projection.operations().len(), 1);
        assert_eq!(projection.history().len(), 1);
        assert!(projection.lock().notified.is_empty());
        let path =
            std::env::temp_dir().join(format!("ghostlight-h4-{}.jsonl", uuid::Uuid::new_v4()));
        let sink = crate::governance::JsonlAuditSink::open(&path).unwrap();
        sink.record(&receipt).unwrap();
        drop(sink);
        let restored = super::super::WorkbenchProjection::default();
        restored.load_history(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            restored.history()[0].steps[0].record,
            projection.history()[0].steps[0].record
        );
        assert_eq!(
            restored.history()[0].steps[1].state,
            StepHistoryState::Unconfirmed
        );
    }

    #[test]
    fn history_limit_evicts_whole_groups_and_duplicate_receipts_do_not_expand_them() {
        let mut history = VecDeque::new();
        for index in 0..=super::super::HISTORY_LIMIT {
            let mut record = child(1);
            record.invocation = format!("parent-{index}");
            merge(&mut history, &record, false);
            merge(&mut history, &record, false);
        }
        assert_eq!(history.len(), super::super::HISTORY_LIMIT);
        assert_eq!(history.front().unwrap().invocation, "parent-1");
        assert!(history.iter().all(|item| item.steps.len() == 4));
    }

    #[test]
    fn saved_parent_never_upgrades_an_unconfirmed_child_receipt() {
        use crate::language::audit_health::Storage;
        let mut history = VecDeque::new();
        let first = child(1);
        merge_stored(&mut history, &first, true, Storage::Unconfirmed);
        let mut parent = first.clone();
        parent.step = None;
        parent.tool = "browser_flow".into();
        parent.unconfirmed_history_steps = 1;
        let item = merge_stored(&mut history, &parent, false, Storage::Saved);
        assert_eq!(item.storage, Storage::Saved);
        assert!(item.storage_detail.contains("Some step history"));
        let child = item.steps[0].record.as_ref().unwrap();
        assert_eq!(child.storage, Storage::Unconfirmed);
        assert_eq!(child.status, "succeeded");
        assert!(child.storage_detail.contains("storage was not confirmed"));
    }
}
