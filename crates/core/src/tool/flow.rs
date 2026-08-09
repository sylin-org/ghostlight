// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ordered Ghostlight operation execution.
//!
//! A sequence is composition, not a second language. Every child is an [`Operation`] and enters
//! the same service chokepoint as a direct call. This module owns only ordering, stopping, and the
//! bounded aggregate result.

use crate::governance::config::reload::ConfigStore;
use crate::hub::authority::AuthorityStore;
use crate::hub::outbound::browser::Browser;
use crate::hub::workspace::WorkspaceRegistry;
use crate::tool::outcome::{CallOutcome, ExecutionOutcome};
use crate::tool::pipeline::run_work;
use crate::work::{CancellationToken, WorkContext};
use ghostlight_transport::operation::{
    BrowserResult, BrowserResultStatus, FlowResultData, FlowStepResult, FlowStepStatus,
    FlowTermination, FlowTerminationReason, Operation, OperationEffect, ResultPart,
    RetryDisposition, RunSequenceArguments,
};
use std::sync::Arc;
use std::time::Instant;

/// Execute one fully validated Ghostlight sequence.
pub(crate) async fn run_sequence(
    browser: &Browser,
    store: &Arc<ConfigStore>,
    authority: &AuthorityStore,
    workspaces: &WorkspaceRegistry,
    work: &WorkContext,
    cancellation: &CancellationToken,
    arguments: &RunSequenceArguments,
) -> ExecutionOutcome {
    let started = Instant::now();
    let mut steps = Vec::with_capacity(arguments.steps.len());
    let mut stop_reason = StopReason::None;
    let mut stopped_at = None;

    for (index, operation) in arguments.steps.iter().enumerate() {
        let step_number = (index + 1) as u32;
        if cancellation.is_cancelled() {
            stop_reason = StopReason::Cancelled;
            stopped_at = Some(step_number);
            break;
        }

        let child_work = work.child(operation.clone());
        let outcome = Box::pin(run_work(
            browser,
            store,
            authority,
            workspaces,
            &child_work,
            cancellation,
        ))
        .await;
        let step = sequence_step(step_number, operation, outcome);
        let status = step.status;
        steps.push(step);
        if status != FlowStepStatus::Ok {
            stop_reason = StopReason::from_status(status);
            stopped_at = Some(step_number);
            break;
        }
    }

    for (index, operation) in arguments.steps.iter().enumerate().skip(steps.len()) {
        let step_number = (index + 1) as u32;
        steps.push(synthetic_step(
            step_number,
            operation,
            FlowStepStatus::NotRun,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
            RetryDisposition::AfterStateChange,
            "",
        ));
    }

    let completed = steps
        .iter()
        .filter(|step| step.status == FlowStepStatus::Ok)
        .count() as u32;
    let total = arguments.steps.len() as u32;
    let data = FlowResultData {
        steps,
        summary: summarize(stop_reason, stopped_at, completed, total),
        duration_ms: started.elapsed().as_millis() as u64,
        termination: FlowTermination {
            reason: stop_reason.termination_reason(),
            step: stopped_at,
        },
    };
    let mut result = crate::tool::result::text_content(data.summary.clone());
    result["structuredContent"] =
        serde_json::to_value(data).expect("Ghostlight sequence result serializes");
    ExecutionOutcome::Success {
        result: result.into(),
    }
}

fn sequence_step(step: u32, operation: &Operation, outcome: CallOutcome) -> FlowStepResult {
    match outcome {
        CallOutcome::Success { result } => FlowStepResult {
            step,
            status: flow_step_status(result.status),
            result: *result,
        },
        CallOutcome::Failure { error } => synthetic_step(
            step,
            operation,
            FlowStepStatus::Unavailable,
            BrowserResultStatus::Unavailable,
            OperationEffect::None,
            RetryDisposition::AfterStateChange,
            &error.to_string(),
        ),
        CallOutcome::NotDispatched { message } => synthetic_step(
            step,
            operation,
            FlowStepStatus::NotDispatched,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
            RetryDisposition::Safe,
            &message,
        ),
        CallOutcome::OutcomeUnknown { message } => synthetic_step(
            step,
            operation,
            FlowStepStatus::OutcomeUnknown,
            BrowserResultStatus::OutcomeUnknown,
            OperationEffect::Unknown,
            RetryDisposition::Unsafe,
            &message,
        ),
        CallOutcome::Denied { message, .. } => synthetic_step(
            step,
            operation,
            FlowStepStatus::Blocked,
            BrowserResultStatus::Blocked,
            OperationEffect::None,
            RetryDisposition::AfterStateChange,
            &message,
        ),
        CallOutcome::Held { prolonged: _ } => synthetic_step(
            step,
            operation,
            FlowStepStatus::Held,
            BrowserResultStatus::Held,
            OperationEffect::None,
            RetryDisposition::AfterStateChange,
            "browser session held by user",
        ),
        CallOutcome::AttentionRequired { message } => synthetic_step(
            step,
            operation,
            FlowStepStatus::AttentionRequired,
            BrowserResultStatus::AttentionRequired,
            OperationEffect::None,
            RetryDisposition::AfterStateChange,
            &message,
        ),
        CallOutcome::Cancelled { message, effect } => synthetic_step(
            step,
            operation,
            FlowStepStatus::Cancelled,
            BrowserResultStatus::Cancelled,
            effect,
            if effect == OperationEffect::None {
                RetryDisposition::Safe
            } else {
                RetryDisposition::Unsafe
            },
            &message,
        ),
    }
}

fn synthetic_step(
    step: u32,
    operation: &Operation,
    status: FlowStepStatus,
    result_status: BrowserResultStatus,
    effect: OperationEffect,
    repeat: RetryDisposition,
    message: &str,
) -> FlowStepResult {
    let mut result = BrowserResult::new(operation.kind(), result_status, effect);
    result.repeat = repeat;
    if !message.is_empty() {
        result.parts.push(ResultPart::Text {
            text: message.to_owned(),
        });
    }
    FlowStepResult {
        step,
        status,
        result,
    }
}

const fn flow_step_status(status: BrowserResultStatus) -> FlowStepStatus {
    match status {
        BrowserResultStatus::Ok => FlowStepStatus::Ok,
        BrowserResultStatus::Partial => FlowStepStatus::Partial,
        BrowserResultStatus::NotMet => FlowStepStatus::NotMet,
        BrowserResultStatus::Blocked => FlowStepStatus::Blocked,
        BrowserResultStatus::Held => FlowStepStatus::Held,
        BrowserResultStatus::AttentionRequired => FlowStepStatus::AttentionRequired,
        BrowserResultStatus::Cancelled => FlowStepStatus::Cancelled,
        BrowserResultStatus::NotDispatched => FlowStepStatus::NotDispatched,
        BrowserResultStatus::OutcomeUnknown => FlowStepStatus::OutcomeUnknown,
        BrowserResultStatus::Unavailable => FlowStepStatus::Unavailable,
    }
}

#[derive(Clone, Copy)]
enum StopReason {
    None,
    Failed,
    Denied,
    Held,
    AttentionRequired,
    Cancelled,
}

impl StopReason {
    fn from_status(status: FlowStepStatus) -> Self {
        match status {
            FlowStepStatus::Blocked | FlowStepStatus::Denied | FlowStepStatus::WouldDeny => {
                Self::Denied
            }
            FlowStepStatus::Held => Self::Held,
            FlowStepStatus::AttentionRequired => Self::AttentionRequired,
            FlowStepStatus::Cancelled => Self::Cancelled,
            _ => Self::Failed,
        }
    }

    const fn termination_reason(self) -> FlowTerminationReason {
        match self {
            Self::None => FlowTerminationReason::Completed,
            Self::Failed => FlowTerminationReason::Failed,
            Self::Denied => FlowTerminationReason::Denied,
            Self::Held => FlowTerminationReason::Held,
            Self::AttentionRequired => FlowTerminationReason::AttentionRequired,
            Self::Cancelled => FlowTerminationReason::Cancelled,
        }
    }
}

fn summarize(reason: StopReason, stopped_at: Option<u32>, completed: u32, total: u32) -> String {
    let step = stopped_at.unwrap_or(completed + 1);
    match reason {
        StopReason::None => format!("{completed}/{total} steps completed"),
        StopReason::Failed => format!("{completed}/{total} steps completed; step {step} failed"),
        StopReason::Denied => format!("{completed}/{total} steps completed; step {step} denied"),
        StopReason::Held => format!("{completed}/{total} steps completed; held at step {step}"),
        StopReason::AttentionRequired => {
            format!("{completed}/{total} steps completed; attention required at step {step}")
        }
        StopReason::Cancelled => {
            format!("{completed}/{total} steps completed; cancelled before step {step}")
        }
    }
}
