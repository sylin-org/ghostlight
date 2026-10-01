//! One projection of frozen Work truth into caller payload and content-minimized retained language.

use serde::{Deserialize, Deserializer, Serialize};

use crate::work::resolution::{ClientPayload, Conclusion, Phase, Resolution, Verification};
use crate::work::result::{Effect, InvocationResult, Status};

use super::audit::{AuditProjection, AuditRefusal};
use super::history::{OutcomePresentation, OutcomeTone};
use super::outcome::{BlockedReason, Refusal};

/// Safe phase vocabulary retained independently of volatile browser receipts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionPhase {
    Admission,
    Preparation,
    RequestedEffect,
    Verification,
    Compensation,
}

/// Declared check state, independent of the requested action's confirmed effects.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    NotRequested,
    Met,
    NotMet,
    Unavailable,
}

/// Closed mechanical counts without values, conditions, selectors, scripts or browser text.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionProgress {
    pub attempted: u32,
    pub acknowledged: u32,
    pub confirmed_effects: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<u32>,
}

/// Additive metadata written only by the canonical frozen-resolution projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedResolution {
    pub phase: ResolutionPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<AuditRefusal>,
    pub verification: VerificationState,
    pub progress: ResolutionProgress,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition_issue: Option<super::composition::StepIssue>,
    pub presentation: OutcomePresentation,
}

/// The two permitted readings of one immutable Work resolution.
pub(crate) struct Projection {
    pub caller: InvocationResult,
    pub retained: AuditProjection,
}

/// Author both receipts once; transient client facts and guidance cannot author retained language.
#[must_use]
pub(crate) fn project(resolution: &Resolution, mut payload: ClientPayload) -> Projection {
    let mut retained = match resolution.conclusion() {
        Conclusion::Outcome(outcome) => outcome.audit(),
        Conclusion::Refusal(refusal) => refusal.audit(),
    };
    let (next_steps, repeat_detail) = recovery(resolution);
    retained.summary = summary(resolution).chars().take(500).collect();
    retained.next_steps = next_steps;
    retained.refusal = resolution
        .cause()
        .and_then(|cause| cause.audit().refusal().cloned());
    if let Some(coverage) = resolution.coverage() {
        retained = retained.with_coverage(coverage);
        retained.summary = retained.summary.chars().take(500).collect();
    }
    retained = retained
        .with_tools(resolution.tools())
        .with_unconfirmed_history(resolution.unconfirmed_history_steps());
    retained.resolution = Some(metadata(resolution, &retained.summary, repeat_detail));
    // The original nested composition vocabulary was strict. New causes live in additive metadata
    // so predecessor readers can still read the existing parent receipt fields.
    if let Some(issue) = retained
        .composition
        .as_mut()
        .and_then(|progress| progress.issue.as_mut())
    {
        if issue.cause == super::composition::StepCause::ScriptException {
            issue.cause = super::composition::StepCause::Failed;
        }
    }
    let progress = resolution.progress();
    if let Some(facts) = payload.facts.as_object_mut() {
        if progress.attempted > 0
            || progress.acknowledged > 0
            || progress.confirmed_effects > 0
            || progress.expected.is_some()
        {
            let mut counts = serde_json::json!({
                "attempted": progress.attempted,
                "acknowledged": progress.acknowledged,
                "confirmed_effects": progress.confirmed_effects,
            });
            if let Some(expected) = progress.expected {
                counts["expected"] = serde_json::json!(expected);
                if progress.confirmed_effects > 0 {
                    facts.insert(
                        "strokes_completed".into(),
                        serde_json::json!(progress.confirmed_effects),
                    );
                }
            }
            facts.insert("execution_progress".into(), counts);
        }
    }

    // Decode validation is the only caller-authored guidance exception. It never enters history.
    let caller_steps = if matches!(resolution.cause(), Some(Refusal::InvalidRequest))
        && !payload.guidance.is_empty()
    {
        payload.guidance
    } else {
        retained.next_steps.clone()
    };
    let mut caller = InvocationResult::new(
        resolution.invocation(),
        resolution.status(),
        resolution.effect(),
        resolution.readiness(),
        resolution.repeat_safe(),
        &retained.summary,
        payload.facts,
        caller_steps,
    );
    caller.content = payload.content;
    Projection { caller, retained }
}

fn summary(resolution: &Resolution) -> String {
    let primary = match resolution.conclusion() {
        Conclusion::Outcome(outcome) => outcome.summary(),
        Conclusion::Refusal(refusal) => refusal_summary(resolution, refusal),
    };
    match resolution.verification() {
        Verification::NotRequested | Verification::Met => primary,
        Verification::NotMet => format!("{primary} {}", Refusal::ExpectedConditionNotMet.summary()),
        Verification::Unavailable(cause) => format!("{primary} {}", verification_summary(cause)),
    }
}

fn refusal_summary(resolution: &Resolution, refusal: &Refusal) -> String {
    let progress = resolution.progress();
    if resolution.phase() == Phase::Preparation && progress.attempted == 0 {
        let problem = match refusal {
            Refusal::ConnectionLost | Refusal::BrowserStopped { .. } => {
                "The browser connection was lost during preparation.".into()
            }
            Refusal::DeadlineExpired { .. } | Refusal::DeadlineBeforeStart => {
                "The time limit expired during preparation.".into()
            }
            Refusal::CancelledAfterDispatch | Refusal::CancelledBeforeStart => {
                "Cancelled during preparation.".into()
            }
            _ => refusal.summary(),
        };
        return format!("{problem} The requested action was not sent.");
    }
    if progress.confirmed_effects > 0 {
        let count = progress.confirmed_effects;
        let prefix = if let Some(expected) = progress.expected {
            format!("Confirmed {count} of {expected} changes.")
        } else {
            format!(
                "Confirmed {count} {}.",
                if count == 1 { "change" } else { "changes" }
            )
        };
        let problem = match refusal {
            Refusal::BrowserStopped { .. } => {
                "The browser disconnected before the remaining work completed.".into()
            }
            Refusal::DeadlineExpired { .. } => {
                "The time limit expired before the remaining work completed.".into()
            }
            Refusal::BrowserAttentionProtected { .. } => {
                "Background protection stopped the remaining work.".into()
            }
            _ => refusal.summary(),
        };
        return format!("{prefix} {problem}");
    }
    refusal.summary()
}

fn verification_summary(cause: &Refusal) -> String {
    match cause {
        Refusal::AuthorityBlocked {
            reason: BlockedReason::Hold,
            ..
        } => "You paused Ghostlight before the follow-up check completed.",
        Refusal::AuthorityBlocked {
            reason: BlockedReason::SessionEnded,
            ..
        } => "You stopped Ghostlight before the follow-up check completed.",
        Refusal::AuthorityBlocked { .. } | Refusal::BrowserAttentionProtected { .. } => {
            "The follow-up check was blocked."
        }
        Refusal::ConnectionLost | Refusal::BrowserStopped { .. } => {
            "The browser connection was lost before the follow-up check confirmed the result."
        }
        Refusal::DeadlineExpired { .. } | Refusal::DeadlineBeforeStart => {
            "The follow-up check ran out of time."
        }
        Refusal::CancelledAfterDispatch | Refusal::CancelledBeforeStart => {
            "The follow-up check was cancelled."
        }
        Refusal::DocumentUnavailable => {
            "Document access could not be verified for the follow-up check."
        }
        _ => "The follow-up check could not confirm the result.",
    }
    .into()
}

fn recovery(resolution: &Resolution) -> (Vec<String>, String) {
    if let Conclusion::Outcome(super::outcome::Outcome::CompositionRan(progress)) =
        resolution.conclusion()
    {
        let detail = if matches!(resolution.effect(), Effect::Unknown | Effect::Partial) {
            "Check the per-step results before preparing unfinished work. Do not repeat the whole operation."
                .into()
        } else {
            progress.next_steps().join(" ")
        };
        return (progress.next_steps(), detail);
    }
    if matches!(resolution.effect(), Effect::Unknown | Effect::Partial) {
        return if matches!(resolution.cause(), Some(Refusal::ScriptException)) {
            (
                Refusal::ScriptException.next_steps(),
                "Check the intended changes before running this script again. Ghostlight has no expected result to verify."
                    .into(),
            )
        } else {
            (
                Refusal::EffectUnknown.next_steps(),
                "Do not repeat this action. Observe the page before preparing unfinished work."
                    .into(),
            )
        };
    }
    if resolution.effect() == Effect::Applied && resolution.status() != Status::Succeeded {
        if matches!(resolution.verification(), Verification::NotMet) {
            return (
                Refusal::ExpectedConditionNotMet.next_steps(),
                "The change was applied. Check the intended result before deciding what work remains.".into(),
            );
        }
        return (
            vec![super::control::APPLIED_BEFORE_CHECK_FAILURE.into()],
            "The change was applied. Inspect the intended result before deciding what work remains.".into(),
        );
    }
    if matches!(
        resolution.verification(),
        Verification::NotMet | Verification::Unavailable(_)
    ) {
        let step = "Inspect the intended result before preparing unfinished work; do not repeat the action to check it.";
        return (vec![step.into()], step.into());
    }
    if matches!(
        resolution.cause(),
        Some(Refusal::OperationCleanupRequired | Refusal::DocumentUnavailable)
    ) {
        let steps = resolution.cause().expect("matched cause").next_steps();
        let detail = steps.join(" ");
        return (steps, detail);
    }
    let steps = match resolution.conclusion() {
        Conclusion::Outcome(outcome) => outcome.next_steps(),
        Conclusion::Refusal(refusal) => refusal.next_steps(),
    };
    (steps, String::new())
}

fn metadata(resolution: &Resolution, summary: &str, repeat_detail: String) -> RetainedResolution {
    let cause = resolution.cause();
    let control = match cause {
        Some(Refusal::AuthorityBlocked {
            reason: BlockedReason::Hold,
            ..
        }) => Some(false),
        Some(Refusal::AuthorityBlocked {
            reason: BlockedReason::SessionEnded,
            ..
        }) => Some(true),
        _ => None,
    };
    let (label, tone) = match resolution.effect() {
        Effect::Unknown => ("Effects uncertain", OutcomeTone::Caution),
        Effect::Partial => ("Partly completed", OutcomeTone::Caution),
        _ if control == Some(false) => ("Paused by you", OutcomeTone::Controlled),
        _ if control == Some(true) => ("Stopped by you", OutcomeTone::Controlled),
        _ if resolution.status() == Status::Succeeded => ("Completed", OutcomeTone::Complete),
        _ if !resolution.decision().allowed
            || matches!(
                cause,
                Some(Refusal::AuthorityBlocked { .. } | Refusal::BrowserAttentionProtected { .. })
            ) =>
        {
            ("Request refused", OutcomeTone::Refused)
        }
        _ => ("Could not complete", OutcomeTone::Failed),
    };
    let human_summary = if resolution.effect() == Effect::None
        && resolution.progress().attempted == 0
        && matches!(resolution.verification(), Verification::NotRequested)
    {
        match control {
            Some(false) => "You paused Ghostlight. This request did not run.".into(),
            Some(true) => "You stopped Ghostlight. This request did not run.".into(),
            None => summary.into(),
        }
    } else {
        summary.into()
    };
    let progress = resolution.progress();
    RetainedResolution {
        phase: match resolution.phase() {
            Phase::Admission => ResolutionPhase::Admission,
            Phase::Preparation => ResolutionPhase::Preparation,
            Phase::RequestedEffect => ResolutionPhase::RequestedEffect,
            Phase::Verification => ResolutionPhase::Verification,
            Phase::Compensation => ResolutionPhase::Compensation,
        },
        cause: cause.and_then(|cause| cause.audit().refusal().cloned()),
        verification: match resolution.verification() {
            Verification::NotRequested => VerificationState::NotRequested,
            Verification::Met => VerificationState::Met,
            Verification::NotMet => VerificationState::NotMet,
            Verification::Unavailable(_) => VerificationState::Unavailable,
        },
        progress: ResolutionProgress {
            attempted: progress.attempted,
            acknowledged: progress.acknowledged,
            confirmed_effects: progress.confirmed_effects,
            expected: progress.expected,
        },
        composition_issue: match resolution.conclusion() {
            Conclusion::Outcome(super::outcome::Outcome::CompositionRan(progress)) => {
                progress.issue
            }
            _ => None,
        },
        presentation: OutcomePresentation {
            label: label.into(),
            tone,
            summary: human_summary,
            repeat_detail,
            reveal_detail: super::control::SHOW_TAB_GUIDANCE.into(),
        },
    }
}

/// Ignore unsupported optional metadata while preserving the otherwise valid historical receipt.
/// Reading never edits the original bytes or accepts unknown live authority semantics.
pub fn read_retained<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<RetainedResolution>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value
        .and_then(|value| {
            let resolution = serde_json::from_value::<RetainedResolution>(value.clone()).ok()?;
            // Serde's internally tagged unit variants may ignore extra fields despite
            // deny_unknown_fields. Require the closed cause to consume its entire value.
            if let Some(cause) = value.get("cause").filter(|cause| !cause.is_null()) {
                if serde_json::to_value(&resolution.cause).ok().as_ref() != Some(cause) {
                    return None;
                }
            }
            Some(resolution)
        })
        .filter(|resolution| {
            let presentation = &resolution.presentation;
            presentation.label.chars().count() <= 64
                && presentation.summary.chars().count() <= 500
                && presentation.repeat_detail.chars().count() <= 500
                && presentation.reveal_detail.chars().count() <= 500
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::composition::{
        CompositionProgress, EffectCounts, StepCause, StepCounts, StepIssue,
    };
    use crate::language::outcome::{ActionSubject, Outcome, TargetRole};
    use crate::work::resolution::Progress;
    use serde_json::json;

    fn clicked() -> Conclusion {
        Conclusion::Outcome(Outcome::TargetClicked {
            host: Some("example.com".into()),
            subject: ActionSubject::unnamed(TargetRole::Button),
        })
    }

    #[test]
    fn acknowledged_action_and_unsuccessful_check_are_one_truthful_projection() {
        for verification in [
            Verification::NotMet,
            Verification::Unavailable(Refusal::ConnectionLost),
        ] {
            let (resolution, payload) = Resolution::fixture(
                clicked(),
                Status::Succeeded,
                Effect::Applied,
                verification,
                Phase::Verification,
                Progress {
                    attempted: 1,
                    acknowledged: 1,
                    confirmed_effects: 1,
                    expected: Some(1),
                },
                json!({"value":"PRIVATE_RESULT"}),
            );
            let projection = project(&resolution, payload);
            assert_eq!(projection.caller.status, Status::Failed);
            assert_eq!(projection.caller.effect, Effect::Applied);
            assert!(!projection.caller.repeat_safe);
            assert!(projection
                .caller
                .summary
                .starts_with("Clicked a button on example.com."));
            let metadata = projection.retained.resolution().unwrap();
            match metadata.verification {
                VerificationState::NotMet => {
                    assert!(projection
                        .caller
                        .summary
                        .ends_with("The expected condition did not hold."));
                    assert_eq!(metadata.cause, Some(AuditRefusal::ExpectedConditionNotMet));
                    assert!(projection.caller.next_steps[0].contains("do not repeat"));
                }
                VerificationState::Unavailable => {
                    assert!(projection
                        .caller
                        .summary
                        .contains("follow-up check confirmed the result"));
                    assert_eq!(metadata.cause, Some(AuditRefusal::ConnectionLost));
                    assert_eq!(
                        projection.caller.next_steps,
                        vec![super::super::control::APPLIED_BEFORE_CHECK_FAILURE]
                    );
                }
                other => panic!("unexpected check {other:?}"),
            }
            assert_eq!(projection.caller.summary, projection.retained.summary());
            assert_eq!(metadata.presentation.summary, projection.retained.summary());
            assert!(
                metadata
                    .presentation
                    .repeat_detail
                    .contains("change was applied")
                    || metadata
                        .presentation
                        .repeat_detail
                        .contains("confirmed change")
            );
            assert!(!serde_json::to_string(metadata)
                .unwrap()
                .contains("PRIVATE_RESULT"));
        }
    }

    #[test]
    fn preparation_loss_cannot_claim_that_the_requested_action_was_sent() {
        let (resolution, payload) = Resolution::fixture(
            Conclusion::Refusal(Refusal::ConnectionLost),
            Status::Unknown,
            Effect::Unknown,
            Verification::NotRequested,
            Phase::Preparation,
            Progress::default(),
            json!({}),
        );
        let projection = project(&resolution, payload);
        assert_eq!(projection.caller.effect, Effect::None);
        assert_eq!(projection.caller.summary,
            "The browser connection was lost during preparation. The requested action was not sent.");
        let metadata = projection.retained.resolution().unwrap();
        assert_eq!(metadata.phase, ResolutionPhase::Preparation);
        assert_eq!(metadata.progress.attempted, 0);
        assert_eq!(metadata.presentation.tone, OutcomeTone::Failed);
        assert_eq!(metadata.presentation.summary, projection.caller.summary);
    }

    #[test]
    fn failed_read_check_and_human_control_never_invent_a_mutation_or_erase_acknowledgement() {
        for verification in [
            Verification::NotMet,
            Verification::Unavailable(Refusal::AuthorityBlocked {
                reason: BlockedReason::Hold,
                host: None,
            }),
            Verification::Unavailable(Refusal::AuthorityBlocked {
                reason: BlockedReason::Capability,
                host: None,
            }),
        ] {
            let (resolution, payload) = Resolution::fixture(
                Conclusion::Outcome(Outcome::TextRead {
                    words: 3,
                    host: Some("example.com".into()),
                }),
                Status::Succeeded,
                Effect::None,
                verification,
                Phase::Verification,
                Progress {
                    attempted: 1,
                    acknowledged: 1,
                    ..Progress::default()
                },
                json!({"text":"PRIVATE_READ"}),
            );
            let projection = project(&resolution, payload);
            assert!(projection
                .caller
                .summary
                .starts_with("Read 3 words from example.com."));
            assert!(projection
                .caller
                .next_steps
                .iter()
                .all(|step| !step.contains("action was applied")));
            assert!(!projection.caller.repeat_safe);
            let metadata = projection.retained.resolution().unwrap();
            assert_eq!(metadata.progress.confirmed_effects, 0);
            match metadata.cause {
                Some(AuditRefusal::AuthorityBlocked {
                    cause: BlockedReason::Hold,
                }) => {
                    assert_eq!(metadata.presentation.tone, OutcomeTone::Controlled);
                    assert!(metadata.presentation.summary.contains("follow-up check"));
                    assert!(!metadata
                        .presentation
                        .summary
                        .contains("This request did not run"));
                }
                Some(AuditRefusal::AuthorityBlocked {
                    cause: BlockedReason::Capability,
                }) => {
                    assert_eq!(metadata.presentation.tone, OutcomeTone::Refused);
                }
                _ => assert_eq!(metadata.presentation.tone, OutcomeTone::Failed),
            }
            assert!(!serde_json::to_string(metadata)
                .unwrap()
                .contains("PRIVATE_READ"));
        }
        let (resolution, payload) = Resolution::fixture(
            Conclusion::Refusal(Refusal::AuthorityBlocked {
                reason: BlockedReason::Hold,
                host: None,
            }),
            Status::Blocked,
            Effect::None,
            Verification::NotRequested,
            Phase::Admission,
            Progress::default(),
            json!({}),
        );
        let projection = project(&resolution, payload);
        assert_eq!(
            projection
                .retained
                .resolution()
                .unwrap()
                .presentation
                .summary,
            "You paused Ghostlight. This request did not run."
        );
        assert_eq!(
            projection.caller.summary,
            "The user paused Ghostlight. Wait for further instructions."
        );
    }

    #[test]
    fn partial_prefix_keeps_confirmed_counts_and_replaces_stale_retry_advice() {
        for effect in [Effect::Partial, Effect::Unknown] {
            let (resolution, payload) = Resolution::fixture(
                Conclusion::Refusal(Refusal::DeadlineExpired {
                    before_dispatch: true,
                }),
                Status::Failed,
                effect,
                Verification::NotRequested,
                Phase::RequestedEffect,
                Progress {
                    attempted: 3,
                    acknowledged: 2,
                    confirmed_effects: 2,
                    expected: Some(3),
                },
                json!({"value":"PRIVATE_RESULT"}),
            );
            let projection = project(&resolution, payload);
            assert!(projection
                .caller
                .summary
                .starts_with("Confirmed 2 of 3 changes."));
            assert_eq!(projection.caller.effect, effect);
            assert!(!projection.caller.repeat_safe);
            assert_eq!(
                projection.retained.next_steps(),
                Refusal::EffectUnknown.next_steps()
            );
            assert!(projection
                .caller
                .next_steps
                .iter()
                .all(|step| !step.starts_with("Repeat")));
            let metadata = projection.retained.resolution().unwrap();
            assert_eq!(metadata.progress.confirmed_effects, 2);
            assert_eq!(
                projection.caller.facts["execution_progress"]["confirmed_effects"],
                2
            );
            assert_eq!(projection.caller.facts["execution_progress"]["expected"], 3);
            assert_eq!(projection.caller.facts["strokes_completed"], 2);
            assert_eq!(metadata.presentation.tone, OutcomeTone::Caution);
            assert!(metadata
                .presentation
                .repeat_detail
                .contains("Do not repeat"));
        }
    }

    #[test]
    fn script_false_is_success_but_exception_keeps_its_closed_cause_and_recovery() {
        let (resolution, payload) = Resolution::fixture(
            Conclusion::Outcome(Outcome::ScriptEvaluated {
                host: Some("example.com".into()),
            }),
            Status::Succeeded,
            Effect::Applied,
            Verification::NotRequested,
            Phase::RequestedEffect,
            Progress {
                attempted: 1,
                acknowledged: 1,
                confirmed_effects: 1,
                expected: None,
            },
            json!({"result":false,"script":"PRIVATE_SCRIPT"}),
        );
        let projection = project(&resolution, payload);
        assert_eq!(projection.caller.status, Status::Succeeded);
        assert_eq!(projection.caller.facts["result"], false);
        assert_eq!(
            projection.caller.summary,
            "Executed JavaScript on example.com."
        );
        assert_eq!(projection.retained.resolution().unwrap().cause, None);
        assert_eq!(
            projection.retained.resolution().unwrap().presentation.tone,
            OutcomeTone::Complete
        );

        let (resolution, payload) = Resolution::fixture(
            Conclusion::Refusal(Refusal::ScriptException),
            Status::Unknown,
            Effect::Unknown,
            Verification::NotRequested,
            Phase::RequestedEffect,
            Progress {
                attempted: 1,
                ..Progress::default()
            },
            json!({"detail":"PRIVATE_EXCEPTION"}),
        );
        let projection = project(&resolution, payload);
        assert_eq!(
            projection.retained.refusal(),
            Some(&AuditRefusal::ScriptException)
        );
        assert_eq!(
            projection.retained.next_steps(),
            Refusal::ScriptException.next_steps()
        );
        assert!(projection.caller.summary.contains("threw an exception"));
        assert!(!serde_json::to_string(&projection.retained.resolution())
            .unwrap()
            .contains("PRIVATE_EXCEPTION"));
    }

    #[test]
    fn client_error_text_and_validation_guidance_cannot_enter_retained_language() {
        for refusal in [Refusal::BrowserPrimitive, Refusal::InvalidRequest] {
            let invalid = refusal == Refusal::InvalidRequest;
            let (resolution, mut payload) = Resolution::fixture(
                Conclusion::Refusal(refusal),
                Status::Failed,
                Effect::None,
                Verification::NotRequested,
                Phase::Admission,
                Progress::default(),
                json!({"detail":"PRIVATE_EXCEPTION"}),
            );
            payload.guidance = vec!["PRIVATE_CALLER_GUIDANCE".into()];
            let projection = project(&resolution, payload);
            assert_eq!(projection.caller.facts["detail"], "PRIVATE_EXCEPTION");
            assert_eq!(
                projection
                    .caller
                    .next_steps
                    .iter()
                    .any(|step| step.contains("PRIVATE_CALLER_GUIDANCE")),
                invalid
            );
            assert!(!projection.retained.summary().contains("PRIVATE"));
            assert!(projection
                .retained
                .next_steps()
                .iter()
                .all(|step| !step.contains("PRIVATE")));
            assert!(!serde_json::to_string(&projection.retained.resolution())
                .unwrap()
                .contains("PRIVATE"));
        }
    }

    #[test]
    fn new_composition_cause_stays_exact_without_changing_the_strict_legacy_vocabulary() {
        let progress = CompositionProgress {
            counts: StepCounts {
                total: 2,
                succeeded: 1,
                unknown: 1,
                ..StepCounts::default()
            },
            effects: EffectCounts {
                applied: 1,
                unknown: 1,
                ..EffectCounts::default()
            },
            stopped: true,
            issue: Some(StepIssue {
                step: 2,
                cause: StepCause::ScriptException,
            }),
        };
        let (resolution, payload) = Resolution::fixture(
            Conclusion::Outcome(Outcome::CompositionRan(progress)),
            Status::Unknown,
            Effect::Unknown,
            Verification::NotRequested,
            Phase::RequestedEffect,
            Progress::default(),
            json!({"steps":"PRIVATE_CHILD_RESULTS"}),
        );
        let projection = project(&resolution, payload);
        assert!(projection
            .caller
            .summary
            .contains("The script in step 2 threw an exception."));
        assert_eq!(
            projection.retained.resolution().unwrap().composition_issue,
            progress.issue
        );
        assert_eq!(
            projection
                .retained
                .composition()
                .unwrap()
                .issue
                .unwrap()
                .cause,
            StepCause::Failed
        );
        assert!(projection
            .retained
            .resolution()
            .unwrap()
            .presentation
            .repeat_detail
            .contains("Do not repeat the whole operation"));
        assert!(!serde_json::to_string(&projection.retained.resolution())
            .unwrap()
            .contains("PRIVATE_CHILD_RESULTS"));
    }
}
