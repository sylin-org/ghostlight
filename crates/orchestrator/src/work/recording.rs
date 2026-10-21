//! Governed recording lifecycle, export, and delivery execution.

use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, EncodedRecording, PhysicalRecordingSummary, RecordingDelivery,
    RecordingDestination, RecordingState, RecordingStopReason, RECORDING_LOCAL_MAX_BYTES,
    RECORDING_TRANSFER_MAX_BYTES,
};
use crate::intake::session::ServiceContent;
use serde_json::json;

use crate::browser::BrowserError;
use crate::governance::{Capability, CapabilitySet, Decision};
use crate::language::{
    outcome::{Outcome, Refusal, SavedTo},
    Record, RecordAction,
};
use crate::workspace::{HeldRecording, WorkspaceLease};

use super::{
    bounded, observed_host, readiness, recording_delivery_name, recording_facts,
    recording_state_name, ApplicationExecutor, Effect, InvocationContext, Readiness, Terminal,
    RECORDING_FILE_NAME,
};

/// Where a governed save puts its replay, and what the save learned on the way there.
struct SaveDestination {
    /// Where the browser delivers the replay.
    destination: RecordingDestination,
    /// The decision that authorized the save.
    decision: Decision,
    /// The tab the save lands on or reads from.
    tab_id: Option<u64>,
    /// How many bytes the delivery may carry.
    budget: usize,
    /// Whether describing a page target followed its handle through a re-render (ADR-0198
    /// Decision 17).
    rebound: bool,
}

impl ApplicationExecutor {
    pub(super) fn perform_record(
        &self,
        context: &InvocationContext<'_>,
        lease: Option<&WorkspaceLease>,
        value: &Record,
    ) -> Terminal {
        match value.action {
            RecordAction::Start => self.start_recording(
                context,
                lease.expect("recording start holds the workspace lease"),
                value,
            ),
            RecordAction::Status => {
                // Needs no capability, but crosses the runtime gate like every path to the
                // browser, so a pause also holds status, stop and discard. See stop_recording
                // and discard_recording for the same gate.
                let decision = self.authorize(context, context.requirements, None);
                if !decision.allowed {
                    return self.blocked(context, decision, None, Effect::None, true, json!({}));
                }
                let held = match self.held_recording(context) {
                    Ok(held) => held,
                    Err(terminal) => return *terminal,
                };
                let outcome = self.dispatch(
                    context,
                    BrowserCommand::StatusRecording {
                        recording_id: held.id,
                    },
                );
                self.note_recording(context, &outcome);
                match outcome {
                    Ok(BrowserOutcome::RecordingStatus { summary }) => {
                        self.recording_observed(context, decision, &summary)
                    }
                    Ok(outcome) => self.recording_selection_failure(context, outcome),
                    Err(error) => self.browser_failure(context, decision, error, None),
                }
            }
            RecordAction::Stop => self.stop_recording(context),
            RecordAction::Save => self.save_recording(context, lease, value),
            RecordAction::Discard => self.discard_recording(context),
        }
    }

    fn start_recording(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Record,
    ) -> Terminal {
        let selected = match lease.select_tab(value.tab.as_deref()) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, context.requirements, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({}),
            );
        }
        if let Some(answer) = self.held_answer(context, decision, &selected) {
            return answer;
        }
        match self.dispatch(
            context,
            BrowserCommand::StartRecording {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::RecordingStarted {
                recording_id,
                summary,
                existing,
            }) if summary.tab_id == selected.physical_id => {
                if let Some(browser) = self.workspaces.browser_of(context.workspace.as_str()) {
                    self.workspaces
                        .hold_recording(context.workspace, &browser, &recording_id);
                }
                self.recording_started(context, decision, &selected, &summary, existing)
            }
            Ok(outcome @ BrowserOutcome::RecordingHeld) => {
                self.recording_selection_failure(context, outcome)
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    /// The answer the recording this workspace already holds gives a start, if it gives one
    /// (ADR-0203 Decision 6.7).
    ///
    /// A workspace holds one recording. The tab it is still capturing answers with that recording,
    /// and any other start is held while the recording is unsaved. A stopped recording a save
    /// delivered is discarded so the start can replace it, and one the browser no longer has is
    /// forgotten. Either way the start goes ahead, and this answers nothing.
    fn held_answer(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        selected: &crate::workspace::SelectedTab,
    ) -> Option<Terminal> {
        let held = self.workspaces.recording(context.workspace)?;
        let status = self.dispatch(
            context,
            BrowserCommand::StatusRecording {
                recording_id: held.id.clone(),
            },
        );
        self.note_recording(context, &status);
        match status {
            Ok(BrowserOutcome::RecordingNotFound) => None,
            Ok(BrowserOutcome::RecordingStatus { summary })
                if summary.state == RecordingState::Recording
                    && summary.tab_id == selected.physical_id =>
            {
                Some(self.recording_started(context, decision, selected, &summary, true))
            }
            Ok(BrowserOutcome::RecordingStatus { summary })
                if summary.state != RecordingState::Recording && held.saved =>
            {
                let discarded = self.dispatch(
                    context,
                    BrowserCommand::DiscardRecording {
                        recording_id: held.id,
                    },
                );
                self.note_recording(context, &discarded);
                match discarded {
                    Ok(
                        BrowserOutcome::RecordingDiscarded { .. }
                        | BrowserOutcome::RecordingNotFound,
                    ) => None,
                    Ok(_) => {
                        Some(self.protocol_failure(context, decision, Some(selected.physical_id)))
                    }
                    Err(error) => Some(self.browser_failure(
                        context,
                        decision,
                        error,
                        Some(selected.physical_id),
                    )),
                }
            }
            Ok(BrowserOutcome::RecordingStatus { .. }) => {
                Some(self.recording_selection_failure(context, BrowserOutcome::RecordingHeld))
            }
            Ok(_) => Some(self.protocol_failure(context, decision, Some(selected.physical_id))),
            Err(error) => {
                Some(self.browser_failure(context, decision, error, Some(selected.physical_id)))
            }
        }
    }

    /// The result of a started recording, or of a start that found the tab's recording running.
    fn recording_started(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        selected: &crate::workspace::SelectedTab,
        summary: &PhysicalRecordingSummary,
        existing: bool,
    ) -> Terminal {
        let mut facts = recording_facts(summary);
        if let Some(object) = facts.as_object_mut() {
            object.insert("tab".into(), json!(selected.handle.as_str()));
        }
        self.succeeded(
            context,
            decision,
            Some(selected.physical_id),
            if existing {
                Effect::None
            } else {
                Effect::Applied
            },
            readiness(selected.readiness),
            existing,
            Outcome::RecordingStarted {
                host: observed_host(&selected.url),
            },
            facts,
        )
    }

    /// The recording this workspace holds, or the refusal that says it holds none.
    fn held_recording(
        &self,
        context: &InvocationContext<'_>,
    ) -> Result<HeldRecording, Box<Terminal>> {
        self.workspaces.recording(context.workspace).ok_or_else(|| {
            Box::new(self.recording_selection_failure(context, BrowserOutcome::RecordingNotFound))
        })
    }

    /// Keep the workspace's recording in step with what its browser reported: a delivered save is
    /// marked, and a discarded recording, or one the browser no longer has, is forgotten.
    fn note_recording(
        &self,
        context: &InvocationContext<'_>,
        outcome: &Result<BrowserOutcome, BrowserError>,
    ) {
        match outcome {
            Ok(BrowserOutcome::RecordingExported { .. }) => {
                self.workspaces.mark_recording_saved(context.workspace);
            }
            Ok(BrowserOutcome::RecordingDiscarded { .. } | BrowserOutcome::RecordingNotFound) => {
                self.workspaces.forget_recording(context.workspace);
            }
            _ => {}
        }
    }

    fn stop_recording(&self, context: &InvocationContext<'_>) -> Terminal {
        // Needs no capability, but every operation that reaches the browser still crosses the
        // runtime pause gate, so a paused session cannot have its recording stopped from
        // underneath it.
        let decision = self.authorize(context, context.requirements, None);
        if !decision.allowed {
            return self.blocked(context, decision, None, Effect::None, true, json!({}));
        }
        let held = match self.held_recording(context) {
            Ok(held) => held,
            Err(terminal) => return *terminal,
        };
        let outcome = self.dispatch(
            context,
            BrowserCommand::StopRecording {
                recording_id: held.id,
            },
        );
        self.note_recording(context, &outcome);
        match outcome {
            Ok(BrowserOutcome::RecordingStopped { summary, changed }) => self.succeeded(
                context,
                decision,
                Some(summary.tab_id),
                if changed {
                    Effect::Applied
                } else {
                    Effect::None
                },
                Readiness::NotApplicable,
                true,
                Outcome::RecordingStopped {
                    duration_ms: summary.duration_ms,
                },
                recording_facts(&summary),
            ),
            Ok(outcome) => self.recording_selection_failure(context, outcome),
            Err(error) => self.browser_failure(context, decision, error, None),
        }
    }

    fn ensure_recording_stopped(
        &self,
        context: &InvocationContext<'_>,
        recording_id: &str,
    ) -> Result<PhysicalRecordingSummary, Box<Terminal>> {
        let outcome = self.dispatch(
            context,
            BrowserCommand::StopRecording {
                recording_id: recording_id.to_owned(),
            },
        );
        self.note_recording(context, &outcome);
        match outcome {
            Ok(BrowserOutcome::RecordingStopped { summary, .. }) => Ok(summary),
            Ok(outcome) => Err(Box::new(self.recording_selection_failure(context, outcome))),
            Err(error) => Err(Box::new(self.browser_failure(
                context,
                Decision::permitted(),
                error,
                None,
            ))),
        }
    }

    /// Govern a save, then let the browser encode and deliver it.
    ///
    /// Ghostlight decides whether the replay may be made and where it may go. The browser does
    /// the rest: it holds the frames, so it encodes them, and for a page or a file it delivers
    /// them without anything crossing (ADR-0109). Only a client return crosses, and then once.
    fn save_recording(
        &self,
        context: &InvocationContext<'_>,
        lease: Option<&WorkspaceLease>,
        value: &Record,
    ) -> Terminal {
        // Capture-source Read and optional destination Write are known before freezing.
        // Recheck their actual hosts below once the browser returns the source inventory.
        let decision = self.authorize(context, context.requirements, None);
        if !decision.allowed {
            return self.blocked(context, decision, None, Effect::None, true, json!({}));
        }
        let held = match self.held_recording(context) {
            Ok(held) => held,
            Err(terminal) => return *terminal,
        };
        let stopped = match self.ensure_recording_stopped(context, &held.id) {
            Ok(summary) => summary,
            Err(terminal) => return *terminal,
        };

        let SaveDestination {
            destination,
            decision,
            tab_id,
            budget,
            rebound,
        } = match self.recording_destination(context, lease, value, &stopped) {
            Ok(resolved) => resolved,
            Err(terminal) => return *terminal,
        };

        let delivery_context = InvocationContext {
            requirements: if matches!(destination, RecordingDestination::Target { .. }) {
                CapabilitySet::WRITE
            } else {
                context.requirements
            },
            ..*context
        };
        let outcome = self.dispatch(
            &delivery_context,
            BrowserCommand::ExportRecording {
                recording_id: held.id,
                destination,
                max_output_bytes: budget,
            },
        );
        self.note_recording(context, &outcome);
        match outcome {
            Ok(BrowserOutcome::RecordingExported {
                summary,
                encoded,
                delivery,
            }) if summary.state != RecordingState::Recording => {
                self.recording_delivered(context, decision, &summary, encoded, delivery, rebound)
            }
            Ok(BrowserOutcome::RecordingExportFailed {
                reason,
                page_refused,
            }) => self.recording_export_failure(context, &reason, page_refused),
            Ok(outcome @ BrowserOutcome::RecordingNotFound) => {
                self.recording_selection_failure(context, outcome)
            }
            Ok(_) => self.protocol_failure(context, decision, tab_id),
            Err(error) => self.browser_failure(context, decision, error, tab_id),
        }
    }

    /// Authorize one save and name where the browser should put the result.
    fn recording_destination(
        &self,
        context: &InvocationContext<'_>,
        lease: Option<&WorkspaceLease>,
        value: &Record,
        stopped: &PhysicalRecordingSummary,
    ) -> Result<SaveDestination, Box<Terminal>> {
        if !stopped.source_urls_complete
            && !context
                .snapshot
                .permits_any_document(Capability::Read.into())
        {
            self.retain_coverage(
                context,
                crate::language::coverage::Coverage {
                    unavailable_documents: 1,
                    ..Default::default()
                },
                vec![],
            );
            return Err(Box::new(self.browser_failure(
                context,
                Decision::permitted(),
                super::BrowserError::DocumentUnavailable,
                Some(stopped.tab_id),
            )));
        }
        // Every destination discloses the captured documents, including a browser file input.
        // Each source is checked once, on the frame's trace, and a source may be embedded.
        let mut denied = None;
        let mut hosts = Vec::new();
        for url in &stopped.source_urls {
            let decision = self.authorize_document(context, Capability::Read, url);
            if !decision.allowed {
                denied.get_or_insert(decision);
                hosts.extend(observed_host(url));
            }
        }
        if let Some(decision) = denied {
            self.retain_coverage(
                context,
                crate::language::coverage::Coverage {
                    excluded_documents: hosts.len(),
                    page_excluded_documents: hosts.len(),
                    ..Default::default()
                },
                hosts,
            );
            return Err(Box::new(self.blocked(
                context,
                decision,
                Some(stopped.tab_id),
                Effect::None,
                true,
                json!({}),
            )));
        }
        let decision = Decision::permitted();
        if let Some(requested_target) = value.target.as_deref() {
            let lease = lease.expect("recording target save holds the workspace lease");
            let (selected, target) =
                match self.resolve_target(context, lease, None, requested_target) {
                    Ok(value) => value,
                    Err(error) => return Err(Box::new(self.workspace_failure(context, error))),
                };
            let decision = self.authorize(context, Capability::Write, Some(selected.url.as_str()));
            if !decision.allowed {
                return Err(Box::new(self.blocked(
                    context,
                    decision,
                    Some(selected.physical_id),
                    Effect::None,
                    true,
                    json!({}),
                )));
            }
            let rebound = match self.dispatch(
                &InvocationContext {
                    requirements: CapabilitySet::WRITE,
                    ..*context
                },
                BrowserCommand::DescribeTargets {
                    tab_id: selected.physical_id,
                    locators: vec![target.locator.clone()],
                },
            ) {
                Ok(BrowserOutcome::TargetsDescribed {
                    tab_id,
                    targets,
                    rebound,
                }) if tab_id == selected.physical_id && targets.len() == 1 => rebound,
                Ok(_) => {
                    return Err(Box::new(self.protocol_failure(
                        context,
                        decision,
                        Some(selected.physical_id),
                    )))
                }
                Err(error) => {
                    return Err(Box::new(self.browser_failure(
                        context,
                        decision,
                        error,
                        Some(selected.physical_id),
                    )))
                }
            };
            return Ok(SaveDestination {
                destination: RecordingDestination::Target {
                    tab_id: selected.physical_id,
                    locator: target.locator,
                    file_name: RECORDING_FILE_NAME.into(),
                },
                decision,
                tab_id: Some(selected.physical_id),
                budget: RECORDING_LOCAL_MAX_BYTES,
                rebound,
            });
        }

        if value.download {
            return Ok(SaveDestination {
                destination: RecordingDestination::Download {
                    file_name: RECORDING_FILE_NAME.into(),
                },
                decision,
                tab_id: Some(stopped.tab_id),
                budget: RECORDING_LOCAL_MAX_BYTES,
                rebound: false,
            });
        }
        Ok(SaveDestination {
            destination: RecordingDestination::Client,
            decision,
            tab_id: Some(stopped.tab_id),
            budget: RECORDING_TRANSFER_MAX_BYTES,
            rebound: false,
        })
    }

    /// Report a delivered replay in the terms a reader cares about.
    fn recording_delivered(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        summary: &PhysicalRecordingSummary,
        encoded: EncodedRecording,
        delivery: RecordingDelivery,
        rebound: bool,
    ) -> Terminal {
        let landing = match &delivery {
            RecordingDelivery::Attached { tab_id } => Some(*tab_id),
            _ => Some(summary.tab_id),
        };
        let facts = json!({
            "state":recording_state_name(summary.state),
            "duration_ms":encoded.duration_ms,
            "frame_count":encoded.frame_count,
            "captured_frame_count":encoded.captured_frame_count,
            "gif_bytes":encoded.byte_count,
            "width":encoded.width,
            "height":encoded.height,
            "delivery":recording_delivery_name(&delivery)
        });
        let outcome = Outcome::RecordingSaved {
            duration_ms: encoded.duration_ms,
            delivery: match &delivery {
                RecordingDelivery::Attached { .. } => SavedTo::PageTarget,
                RecordingDelivery::Downloaded => SavedTo::Download,
                RecordingDelivery::Returned { .. } => SavedTo::Client,
            },
            rebound,
        };
        // Encoding the same recording twice produces the same replay, but putting it on a page or
        // on disk again is a fresh effect on the world, so only a client return is repeat-safe.
        let landed = !matches!(delivery, RecordingDelivery::Returned { .. });
        let mut terminal = self.succeeded(
            context,
            decision,
            landing,
            if landed {
                Effect::Applied
            } else {
                Effect::None
            },
            Readiness::NotApplicable,
            !landed,
            outcome,
            facts,
        );
        if let RecordingDelivery::Returned { mime_type, data } = delivery {
            terminal.result = terminal
                .result
                .with_content(ServiceContent::Image { mime_type, data });
        }
        terminal
    }

    fn discard_recording(&self, context: &InvocationContext<'_>) -> Terminal {
        // Needs no capability, but every operation that reaches the browser still crosses the
        // runtime pause gate.
        let decision = self.authorize(context, context.requirements, None);
        if !decision.allowed {
            return self.blocked(context, decision, None, Effect::None, true, json!({}));
        }
        let held = match self.held_recording(context) {
            Ok(held) => held,
            Err(terminal) => return *terminal,
        };
        let outcome = self.dispatch(
            context,
            BrowserCommand::DiscardRecording {
                recording_id: held.id,
            },
        );
        self.note_recording(context, &outcome);
        match outcome {
            Ok(BrowserOutcome::RecordingDiscarded { released_bytes }) => self.succeeded(
                context,
                decision,
                None,
                Effect::Applied,
                Readiness::NotApplicable,
                true,
                Outcome::RecordingDiscarded,
                json!({"discarded":true,"released_bytes":released_bytes}),
            ),
            Ok(outcome) => self.recording_selection_failure(context, outcome),
            Err(error) => self.browser_failure(context, decision, error, None),
        }
    }

    fn recording_observed(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        summary: &PhysicalRecordingSummary,
    ) -> Terminal {
        self.succeeded(
            context,
            decision,
            None,
            Effect::None,
            Readiness::NotApplicable,
            true,
            if summary.stop_reason == Some(RecordingStopReason::DocumentBoundary) {
                Outcome::RecordingBoundary {
                    duration_ms: summary.duration_ms,
                }
            } else {
                Outcome::RecordingObserved {
                    frames: summary.frame_count,
                    duration_ms: summary.duration_ms,
                    active: summary.state == RecordingState::Recording,
                }
            },
            recording_facts(summary),
        )
    }

    fn recording_selection_failure(
        &self,
        context: &InvocationContext<'_>,
        outcome: BrowserOutcome,
    ) -> Terminal {
        let (refusal, facts) = match outcome {
            BrowserOutcome::RecordingHeld => (Refusal::RecordingHeld, json!({})),
            BrowserOutcome::RecordingNotFound => (Refusal::RecordingUnavailable, json!({})),
            _ => return self.protocol_failure(context, Decision::permitted(), None),
        };
        self.failed(context, Decision::permitted(), None, refusal, facts)
    }

    /// A save that did not finish. A page that did not take the replay is a target to change, not
    /// a recording to give up: the recording is kept for another save.
    fn recording_export_failure(
        &self,
        context: &InvocationContext<'_>,
        reason: &str,
        page_refused: bool,
    ) -> Terminal {
        self.failed(
            context,
            Decision::permitted(),
            None,
            if page_refused {
                Refusal::RecordingNotAttached
            } else {
                Refusal::RecordingExportFailed
            },
            json!({"detail":bounded(reason, 160)}),
        )
    }
}
