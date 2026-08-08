// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Canonical browser-flow execution.
//!
//! A flow contains only typed [`BrowserOperation`] steps. Each step re-enters the ordinary
//! operation pipeline, so validation, governance, scheduling, audit, post-processing, result
//! conversion, and provenance are identical to a direct call. This module owns composition and
//! produces one protocol-neutral [`FlowResultData`]. Surface-specific compact or flattened result
//! shapes are rendered later by the MCP edge.

use crate::governance::config::reload::ConfigStore;
use crate::hub::authority::AuthorityStore;
use crate::hub::outbound::browser::Browser;
use crate::hub::scheduling::ExecutionContext;
use crate::hub::workspace::WorkspaceRegistry;
use crate::operation::registry::{self as operation_registry, SchedulingScope, SuccessDisposition};
use crate::operation::result::canonicalize_success;
use crate::tool::outcome::{CallOutcome, LocalCtx, LocalFuture};
use crate::tool::pipeline::{run_operation_call, schedule_failure_message};
use crate::tool::refs::resolve_refs;
use crate::work::{CancellationToken, WorkContext};
use ghostlight_transport::operation::{
    BrowserOperation, BrowserResult, BrowserResultStatus, FlowResultData, FlowStepResult,
    FlowStepStatus, FlowTermination, FlowTerminationReason, IntentId, OperationEffect,
    OperationKey, ResultPart, RetryDisposition,
};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

const FLOW_ORCHESTRATOR: &str = "browser.flow";
const COMPOSITION_QUANTUM: Duration = Duration::from_secs(60);

/// Testable seam for executing one resolved canonical flow step.
pub(crate) trait StepRunner {
    fn run(
        &mut self,
        operation: &BrowserOperation,
        orchestration: Option<(&'static str, &str, u32)>,
        dry_run: bool,
    ) -> CallOutcome;

    /// Return whether the parent work was cancelled at a composition boundary.
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct PipelineRunner<'a> {
    browser: &'a Browser,
    store: &'a Arc<ConfigStore>,
    authority: &'a AuthorityStore,
    guid: &'a str,
    overlay: Option<&'a crate::governance::overlay::SessionOverlay>,
    retained_tab: Option<i64>,
    retained_execution: Option<ExecutionContext>,
    retained_started: Option<Instant>,
    work: Option<&'a WorkContext>,
    cancellation: Option<&'a CancellationToken>,
    workspaces: Option<&'a WorkspaceRegistry>,
}

impl StepRunner for PipelineRunner<'_> {
    fn run(
        &mut self,
        operation: &BrowserOperation,
        orchestration: Option<(&'static str, &str, u32)>,
        dry_run: bool,
    ) -> CallOutcome {
        let inherited = if self.retained_tab.is_some() {
            if self.retained_execution.is_none()
                || self
                    .retained_started
                    .is_some_and(|started| started.elapsed() >= COMPOSITION_QUANTUM)
            {
                self.retained_execution.take();
                let epoch = self.authority.current().epoch;
                let Some(tab_id) = self.retained_tab else {
                    unreachable!("retained surface has a tab id")
                };
                match acquire_retained_surface(
                    self.browser,
                    self.guid,
                    tab_id,
                    epoch,
                    self.cancellation,
                ) {
                    Ok(execution) => {
                        self.retained_execution = Some(execution);
                        self.retained_started = Some(Instant::now());
                    }
                    Err(crate::hub::scheduling::ScheduleFailure::Cancelled) => {
                        return CallOutcome::Cancelled {
                            message: schedule_failure_message(
                                crate::hub::scheduling::ScheduleFailure::Cancelled,
                            ),
                            effect: OperationEffect::None,
                        };
                    }
                    Err(error) => {
                        return CallOutcome::NotDispatched {
                            message: schedule_failure_message(error),
                        };
                    }
                }
            }
            self.retained_execution.as_ref()
        } else {
            None
        };

        await_operation(
            operation,
            orchestration,
            dry_run,
            self.browser,
            self.store,
            self.authority,
            self.guid,
            self.overlay,
            inherited,
            self.work,
            self.cancellation,
            self.workspaces,
        )
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation
            .is_some_and(CancellationToken::is_cancelled)
    }
}

struct FlowRun {
    data: FlowResultData,
    batch_id: String,
}

/// Return the one concrete tab shared by every scheduled surface step.
fn single_surface_tab(arguments: &Value) -> Option<i64> {
    let inherited_tab = arguments.get("tab").and_then(Value::as_i64);
    let steps = arguments.get("steps").and_then(Value::as_array)?;
    let mut surface = None;
    for step in steps {
        let operation: BrowserOperation = serde_json::from_value(step.clone()).ok()?;
        let descriptor = operation_registry::descriptor(operation.key())?;
        match descriptor.scheduling.scope {
            SchedulingScope::Surface => {
                let tab = operation
                    .arguments
                    .get("tab")
                    .and_then(Value::as_i64)
                    .or(inherited_tab)?;
                if surface.is_some_and(|current| current != tab) {
                    return None;
                }
                surface = Some(tab);
            }
            SchedulingScope::Local | SchedulingScope::Presentation => {}
            SchedulingScope::WorkspaceTopology
            | SchedulingScope::Browser
            | SchedulingScope::Composition => return None,
        }
    }
    surface
}

fn run_flow<R: StepRunner>(
    arguments: &Value,
    runner: &mut R,
    config_budget_ms: u64,
    dry_run: bool,
) -> FlowRun {
    let started = Instant::now();
    let root_tab = arguments.get("tab").cloned();
    let on_continue = arguments.get("on_error").and_then(Value::as_str) == Some("continue");
    let mut budget_ms = config_budget_ms;
    if let Some(requested) = arguments.get("budget_ms").and_then(Value::as_u64) {
        budget_ms = budget_ms.min(requested);
    }
    let deadline = started + Duration::from_millis(budget_ms);

    let Some(raw_steps) = arguments.get("steps").and_then(Value::as_array) else {
        return error_flow("browser flow requires a steps array");
    };
    if raw_steps.is_empty() {
        return error_flow("browser flow requires at least one step");
    }
    let operations = match raw_steps
        .iter()
        .cloned()
        .map(serde_json::from_value::<BrowserOperation>)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(operations) => operations,
        Err(_) => return error_flow("browser flow contains a malformed operation step"),
    };

    let total = operations.len() as u32;
    let batch_id = uuid::Uuid::new_v4().to_string();
    let mut steps = Vec::with_capacity(operations.len());
    let mut structured = Vec::with_capacity(operations.len());
    let mut stop_reason = StopReason::None;
    let mut stopped_at = None;
    let mut cancelled_before_step = None;

    for (index, declared) in operations.iter().enumerate() {
        let step_number = (index + 1) as u32;
        if runner.is_cancelled() {
            stop_reason = StopReason::Cancelled;
            stopped_at = Some(step_number);
            cancelled_before_step = Some(step_number);
            break;
        }
        if step_number > 1 && Instant::now() > deadline {
            stop_reason = StopReason::Budget;
            stopped_at = Some(step_number - 1);
            break;
        }

        let operation = inherit_root_tab(declared.clone(), root_tab.as_ref());
        if operation.id == ghostlight_transport::operation::OperationId::BrowserFlow {
            let message = "browser flows cannot contain another flow";
            steps.push(synthetic_step(
                step_number,
                operation.key(),
                FlowStepStatus::NotDispatched,
                BrowserResultStatus::NotDispatched,
                OperationEffect::None,
                None,
                message,
            ));
            structured.push(None);
            stop_reason = StopReason::Failed;
            stopped_at = Some(step_number);
            break;
        }

        let resolved_arguments = match resolve_refs(&operation.arguments, &structured) {
            Ok(arguments) => arguments,
            Err(message) => {
                steps.push(synthetic_step(
                    step_number,
                    operation.key(),
                    FlowStepStatus::NotDispatched,
                    BrowserResultStatus::NotDispatched,
                    OperationEffect::None,
                    Some(RetryDisposition::Safe),
                    &message,
                ));
                structured.push(None);
                if dry_run || on_continue {
                    continue;
                }
                stop_reason = StopReason::Failed;
                stopped_at = Some(step_number);
                break;
            }
        };
        let resolved = BrowserOperation::new(operation.id, operation.intent, resolved_arguments);
        let outcome = runner.run(
            &resolved,
            Some((FLOW_ORCHESTRATOR, &batch_id, step_number)),
            dry_run,
        );
        let step = canonical_step(step_number, &resolved, &outcome, dry_run);
        let flow_status = step.status;
        let result_data =
            if matches!(outcome, CallOutcome::Success { .. }) && !step.result.data.is_null() {
                Some(step.result.data.clone())
            } else {
                None
            };
        steps.push(step);
        structured.push(result_data);

        if matches!(
            flow_status,
            FlowStepStatus::Held
                | FlowStepStatus::AttentionRequired
                | FlowStepStatus::Cancelled
                | FlowStepStatus::OutcomeUnknown
        ) {
            stop_reason = StopReason::from_status(flow_status);
            stopped_at = Some(step_number);
            break;
        }
        if !dry_run && flow_status != FlowStepStatus::Ok && !on_continue {
            stop_reason = StopReason::from_status(flow_status);
            stopped_at = Some(step_number);
            break;
        }
    }

    for (index, operation) in operations.iter().enumerate().skip(steps.len()) {
        let step_number = (index + 1) as u32;
        steps.push(synthetic_step(
            step_number,
            operation.key(),
            FlowStepStatus::NotRun,
            if cancelled_before_step == Some(step_number) {
                BrowserResultStatus::Cancelled
            } else {
                BrowserResultStatus::NotDispatched
            },
            OperationEffect::None,
            None,
            "",
        ));
    }

    let completed = steps
        .iter()
        .filter(|step| step.status == FlowStepStatus::Ok)
        .count() as u32;
    FlowRun {
        data: FlowResultData {
            steps,
            summary: summarize(stop_reason, stopped_at, completed, total),
            duration_ms: started.elapsed().as_millis() as u64,
            termination: FlowTermination {
                reason: stop_reason.termination_reason(),
                step: stopped_at,
            },
        },
        batch_id,
    }
}

fn inherit_root_tab(mut operation: BrowserOperation, root_tab: Option<&Value>) -> BrowserOperation {
    let Some(root_tab) = root_tab else {
        return operation;
    };
    if !operation_registry::descriptor(operation.key())
        .is_some_and(|descriptor| descriptor.accepts_flow_parent_tab())
    {
        return operation;
    }
    let Some(arguments) = operation.arguments.as_object_mut() else {
        return operation;
    };
    arguments.entry("tab").or_insert_with(|| root_tab.clone());
    operation
}

fn canonical_step(
    step: u32,
    operation: &BrowserOperation,
    outcome: &CallOutcome,
    dry_run: bool,
) -> FlowStepResult {
    match outcome {
        CallOutcome::Success { result } => {
            let Some(descriptor) = operation_registry::descriptor(operation.key()) else {
                return synthetic_step(
                    step,
                    operation.key(),
                    FlowStepStatus::Unavailable,
                    BrowserResultStatus::Unavailable,
                    OperationEffect::None,
                    None,
                    "flow step operation is unavailable",
                );
            };
            let disposition = if dry_run {
                SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::None, None)
            } else {
                descriptor.success_disposition_for(result)
            };
            match canonicalize_success(operation.key(), disposition, None, result.clone()) {
                Ok(result) => FlowStepResult {
                    step,
                    status: if dry_run {
                        FlowStepStatus::WouldAllow
                    } else {
                        FlowStepStatus::Ok
                    },
                    result,
                },
                Err(error) => synthetic_step(
                    step,
                    operation.key(),
                    FlowStepStatus::Unavailable,
                    BrowserResultStatus::Unavailable,
                    OperationEffect::None,
                    None,
                    &format!("flow step result could not be normalized: {error}"),
                ),
            }
        }
        CallOutcome::Failure { error } => synthetic_step(
            step,
            operation.key(),
            FlowStepStatus::Unavailable,
            BrowserResultStatus::Unavailable,
            OperationEffect::None,
            None,
            &error.to_string(),
        ),
        CallOutcome::NotDispatched { message } => synthetic_step(
            step,
            operation.key(),
            FlowStepStatus::NotDispatched,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
            Some(RetryDisposition::Safe),
            message,
        ),
        CallOutcome::OutcomeUnknown { message } => synthetic_step(
            step,
            operation.key(),
            FlowStepStatus::OutcomeUnknown,
            BrowserResultStatus::OutcomeUnknown,
            if dry_run {
                OperationEffect::None
            } else {
                OperationEffect::Unknown
            },
            if dry_run {
                None
            } else {
                Some(RetryDisposition::Unsafe)
            },
            message,
        ),
        CallOutcome::Denied { message, .. } => synthetic_step(
            step,
            operation.key(),
            if dry_run {
                FlowStepStatus::WouldDeny
            } else {
                FlowStepStatus::Denied
            },
            BrowserResultStatus::Blocked,
            OperationEffect::None,
            if dry_run {
                None
            } else {
                Some(RetryDisposition::AfterStateChange)
            },
            message,
        ),
        CallOutcome::Held { message } => synthetic_step(
            step,
            operation.key(),
            FlowStepStatus::Held,
            BrowserResultStatus::Held,
            OperationEffect::None,
            None,
            message,
        ),
        CallOutcome::AttentionRequired { message } => synthetic_step(
            step,
            operation.key(),
            FlowStepStatus::AttentionRequired,
            BrowserResultStatus::AttentionRequired,
            OperationEffect::None,
            None,
            message,
        ),
        CallOutcome::Cancelled { message, effect } => {
            let effect = if dry_run {
                OperationEffect::None
            } else {
                *effect
            };
            synthetic_step(
                step,
                operation.key(),
                FlowStepStatus::Cancelled,
                BrowserResultStatus::Cancelled,
                effect,
                match effect {
                    OperationEffect::None => None,
                    OperationEffect::Dispatched
                    | OperationEffect::Committed
                    | OperationEffect::Unknown => Some(RetryDisposition::Unsafe),
                },
                message,
            )
        }
    }
}

fn synthetic_step(
    step: u32,
    key: OperationKey,
    status: FlowStepStatus,
    result_status: BrowserResultStatus,
    effect: OperationEffect,
    retry: Option<RetryDisposition>,
    message: &str,
) -> FlowStepResult {
    let mut result = BrowserResult::new(key.id, key.intent, result_status, effect);
    result.retry = retry;
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

fn error_flow(message: &str) -> FlowRun {
    FlowRun {
        data: FlowResultData {
            steps: Vec::new(),
            summary: message.to_owned(),
            duration_ms: 0,
            termination: FlowTermination {
                reason: FlowTerminationReason::Failed,
                step: None,
            },
        },
        batch_id: uuid::Uuid::new_v4().to_string(),
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
    Budget,
}

impl StopReason {
    fn from_status(status: FlowStepStatus) -> Self {
        match status {
            FlowStepStatus::Denied | FlowStepStatus::WouldDeny | FlowStepStatus::Blocked => {
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
            Self::Budget => FlowTerminationReason::BudgetExhausted,
        }
    }
}

fn summarize(reason: StopReason, stopped_at: Option<u32>, completed: u32, total: u32) -> String {
    match reason {
        StopReason::None => format!("{completed}/{total} steps completed"),
        StopReason::Budget => format!(
            "{completed}/{total} steps completed; budget exhausted after step {}",
            stopped_at.unwrap_or(completed)
        ),
        StopReason::Failed => format!(
            "{completed}/{total} steps completed; step {} failed",
            stopped_at.unwrap_or(completed + 1)
        ),
        StopReason::Denied => format!(
            "{completed}/{total} steps completed; step {} denied",
            stopped_at.unwrap_or(completed + 1)
        ),
        StopReason::Held => format!(
            "{completed}/{total} steps completed; held at step {}",
            stopped_at.unwrap_or(completed + 1)
        ),
        StopReason::AttentionRequired => format!(
            "{completed}/{total} steps completed; attention required at step {}",
            stopped_at.unwrap_or(completed + 1)
        ),
        StopReason::Cancelled => format!(
            "{completed}/{total} steps completed; cancelled before step {}",
            stopped_at.unwrap_or(completed + 1)
        ),
    }
}

/// Registry entry point for execute and preflight flow intents.
pub(crate) fn flow_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move {
        let dry_run = ctx.operation.intent == IntentId::FlowPreflight;
        let arguments = &ctx.operation.arguments;
        let mut runner = PipelineRunner {
            browser: ctx.browser,
            store: ctx.store,
            authority: ctx.authority,
            guid: ctx.guid,
            overlay: ctx.overlay,
            retained_tab: if dry_run {
                None
            } else {
                single_surface_tab(arguments)
            },
            retained_execution: None,
            retained_started: None,
            work: ctx.work,
            cancellation: ctx.cancellation,
            workspaces: ctx.workspaces,
        };
        let run = run_flow(
            arguments,
            &mut runner,
            ctx.config.script_budget_ms(),
            dry_run,
        );
        let mut result = crate::tool::result::text_content(run.data.summary.clone());
        if let Some(object) = result.as_object_mut() {
            object.insert(
                "structuredContent".into(),
                serde_json::to_value(run.data).expect("flow result serializes"),
            );
            object.insert("_batch_id".into(), Value::String(run.batch_id));
            if dry_run {
                object.insert("_dry_run".into(), Value::Bool(true));
            }
        }
        CallOutcome::Success { result }
    })
}

#[allow(clippy::too_many_arguments)]
fn await_operation(
    operation: &BrowserOperation,
    orchestration: Option<(&'static str, &str, u32)>,
    dry_run: bool,
    browser: &Browser,
    store: &Arc<ConfigStore>,
    authority: &AuthorityStore,
    guid: &str,
    overlay: Option<&crate::governance::overlay::SessionOverlay>,
    inherited_execution: Option<&ExecutionContext>,
    work: Option<&WorkContext>,
    cancellation: Option<&CancellationToken>,
    workspaces: Option<&WorkspaceRegistry>,
) -> CallOutcome {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(run_operation_call(
            browser,
            store,
            authority,
            guid,
            operation,
            orchestration,
            dry_run,
            overlay,
            inherited_execution,
            None,
            work,
            cancellation,
            workspaces,
        ))
    })
}

fn acquire_retained_surface(
    browser: &Browser,
    guid: &str,
    tab_id: i64,
    authority_epoch: u64,
    cancellation: Option<&CancellationToken>,
) -> Result<ExecutionContext, crate::hub::scheduling::ScheduleFailure> {
    tokio::task::block_in_place(|| {
        let handle = tokio::runtime::Handle::current();
        let acquired = match cancellation {
            Some(cancellation) => handle.block_on(browser.acquire_composition_surface_cancellable(
                guid,
                tab_id,
                authority_epoch,
                cancellation,
            )),
            None => {
                handle.block_on(browser.acquire_composition_surface(guid, tab_id, authority_epoch))
            }
        };
        acquired
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::outcome::DenialSource;
    use ghostlight_transport::operation::{OperationId, PageProvenance};
    use std::collections::VecDeque;

    struct StubRunner {
        outcomes: VecDeque<CallOutcome>,
        calls: Vec<BrowserOperation>,
        orchestration: Vec<(&'static str, String, u32)>,
    }

    impl StubRunner {
        fn new(outcomes: Vec<CallOutcome>) -> Self {
            Self {
                outcomes: outcomes.into(),
                calls: Vec::new(),
                orchestration: Vec::new(),
            }
        }
    }

    impl StepRunner for StubRunner {
        fn run(
            &mut self,
            operation: &BrowserOperation,
            orchestration: Option<(&'static str, &str, u32)>,
            _dry_run: bool,
        ) -> CallOutcome {
            self.calls.push(operation.clone());
            if let Some((name, batch, step)) = orchestration {
                self.orchestration.push((name, batch.to_owned(), step));
            }
            self.outcomes
                .pop_front()
                .unwrap_or_else(|| CallOutcome::Success {
                    result: json!({"content": [{"type": "text", "text": "ok"}]}),
                })
        }
    }

    struct BoundaryCancelledRunner;

    impl StepRunner for BoundaryCancelledRunner {
        fn run(
            &mut self,
            _operation: &BrowserOperation,
            _orchestration: Option<(&'static str, &str, u32)>,
            _dry_run: bool,
        ) -> CallOutcome {
            panic!("a boundary-cancelled flow must not run a step")
        }

        fn is_cancelled(&self) -> bool {
            true
        }
    }

    fn operation(id: OperationId, intent: IntentId, arguments: Value) -> BrowserOperation {
        BrowserOperation::new(id, intent, arguments)
    }

    fn flow(steps: Vec<BrowserOperation>) -> Value {
        json!({
            "steps": steps
                .into_iter()
                .map(|step| serde_json::to_value(step).expect("step serializes"))
                .collect::<Vec<_>>()
        })
    }

    fn ok(text: &str) -> CallOutcome {
        CallOutcome::Success {
            result: json!({"content": [{"type": "text", "text": text}]}),
        }
    }

    #[test]
    fn successful_steps_keep_all_parts_data_and_typed_provenance() {
        let args = flow(vec![operation(
            OperationId::BrowserScreenshot,
            IntentId::ScreenshotViewport,
            json!({"tab": 7}),
        )]);
        let mut runner = StubRunner::new(vec![CallOutcome::Success {
            result: json!({
                "content": [
                    {"type": "text", "text": "captured"},
                    {"type": "image", "data": "AAAA", "mimeType": "image/jpeg"}
                ],
                "structuredContent": {
                    "imageId": "img_1",
                    "provenance": {
                        "pageSourced": true,
                        "untrusted": true,
                        "topOrigin": "https://example.com",
                        "sessionNonce": "00112233445566778899aabbccddeeff"
                    }
                }
            }),
        }]);
        let run = run_flow(&args, &mut runner, 120_000, false);
        let step = &run.data.steps[0];
        assert_eq!(step.status, FlowStepStatus::Ok);
        assert_eq!(step.result.parts.len(), 2);
        assert_eq!(step.result.data, json!({"imageId": "img_1"}));
        assert!(matches!(
            step.result.provenance,
            Some(ref provenance) if provenance.top_origin() == Some("https://example.com")
        ));
        let encoded = serde_json::to_string(&run.data).expect("serialize flow data");
        assert!(!encoded.contains("\"tool\""));
        assert!(!encoded.contains("\"name\""));
        assert!(!encoded.contains("structuredContent"));
        assert!(!encoded.contains("sessionNonce"));
    }

    #[test]
    fn first_step_denial_is_blocked_and_remaining_steps_are_not_run() {
        let args = flow(vec![
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"tab": 7, "query": "Save"}),
            ),
            operation(
                OperationId::BrowserAct,
                IntentId::ActClick,
                json!({"tab": 7, "target": {"ref": "r_1"}}),
            ),
        ]);
        let mut runner = StubRunner::new(vec![CallOutcome::Denied {
            message: "denied".into(),
            source: DenialSource::Policy,
        }]);
        let run = run_flow(&args, &mut runner, 120_000, false);
        assert_eq!(run.data.steps[0].status, FlowStepStatus::Denied);
        assert_eq!(
            run.data.steps[0].result.status,
            BrowserResultStatus::Blocked
        );
        assert_eq!(run.data.steps[0].result.effect, OperationEffect::None);
        assert_eq!(run.data.steps[1].status, FlowStepStatus::NotRun);
        assert_eq!(run.data.steps[1].result.effect, OperationEffect::None);
        assert_eq!(run.data.summary, "0/2 steps completed; step 1 denied");
    }

    #[test]
    fn boundary_cancellation_is_not_misreported_as_aggregate_success() {
        let args = flow(vec![
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"tab": 7, "query": "one"}),
            ),
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"tab": 7, "query": "two"}),
            ),
        ]);
        let run = run_flow(&args, &mut BoundaryCancelledRunner, 120_000, false);
        assert_eq!(run.data.steps.len(), 2);
        assert!(run
            .data
            .steps
            .iter()
            .all(|step| step.status == FlowStepStatus::NotRun));
        assert_eq!(
            run.data.steps[0].result.status,
            BrowserResultStatus::Cancelled
        );
        assert_eq!(
            run.data.steps[1].result.status,
            BrowserResultStatus::NotDispatched
        );

        let descriptor = operation_registry::descriptor(OperationKey::new(
            OperationId::BrowserFlow,
            IntentId::FlowExecute,
        ))
        .expect("flow descriptor");
        let disposition = descriptor.success_disposition_for(&json!({
            "content": [],
            "structuredContent": run.data,
        }));
        assert_eq!(
            disposition,
            SuccessDisposition::new(BrowserResultStatus::Cancelled, OperationEffect::None, None)
        );
    }

    #[test]
    fn committed_step_then_failure_keeps_both_truthful_step_effects() {
        let args = flow(vec![
            operation(
                OperationId::BrowserAct,
                IntentId::ActClick,
                json!({"tab": 7, "target": {"ref": "r_1"}}),
            ),
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"tab": 7, "query": "Save"}),
            ),
        ]);
        let mut runner = StubRunner::new(vec![
            ok("clicked"),
            CallOutcome::Failure {
                error: crate::ToolError::invalid_request("failed"),
            },
        ]);
        let run = run_flow(&args, &mut runner, 120_000, false);
        assert_eq!(run.data.steps[0].result.effect, OperationEffect::Committed);
        assert_eq!(run.data.steps[1].status, FlowStepStatus::Unavailable);
        assert_eq!(run.data.steps[1].result.effect, OperationEffect::None);
        assert_eq!(run.data.summary, "1/2 steps completed; step 2 failed");
    }

    #[test]
    fn acknowledged_error_success_keeps_legacy_flow_control_and_truthful_result() {
        let args = flow(vec![
            operation(
                OperationId::BrowserAct,
                IntentId::ActClick,
                json!({"tab": 7, "target": {"ref": "r_1"}}),
            ),
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"tab": 7, "query": "Saved"}),
            ),
        ]);
        let mut runner = StubRunner::new(vec![
            CallOutcome::Success {
                result: json!({
                    "content": [{"type": "text", "text": "clicked; expectation timed out"}],
                    "isError": true,
                    "structuredContent": {
                        "interactionReceipt": {
                            "blockers": [{"kind": "expect_timeout"}]
                        }
                    }
                }),
            },
            ok("found"),
        ]);
        let run = run_flow(&args, &mut runner, 120_000, false);

        assert_eq!(runner.calls.len(), 2, "acknowledged successes continue");
        assert_eq!(run.data.steps[0].status, FlowStepStatus::Ok);
        assert_eq!(
            run.data.steps[0].result.status,
            BrowserResultStatus::Partial
        );
        assert_eq!(run.data.steps[0].result.effect, OperationEffect::Committed);
        assert_eq!(run.data.steps[1].status, FlowStepStatus::Ok);
        assert_eq!(run.data.summary, "2/2 steps completed");
    }

    #[test]
    fn executed_cancellation_keeps_a_canonical_not_run_tail() {
        let args = flow(vec![
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"tab": 7, "query": "one"}),
            ),
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"tab": 7, "query": "two"}),
            ),
        ]);
        let mut runner = StubRunner::new(vec![CallOutcome::Cancelled {
            message: "cancelled".into(),
            effect: OperationEffect::None,
        }]);
        let run = run_flow(&args, &mut runner, 120_000, false);

        assert_eq!(run.data.steps.len(), 2);
        assert_eq!(run.data.steps[0].status, FlowStepStatus::Cancelled);
        assert_eq!(run.data.steps[1].status, FlowStepStatus::NotRun);
        assert_eq!(
            run.data.summary,
            "0/2 steps completed; cancelled before step 1"
        );
    }

    #[test]
    fn cancellation_after_an_acknowledged_step_effect_is_committed_and_unsafe() {
        let args = flow(vec![operation(
            OperationId::BrowserAct,
            IntentId::ActClick,
            json!({"tab": 7, "target": {"ref": "r_1"}}),
        )]);
        let mut runner = StubRunner::new(vec![CallOutcome::Cancelled {
            message: "action completed before cancellation".into(),
            effect: OperationEffect::Committed,
        }]);
        let run = run_flow(&args, &mut runner, 120_000, false);
        let result = &run.data.steps[0].result;

        assert_eq!(run.data.steps[0].status, FlowStepStatus::Cancelled);
        assert_eq!(result.status, BrowserResultStatus::Cancelled);
        assert_eq!(result.effect, OperationEffect::Committed);
        assert_eq!(result.retry, Some(RetryDisposition::Unsafe));
    }

    #[test]
    fn preflight_never_claims_effects_and_continues_after_would_deny() {
        let args = flow(vec![
            operation(
                OperationId::BrowserAct,
                IntentId::ActClick,
                json!({"tab": 7, "target": {"ref": "r_1"}}),
            ),
            operation(
                OperationId::BrowserAct,
                IntentId::ActClick,
                json!({"tab": 7, "target": {"ref": "r_2"}}),
            ),
        ]);
        let mut runner = StubRunner::new(vec![
            ok("would run"),
            CallOutcome::Denied {
                message: "would deny".into(),
                source: DenialSource::Policy,
            },
        ]);
        let run = run_flow(&args, &mut runner, 120_000, true);
        assert_eq!(run.data.steps[0].status, FlowStepStatus::WouldAllow);
        assert_eq!(run.data.steps[1].status, FlowStepStatus::WouldDeny);
        assert!(run
            .data
            .steps
            .iter()
            .all(|step| step.result.effect == OperationEffect::None));
    }

    #[test]
    fn outcome_unknown_is_explicit_and_never_invites_retry() {
        let args = flow(vec![operation(
            OperationId::BrowserAct,
            IntentId::ActClick,
            json!({"tab": 7, "target": {"ref": "r_1"}}),
        )]);
        let mut runner = StubRunner::new(vec![CallOutcome::OutcomeUnknown {
            message: "unknown".into(),
        }]);
        let run = run_flow(&args, &mut runner, 120_000, false);
        let result = &run.data.steps[0].result;
        assert_eq!(run.data.steps[0].status, FlowStepStatus::OutcomeUnknown);
        assert_eq!(result.status, BrowserResultStatus::OutcomeUnknown);
        assert_eq!(result.effect, OperationEffect::Unknown);
        assert_eq!(result.retry, Some(RetryDisposition::Unsafe));
    }

    #[test]
    fn references_resolve_against_prior_canonical_data() {
        let args = flow(vec![
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"tab": 7, "query": "Save"}),
            ),
            operation(
                OperationId::BrowserAct,
                IntentId::ActClick,
                json!({"tab": 7, "target": {"ref": "$prev.results.0.ref"}}),
            ),
        ]);
        let mut runner = StubRunner::new(vec![
            CallOutcome::Success {
                result: json!({
                    "content": [{"type": "text", "text": "found"}],
                    "structuredContent": {"results": [{"ref": "r_42"}]}
                }),
            },
            ok("clicked"),
        ]);
        let run = run_flow(&args, &mut runner, 120_000, false);
        assert_eq!(run.data.steps.len(), 2);
        assert_eq!(runner.calls[1].arguments["target"]["ref"], "r_42");
        assert_eq!(runner.orchestration[0].0, FLOW_ORCHESTRATOR);
        assert_eq!(runner.orchestration[0].1, runner.orchestration[1].1);
    }

    #[test]
    fn static_surface_detection_reads_canonical_steps_only() {
        let same = json!({
            "steps": [
                serde_json::to_value(operation(
                    OperationId::BrowserFind,
                    IntentId::FindQuery,
                    json!({"tab": 7, "query": "one"})
                )).unwrap(),
                serde_json::to_value(operation(
                    OperationId::BrowserRead,
                    IntentId::ReadText,
                    json!({"tab": 7})
                )).unwrap()
            ]
        });
        assert_eq!(single_surface_tab(&same), Some(7));
        let mut mixed = same;
        mixed["steps"][1]["arguments"]["tab"] = json!(8);
        assert_eq!(single_surface_tab(&mixed), None);
    }

    #[test]
    fn root_tab_is_inherited_only_by_operations_that_declare_tab() {
        let mut args = flow(vec![
            operation(OperationId::BrowserTabs, IntentId::TabsNew, json!({})),
            operation(
                OperationId::BrowserContext,
                IntentId::ContextDescribe,
                json!({}),
            ),
            operation(
                OperationId::WorkflowPlan,
                IntentId::PlanUpdate,
                json!({"plan": []}),
            ),
            operation(
                OperationId::BrowserFind,
                IntentId::FindQuery,
                json!({"query": "Save"}),
            ),
        ]);
        args["tab"] = json!(7);
        let mut runner = StubRunner::new(Vec::new());
        let _ = run_flow(&args, &mut runner, 120_000, false);

        for call in &runner.calls[..3] {
            assert!(call.arguments.get("tab").is_none());
        }
        assert_eq!(runner.calls[3].arguments["tab"], 7);
    }

    #[test]
    fn typed_provenance_type_is_retained_by_nested_result() {
        let provenance = PageProvenance::new(
            vec!["/parts/0/text".into()],
            Some("https://example.com".into()),
            Some("session".into()),
            None,
        )
        .expect("valid provenance");
        let mut result = BrowserResult::new(
            OperationId::BrowserRead,
            IntentId::ReadText,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.provenance = Some(provenance.clone());
        let step = FlowStepResult {
            step: 1,
            status: FlowStepStatus::Ok,
            result,
        };
        assert_eq!(step.result.provenance, Some(provenance));
    }
}
