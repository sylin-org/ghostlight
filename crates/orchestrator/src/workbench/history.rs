//! Bounded history groups reconstructed from safe receipts, with explicit missing evidence.

use super::{AuditRecord, HistoryItem, VecDeque, HISTORY_LIMIT};
use crate::governance::CapabilitySet;
use crate::language::composition;
use crate::language::composition::COMPOSITION_STEP_LIMIT;
use crate::language::history::{CompositionKind, OutcomeClass};
use crate::language::outcome::Observed;
use crate::work::result::{Effect, Status};
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
        None,
    )
}

/// Preserve each child's storage confirmation when the parent later completes or saving recovers.
pub(super) fn merge_stored(
    history: &mut VecDeque<HistoryItem>,
    record: &AuditRecord,
    live: bool,
    storage: crate::language::audit_health::Storage,
    provenance: Option<crate::provenance::ConnectionDetails>,
) -> HistoryItem {
    let stored_item = || {
        let mut item = HistoryItem::from(record.clone());
        item.storage = storage;
        if let Some(provenance) = &provenance {
            item.agent = provenance.agent.clone();
            item.provenance = Some(provenance.clone());
        }
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
            let mut parent = stored_item();
            parent.tool = step.parent.tool().into();
            parent.capability = CapabilitySet::EMPTY.label();
            parent.requirements = CapabilitySet::EMPTY;
            parent.status = Status::Unknown.stored_name();
            parent.outcome = OutcomeClass::Unknown;
            parent.effect = Effect::Unknown.stored_name();
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
                        .is_some_and(|record| record.outcome == OutcomeClass::Succeeded)
                })
                .count();
            item.summary = if live {
                composition::running_summary(completed, item.steps.len())
            } else {
                composition::UNRECORDED_SUMMARY.into()
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
                && item.tool != CompositionKind::Flow.tool()
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

/// Keep `item` as the newest of `history`, evicting the oldest once [`HISTORY_LIMIT`] are held.
fn push_bounded(history: &mut VecDeque<HistoryItem>, item: HistoryItem) {
    if history.len() == HISTORY_LIMIT {
        history.pop_front();
    }
    history.push_back(item);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::Decision;
    use crate::language::composition::{CompositionProgress, StepCounts};
    use crate::language::history::StepReceipt;
    use crate::language::outcome::Outcome;
    use crate::workbench::{WorkbenchProjection, HISTORY_LIMIT};

    /// How many steps the flow these tests record has.
    const STEPS: usize = 4;

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
            total: STEPS,
            preparation_failed: false,
        });
        record
    }

    /// Protects Grouped history 3 and 4 (ACCEPTANCE, ADR-0156) and ARCHITECTURE's grouped
    /// history: a child receipt updates its group while the parent stays live and unsettled; a
    /// repeated receipt adds nothing; a restored or finished group says its completion was not
    /// recorded, marks a missing receipt unconfirmed and only the parent's own count as never
    /// run, so missing evidence never becomes a no-effect claim; and a policy preview counts only
    /// the recorded operations.
    #[test]
    fn a_group_shows_its_children_as_they_arrive_and_never_guesses_missing_execution() {
        let projection = WorkbenchProjection::default();
        projection.react(&crate::events::DomainEvent::WorkStarted {
            provenance: crate::provenance::ConnectionEvidence::claimed(
                "test",
                crate::intake::session::IntakeChannel::Mcp,
            )
            .details(),
            invocation: "parent".into(),
            workspace: "workspace".into(),
            tool: "browser_flow".into(),
            activity: crate::browser::adapter::PresentationActivity::Quiet,
            capabilities: CapabilitySet::EMPTY,
            received_at_ms: 1,
        });
        projection.record(&child(1), crate::language::audit_health::Storage::Saved);
        assert_eq!(projection.operations().len(), 1, "the parent stays live");
        let live = projection.history();
        assert_eq!(live.len(), 1);
        assert!(
            !live[0].complete,
            "a child receipt does not settle its parent"
        );

        let mut history = VecDeque::new();
        let record = child(1);
        let item = merge(&mut history, &record, true);
        assert!(!item.complete);
        assert_eq!(item.summary, composition::running_summary(1, STEPS));
        assert_eq!(item.steps[1].state, StepHistoryState::Pending);
        merge(&mut history, &record, true);
        assert_eq!(history.len(), 1, "a repeated receipt adds no group");
        assert_eq!(
            operations(&history.iter().cloned().collect::<Vec<_>>()).len(),
            1
        );
        // Dry-run and historical wrappers can have no child or progress metadata at all.
        let mut dry_run = item.clone();
        dry_run.steps.clear();
        dry_run.complete = true;
        assert!(operations(&[dry_run]).is_empty());

        let mut restored = VecDeque::new();
        let item = merge(&mut restored, &record, false);
        assert_eq!(item.summary, composition::UNRECORDED_SUMMARY);
        assert_eq!(item.steps[1].state, StepHistoryState::Unconfirmed);
        let mut preparation = child(2);
        preparation.step.as_mut().unwrap().preparation_failed = true;
        preparation.status = "not_started".into();
        merge(&mut restored, &preparation, false);
        let progress = CompositionProgress {
            counts: StepCounts {
                total: STEPS,
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
                .with_tools(["browser_read"; STEPS]),
            30,
        );
        let item = merge(&mut restored, &parent, false);
        assert!(item.complete);
        assert_eq!(item.steps[1].state, StepHistoryState::NotStarted);
        assert_eq!(
            item.steps[2].state,
            StepHistoryState::Unconfirmed,
            "a missing failed receipt is not an unrun operation"
        );
        assert!(item.steps[2].record.is_none());
        assert_eq!(item.steps[3].state, StepHistoryState::NotRun);
        assert_eq!(
            operations(&[item]).len(),
            1,
            "a preview counts no wrapper, preparation or missing row"
        );
    }

    /// Protects Grouped history 4 ("History retains at most 500 whole groups"): history holds at
    /// most `HISTORY_LIMIT` whole groups, evicting the oldest, a repeated receipt never grows a
    /// group, and the window keeps as many recorded actions as the service holds (`HISTORY_HELD`
    /// in `ui/lib/words.js`, a limit on both sides).
    #[test]
    fn history_holds_at_most_the_limit_in_whole_groups_and_the_window_as_many() {
        let mut history = VecDeque::new();
        for index in 0..=HISTORY_LIMIT {
            let mut record = child(1);
            record.invocation = format!("parent-{index}");
            merge(&mut history, &record, false);
            merge(&mut history, &record, false);
        }
        assert_eq!(history.len(), HISTORY_LIMIT);
        assert_eq!(history.front().unwrap().invocation, "parent-1");
        assert!(history.iter().all(|item| item.steps.len() == STEPS));
        assert!(include_str!("../../ui/lib/words.js")
            .contains(&format!("const HISTORY_HELD = {HISTORY_LIMIT};")));
    }
}
