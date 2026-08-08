// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The canonical browser-operation registry.
//!
//! One row exists for each implemented `(OperationId, IntentId)` pair. Surface names and action
//! discriminators are translated before lookup and are never validity, capability, scheduling,
//! or dispatch keys. During the R1 bridge migration, validation schemas and old extension command
//! aliases are shared with the frozen legacy implementation. Those are bounded compatibility
//! references, not a second execution registry.

use crate::governance::manifest::document::Grant;
use crate::governance::ports::{capability_subset, Capability};
use crate::tool::outcome::{LocalCtx, LocalFuture};
use crate::ToolError;
use ghostlight_transport::bridge::{CatalogProjection, OperationAvailability, WorkspaceUse};
#[cfg(test)]
use ghostlight_transport::operation::{BrowserOperation, FlowTermination};
use ghostlight_transport::operation::{
    BrowserResultStatus, FlowResultData, FlowStepStatus, FlowTerminationReason, IntentId,
    OperationEffect, OperationId, OperationKey, RetryDisposition,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

const CONTEXT_RESULT_SCHEMA: &str = "ghostlight.browser.context/v1";
const MAX_MANAGED_TIMESTAMP_CHARS: usize = 128;
const CAPABILITY_SEMANTICS: &[(Capability, &str)] = &[
    (Capability::Read, "retrieve_observe_only"),
    (Capability::Action, "page_determined_ui_input"),
    (Capability::Write, "declared_state_change"),
    (Capability::Execute, "arbitrary_code"),
];

/// The resource shape used to resolve governance authority for one operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceShape {
    /// No governed page resource is needed.
    DomainLess,
    /// Resolve authority from the addressed tab's current URL.
    TabScoped,
    /// Resolve authority from an explicit target URL.
    TargetArg,
    /// Use already-established in-memory recording authority.
    RecordingScoped,
}

/// How an admitted operation reaches its implementation.
#[derive(Clone, Copy)]
pub enum Handler {
    /// Compile and dispatch one typed physical browser mechanism.
    Mechanism,
    /// Execute a service-local semantic handler.
    Local(for<'a> fn(LocalCtx<'a>) -> LocalFuture<'a>),
}

/// Browser-dependent behavior performed after the primary dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostDispatch {
    /// No post-dispatch operation behavior.
    None,
    /// Re-check the committed navigation landing before returning it.
    NavigateLanding,
}

/// How page-derived output is bounded and marked at the service result seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOutput {
    /// No page-authored output.
    None,
    /// Page-authored text.
    Text,
    /// A mixed service-authored interaction receipt containing page facts.
    Receipt,
    /// Only named structured fields are page-authored.
    Structured,
}

/// Scheduler resource selected for one canonical operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingScope {
    /// Serialize on one native tab surface.
    Surface,
    /// Serialize workspace topology changes.
    WorkspaceTopology,
    /// Exclude other work in one browser slot.
    Browser,
    /// Presentation-only lane.
    Presentation,
    /// Service-local lane.
    Local,
    /// Resolve scheduling at each concrete flow step.
    Composition,
}

/// Lifetime of an admitted scheduler lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseMode {
    /// One physical dispatch.
    Dispatch,
    /// Retain one surface across bounded semantic sub-operations.
    RetainSurface,
    /// Schedule each concrete composition step.
    Composition,
}

/// Canonical scheduling declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scheduling {
    /// Resource class.
    pub scope: SchedulingScope,
    /// Lease lifetime.
    pub lease: LeaseMode,
}

/// Canonical terminal meaning of one successful handler return.
///
/// A handler success means that the operation pipeline received a result value. It does not by
/// itself prove that the requested browser effect committed. The operation registry derives this
/// disposition from the canonical operation and its bounded result receipt before any surface
/// renderer sees the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuccessDisposition {
    /// Canonical terminal status.
    pub status: BrowserResultStatus,
    /// Proven physical-effect disposition.
    pub effect: OperationEffect,
    /// Corrective retry guidance, when the receipt supports one.
    pub retry: Option<RetryDisposition>,
}

impl SuccessDisposition {
    /// Construct one explicit canonical success disposition.
    pub const fn new(
        status: BrowserResultStatus,
        effect: OperationEffect,
        retry: Option<RetryDisposition>,
    ) -> Self {
        Self {
            status,
            effect,
            retry,
        }
    }
}

impl Scheduling {
    /// A normal surface operation.
    pub const SURFACE: Self = Self {
        scope: SchedulingScope::Surface,
        lease: LeaseMode::Dispatch,
    };
    /// A compound semantic operation retaining one surface.
    pub const RETAIN_SURFACE: Self = Self {
        scope: SchedulingScope::Surface,
        lease: LeaseMode::RetainSurface,
    };
    /// Workspace topology work.
    pub const WORKSPACE_TOPOLOGY: Self = Self {
        scope: SchedulingScope::WorkspaceTopology,
        lease: LeaseMode::Dispatch,
    };
    /// Browser-wide work.
    pub const BROWSER: Self = Self {
        scope: SchedulingScope::Browser,
        lease: LeaseMode::Dispatch,
    };
    /// Presentation traffic.
    pub const PRESENTATION: Self = Self {
        scope: SchedulingScope::Presentation,
        lease: LeaseMode::Dispatch,
    };
    /// Service-local work.
    pub const LOCAL: Self = Self {
        scope: SchedulingScope::Local,
        lease: LeaseMode::Dispatch,
    };
    /// Canonical composition.
    pub const COMPOSITION: Self = Self {
        scope: SchedulingScope::Composition,
        lease: LeaseMode::Composition,
    };
}

/// All execution semantics for one implemented canonical operation intent.
#[derive(Clone, Copy)]
pub struct OperationDescriptor {
    /// Closed semantic identity.
    pub key: OperationKey,
    /// Workspace relationship.
    pub workspace_use: WorkspaceUse,
    /// Baseline RAWX requirements.
    pub requires: &'static [Capability],
    /// Governance resource shape.
    pub resource: ResourceShape,
    /// Scheduler and lease declaration.
    pub scheduling: Scheduling,
    /// Service or browser dispatch implementation.
    pub handler: Handler,
    /// Result redaction hook.
    pub postprocess: Option<fn(&mut Value, bool)>,
    /// Page-output provenance mode.
    pub page_output: PageOutput,
    /// Browser-dependent post-dispatch behavior.
    pub post_dispatch: PostDispatch,
    /// Proven effect classification after an ordinary acknowledged success.
    pub success_effect: OperationEffect,
}

impl OperationDescriptor {
    /// Validate canonical arguments before workspace admission or browser traffic.
    pub fn validate(&self, arguments: &Value) -> Result<(), ToolError> {
        let schema = crate::tool::validation::ToolSchema {
            input_schema: canonical_schema(self.key),
            example_call: None,
        };
        crate::tool::validation::validate_arguments(&schema, arguments)?;
        if self.key.id == OperationId::BrowserFlow {
            validate_canonical_flow(arguments)?;
        }
        validate_semantic_shape(self.key, arguments)
    }

    /// Return whether a flow parent may inherit its concrete tab into this operation.
    ///
    /// Canonical schemas are closed. Composition must not add `tab` to an operation whose
    /// semantic arguments do not declare it.
    pub fn accepts_flow_parent_tab(&self) -> bool {
        canonical_schema(self.key)
            .pointer("/properties/tab")
            .is_some()
    }

    /// Apply argument-dependent capability refinements without changing the canonical key.
    pub fn requirements_for_call(&self, arguments: &Value) -> &'static [Capability] {
        if self.key == OperationKey::new(OperationId::BrowserRecord, IntentId::RecordExport)
            && (arguments.get("point").is_some_and(|value| !value.is_null())
                || arguments
                    .pointer("/target/ref")
                    .is_some_and(|value| !value.is_null()))
        {
            return &[Capability::Write];
        }
        if self.key.id != OperationId::BrowserAct {
            return self.requires;
        }
        let semantic_target = arguments
            .get("target")
            .and_then(Value::as_object)
            .is_some_and(|target| target.get("query").is_some() || target.get("name").is_some());
        let observes_postcondition = arguments.get("expect").is_some();
        if !semantic_target && !observes_postcondition {
            return self.requires;
        }
        match self.requires.first() {
            Some(Capability::Action) => &[Capability::Read, Capability::Action],
            Some(Capability::Write) => &[Capability::Read, Capability::Write],
            _ => &[Capability::Read],
        }
    }

    /// Resolve an argument-dependent governance resource shape.
    pub fn resource_for_call(&self, arguments: &Value) -> ResourceShape {
        if self.key.id != OperationId::BrowserRecord {
            return self.resource;
        }
        if self.key.intent == IntentId::RecordStart
            || (self.key.intent == IntentId::RecordExport
                && (arguments.get("point").is_some_and(|value| !value.is_null())
                    || arguments
                        .pointer("/target/ref")
                        .is_some_and(|value| !value.is_null())))
        {
            ResourceShape::TabScoped
        } else {
            ResourceShape::RecordingScoped
        }
    }

    /// Classify one successful handler return without overstating status, effect, or retry.
    ///
    /// Local semantic handlers retain their legacy result value during the R1 migration. This
    /// hook is the canonical authority that interprets the bounded receipt fields in that value.
    pub fn success_disposition_for(&self, result: &Value) -> SuccessDisposition {
        if self.key.id == OperationId::BrowserFlow {
            return flow_success_disposition(result, self.key.intent == IntentId::FlowPreflight);
        }

        if self.post_dispatch == PostDispatch::NavigateLanding && is_error_success(result) {
            return SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe),
            );
        }

        if self.key.id == OperationId::BrowserAct {
            if has_interaction_blocker(result, "expect_timeout")
                || has_interaction_blocker(result, "postcondition_paused")
                || has_interaction_blocker(result, "postcondition_interrupted")
            {
                return SuccessDisposition::new(
                    BrowserResultStatus::Partial,
                    OperationEffect::Committed,
                    None,
                );
            }
            if has_interaction_blocker(result, "stale_ref")
                || has_interaction_blocker(result, "target_missing")
                || has_interaction_blocker(result, "covered_target")
            {
                return SuccessDisposition::new(
                    BrowserResultStatus::Blocked,
                    OperationEffect::None,
                    Some(RetryDisposition::AfterStateChange),
                );
            }
            if has_interaction_blockers(result) || is_error_success(result) {
                return SuccessDisposition::new(
                    BrowserResultStatus::Blocked,
                    OperationEffect::None,
                    None,
                );
            }
        }

        if self.key.id == OperationId::BrowserFill
            && matches!(
                self.key.intent,
                IntentId::FillFields | IntentId::FillFieldsAndSubmit
            )
        {
            let committed = result
                .pointer("/structuredContent/filled")
                .and_then(Value::as_array)
                .is_some_and(|filled| !filled.is_empty())
                || result
                    .pointer("/structuredContent/submitted")
                    .and_then(Value::as_bool)
                    == Some(true);
            if is_error_success(result) {
                return SuccessDisposition::new(
                    if committed {
                        BrowserResultStatus::Partial
                    } else {
                        BrowserResultStatus::Blocked
                    },
                    if committed {
                        OperationEffect::Committed
                    } else {
                        OperationEffect::None
                    },
                    None,
                );
            }
            return SuccessDisposition::new(
                BrowserResultStatus::Ok,
                if committed {
                    OperationEffect::Committed
                } else {
                    OperationEffect::None
                },
                None,
            );
        }

        if self.key.id == OperationId::BrowserRecord {
            let changed = result
                .pointer("/structuredContent/changed")
                .and_then(Value::as_bool);
            match self.key.intent {
                IntentId::RecordStart => {
                    let start_acknowledged = result
                        .pointer("/structuredContent/start_acknowledged")
                        .and_then(Value::as_bool);
                    let start_committed = result
                        .pointer("/structuredContent/start_committed")
                        .and_then(Value::as_bool);
                    let (Some(changed), Some(start_acknowledged), Some(start_committed)) =
                        (changed, start_acknowledged, start_committed)
                    else {
                        return SuccessDisposition::new(
                            BrowserResultStatus::Unavailable,
                            OperationEffect::None,
                            None,
                        );
                    };
                    if changed != start_acknowledged || start_committed != start_acknowledged {
                        return SuccessDisposition::new(
                            BrowserResultStatus::Unavailable,
                            OperationEffect::None,
                            None,
                        );
                    }
                    let partial = is_error_success(result);
                    return SuccessDisposition::new(
                        if partial {
                            BrowserResultStatus::Partial
                        } else {
                            BrowserResultStatus::Ok
                        },
                        if changed {
                            OperationEffect::Committed
                        } else {
                            OperationEffect::None
                        },
                        partial.then_some(RetryDisposition::Unsafe),
                    );
                }
                IntentId::RecordStop => {
                    let stop_committed = result
                        .pointer("/structuredContent/stop_committed")
                        .and_then(Value::as_bool);
                    let finalization_effect = result
                        .pointer("/structuredContent/finalization_effect")
                        .cloned()
                        .and_then(|value| serde_json::from_value::<OperationEffect>(value).ok());
                    let (Some(changed), Some(stop_committed), Some(effect)) =
                        (changed, stop_committed, finalization_effect)
                    else {
                        return SuccessDisposition::new(
                            BrowserResultStatus::Unavailable,
                            OperationEffect::None,
                            None,
                        );
                    };
                    if effect == OperationEffect::Unknown
                        || changed != (effect != OperationEffect::None)
                        || (stop_committed && effect == OperationEffect::None)
                    {
                        return SuccessDisposition::new(
                            BrowserResultStatus::Unavailable,
                            OperationEffect::None,
                            None,
                        );
                    }
                    if effect == OperationEffect::Dispatched {
                        return SuccessDisposition::new(
                            BrowserResultStatus::OutcomeUnknown,
                            OperationEffect::Unknown,
                            Some(RetryDisposition::Unsafe),
                        );
                    }
                    return SuccessDisposition::new(
                        if is_error_success(result) {
                            if changed {
                                BrowserResultStatus::Partial
                            } else {
                                BrowserResultStatus::Blocked
                            }
                        } else {
                            BrowserResultStatus::Ok
                        },
                        effect,
                        is_error_success(result).then_some(RetryDisposition::Unsafe),
                    );
                }
                IntentId::RecordClear => {
                    let clear_committed = result
                        .pointer("/structuredContent/clear_committed")
                        .and_then(Value::as_bool);
                    let (Some(changed), Some(clear_committed)) = (changed, clear_committed) else {
                        return SuccessDisposition::new(
                            BrowserResultStatus::Unavailable,
                            OperationEffect::None,
                            None,
                        );
                    };
                    if clear_committed != changed {
                        return SuccessDisposition::new(
                            BrowserResultStatus::Unavailable,
                            OperationEffect::None,
                            None,
                        );
                    }
                    return SuccessDisposition::new(
                        BrowserResultStatus::Ok,
                        if changed {
                            OperationEffect::Committed
                        } else {
                            OperationEffect::None
                        },
                        None,
                    );
                }
                IntentId::RecordExport => {
                    let export_completed = result
                        .pointer("/structuredContent/export_completed")
                        .and_then(Value::as_bool);
                    let finalization_effect = result
                        .pointer("/structuredContent/finalization_effect")
                        .cloned()
                        .and_then(|value| serde_json::from_value::<OperationEffect>(value).ok());
                    let (Some(changed), Some(export_completed), Some(finalization_effect)) =
                        (changed, export_completed, finalization_effect)
                    else {
                        return SuccessDisposition::new(
                            BrowserResultStatus::Unavailable,
                            OperationEffect::None,
                            None,
                        );
                    };
                    if finalization_effect == OperationEffect::Unknown
                        || changed
                            != (export_completed || finalization_effect != OperationEffect::None)
                    {
                        return SuccessDisposition::new(
                            BrowserResultStatus::Unavailable,
                            OperationEffect::None,
                            None,
                        );
                    }
                    if !export_completed && finalization_effect == OperationEffect::Dispatched {
                        return SuccessDisposition::new(
                            BrowserResultStatus::OutcomeUnknown,
                            OperationEffect::Unknown,
                            Some(RetryDisposition::Unsafe),
                        );
                    }
                    let partial = is_error_success(result) || (changed && !export_completed);
                    let retry = (result
                        .pointer("/structuredContent/retry_safe")
                        .and_then(Value::as_bool)
                        == Some(false))
                    .then_some(RetryDisposition::Unsafe)
                    .or_else(|| partial.then_some(RetryDisposition::Unsafe));
                    return SuccessDisposition::new(
                        if partial {
                            BrowserResultStatus::Partial
                        } else {
                            BrowserResultStatus::Ok
                        },
                        if export_completed {
                            OperationEffect::Committed
                        } else {
                            finalization_effect
                        },
                        retry,
                    );
                }
                _ => {}
            }
        }

        if is_error_success(result) {
            SuccessDisposition::new(BrowserResultStatus::Partial, OperationEffect::None, None)
        } else {
            SuccessDisposition::new(BrowserResultStatus::Ok, self.success_effect, None)
        }
    }
}

fn flow_success_disposition(result: &Value, preflight: bool) -> SuccessDisposition {
    let Some(data) = result.get("structuredContent") else {
        return SuccessDisposition::new(
            BrowserResultStatus::Unavailable,
            OperationEffect::None,
            None,
        );
    };
    let Ok(flow) = serde_json::from_value::<FlowResultData>(data.clone()) else {
        return SuccessDisposition::new(
            BrowserResultStatus::Unavailable,
            OperationEffect::None,
            None,
        );
    };

    if preflight
        && flow
            .steps
            .iter()
            .any(|step| step.result.effect != OperationEffect::None)
    {
        return SuccessDisposition::new(
            BrowserResultStatus::Unavailable,
            OperationEffect::None,
            None,
        );
    }

    let uncertain = flow.steps.iter().any(|step| {
        matches!(
            step.result.effect,
            OperationEffect::Dispatched | OperationEffect::Unknown
        ) || step.status == FlowStepStatus::OutcomeUnknown
            || step.result.status == BrowserResultStatus::OutcomeUnknown
    });
    if uncertain {
        if preflight {
            return SuccessDisposition::new(
                BrowserResultStatus::Unavailable,
                OperationEffect::None,
                None,
            );
        }
        return SuccessDisposition::new(
            BrowserResultStatus::OutcomeUnknown,
            OperationEffect::Unknown,
            Some(RetryDisposition::Unsafe),
        );
    }

    let committed = !preflight
        && flow
            .steps
            .iter()
            .any(|step| step.result.effect == OperationEffect::Committed);
    let effect = if committed {
        OperationEffect::Committed
    } else {
        OperationEffect::None
    };
    let terminal = flow
        .steps
        .iter()
        .filter_map(|step| aggregate_terminal_status(step.status, step.result.status, preflight))
        .max_by_key(|status| aggregate_status_priority(*status));

    let typed_terminal = match flow.termination.reason {
        FlowTerminationReason::Completed => None,
        FlowTerminationReason::Failed => flow
            .steps
            .is_empty()
            .then_some(BrowserResultStatus::Unavailable),
        FlowTerminationReason::Denied => Some(BrowserResultStatus::Blocked),
        FlowTerminationReason::Held => Some(BrowserResultStatus::Held),
        FlowTerminationReason::AttentionRequired => Some(BrowserResultStatus::AttentionRequired),
        FlowTerminationReason::Cancelled => Some(BrowserResultStatus::Cancelled),
        FlowTerminationReason::BudgetExhausted => Some(BrowserResultStatus::Partial),
    };
    let terminal = typed_terminal.or(terminal);

    if matches!(
        terminal,
        Some(BrowserResultStatus::Cancelled | BrowserResultStatus::Blocked)
    ) {
        return SuccessDisposition::new(
            terminal.expect("matched a controlling flow terminal"),
            effect,
            (effect != OperationEffect::None).then_some(RetryDisposition::Unsafe),
        );
    }
    if matches!(
        terminal,
        Some(BrowserResultStatus::Held | BrowserResultStatus::AttentionRequired)
    ) {
        return if effect == OperationEffect::None {
            SuccessDisposition::new(
                terminal.expect("matched a pre-dispatch flow terminal"),
                effect,
                None,
            )
        } else {
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                effect,
                Some(RetryDisposition::Unsafe),
            )
        };
    }

    match (committed, terminal) {
        (false, None) => SuccessDisposition::new(BrowserResultStatus::Ok, effect, None),
        (true, None) => SuccessDisposition::new(BrowserResultStatus::Ok, effect, None),
        (true, Some(_)) => SuccessDisposition::new(
            BrowserResultStatus::Partial,
            effect,
            Some(RetryDisposition::Unsafe),
        ),
        (false, Some(status)) => SuccessDisposition::new(status, effect, None),
    }
}

const fn aggregate_status_priority(status: BrowserResultStatus) -> u8 {
    match status {
        BrowserResultStatus::OutcomeUnknown => 100,
        BrowserResultStatus::Cancelled => 90,
        BrowserResultStatus::AttentionRequired => 80,
        BrowserResultStatus::Held => 70,
        BrowserResultStatus::Unavailable => 60,
        BrowserResultStatus::NotDispatched => 50,
        BrowserResultStatus::Blocked => 40,
        BrowserResultStatus::Partial => 30,
        BrowserResultStatus::NotMet => 20,
        BrowserResultStatus::Ok => 0,
    }
}

fn aggregate_terminal_status(
    flow_status: FlowStepStatus,
    result_status: BrowserResultStatus,
    preflight: bool,
) -> Option<BrowserResultStatus> {
    match flow_status {
        FlowStepStatus::Ok => (result_status != BrowserResultStatus::Ok).then_some(result_status),
        FlowStepStatus::NotRun if result_status == BrowserResultStatus::Cancelled => {
            Some(BrowserResultStatus::Cancelled)
        }
        FlowStepStatus::NotRun => None,
        FlowStepStatus::WouldAllow if preflight => None,
        FlowStepStatus::WouldDeny if preflight => Some(BrowserResultStatus::Blocked),
        FlowStepStatus::WouldAllow | FlowStepStatus::WouldDeny => {
            Some(BrowserResultStatus::Unavailable)
        }
        FlowStepStatus::Partial => Some(BrowserResultStatus::Partial),
        FlowStepStatus::NotMet => Some(BrowserResultStatus::NotMet),
        FlowStepStatus::Blocked | FlowStepStatus::Denied => Some(BrowserResultStatus::Blocked),
        FlowStepStatus::Held => Some(BrowserResultStatus::Held),
        FlowStepStatus::AttentionRequired => Some(BrowserResultStatus::AttentionRequired),
        FlowStepStatus::Cancelled => Some(BrowserResultStatus::Cancelled),
        FlowStepStatus::NotDispatched => Some(BrowserResultStatus::NotDispatched),
        FlowStepStatus::OutcomeUnknown => Some(BrowserResultStatus::OutcomeUnknown),
        FlowStepStatus::Unavailable => Some(BrowserResultStatus::Unavailable),
    }
}

fn is_error_success(result: &Value) -> bool {
    result.get("isError").and_then(Value::as_bool) == Some(true)
}

fn interaction_blockers(result: &Value) -> Option<&[Value]> {
    result
        .pointer("/structuredContent/interactionReceipt/blockers")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
}

fn has_interaction_blockers(result: &Value) -> bool {
    interaction_blockers(result).is_some_and(|blockers| !blockers.is_empty())
}

fn has_interaction_blocker(result: &Value, expected: &str) -> bool {
    interaction_blockers(result).is_some_and(|blockers| {
        blockers
            .iter()
            .any(|blocker| blocker.get("kind").and_then(Value::as_str) == Some(expected))
    })
}

#[derive(Clone, Copy)]
struct Seed {
    key: OperationKey,
    workspace_use: WorkspaceUse,
    requires: &'static [Capability],
    resource: ResourceShape,
    scheduling: Scheduling,
    handler: Handler,
    postprocess: Option<fn(&mut Value, bool)>,
    page_output: PageOutput,
    post_dispatch: PostDispatch,
    success_effect: OperationEffect,
}

macro_rules! seed {
    ($id:ident, $intent:ident, $workspace:ident, $requires:expr,
     $resource:ident, $scheduling:expr, $handler:expr, $postprocess:expr,
     $page:ident, $post_dispatch:ident, $effect:ident $(,)?) => {
        Seed {
            key: OperationKey::new(OperationId::$id, IntentId::$intent),
            workspace_use: WorkspaceUse::$workspace,
            requires: $requires,
            resource: ResourceShape::$resource,
            scheduling: $scheduling,
            handler: $handler,
            postprocess: $postprocess,
            page_output: PageOutput::$page,
            post_dispatch: PostDispatch::$post_dispatch,
            success_effect: OperationEffect::$effect,
        }
    };
}

const REDACT: Option<fn(&mut Value, bool)> = Some(crate::browser::redact::apply_to_result);

const SEEDS: &[Seed] = &[
    seed!(
        BrowserTabs,
        TabsList,
        Creates,
        &[Capability::Read],
        DomainLess,
        Scheduling::WORKSPACE_TOPOLOGY,
        Handler::Mechanism,
        None,
        Text,
        None,
        None,
    ),
    seed!(
        BrowserTabs,
        TabsNew,
        Creates,
        &[],
        DomainLess,
        Scheduling::WORKSPACE_TOPOLOGY,
        Handler::Mechanism,
        None,
        Text,
        None,
        Committed,
    ),
    seed!(
        BrowserTabs,
        TabsFocus,
        Uses,
        &[],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        None,
        None,
        Committed,
    ),
    seed!(
        BrowserTabs,
        TabsClose,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        None,
        None,
        Committed,
    ),
    seed!(
        BrowserNavigate,
        NavigateUrl,
        Uses,
        &[Capability::Read],
        TargetArg,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        NavigateLanding,
        Committed,
    ),
    seed!(
        BrowserNavigate,
        NavigateBack,
        Uses,
        &[Capability::Read],
        TargetArg,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        NavigateLanding,
        Committed,
    ),
    seed!(
        BrowserNavigate,
        NavigateForward,
        Uses,
        &[Capability::Read],
        TargetArg,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        NavigateLanding,
        Committed,
    ),
    seed!(
        BrowserNavigate,
        NavigateReload,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        None,
        None,
        Committed,
    ),
    seed!(
        BrowserSnapshot,
        SnapshotCapture,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        None,
    ),
    seed!(
        BrowserRead,
        ReadText,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        None,
        None,
    ),
    seed!(
        BrowserFind,
        FindQuery,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        None,
    ),
    seed!(
        BrowserScreenshot,
        ScreenshotViewport,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        None,
    ),
    seed!(
        BrowserScreenshot,
        ScreenshotRegion,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        None,
    ),
    seed!(
        BrowserAct,
        ActClick,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserAct,
        ActRightClick,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserAct,
        ActDoubleClick,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserAct,
        ActTripleClick,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserAct,
        ActHover,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserAct,
        ActScrollIntoView,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserAct,
        ActSetValue,
        Uses,
        &[Capability::Write],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserFill,
        FillField,
        Uses,
        &[Capability::Write],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserFill,
        FillFields,
        Uses,
        &[Capability::Read, Capability::Write],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::form_fill::form_fill_handler),
        None,
        Structured,
        None,
        Committed,
    ),
    seed!(
        BrowserFill,
        FillFieldsAndSubmit,
        Uses,
        &[Capability::Read, Capability::Write, Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::form_fill::form_fill_handler),
        None,
        Structured,
        None,
        Committed,
    ),
    seed!(
        BrowserWait,
        WaitDelay,
        Uses,
        &[],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        None,
    ),
    seed!(
        BrowserWait,
        WaitUntil,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        None,
        None,
    ),
    seed!(
        BrowserFlow,
        FlowExecute,
        Uses,
        &[],
        DomainLess,
        Scheduling::COMPOSITION,
        Handler::Local(crate::tool::flow::flow_handler),
        None,
        None,
        None,
        None,
    ),
    seed!(
        BrowserFlow,
        FlowPreflight,
        Uses,
        &[],
        DomainLess,
        Scheduling::COMPOSITION,
        Handler::Local(crate::tool::flow::flow_handler),
        None,
        None,
        None,
        None,
    ),
    seed!(
        BrowserDialog,
        DialogStatus,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        None,
    ),
    seed!(
        BrowserDialog,
        DialogAccept,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        Committed,
    ),
    seed!(
        BrowserDialog,
        DialogDismiss,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        Committed,
    ),
    seed!(
        BrowserDialog,
        DialogRespond,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputPointerClick,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputPointerRightClick,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputPointerDoubleClick,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputPointerTripleClick,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputPointerHover,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputPointerDrag,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputTypeText,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputPressKey,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputWheel,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserInput,
        InputScrollToOffset,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserViewport,
        ViewportResizeWindow,
        Uses,
        &[],
        TabScoped,
        Scheduling::BROWSER,
        Handler::Mechanism,
        None,
        None,
        None,
        Committed,
    ),
    seed!(
        BrowserUpload,
        UploadClientFiles,
        Uses,
        &[Capability::Write],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserUpload,
        UploadCapturedArtifact,
        Uses,
        &[Capability::Write],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::upload_image::upload_image_handler),
        REDACT,
        Receipt,
        None,
        Committed,
    ),
    seed!(
        BrowserConsole,
        ConsoleRead,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        None,
        None,
    ),
    seed!(
        BrowserConsole,
        ConsoleReadAndClear,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        None,
        Committed,
    ),
    seed!(
        BrowserNetwork,
        NetworkRead,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        None,
        None,
    ),
    seed!(
        BrowserNetwork,
        NetworkReadAndClear,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        None,
        Committed,
    ),
    seed!(
        BrowserEvaluate,
        EvaluateJavascript,
        Uses,
        &[Capability::Execute],
        TabScoped,
        Scheduling::SURFACE,
        Handler::Mechanism,
        None,
        Text,
        None,
        Committed,
    ),
    seed!(
        BrowserRecord,
        RecordStart,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        Committed,
    ),
    seed!(
        BrowserRecord,
        RecordStop,
        Uses,
        &[],
        RecordingScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        Committed,
    ),
    seed!(
        BrowserRecord,
        RecordStatus,
        Uses,
        &[],
        RecordingScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        None,
    ),
    seed!(
        BrowserRecord,
        RecordClear,
        Uses,
        &[],
        RecordingScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        Committed,
    ),
    seed!(
        BrowserRecord,
        RecordExport,
        Uses,
        &[Capability::Read],
        RecordingScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        Committed,
    ),
    seed!(
        BrowserPresent,
        PresentNarrate,
        Uses,
        &[],
        DomainLess,
        Scheduling::PRESENTATION,
        Handler::Mechanism,
        None,
        None,
        None,
        Committed,
    ),
    seed!(
        WorkflowPlan,
        PlanUpdate,
        Independent,
        &[],
        DomainLess,
        Scheduling::LOCAL,
        Handler::Local(crate::tool::update_plan::update_plan_handler),
        None,
        None,
        None,
        None,
    ),
    seed!(
        BrowserContext,
        ContextDescribe,
        Independent,
        &[],
        DomainLess,
        Scheduling::LOCAL,
        Handler::Local(explain_handler),
        None,
        None,
        None,
        None,
    ),
];

static REGISTRY: OnceLock<Vec<OperationDescriptor>> = OnceLock::new();

/// Return every implemented operation descriptor in stable availability order.
pub fn descriptors() -> &'static [OperationDescriptor] {
    REGISTRY.get_or_init(|| {
        SEEDS
            .iter()
            .map(|seed| OperationDescriptor {
                key: seed.key,
                workspace_use: seed.workspace_use,
                requires: seed.requires,
                resource: seed.resource,
                scheduling: seed.scheduling,
                handler: seed.handler,
                postprocess: seed.postprocess,
                page_output: seed.page_output,
                post_dispatch: seed.post_dispatch,
                success_effect: seed.success_effect,
            })
            .collect()
    })
}

/// Look up one exact canonical operation key. Invalid family/intent pairs fail closed.
pub fn descriptor(key: OperationKey) -> Option<&'static OperationDescriptor> {
    descriptors()
        .iter()
        .find(|descriptor| descriptor.key == key)
}

/// Whether a descriptor is reachable under one grant set.
pub fn reachable(descriptor: &OperationDescriptor, grants: Option<&[Grant]>) -> bool {
    let Some(grants) = grants else {
        return true;
    };
    descriptor.requires.is_empty()
        || grants
            .iter()
            .any(|grant| capability_subset(descriptor.requires, &grant.allowed))
}

/// Project ordered canonical operation availability under service and request restrictions.
pub fn project_availability(
    governance: &crate::governance::dispatch::Governance,
    restriction: Option<&crate::governance::overlay::SessionOverlay>,
    generation: u64,
) -> CatalogProjection {
    let operations = descriptors()
        .iter()
        .filter(|descriptor| reachable(descriptor, governance.grants()))
        .filter(|descriptor| {
            restriction.is_none_or(|restriction| reachable(descriptor, restriction.grants()))
        })
        .map(|descriptor| OperationAvailability {
            id: descriptor.key.id,
            intent: descriptor.key.intent,
            workspace_use: descriptor.workspace_use,
        })
        .collect();
    CatalogProjection {
        generation,
        operations,
        restricted: restriction.is_some(),
    }
}

fn canonical_schema(key: OperationKey) -> Value {
    let target = json!({
        "type": "object",
        "properties": {
            "ref": { "type": "string" },
            "query": { "type": "string" },
            "name": { "type": "string" },
            "role": { "type": "string" }
        },
        "additionalProperties": false
    });
    let ref_target = json!({
        "type": "object",
        "properties": {
            "ref": { "type": "string", "minLength": 1 }
        },
        "required": ["ref"],
        "additionalProperties": false
    });
    let query_target = json!({
        "type": "object",
        "properties": {
            "query": { "type": "string", "minLength": 1 }
        },
        "required": ["query"],
        "additionalProperties": false
    });
    let point = json!({
        "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2
    });
    let tab_only = || {
        json!({
            "type": "object", "properties": { "tab": { "type": "number" } },
            "required": ["tab"], "additionalProperties": false
        })
    };
    match (key.id, key.intent) {
        (OperationId::BrowserTabs, IntentId::TabsList) => json!({
            "type": "object", "properties": { "create_if_empty": { "type": "boolean" } },
            "additionalProperties": false
        }),
        (OperationId::BrowserTabs, IntentId::TabsNew)
        | (OperationId::BrowserContext, IntentId::ContextDescribe) => json!({
            "type": "object", "properties": {}, "additionalProperties": false
        }),
        (OperationId::BrowserTabs, IntentId::TabsFocus | IntentId::TabsClose)
        | (OperationId::BrowserNavigate, IntentId::NavigateReload)
        | (
            OperationId::BrowserDialog,
            IntentId::DialogStatus | IntentId::DialogAccept | IntentId::DialogDismiss,
        )
        | (
            OperationId::BrowserRecord,
            IntentId::RecordStart
            | IntentId::RecordStop
            | IntentId::RecordStatus
            | IntentId::RecordClear,
        ) => tab_only(),
        (OperationId::BrowserNavigate, IntentId::NavigateUrl) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "url": { "type": "string" },
                "force": { "type": "boolean" }
            }, "required": ["tab", "url"], "additionalProperties": false
        }),
        (OperationId::BrowserNavigate, IntentId::NavigateBack | IntentId::NavigateForward) => {
            json!({
                "type": "object", "properties": {
                    "tab": { "type": "number" }, "force": { "type": "boolean" }
                }, "required": ["tab"], "additionalProperties": false
            })
        }
        (OperationId::BrowserSnapshot, IntentId::SnapshotCapture) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "filter": { "type": "string", "enum": ["interactive", "all"] },
                "depth": { "type": "number" }, "scope_ref": { "type": "string" },
                "max_chars": { "type": "number" }, "diff": { "type": "boolean" }
            }, "required": ["tab"], "additionalProperties": false
        }),
        (OperationId::BrowserRead, IntentId::ReadText) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "max_chars": { "type": "number" }
            }, "required": ["tab"], "additionalProperties": false
        }),
        (OperationId::BrowserFind, IntentId::FindQuery) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "query": { "type": "string" }
            }, "required": ["tab", "query"], "additionalProperties": false
        }),
        (OperationId::BrowserScreenshot, IntentId::ScreenshotViewport) => tab_only(),
        (OperationId::BrowserScreenshot, IntentId::ScreenshotRegion) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" },
                "region": { "type": "array", "items": { "type": "number" }, "minItems": 4, "maxItems": 4 }
            }, "required": ["tab", "region"], "additionalProperties": false
        }),
        (OperationId::BrowserAct, _) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "target": target,
                "value": {}, "expect": { "type": "object" }, "modifiers": { "type": "string" }
            }, "required": ["tab", "target"], "additionalProperties": false
        }),
        (OperationId::BrowserFill, IntentId::FillField) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "target": ref_target, "value": {}
            }, "required": ["tab", "target", "value"], "additionalProperties": false
        }),
        (OperationId::BrowserFill, IntentId::FillFields | IntentId::FillFieldsAndSubmit) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" },
                "fields": {
                    "type": "array", "minItems": 1,
                    "items": {
                        "type": "object", "properties": {
                            "target": query_target, "value": {}
                        }, "required": ["target", "value"], "additionalProperties": false
                    }
                }
            }, "required": ["tab", "fields"], "additionalProperties": false
        }),
        (OperationId::BrowserWait, IntentId::WaitDelay) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "seconds": { "type": "number", "minimum": 0, "maximum": 10 }
            }, "required": ["tab", "seconds"], "additionalProperties": false
        }),
        (OperationId::BrowserWait, IntentId::WaitUntil) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "selector": { "type": "string" }, "text": { "type": "string" },
                "state": { "type": "string", "enum": ["visible", "present", "gone", "settled"] },
                "timeout_ms": { "type": "number" }, "min_ms": { "type": "number" }, "settle": { "type": "boolean" }
            }, "required": ["tab"], "additionalProperties": false
        }),
        (OperationId::BrowserFlow, _) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "steps": { "type": "array", "minItems": 1, "maxItems": 20 },
                "on_error": { "type": "string", "enum": ["stop", "continue"] },
                "budget_ms": { "type": "number" }
            }, "required": ["steps"], "additionalProperties": false
        }),
        (OperationId::BrowserDialog, IntentId::DialogRespond) => json!({
            "type": "object", "properties": { "tab": { "type": "number" }, "text": { "type": "string" } },
            "required": ["tab", "text"], "additionalProperties": false
        }),
        (
            OperationId::BrowserInput,
            IntentId::InputPointerClick
            | IntentId::InputPointerRightClick
            | IntentId::InputPointerDoubleClick
            | IntentId::InputPointerTripleClick
            | IntentId::InputPointerHover,
        ) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "point": point, "modifiers": { "type": "string" }
            }, "required": ["tab", "point"], "additionalProperties": false
        }),
        (OperationId::BrowserInput, IntentId::InputPointerDrag) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "from": point, "to": point,
                "modifiers": { "type": "string" }
            }, "required": ["tab", "from", "to"], "additionalProperties": false
        }),
        (OperationId::BrowserInput, IntentId::InputTypeText) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "text": { "type": "string" }
            }, "required": ["tab", "text"], "additionalProperties": false
        }),
        (OperationId::BrowserInput, IntentId::InputPressKey) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "key": { "type": "string" },
                "repeat": { "type": "number" }
            }, "required": ["tab", "key"], "additionalProperties": false
        }),
        (OperationId::BrowserInput, IntentId::InputWheel) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "point": point, "target": ref_target,
                "direction": { "type": "string", "enum": ["up", "down", "left", "right"] },
                "amount": { "type": "number" }, "modifiers": { "type": "string" }
            }, "required": ["tab", "direction"], "additionalProperties": false
        }),
        (OperationId::BrowserInput, IntentId::InputScrollToOffset) => json!({
            "type": "object", "properties": { "tab": { "type": "number" }, "point": point },
            "required": ["tab", "point"], "additionalProperties": false
        }),
        (OperationId::BrowserViewport, IntentId::ViewportResizeWindow) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "width": { "type": "number" }, "height": { "type": "number" }
            }, "required": ["tab", "width", "height"], "additionalProperties": false
        }),
        (OperationId::BrowserUpload, IntentId::UploadClientFiles) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "target": ref_target,
                "files": { "type": "array" }, "paths": { "type": "array" }
            }, "required": ["tab", "target"], "additionalProperties": false
        }),
        (OperationId::BrowserUpload, IntentId::UploadCapturedArtifact) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "artifact": { "type": "string" },
                "target": ref_target, "point": point, "filename": { "type": "string" }
            }, "required": ["tab", "artifact"], "additionalProperties": false
        }),
        (OperationId::BrowserConsole, _) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "pattern": { "type": "string" },
                "limit": { "type": "number" }, "only_errors": { "type": "boolean" }
            }, "required": ["tab"], "additionalProperties": false
        }),
        (OperationId::BrowserNetwork, _) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "url_pattern": { "type": "string" }, "limit": { "type": "number" }
            }, "required": ["tab"], "additionalProperties": false
        }),
        (OperationId::BrowserEvaluate, IntentId::EvaluateJavascript) => json!({
            "type": "object", "properties": { "tab": { "type": "number" }, "script": { "type": "string" } },
            "required": ["tab", "script"], "additionalProperties": false
        }),
        (OperationId::BrowserRecord, IntentId::RecordExport) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "target": ref_target, "point": point,
                "download": { "type": "boolean" }, "filename": { "type": "string" }, "options": { "type": "object" }
            }, "required": ["tab"], "additionalProperties": false
        }),
        (OperationId::BrowserPresent, IntentId::PresentNarrate) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "text": { "type": "string" },
                "position": { "type": "string", "enum": ["auto", "top", "bottom"] },
                "duration_ms": { "type": "number" }
            }, "required": ["tab", "text"], "additionalProperties": false
        }),
        (OperationId::WorkflowPlan, IntentId::PlanUpdate) => json!({
            "type": "object", "properties": {
                "domains": { "type": "array" }, "approach": { "type": "array" }
            }, "required": ["domains", "approach"], "additionalProperties": false
        }),
        _ => json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    }
}

fn validate_semantic_shape(key: OperationKey, arguments: &Value) -> Result<(), ToolError> {
    let exactly_one_target = |target: &Value| {
        let Some(target) = target.as_object() else {
            return false;
        };
        ["ref", "query", "name"]
            .iter()
            .filter(|field| {
                target
                    .get(**field)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty())
            })
            .count()
            == 1
            && (!target.contains_key("role") || target.contains_key("name"))
    };

    if matches!(key.id, OperationId::BrowserAct | OperationId::BrowserFill)
        && key.intent != IntentId::FillFields
        && key.intent != IntentId::FillFieldsAndSubmit
        && !exactly_one_target(&arguments["target"])
    {
        return Err(ToolError::invalid_request(
            "canonical target must contain exactly one non-empty ref, query, or name",
        ));
    }
    if key.id == OperationId::BrowserFill
        && key.intent == IntentId::FillField
        && arguments
            .pointer("/target/ref")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(ToolError::invalid_request("fill.field requires target.ref"));
    }
    if key.id == OperationId::BrowserFill
        && matches!(
            key.intent,
            IntentId::FillFields | IntentId::FillFieldsAndSubmit
        )
    {
        let valid = arguments["fields"].as_array().is_some_and(|fields| {
            fields.iter().all(|field| {
                field
                    .pointer("/target/query")
                    .and_then(Value::as_str)
                    .is_some_and(|query| !query.is_empty())
                    && field.get("value").is_some()
            })
        });
        if !valid {
            return Err(ToolError::invalid_request(
                "fill fields require a non-empty target.query and value",
            ));
        }
    }
    if key.id == OperationId::BrowserAct {
        let needs_value = key.intent == IntentId::ActSetValue;
        if needs_value != arguments.get("value").is_some() {
            return Err(ToolError::invalid_request(if needs_value {
                "act.set_value requires value"
            } else {
                "value is valid only for act.set_value"
            }));
        }
    }
    if matches!(
        key,
        OperationKey {
            id: OperationId::BrowserInput,
            intent: IntentId::InputWheel,
        } | OperationKey {
            id: OperationId::BrowserUpload,
            intent: IntentId::UploadCapturedArtifact,
        } | OperationKey {
            id: OperationId::BrowserRecord,
            intent: IntentId::RecordExport,
        }
    ) && arguments.get("target").is_some()
        && arguments
            .pointer("/target/ref")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(ToolError::invalid_request(
            "physical target requires a non-empty target.ref",
        ));
    }
    if key.id == OperationId::BrowserUpload {
        let target_ref = arguments.pointer("/target/ref").and_then(Value::as_str);
        if key.intent == IntentId::UploadClientFiles
            && target_ref.is_none_or(|reference| reference.is_empty())
        {
            return Err(ToolError::invalid_request(
                "upload.client_files requires target.ref",
            ));
        }
        if key.intent == IntentId::UploadCapturedArtifact {
            let ref_target = target_ref.is_some();
            let point = arguments.get("point").is_some();
            if ref_target == point {
                return Err(ToolError::invalid_request(
                    "upload.captured_artifact requires exactly one target.ref or point",
                ));
            }
        }
    }
    Ok(())
}

fn is_deferred_flow_reference(value: &Value) -> bool {
    let Some(body) = value.as_str().and_then(|value| value.strip_prefix('$')) else {
        return false;
    };
    if body.starts_with('$') {
        return false;
    }
    let rest = if let Some(rest) = body.strip_prefix("prev") {
        rest
    } else {
        let digit_count = body
            .bytes()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if digit_count == 0 || body.as_bytes()[0] == b'0' {
            return false;
        }
        &body[digit_count..]
    };
    rest.is_empty()
        || rest
            .strip_prefix('.')
            .is_some_and(|path| !path.is_empty() && path.split('.').all(|part| !part.is_empty()))
}

fn deferred_flow_validation_view(schema: &Value, instance: &Value) -> Value {
    if is_deferred_flow_reference(instance) {
        return deferred_flow_placeholder(schema);
    }
    match instance {
        Value::Object(object) => {
            let properties = schema.get("properties").and_then(Value::as_object);
            let additional = schema.get("additionalProperties");
            Value::Object(
                object
                    .iter()
                    .map(|(field, value)| {
                        let field_schema = properties
                            .and_then(|properties| properties.get(field))
                            .or_else(|| additional.filter(|value| value.is_object()))
                            .unwrap_or(&Value::Null);
                        (
                            field.clone(),
                            deferred_flow_validation_view(field_schema, value),
                        )
                    })
                    .collect(),
            )
        }
        Value::Array(items) => {
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            Value::Array(
                items
                    .iter()
                    .map(|item| deferred_flow_validation_view(item_schema, item))
                    .collect(),
            )
        }
        _ => instance.clone(),
    }
}

fn deferred_flow_placeholder(schema: &Value) -> Value {
    if let Some(value) = schema.get("const") {
        return value.clone();
    }
    if let Some(value) = schema
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
    {
        return value.clone();
    }
    let type_name = match schema.get("type") {
        Some(Value::String(name)) => Some(name.as_str()),
        Some(Value::Array(names)) => names.first().and_then(Value::as_str),
        _ => None,
    };
    match type_name {
        Some("string") => Value::String("deferred".into()),
        Some("number") | Some("integer") => {
            schema.get("minimum").cloned().unwrap_or_else(|| json!(0))
        }
        Some("boolean") => Value::Bool(false),
        Some("array") => {
            let count = schema.get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize;
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            Value::Array(
                (0..count)
                    .map(|_| deferred_flow_placeholder(item_schema))
                    .collect(),
            )
        }
        Some("object") => {
            let properties = schema.get("properties").and_then(Value::as_object);
            let mut object = serde_json::Map::new();
            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                for field in required.iter().filter_map(Value::as_str) {
                    let field_schema = properties
                        .and_then(|properties| properties.get(field))
                        .unwrap_or(&Value::Null);
                    object.insert(field.into(), deferred_flow_placeholder(field_schema));
                }
            }
            Value::Object(object)
        }
        Some("null") | None => Value::Null,
        Some(_) => Value::Null,
    }
}

fn flow_step_validation_arguments(key: OperationKey, arguments: &Value) -> Value {
    let schema = canonical_schema(key);
    let mut validation = deferred_flow_validation_view(&schema, arguments);
    let Some(source) = arguments.as_object() else {
        return validation;
    };
    let Some(projected) = validation.as_object_mut() else {
        return validation;
    };

    if source.get("target").is_some_and(is_deferred_flow_reference)
        && matches!(
            key.id,
            OperationId::BrowserAct | OperationId::BrowserFill | OperationId::BrowserUpload
        )
    {
        projected.insert("target".into(), json!({"ref": "deferred"}));
    }

    if matches!(
        key,
        OperationKey {
            id: OperationId::BrowserFill,
            intent: IntentId::FillFields | IntentId::FillFieldsAndSubmit,
        }
    ) {
        if source.get("fields").is_some_and(is_deferred_flow_reference) {
            projected.insert(
                "fields".into(),
                json!([{"target":{"query":"deferred"},"value":null}]),
            );
        } else if let (Some(source_fields), Some(projected_fields)) = (
            source.get("fields").and_then(Value::as_array),
            projected.get_mut("fields").and_then(Value::as_array_mut),
        ) {
            for (source_field, projected_field) in
                source_fields.iter().zip(projected_fields.iter_mut())
            {
                if source_field
                    .get("target")
                    .is_some_and(is_deferred_flow_reference)
                {
                    if let Some(projected_field) = projected_field.as_object_mut() {
                        projected_field.insert("target".into(), json!({"query":"deferred"}));
                    }
                }
            }
        }
    }
    validation
}

fn validate_canonical_flow(arguments: &Value) -> Result<(), ToolError> {
    let steps = arguments
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| ToolError::invalid_request("canonical flow requires a steps array"))?;
    for (index, step) in steps.iter().enumerate() {
        let operation: ghostlight_transport::operation::BrowserOperation =
            serde_json::from_value(step.clone()).map_err(|error| {
                ToolError::invalid_request(format!(
                    "canonical flow step {} is invalid: {error}",
                    index + 1
                ))
            })?;
        if operation.id == OperationId::BrowserFlow {
            return Err(ToolError::invalid_request(
                "canonical flows cannot contain another flow",
            ));
        }
        let Some(step_descriptor) = descriptor(operation.key()) else {
            return Err(ToolError::invalid_request(format!(
                "canonical flow step {} uses an unavailable operation pair",
                index + 1
            )));
        };
        let validation_arguments =
            flow_step_validation_arguments(operation.key(), &operation.arguments);
        step_descriptor.validate(&validation_arguments)?;
    }
    Ok(())
}

fn explain_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move {
        let _ = ctx;
        crate::tool::outcome::CallOutcome::Success {
            result: json!({ "structuredContent": context_result_data() }),
        }
    })
}

fn context_result_data() -> Value {
    let capabilities = CAPABILITY_SEMANTICS
        .iter()
        .map(|(capability, semantics)| {
            json!({
                "id": capability,
                "semantics": semantics,
            })
        })
        .collect::<Vec<_>>();
    let operations = descriptors()
        .iter()
        .map(|descriptor| {
            json!({
                "id": descriptor.key.id,
                "intent": descriptor.key.intent,
                "requires": descriptor.requires,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema": CONTEXT_RESULT_SCHEMA,
        "capabilities": capabilities,
        "operations": operations,
        "managedGovernance": managed_governance_context(),
    })
}

fn managed_governance_context() -> Option<Value> {
    let paths = crate::governance::paths::GovernancePaths::production();
    if !paths.managed_bootstrap.exists() {
        return None;
    }
    let cache_path = paths.managed_cache.as_ref()?;
    let sidecar = crate::governance::managed::status::sidecar_path(cache_path);
    let status = crate::governance::managed::status::read_sidecar(&sidecar)?;
    Some(bounded_managed_governance_context(&status))
}

fn bounded_managed_governance_context(
    status: &crate::governance::managed::status::ManagedStatus,
) -> Value {
    let freshness = match status.freshness.as_str() {
        "fresh" => "fresh",
        "last_known_good" => "last_known_good",
        _ => "other",
    };
    let stale_reason = match status.stale_reason.as_deref() {
        Some("source_unreachable") => Some("source_unreachable"),
        Some("update_rejected") => Some("update_rejected"),
        Some("rollback_refused") => Some("rollback_refused"),
        _ => None,
    };
    let fetched_at =
        bounded_single_line(&status.fetched_at, MAX_MANAGED_TIMESTAMP_CHARS).unwrap_or("-");
    let presentation = status.presentation.as_ref().filter(|presentation| {
        crate::governance::manifest::bundle::validate_presentation(presentation).is_ok()
    });
    let organization = presentation.and_then(|value| value.org_name.as_deref());
    let rationale = presentation.and_then(|value| value.rationale.as_deref());
    let contact = presentation
        .and_then(|value| value.contacts.first())
        .map(|value| value.value.as_str());

    json!({
        "active": true,
        "organization": organization,
        "policySequence": status.seq,
        "freshness": freshness,
        "staleReason": stale_reason,
        "fetchedAt": fetched_at,
        "rationale": rationale,
        "contact": contact,
    })
}

fn bounded_single_line(value: &str, max_chars: usize) -> Option<&str> {
    (!value.is_empty()
        && value.chars().count() <= max_chars
        && !value.chars().any(char::is_control))
    .then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::operation::{BrowserResult, FlowStepResult};
    use std::collections::HashSet;

    fn flow_disposition(intent: IntentId, steps: Vec<FlowStepResult>) -> SuccessDisposition {
        flow_disposition_with_termination(
            intent,
            steps,
            FlowTermination {
                reason: FlowTerminationReason::Completed,
                step: None,
            },
        )
    }

    fn flow_disposition_with_termination(
        intent: IntentId,
        steps: Vec<FlowStepResult>,
        termination: FlowTermination,
    ) -> SuccessDisposition {
        let descriptor = super::descriptor(OperationKey::new(OperationId::BrowserFlow, intent))
            .expect("flow descriptor");
        descriptor.success_disposition_for(&json!({
            "content": [],
            "structuredContent": FlowResultData {
                steps,
                summary: "test flow".into(),
                duration_ms: 1,
                termination,
            }
        }))
    }

    fn flow_step(
        step: u32,
        flow_status: FlowStepStatus,
        id: OperationId,
        intent: IntentId,
        status: BrowserResultStatus,
        effect: OperationEffect,
    ) -> FlowStepResult {
        FlowStepResult {
            step,
            status: flow_status,
            result: BrowserResult::new(id, intent, status, effect),
        }
    }

    #[test]
    fn canonical_keys_are_unique_and_every_pair_lookup_is_exact() {
        let mut seen = HashSet::new();
        for descriptor in descriptors() {
            assert!(
                seen.insert(descriptor.key),
                "duplicate {:?}",
                descriptor.key
            );
            assert_eq!(
                super::descriptor(descriptor.key).map(|row| row.key),
                Some(descriptor.key)
            );
        }
        assert!(super::descriptor(OperationKey::new(
            OperationId::BrowserContext,
            IntentId::ActClick,
        ))
        .is_none());
    }

    #[test]
    fn availability_is_ordered_and_contains_no_model_declarations() {
        let governance = crate::governance::dispatch::Governance::all_open(std::sync::Arc::new(
            crate::governance::ports::NullSink,
        ));
        let projection = project_availability(&governance, None, 7);
        assert_eq!(projection.generation, 7);
        assert!(!projection.restricted);
        assert_eq!(projection.operations.len(), descriptors().len());
        for (availability, descriptor) in projection.operations.iter().zip(descriptors()) {
            assert_eq!(availability.id, descriptor.key.id);
            assert_eq!(availability.intent, descriptor.key.intent);
        }
        let encoded = serde_json::to_value(projection).expect("serialize availability");
        let text = encoded.to_string();
        assert!(!text.contains("inputSchema"));
        assert!(!text.contains("instructions"));
        assert!(!text.contains("description"));
    }

    #[test]
    fn canonical_act_validation_uses_the_intent_instead_of_an_action_field() {
        let descriptor = super::descriptor(OperationKey::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
        ))
        .expect("click descriptor");
        assert!(descriptor
            .validate(&json!({
                "tab": 1,
                "target": {"ref": "ref_1"}
            }))
            .is_ok());
        assert!(descriptor
            .validate(&json!({
                "tab": 1,
                "target": {"ref": "ref_1"},
                "action": "right_click"
            }))
            .is_err());
    }

    #[test]
    fn canonical_input_schemas_reject_physically_ignored_legacy_options() {
        let type_text = super::descriptor(OperationKey::new(
            OperationId::BrowserInput,
            IntentId::InputTypeText,
        ))
        .expect("type descriptor");
        assert!(type_text.validate(&json!({"tab":1,"text":"hello"})).is_ok());
        assert!(type_text
            .validate(&json!({"tab":1,"text":"hello","repeat":2}))
            .is_err());

        let press_key = super::descriptor(OperationKey::new(
            OperationId::BrowserInput,
            IntentId::InputPressKey,
        ))
        .expect("key descriptor");
        assert!(press_key
            .validate(&json!({"tab":1,"key":"Enter","repeat":2}))
            .is_ok());
        assert!(press_key
            .validate(&json!({"tab":1,"key":"Enter","modifiers":"ctrl"}))
            .is_err());
    }

    #[test]
    fn canonical_target_schemas_advertise_only_supported_target_forms() {
        let ref_target = json!({
            "type": "object",
            "properties": {
                "ref": { "type": "string", "minLength": 1 }
            },
            "required": ["ref"],
            "additionalProperties": false
        });
        let query_target = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "minLength": 1 }
            },
            "required": ["query"],
            "additionalProperties": false
        });

        for (key, pointer, expected) in [
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillField),
                "/properties/target",
                ref_target.clone(),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillFields),
                "/properties/fields/items/properties/target",
                query_target.clone(),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillFieldsAndSubmit),
                "/properties/fields/items/properties/target",
                query_target.clone(),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadClientFiles),
                "/properties/target",
                ref_target.clone(),
            ),
            (
                OperationKey::new(OperationId::BrowserInput, IntentId::InputWheel),
                "/properties/target",
                ref_target.clone(),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadCapturedArtifact),
                "/properties/target",
                ref_target.clone(),
            ),
            (
                OperationKey::new(OperationId::BrowserRecord, IntentId::RecordExport),
                "/properties/target",
                ref_target.clone(),
            ),
        ] {
            let schema = canonical_schema(key);
            assert_eq!(schema.pointer(pointer), Some(&expected), "{key:?}");
        }

        let general_target = json!({
            "type": "object",
            "properties": {
                "ref": { "type": "string" },
                "query": { "type": "string" },
                "name": { "type": "string" },
                "role": { "type": "string" }
            },
            "additionalProperties": false
        });
        let key = OperationKey::new(OperationId::BrowserAct, IntentId::ActClick);
        assert_eq!(
            canonical_schema(key).pointer("/properties/target"),
            Some(&general_target),
            "general target changed for {key:?}"
        );
    }

    #[test]
    fn canonical_target_semantics_accept_exactly_the_advertised_forms() {
        for (key, arguments) in [
            (
                OperationKey::new(OperationId::BrowserAct, IntentId::ActClick),
                json!({"tab":1,"target":{"query":"Submit order"}}),
            ),
            (
                OperationKey::new(OperationId::BrowserAct, IntentId::ActClick),
                json!({"tab":1,"target":{"name":"Submit","role":"button"}}),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillField),
                json!({"tab":1,"target":{"ref":"ref_1"},"value":"Ada"}),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillFields),
                json!({
                    "tab":1,
                    "fields":[{"target":{"query":"Email"},"value":"a@example.com"}]
                }),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillFieldsAndSubmit),
                json!({
                    "tab":1,
                    "fields":[{"target":{"query":"Email"},"value":"a@example.com"}]
                }),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadClientFiles),
                json!({"tab":1,"target":{"ref":"ref_2"}}),
            ),
            (
                OperationKey::new(OperationId::BrowserInput, IntentId::InputWheel),
                json!({"tab":1,"target":{"ref":"ref_3"},"direction":"down"}),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadCapturedArtifact),
                json!({"tab":1,"artifact":"artifact_1","target":{"ref":"ref_4"}}),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadCapturedArtifact),
                json!({"tab":1,"artifact":"artifact_1","point":[10,20]}),
            ),
            (
                OperationKey::new(OperationId::BrowserRecord, IntentId::RecordExport),
                json!({"tab":1,"target":{"ref":"ref_5"}}),
            ),
            (
                OperationKey::new(OperationId::BrowserRecord, IntentId::RecordExport),
                json!({"tab":1,"point":[10,20]}),
            ),
        ] {
            assert!(
                descriptor(key)
                    .expect("target-form descriptor")
                    .validate(&arguments)
                    .is_ok(),
                "advertised target form failed semantic validation for {key:?}"
            );
        }

        for (key, arguments) in [
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillField),
                json!({"tab":1,"target":{"query":"Email"},"value":"Ada"}),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillField),
                json!({"tab":1,"target":{"ref":""},"value":"Ada"}),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillFields),
                json!({"tab":1,"fields":[{"target":{"ref":"ref_1"},"value":"Ada"}]}),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillFieldsAndSubmit),
                json!({
                    "tab":1,
                    "fields":[{"target":{"name":"Email","role":"textbox"},"value":"Ada"}]
                }),
            ),
            (
                OperationKey::new(OperationId::BrowserFill, IntentId::FillFields),
                json!({"tab":1,"fields":[{"target":{"query":""},"value":"Ada"}]}),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadClientFiles),
                json!({"tab":1,"target":{"query":"input[type=file]"}}),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadClientFiles),
                json!({"tab":1,"target":{"ref":""}}),
            ),
            (
                OperationKey::new(OperationId::BrowserInput, IntentId::InputWheel),
                json!({"tab":1,"target":{"query":"Scrollable area"},"direction":"down"}),
            ),
            (
                OperationKey::new(OperationId::BrowserInput, IntentId::InputWheel),
                json!({"tab":1,"target":{"ref":""},"direction":"down"}),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadCapturedArtifact),
                json!({"tab":1,"artifact":"artifact_1","target":{"query":"Drop zone"}}),
            ),
            (
                OperationKey::new(OperationId::BrowserUpload, IntentId::UploadCapturedArtifact),
                json!({"tab":1,"artifact":"artifact_1","target":{"ref":""}}),
            ),
            (
                OperationKey::new(OperationId::BrowserRecord, IntentId::RecordExport),
                json!({"tab":1,"target":{"query":"Drop zone"}}),
            ),
            (
                OperationKey::new(OperationId::BrowserRecord, IntentId::RecordExport),
                json!({"tab":1,"target":{"ref":""}}),
            ),
        ] {
            assert!(
                descriptor(key)
                    .expect("target-form descriptor")
                    .validate(&arguments)
                    .is_err(),
                "unadvertised target form passed semantic validation for {key:?}"
            );
        }
    }

    #[test]
    fn canonical_flow_validates_concrete_fields_while_preserving_typed_references() {
        let flow = super::descriptor(OperationKey::new(
            OperationId::BrowserFlow,
            IntentId::FlowExecute,
        ))
        .expect("flow descriptor");
        let referenced_steps = [
            BrowserOperation::new(
                OperationId::BrowserRead,
                IntentId::ReadText,
                json!({"tab":"$prev.tab","max_chars":"$1.limit"}),
            ),
            BrowserOperation::new(
                OperationId::BrowserFill,
                IntentId::FillFields,
                json!({"tab":1,"fields":"$prev.fields"}),
            ),
            BrowserOperation::new(
                OperationId::BrowserAct,
                IntentId::ActClick,
                json!({"tab":1,"target":"$prev.target"}),
            ),
        ]
        .into_iter()
        .map(|step| serde_json::to_value(step).expect("serialize step"))
        .collect::<Vec<_>>();
        assert!(flow
            .validate(&json!({"steps":referenced_steps,"on_error":"stop"}))
            .is_ok());

        let invalid_step = BrowserOperation::new(
            OperationId::BrowserRead,
            IntentId::ReadText,
            json!({"tab":"$prev.tab","max_chars":true}),
        );
        assert!(flow
            .validate(&json!({
                "steps":[serde_json::to_value(invalid_step).expect("serialize step")]
            }))
            .is_err());
    }

    #[test]
    fn ordinary_success_uses_the_descriptor_effect_and_generic_errors_do_not_commit() {
        let descriptor = super::descriptor(OperationKey::new(
            OperationId::BrowserNavigate,
            IntentId::NavigateUrl,
        ))
        .expect("navigate descriptor");
        assert_eq!(
            descriptor.success_disposition_for(&json!({"content": []})),
            SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::Committed, None)
        );
        let read = super::descriptor(OperationKey::new(
            OperationId::BrowserRead,
            IntentId::ReadText,
        ))
        .expect("read descriptor");
        assert_eq!(
            read.success_disposition_for(&json!({"content": [], "isError": true})),
            SuccessDisposition::new(BrowserResultStatus::Partial, OperationEffect::None, None)
        );
    }

    #[test]
    fn landing_denial_after_navigation_is_partial_committed_and_unsafe_to_retry() {
        for intent in [
            IntentId::NavigateUrl,
            IntentId::NavigateBack,
            IntentId::NavigateForward,
        ] {
            let descriptor =
                super::descriptor(OperationKey::new(OperationId::BrowserNavigate, intent))
                    .expect("navigate landing descriptor");
            assert_eq!(
                descriptor.success_disposition_for(&json!({
                    "content": [{"type":"text","text":"blocked landing"}],
                    "isError": true
                })),
                SuccessDisposition::new(
                    BrowserResultStatus::Partial,
                    OperationEffect::Committed,
                    Some(RetryDisposition::Unsafe)
                )
            );
        }
    }

    #[test]
    fn act_pre_action_blockers_are_no_effect_and_retry_only_with_truthful_guidance() {
        let descriptor = super::descriptor(OperationKey::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
        ))
        .expect("act descriptor");
        assert_eq!(
            descriptor.success_disposition_for(&json!({
                "content": [],
                "isError": true,
                "structuredContent": {
                    "interactionReceipt": {
                        "blockers": [{"kind": "stale_ref"}]
                    }
                }
            })),
            SuccessDisposition::new(
                BrowserResultStatus::Blocked,
                OperationEffect::None,
                Some(RetryDisposition::AfterStateChange)
            )
        );
        assert_eq!(
            descriptor.success_disposition_for(&json!({
                "content": [],
                "isError": true,
                "structuredContent": {
                    "interactionReceipt": {
                        "blockers": [{"kind": "ambiguous_target"}]
                    }
                }
            })),
            SuccessDisposition::new(BrowserResultStatus::Blocked, OperationEffect::None, None)
        );
        assert_eq!(
            descriptor.success_disposition_for(&json!({"content": [], "isError": true})),
            SuccessDisposition::new(BrowserResultStatus::Blocked, OperationEffect::None, None)
        );
    }

    #[test]
    fn act_expect_timeout_is_partial_after_a_committed_action() {
        let descriptor = super::descriptor(OperationKey::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
        ))
        .expect("act descriptor");
        assert_eq!(
            descriptor.success_disposition_for(&json!({
                "content": [],
                "isError": true,
                "structuredContent": {
                    "interactionReceipt": {
                        "blockers": [{"kind": "expect_timeout"}]
                    }
                }
            })),
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                None
            )
        );
    }

    #[test]
    fn act_post_action_safety_refusals_are_partial_after_commit() {
        let descriptor = super::descriptor(OperationKey::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
        ))
        .expect("act descriptor");
        for kind in ["postcondition_paused", "postcondition_interrupted"] {
            assert_eq!(
                descriptor.success_disposition_for(&json!({
                    "content": [],
                    "isError": true,
                    "structuredContent": {
                        "interactionReceipt": {
                            "blockers": [{"kind": kind}]
                        }
                    }
                })),
                SuccessDisposition::new(
                    BrowserResultStatus::Partial,
                    OperationEffect::Committed,
                    None
                )
            );
        }
    }

    #[test]
    fn form_fill_commits_only_when_a_field_or_submit_committed() {
        let fill = super::descriptor(OperationKey::new(
            OperationId::BrowserFill,
            IntentId::FillFields,
        ))
        .expect("fill descriptor");
        assert_eq!(
            fill.success_disposition_for(&json!({"content": [], "isError": true})),
            SuccessDisposition::new(BrowserResultStatus::Blocked, OperationEffect::None, None)
        );
        assert_eq!(
            fill.success_disposition_for(&json!({
                "content": [],
                "structuredContent": {"filled": [], "submitted": false}
            })),
            SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::None, None)
        );
        assert_eq!(
            fill.success_disposition_for(&json!({
                "content": [],
                "structuredContent": {
                    "filled": [{"label": "Email"}],
                    "submitted": false
                }
            })),
            SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::Committed, None)
        );
        assert_eq!(
            fill.success_disposition_for(&json!({
                "content": [],
                "isError": true,
                "structuredContent": {
                    "filled": [{"label": "Email"}],
                    "skipped": [{"label": "Name", "reason": "not_run_after_pause"}],
                    "submitted": false
                }
            })),
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                None
            )
        );

        let submit = super::descriptor(OperationKey::new(
            OperationId::BrowserFill,
            IntentId::FillFieldsAndSubmit,
        ))
        .expect("submit descriptor");
        assert_eq!(
            submit.success_disposition_for(&json!({
                "content": [],
                "structuredContent": {"filled": [], "submitted": true}
            })),
            SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::Committed, None)
        );
    }

    #[test]
    fn recording_no_ops_and_post_effect_failures_have_exact_dispositions() {
        let descriptor = |intent| {
            super::descriptor(OperationKey::new(OperationId::BrowserRecord, intent))
                .expect("record descriptor")
        };
        for (intent, structured) in [
            (
                IntentId::RecordStart,
                json!({
                    "changed":false,
                    "start_acknowledged":false,
                    "start_committed":false
                }),
            ),
            (
                IntentId::RecordStop,
                json!({
                    "changed":false,
                    "stop_committed":false,
                    "finalization_effect":"none"
                }),
            ),
            (
                IntentId::RecordClear,
                json!({"changed":false,"clear_committed":false}),
            ),
            (
                IntentId::RecordExport,
                json!({
                    "changed":false,
                    "stop_committed":false,
                    "finalization_effect":"none",
                    "export_completed":false,
                    "delivery":"not_started"
                }),
            ),
        ] {
            assert_eq!(
                descriptor(intent).success_disposition_for(&json!({
                    "content": [],
                    "structuredContent": structured
                })),
                SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::None, None)
            );
        }

        let stop_partial = descriptor(IntentId::RecordStop).success_disposition_for(&json!({
            "content": [],
            "isError": true,
            "structuredContent": {
                "changed":true,
                "stop_committed":true,
                "stop_acknowledged":true,
                "recording_state_changed":false,
                "cancel_enqueued":false,
                "finalization_effect":"committed"
            }
        }));
        assert_eq!(
            stop_partial,
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );

        assert_eq!(
            descriptor(IntentId::RecordStart).success_disposition_for(&json!({
                "content": [],
                "isError": true,
                "structuredContent": {
                    "changed":true,
                    "start_acknowledged":true,
                    "start_committed":true,
                    "retry_safe":false
                }
            })),
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );

        let export = descriptor(IntentId::RecordExport);
        assert_eq!(
            export.success_disposition_for(&json!({
                "content": [],
                "isError": true,
                "structuredContent": {
                    "changed":true,
                    "stop_committed":true,
                    "finalization_effect":"committed",
                    "export_completed":false,
                    "delivery":"not_completed"
                }
            })),
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );
        assert_eq!(
            export.success_disposition_for(&json!({
                "content": [],
                "structuredContent": {
                    "changed":true,
                    "stop_committed":false,
                    "finalization_effect":"none",
                    "export_completed":true,
                    "delivery":"dispatched",
                    "retry_safe":false
                }
            })),
            SuccessDisposition::new(
                BrowserResultStatus::Ok,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );

        assert_eq!(
            export.success_disposition_for(&json!({"content":[]})),
            SuccessDisposition::new(
                BrowserResultStatus::Unavailable,
                OperationEffect::None,
                None
            )
        );

        assert_eq!(
            descriptor(IntentId::RecordStop).success_disposition_for(&json!({
                "content": [],
                "isError": true,
                "structuredContent": {
                    "changed":true,
                    "stop_committed":true,
                    "finalization_effect":"dispatched"
                }
            })),
            SuccessDisposition::new(
                BrowserResultStatus::OutcomeUnknown,
                OperationEffect::Unknown,
                Some(RetryDisposition::Unsafe)
            )
        );
    }

    #[test]
    fn recording_receipts_keep_root_and_flow_effects_truthful() {
        let stop = super::descriptor(OperationKey::new(
            OperationId::BrowserRecord,
            IntentId::RecordStop,
        ))
        .expect("record stop descriptor");
        let no_op = stop.success_disposition_for(&json!({
            "content": [],
            "structuredContent": {
                "changed":false,
                "stop_committed":false,
                "finalization_effect":"none"
            }
        }));
        assert_eq!(no_op.effect, OperationEffect::None);
        assert_eq!(
            flow_disposition(
                IntentId::FlowExecute,
                vec![flow_step(
                    1,
                    FlowStepStatus::Ok,
                    OperationId::BrowserRecord,
                    IntentId::RecordStop,
                    no_op.status,
                    no_op.effect,
                )],
            ),
            SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::None, None)
        );

        let partial = stop.success_disposition_for(&json!({
            "content": [],
            "isError": true,
            "structuredContent": {
                "changed":true,
                "stop_committed":true,
                "finalization_effect":"committed"
            }
        }));
        assert_eq!(partial.effect, OperationEffect::Committed);
        assert_eq!(partial.retry, Some(RetryDisposition::Unsafe));
        assert_eq!(
            flow_disposition(
                IntentId::FlowExecute,
                vec![flow_step(
                    1,
                    FlowStepStatus::Partial,
                    OperationId::BrowserRecord,
                    IntentId::RecordStop,
                    partial.status,
                    partial.effect,
                )],
            ),
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );

        let export = super::descriptor(OperationKey::new(
            OperationId::BrowserRecord,
            IntentId::RecordExport,
        ))
        .expect("record export descriptor");
        let export_partial = export.success_disposition_for(&json!({
            "content": [],
            "isError": true,
            "structuredContent": {
                "changed":true,
                "stop_committed":true,
                "finalization_effect":"committed",
                "export_completed":false,
                "delivery":"not_completed"
            }
        }));
        assert_eq!(export_partial.retry, Some(RetryDisposition::Unsafe));
        assert_eq!(
            flow_disposition(
                IntentId::FlowExecute,
                vec![flow_step(
                    1,
                    FlowStepStatus::Partial,
                    OperationId::BrowserRecord,
                    IntentId::RecordExport,
                    export_partial.status,
                    export_partial.effect,
                )],
            ),
            export_partial
        );

        let uncertain = stop.success_disposition_for(&json!({
            "content": [],
            "isError": true,
            "structuredContent": {
                "changed":true,
                "stop_committed":true,
                "finalization_effect":"dispatched"
            }
        }));
        assert_eq!(
            flow_disposition(
                IntentId::FlowExecute,
                vec![flow_step(
                    1,
                    FlowStepStatus::OutcomeUnknown,
                    OperationId::BrowserRecord,
                    IntentId::RecordStop,
                    uncertain.status,
                    uncertain.effect,
                )],
            ),
            uncertain
        );
    }

    #[test]
    fn read_only_flow_has_no_aggregate_effect() {
        let disposition = flow_disposition(
            IntentId::FlowExecute,
            vec![flow_step(
                1,
                FlowStepStatus::Ok,
                OperationId::BrowserFind,
                IntentId::FindQuery,
                BrowserResultStatus::Ok,
                OperationEffect::None,
            )],
        );
        assert_eq!(
            disposition,
            SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::None, None)
        );
    }

    #[test]
    fn flow_denied_before_any_effect_is_blocked_without_retry() {
        let disposition = flow_disposition(
            IntentId::FlowExecute,
            vec![
                flow_step(
                    1,
                    FlowStepStatus::Denied,
                    OperationId::BrowserAct,
                    IntentId::ActClick,
                    BrowserResultStatus::Blocked,
                    OperationEffect::None,
                ),
                flow_step(
                    2,
                    FlowStepStatus::NotRun,
                    OperationId::BrowserFind,
                    IntentId::FindQuery,
                    BrowserResultStatus::NotDispatched,
                    OperationEffect::None,
                ),
            ],
        );
        assert_eq!(
            disposition,
            SuccessDisposition::new(BrowserResultStatus::Blocked, OperationEffect::None, None)
        );
    }

    #[test]
    fn flow_effect_then_failure_is_partial_and_committed() {
        let disposition = flow_disposition(
            IntentId::FlowExecute,
            vec![
                flow_step(
                    1,
                    FlowStepStatus::Ok,
                    OperationId::BrowserNavigate,
                    IntentId::NavigateUrl,
                    BrowserResultStatus::Ok,
                    OperationEffect::Committed,
                ),
                flow_step(
                    2,
                    FlowStepStatus::Unavailable,
                    OperationId::BrowserFind,
                    IntentId::FindQuery,
                    BrowserResultStatus::Unavailable,
                    OperationEffect::None,
                ),
            ],
        );
        assert_eq!(
            disposition,
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );
    }

    #[test]
    fn acknowledged_partial_step_keeps_ok_flow_control_but_weakens_the_root() {
        let disposition = flow_disposition(
            IntentId::FlowExecute,
            vec![
                flow_step(
                    1,
                    FlowStepStatus::Ok,
                    OperationId::BrowserAct,
                    IntentId::ActClick,
                    BrowserResultStatus::Partial,
                    OperationEffect::Committed,
                ),
                flow_step(
                    2,
                    FlowStepStatus::Ok,
                    OperationId::BrowserFind,
                    IntentId::FindQuery,
                    BrowserResultStatus::Ok,
                    OperationEffect::None,
                ),
            ],
        );
        assert_eq!(
            disposition,
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );
    }

    #[test]
    fn boundary_and_post_effect_flow_cancellation_are_distinct() {
        let boundary = flow_disposition(
            IntentId::FlowExecute,
            vec![flow_step(
                1,
                FlowStepStatus::NotRun,
                OperationId::BrowserFind,
                IntentId::FindQuery,
                BrowserResultStatus::Cancelled,
                OperationEffect::None,
            )],
        );
        assert_eq!(
            boundary,
            SuccessDisposition::new(BrowserResultStatus::Cancelled, OperationEffect::None, None)
        );

        let committed = flow_disposition(
            IntentId::FlowExecute,
            vec![flow_step(
                1,
                FlowStepStatus::Cancelled,
                OperationId::BrowserAct,
                IntentId::ActClick,
                BrowserResultStatus::Cancelled,
                OperationEffect::Committed,
            )],
        );
        assert_eq!(
            committed,
            SuccessDisposition::new(
                BrowserResultStatus::Cancelled,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );
    }

    #[test]
    fn controlling_flow_terminals_win_regardless_of_earlier_continue_errors() {
        for statuses in [
            [FlowStepStatus::Denied, FlowStepStatus::Held],
            [FlowStepStatus::Held, FlowStepStatus::Denied],
        ] {
            let steps = statuses
                .into_iter()
                .enumerate()
                .map(|(index, status)| {
                    let result_status = match status {
                        FlowStepStatus::Held => BrowserResultStatus::Held,
                        FlowStepStatus::Denied => BrowserResultStatus::Blocked,
                        _ => unreachable!(),
                    };
                    flow_step(
                        (index + 1) as u32,
                        status,
                        OperationId::BrowserFind,
                        IntentId::FindQuery,
                        result_status,
                        OperationEffect::None,
                    )
                })
                .collect();
            assert_eq!(
                flow_disposition(IntentId::FlowExecute, steps),
                SuccessDisposition::new(BrowserResultStatus::Held, OperationEffect::None, None)
            );
        }

        let committed_then_attention = flow_disposition(
            IntentId::FlowExecute,
            vec![
                flow_step(
                    1,
                    FlowStepStatus::Ok,
                    OperationId::BrowserAct,
                    IntentId::ActClick,
                    BrowserResultStatus::Ok,
                    OperationEffect::Committed,
                ),
                flow_step(
                    2,
                    FlowStepStatus::AttentionRequired,
                    OperationId::BrowserFind,
                    IntentId::FindQuery,
                    BrowserResultStatus::AttentionRequired,
                    OperationEffect::None,
                ),
            ],
        );
        assert_eq!(
            committed_then_attention,
            SuccessDisposition::new(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe)
            )
        );
    }

    #[test]
    fn typed_budget_and_denial_termination_cannot_be_lost_in_not_run_tails() {
        let read = flow_step(
            1,
            FlowStepStatus::Ok,
            OperationId::BrowserFind,
            IntentId::FindQuery,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        let tail = flow_step(
            2,
            FlowStepStatus::NotRun,
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
        );
        assert_eq!(
            flow_disposition_with_termination(
                IntentId::FlowExecute,
                vec![read.clone(), tail.clone()],
                FlowTermination {
                    reason: FlowTerminationReason::BudgetExhausted,
                    step: Some(1),
                },
            ),
            SuccessDisposition::new(BrowserResultStatus::Partial, OperationEffect::None, None)
        );

        assert_eq!(
            flow_disposition_with_termination(
                IntentId::FlowExecute,
                vec![read, tail],
                FlowTermination {
                    reason: FlowTerminationReason::Denied,
                    step: Some(2),
                },
            ),
            SuccessDisposition::new(BrowserResultStatus::Blocked, OperationEffect::None, None)
        );
    }

    #[test]
    fn uncertain_flow_terminal_is_not_hidden_or_safe_to_retry() {
        let disposition = flow_disposition(
            IntentId::FlowExecute,
            vec![flow_step(
                1,
                FlowStepStatus::OutcomeUnknown,
                OperationId::BrowserAct,
                IntentId::ActClick,
                BrowserResultStatus::OutcomeUnknown,
                OperationEffect::Unknown,
            )],
        );
        assert_eq!(
            disposition,
            SuccessDisposition::new(
                BrowserResultStatus::OutcomeUnknown,
                OperationEffect::Unknown,
                Some(RetryDisposition::Unsafe)
            )
        );
    }

    #[test]
    fn flow_preflight_never_claims_a_physical_effect() {
        let disposition = flow_disposition(
            IntentId::FlowPreflight,
            vec![
                flow_step(
                    1,
                    FlowStepStatus::WouldAllow,
                    OperationId::BrowserFind,
                    IntentId::FindQuery,
                    BrowserResultStatus::Ok,
                    OperationEffect::None,
                ),
                flow_step(
                    2,
                    FlowStepStatus::WouldDeny,
                    OperationId::BrowserAct,
                    IntentId::ActClick,
                    BrowserResultStatus::Blocked,
                    OperationEffect::None,
                ),
            ],
        );
        assert_eq!(
            disposition,
            SuccessDisposition::new(BrowserResultStatus::Blocked, OperationEffect::None, None)
        );

        let malformed_uncertain = flow_disposition(
            IntentId::FlowPreflight,
            vec![flow_step(
                1,
                FlowStepStatus::OutcomeUnknown,
                OperationId::BrowserAct,
                IntentId::ActClick,
                BrowserResultStatus::OutcomeUnknown,
                OperationEffect::None,
            )],
        );
        assert_eq!(
            malformed_uncertain,
            SuccessDisposition::new(
                BrowserResultStatus::Unavailable,
                OperationEffect::None,
                None
            )
        );
    }

    #[test]
    fn context_result_contains_only_canonical_operation_and_capability_facts() {
        let value = context_result_data();
        assert_eq!(value["schema"], CONTEXT_RESULT_SCHEMA);
        assert_eq!(
            value["capabilities"],
            json!([
                {"id":"read","semantics":"retrieve_observe_only"},
                {"id":"action","semantics":"page_determined_ui_input"},
                {"id":"write","semantics":"declared_state_change"},
                {"id":"execute","semantics":"arbitrary_code"},
            ])
        );
        let operations = value["operations"].as_array().expect("operation facts");
        assert_eq!(operations.len(), descriptors().len());
        for (fact, descriptor) in operations.iter().zip(descriptors()) {
            assert_eq!(fact["id"], json!(descriptor.key.id));
            assert_eq!(fact["intent"], json!(descriptor.key.intent));
            assert_eq!(fact["requires"], json!(descriptor.requires));
            assert_eq!(fact.as_object().expect("operation fact").len(), 3);
        }
        let serialized = serde_json::to_string(&value).expect("semantic context serializes");
        for legacy_fragment in [
            "tabs_context_mcp",
            "tabs_create_mcp",
            "computer",
            "get_page_text",
            "requires nothing",
            "Show every action available here",
        ] {
            assert!(
                !serialized.contains(legacy_fragment),
                "context data leaked legacy surface prose: {legacy_fragment}"
            );
        }
    }

    #[test]
    fn managed_context_is_bounded_and_preserves_valid_passport_facts() {
        use crate::governance::managed::status::ManagedStatus;
        use crate::governance::manifest::bundle::{Contact, Presentation};

        let status = ManagedStatus {
            v: 1,
            freshness: "last_known_good".into(),
            stale_reason: Some("update_rejected".into()),
            seq: Some(42),
            fetched_at: "2026-07-10T14:02:00+00:00".into(),
            source: "ignored source".into(),
            presentation: Some(Presentation {
                org_name: Some("Acme Security".into()),
                rationale: Some("Baseline policy.".into()),
                contacts: vec![Contact {
                    kind: "email".into(),
                    value: "security@example.com".into(),
                    label: None,
                }],
            }),
            last_error: Some("ignored internal error".into()),
        };
        assert_eq!(
            bounded_managed_governance_context(&status),
            json!({
                "active": true,
                "organization": "Acme Security",
                "policySequence": 42,
                "freshness": "last_known_good",
                "staleReason": "update_rejected",
                "fetchedAt": "2026-07-10T14:02:00+00:00",
                "rationale": "Baseline policy.",
                "contact": "security@example.com",
            })
        );

        let mut invalid = status;
        invalid.freshness = "future_state".into();
        invalid.stale_reason = Some("future_reason".into());
        invalid.fetched_at = format!("bad\n{}", "x".repeat(256));
        invalid.presentation = Some(Presentation {
            org_name: Some("x".repeat(121)),
            rationale: None,
            contacts: Vec::new(),
        });
        assert_eq!(
            bounded_managed_governance_context(&invalid),
            json!({
                "active": true,
                "organization": null,
                "policySequence": 42,
                "freshness": "other",
                "staleReason": null,
                "fetchedAt": "-",
                "rationale": null,
                "contact": null,
            })
        );
    }
}
