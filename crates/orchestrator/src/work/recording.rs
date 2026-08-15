//! Governed recording lifecycle, export, and delivery execution.

use ghostlight_bridge::browser::{
    BrowserCommand, BrowserOutcome, EncodedRecording, PhysicalRecordingSummary, RecordingDelivery,
    RecordingDestination, RecordingState, RECORDING_LOCAL_MAX_BYTES, RECORDING_TRANSFER_MAX_BYTES,
};
use ghostlight_bridge::service::ServiceContent;
use serde_json::json;

use crate::governance::{Capability, CapabilitySet, Decision};
use crate::language::{
    outcome::{Outcome, Refusal, SavedTo},
    Record,
};
use crate::workspace::WorkspaceLease;

use super::{
    bounded, observed_host, permitted, readiness, recording_delivery_name, recording_facts,
    recording_state_name, ApplicationExecutor, Effect, InvocationContext, Readiness, Terminal,
    RECORDING_FILE_NAME,
};

impl ApplicationExecutor {
    pub(super) fn perform_record(
        &self,
        context: &InvocationContext<'_>,
        lease: Option<&WorkspaceLease>,
        value: &Record,
    ) -> Terminal {
        match value.action.as_str() {
            "start" => self.start_recording(
                context,
                lease.expect("recording start holds the workspace lease"),
                value,
            ),
            "status" => {
                // Needs no capability, but every path to the browser still crosses the runtime
                // gate -- status/stop/discard used to dispatch straight through, the one family
                // of operations in this executor that ignored a pause. See stop_recording and
                // discard_recording for the same fix and the same reasoning.
                let decision = self.authorize(context, CapabilitySet::EMPTY, None);
                if !decision.allowed {
                    return self.blocked(
                        context,
                        decision,
                        None,
                        Effect::None,
                        true,
                        json!({"reason":decision.reason.as_str()}),
                    );
                }
                match self.dispatch(
                    context,
                    BrowserCommand::StatusRecording {
                        recording_id: value.recording.clone(),
                    },
                ) {
                    Ok(BrowserOutcome::RecordingStatus { summary }) => {
                        self.recording_observed(context, decision, &summary)
                    }
                    Ok(outcome) => self.recording_selection_failure(context, outcome),
                    Err(error) => self.browser_failure(context, decision, error, None),
                }
            }
            "stop" => self.stop_recording(context, value.recording.as_deref()),
            "save" => self.save_recording(context, lease, value),
            "discard" => self.discard_recording(context, value.recording.as_deref()),
            _ => unreachable!("recording action was validated"),
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
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::StartRecording {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::RecordingStarted { summary, existing })
                if summary.tab_id == selected.physical_id =>
            {
                let mut facts = recording_facts(&summary);
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
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn stop_recording(&self, context: &InvocationContext<'_>, requested: Option<&str>) -> Terminal {
        // Needs no capability, but every operation that reaches the browser still crosses the
        // runtime pause/attention gate -- this one used to dispatch straight through it, so a
        // paused session could still have its recording stopped from underneath it.
        let decision = self.authorize(context, CapabilitySet::EMPTY, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::StopRecording {
                recording_id: requested.map(str::to_owned),
            },
        ) {
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
        requested: Option<&str>,
    ) -> Result<PhysicalRecordingSummary, Box<Terminal>> {
        match self.dispatch(
            context,
            BrowserCommand::StopRecording {
                recording_id: requested.map(str::to_owned),
            },
        ) {
            Ok(BrowserOutcome::RecordingStopped { summary, .. }) => Ok(summary),
            Ok(outcome) => Err(Box::new(self.recording_selection_failure(context, outcome))),
            Err(error) => Err(Box::new(self.browser_failure(
                context,
                permitted(),
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
        let stopped = match self.ensure_recording_stopped(context, value.recording.as_deref()) {
            Ok(summary) => summary,
            Err(terminal) => return *terminal,
        };

        let (destination, decision, tab_id, budget) =
            match self.recording_destination(context, lease, value, &stopped) {
                Ok(resolved) => resolved,
                Err(terminal) => return *terminal,
            };

        match self.dispatch(
            context,
            BrowserCommand::ExportRecording {
                recording_id: Some(stopped.recording_id.clone()),
                destination,
                max_output_bytes: budget,
            },
        ) {
            Ok(BrowserOutcome::RecordingExported {
                summary,
                encoded,
                delivery,
            }) if summary.recording_id == stopped.recording_id
                && summary.state != RecordingState::Recording =>
            {
                self.recording_delivered(context, decision, &summary, encoded, delivery)
            }
            Ok(BrowserOutcome::RecordingExportFailed { reason }) => {
                self.recording_export_failure(context, &reason)
            }
            Ok(
                outcome @ (BrowserOutcome::RecordingAmbiguous { .. }
                | BrowserOutcome::RecordingNotFound),
            ) => self.recording_selection_failure(context, outcome),
            Ok(_) => self.protocol_failure(context, decision, tab_id),
            Err(error) => self.browser_failure(context, decision, error, tab_id),
        }
    }

    /// Authorize one save and name where the browser should put the result.
    #[allow(clippy::type_complexity)]
    fn recording_destination(
        &self,
        context: &InvocationContext<'_>,
        lease: Option<&WorkspaceLease>,
        value: &Record,
        stopped: &PhysicalRecordingSummary,
    ) -> Result<(RecordingDestination, Decision, Option<u64>, usize), Box<Terminal>> {
        if let Some(requested_target) = value.target.as_deref() {
            let lease = lease.expect("recording target save holds the workspace lease");
            let (selected, target) = match self.resolve_target(lease, None, requested_target) {
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
                    json!({"reason":decision.reason.as_str()}),
                )));
            }
            match self.dispatch(
                context,
                BrowserCommand::DescribeTargets {
                    tab_id: selected.physical_id,
                    locators: vec![target.locator.clone()],
                },
            ) {
                Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                    if tab_id == selected.physical_id && targets.len() == 1 =>
                {
                    if targets[0].credential_class {
                        return Err(Box::new(
                            self.credential_handoff(context, decision, &selected),
                        ));
                    }
                }
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
            }
            return Ok((
                RecordingDestination::Target {
                    tab_id: selected.physical_id,
                    locator: target.locator,
                    file_name: RECORDING_FILE_NAME.into(),
                },
                decision,
                Some(selected.physical_id),
                RECORDING_LOCAL_MAX_BYTES,
            ));
        }

        // A download stays in the browser, but the recording still pictures pages the caller
        // must be allowed to read, so both remaining destinations are authorized the same way.
        let denied = stopped.source_urls.iter().find_map(|url| {
            let decision = context.snapshot.authorize_landing(Capability::Read, url);
            (!decision.allowed).then_some(decision)
        });
        let decision = denied.unwrap_or_else(permitted);
        if !decision.allowed {
            return Err(Box::new(self.blocked(
                context,
                decision,
                Some(stopped.tab_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            )));
        }
        if value.download {
            return Ok((
                RecordingDestination::Download {
                    file_name: RECORDING_FILE_NAME.into(),
                },
                decision,
                Some(stopped.tab_id),
                RECORDING_LOCAL_MAX_BYTES,
            ));
        }
        Ok((
            RecordingDestination::Client,
            decision,
            Some(stopped.tab_id),
            RECORDING_TRANSFER_MAX_BYTES,
        ))
    }

    /// Report a delivered replay in the terms a reader cares about.
    fn recording_delivered(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        summary: &PhysicalRecordingSummary,
        encoded: EncodedRecording,
        delivery: RecordingDelivery,
    ) -> Terminal {
        let landing = match &delivery {
            RecordingDelivery::Attached { tab_id } => Some(*tab_id),
            _ => Some(summary.tab_id),
        };
        let facts = json!({
            "recording":summary.recording_id,
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

    fn discard_recording(
        &self,
        context: &InvocationContext<'_>,
        requested: Option<&str>,
    ) -> Terminal {
        // Needs no capability, but every operation that reaches the browser still crosses the
        // runtime pause/attention gate -- this one used to dispatch straight through it.
        let decision = self.authorize(context, CapabilitySet::EMPTY, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::DiscardRecording {
                recording_id: requested.map(str::to_owned),
            },
        ) {
            Ok(BrowserOutcome::RecordingDiscarded {
                recording_id,
                released_bytes,
            }) => self.succeeded(
                context,
                decision,
                None,
                Effect::Applied,
                Readiness::NotApplicable,
                true,
                Outcome::RecordingDiscarded,
                json!({
                    "recording":recording_id,
                    "discarded":true,
                    "released_bytes":released_bytes
                }),
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
            Outcome::RecordingObserved {
                frames: summary.frame_count,
                duration_ms: summary.duration_ms,
            },
            recording_facts(summary),
        )
    }

    fn recording_selection_failure(
        &self,
        context: &InvocationContext<'_>,
        outcome: BrowserOutcome,
    ) -> Terminal {
        let facts = match outcome {
            BrowserOutcome::RecordingAmbiguous { recording_ids } => {
                json!({"reason":"ambiguous","recordings":recording_ids})
            }
            BrowserOutcome::RecordingNotFound => json!({"reason":"not_found"}),
            _ => return self.protocol_failure(context, permitted(), None),
        };
        self.failed(
            context,
            permitted(),
            None,
            Refusal::RecordingUnavailable,
            facts,
        )
    }

    fn recording_export_failure(&self, context: &InvocationContext<'_>, reason: &str) -> Terminal {
        self.failed(
            context,
            permitted(),
            None,
            Refusal::RecordingExportFailed,
            json!({"reason":bounded(reason, 160)}),
        )
    }
}
