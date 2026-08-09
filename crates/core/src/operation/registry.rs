// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The single Ghostlight operation registry.
//!
//! Each public operation has one row. The row owns authorization requirements, browser-resource
//! proof, scheduling, dispatch, provenance, and ordinary result meaning. Browser mechanisms are
//! private implementation details and do not form a second operation vocabulary.

use crate::governance::manifest::document::Grant;
use crate::governance::ports::{capability_subset, Capability};
use crate::tool::outcome::{LocalCtx, LocalFuture};
use ghostlight_transport::bridge::{CatalogProjection, OperationAvailability, WorkspaceUse};
use ghostlight_transport::operation::{
    BrowserResultStatus, FlowResultData, FlowStepStatus, FlowTerminationReason, Operation,
    OperationEffect, OperationKind, RetryDisposition,
};
use serde_json::{json, Value};

const CONTEXT_RESULT_SCHEMA: &str = "ghostlight.browser.context/v1";
const MAX_MANAGED_TIMESTAMP_CHARS: usize = 128;
const READ: &[Capability] = &[Capability::Read];
const INTERACT: &[Capability] = &[Capability::Interact];
const WRITE: &[Capability] = &[Capability::Write];
const CAPABILITY_SEMANTICS: &[(Capability, &str)] = &[
    (Capability::Read, "retrieve_observe_only"),
    (Capability::Interact, "page_determined_ui_input"),
    (Capability::Write, "declared_state_change"),
    (Capability::Execute, "arbitrary_code"),
];

/// Page resource evidence required for authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationResource {
    None,
    OptionalTargetUrl,
    CurrentTab,
    TargetAndLandings,
    CurrentAndLandings,
    TabInventory,
}

/// How an admitted operation reaches its implementation.
#[derive(Debug, Clone, Copy)]
pub enum Handler {
    Mechanism,
    Local(for<'a> fn(LocalCtx<'a>) -> LocalFuture<'a>),
    Composition,
}

/// Browser-dependent behavior after the primary dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostDispatch {
    None,
    NavigateLanding,
}

/// Page-authored output carried by an operation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOutput {
    None,
    Text,
    Receipt,
    Structured,
}

/// Scheduler resource selected for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingScope {
    Surface,
    WorkspaceTopology,
    Browser,
    Presentation,
    Local,
    Composition,
}

/// Lifetime of the scheduler lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseMode {
    Dispatch,
    RetainSurface,
    Composition,
}

/// Scheduling declaration stored in the operation registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scheduling {
    pub scope: SchedulingScope,
    pub lease: LeaseMode,
}

impl Scheduling {
    pub const SURFACE: Self = Self {
        scope: SchedulingScope::Surface,
        lease: LeaseMode::Dispatch,
    };
    pub const RETAIN_SURFACE: Self = Self {
        scope: SchedulingScope::Surface,
        lease: LeaseMode::RetainSurface,
    };
    pub const WORKSPACE_TOPOLOGY: Self = Self {
        scope: SchedulingScope::WorkspaceTopology,
        lease: LeaseMode::Dispatch,
    };
    pub const BROWSER: Self = Self {
        scope: SchedulingScope::Browser,
        lease: LeaseMode::Dispatch,
    };
    pub const PRESENTATION: Self = Self {
        scope: SchedulingScope::Presentation,
        lease: LeaseMode::Dispatch,
    };
    pub const LOCAL: Self = Self {
        scope: SchedulingScope::Local,
        lease: LeaseMode::Dispatch,
    };
    pub const COMPOSITION: Self = Self {
        scope: SchedulingScope::Composition,
        lease: LeaseMode::Composition,
    };
}

/// One complete operation contract.
#[derive(Debug, Clone, Copy)]
pub struct OperationDescriptor {
    pub operation: OperationKind,
    pub workspace_use: WorkspaceUse,
    pub requires: &'static [Capability],
    pub resource: OperationResource,
    pub scheduling: Scheduling,
    pub sequence_child: bool,
    pub handler: Handler,
    pub postprocess: Option<fn(&mut Value, bool)>,
    pub page_output: PageOutput,
    pub post_dispatch: PostDispatch,
    pub success_effect: OperationEffect,
}

/// Terminal meaning of one acknowledged handler result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuccessDisposition {
    pub status: BrowserResultStatus,
    pub effect: OperationEffect,
    pub retry: Option<RetryDisposition>,
}

impl SuccessDisposition {
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

macro_rules! descriptor {
    ($operation:ident, $workspace:ident, $requires:expr, $resource:ident, $scheduling:expr, $sequence:expr, $handler:expr, $postprocess:expr, $page:ident, $post_dispatch:ident, $effect:ident) => {
        OperationDescriptor {
            operation: OperationKind::$operation,
            workspace_use: WorkspaceUse::$workspace,
            requires: $requires,
            resource: OperationResource::$resource,
            scheduling: $scheduling,
            sequence_child: $sequence,
            handler: $handler,
            postprocess: $postprocess,
            page_output: PageOutput::$page,
            post_dispatch: PostDispatch::$post_dispatch,
            success_effect: OperationEffect::$effect,
        }
    };
}

const REDACT: Option<fn(&mut Value, bool)> = Some(crate::browser::redact::apply_to_result);

const DESCRIPTORS: &[OperationDescriptor] = &[
    descriptor!(
        BrowserGetStatus,
        Independent,
        &[],
        None,
        Scheduling::LOCAL,
        false,
        Handler::Local(status_handler),
        None,
        None,
        None,
        None
    ),
    descriptor!(
        BrowserOpenTab,
        Creates,
        INTERACT,
        OptionalTargetUrl,
        Scheduling::WORKSPACE_TOPOLOGY,
        false,
        Handler::Mechanism,
        None,
        Text,
        NavigateLanding,
        Committed
    ),
    descriptor!(
        BrowserListTabs,
        Uses,
        READ,
        TabInventory,
        Scheduling::WORKSPACE_TOPOLOGY,
        false,
        Handler::Mechanism,
        None,
        Text,
        None,
        None
    ),
    descriptor!(
        BrowserFocusTab,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::WORKSPACE_TOPOLOGY,
        false,
        Handler::Mechanism,
        REDACT,
        None,
        None,
        Committed
    ),
    descriptor!(
        BrowserCloseTab,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::WORKSPACE_TOPOLOGY,
        false,
        Handler::Mechanism,
        REDACT,
        None,
        None,
        Committed
    ),
    descriptor!(
        BrowserNavigate,
        Creates,
        INTERACT,
        TargetAndLandings,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Local(crate::tool::tab_navigation::tab_navigation_handler),
        None,
        Text,
        NavigateLanding,
        Committed
    ),
    descriptor!(
        BrowserGoBack,
        Uses,
        INTERACT,
        CurrentAndLandings,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Mechanism,
        None,
        Text,
        NavigateLanding,
        Committed
    ),
    descriptor!(
        BrowserGoForward,
        Uses,
        INTERACT,
        CurrentAndLandings,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Mechanism,
        None,
        Text,
        NavigateLanding,
        Committed
    ),
    descriptor!(
        BrowserReloadPage,
        Uses,
        INTERACT,
        CurrentAndLandings,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Mechanism,
        REDACT,
        None,
        NavigateLanding,
        Committed
    ),
    descriptor!(
        BrowserInspectPage,
        Uses,
        READ,
        CurrentTab,
        Scheduling::SURFACE,
        true,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        None
    ),
    descriptor!(
        BrowserReadPage,
        Uses,
        READ,
        CurrentTab,
        Scheduling::SURFACE,
        true,
        Handler::Local(crate::tool::page_read::page_read_handler),
        REDACT,
        Text,
        None,
        None
    ),
    descriptor!(
        BrowserTakeScreenshot,
        Uses,
        READ,
        CurrentTab,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Local(crate::tool::target_screenshot::target_screenshot_handler),
        REDACT,
        Receipt,
        None,
        None
    ),
    descriptor!(
        BrowserClick,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed
    ),
    descriptor!(
        BrowserHover,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed
    ),
    descriptor!(
        BrowserScrollToTarget,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed
    ),
    descriptor!(
        BrowserScrollPage,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::SURFACE,
        true,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed
    ),
    descriptor!(
        BrowserPressKey,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed
    ),
    descriptor!(
        BrowserPressEscape,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::SURFACE,
        true,
        Handler::Mechanism,
        REDACT,
        Receipt,
        None,
        Committed
    ),
    descriptor!(
        BrowserDrag,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Local(crate::tool::drag::drag_handler),
        REDACT,
        Receipt,
        None,
        Committed
    ),
    descriptor!(
        BrowserFillForm,
        Uses,
        WRITE,
        CurrentTab,
        Scheduling::RETAIN_SURFACE,
        true,
        Handler::Local(crate::tool::form_fill::form_fill_handler),
        None,
        Structured,
        None,
        Committed
    ),
    descriptor!(
        BrowserWaitFor,
        Uses,
        READ,
        CurrentTab,
        Scheduling::SURFACE,
        true,
        Handler::Local(crate::tool::wait::wait_handler),
        None,
        Text,
        None,
        None
    ),
    descriptor!(
        BrowserRunSequence,
        Creates,
        &[],
        CurrentTab,
        Scheduling::COMPOSITION,
        false,
        Handler::Composition,
        None,
        Structured,
        None,
        None
    ),
    descriptor!(
        BrowserGetDialog,
        Uses,
        READ,
        CurrentTab,
        Scheduling::SURFACE,
        true,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        None
    ),
    descriptor!(
        BrowserHandleDialog,
        Uses,
        INTERACT,
        CurrentTab,
        Scheduling::SURFACE,
        true,
        Handler::Mechanism,
        REDACT,
        Text,
        None,
        Committed
    ),
];

/// Return all operation contracts in public surface order.
pub const fn descriptors() -> &'static [OperationDescriptor] {
    DESCRIPTORS
}

/// Look up one operation contract.
pub fn descriptor(operation: OperationKind) -> &'static OperationDescriptor {
    DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.operation == operation)
        .expect("every Ghostlight operation has one descriptor")
}

/// Workspace relationship for one operation.
pub fn workspace_use(operation: OperationKind) -> WorkspaceUse {
    descriptor(operation).workspace_use
}

/// Complete capability set for one operation, including sequence children.
pub fn requirements(operation: &Operation) -> Vec<Capability> {
    let mut present = [false; 4];
    collect_requirements(operation, &mut present);
    [
        Capability::Read,
        Capability::Interact,
        Capability::Write,
        Capability::Execute,
    ]
    .into_iter()
    .zip(present)
    .filter_map(|(capability, present)| present.then_some(capability))
    .collect()
}

fn collect_requirements(operation: &Operation, present: &mut [bool; 4]) {
    if let Operation::BrowserRunSequence(arguments) = operation {
        for step in &arguments.steps {
            collect_requirements(step, present);
        }
        return;
    }
    for capability in descriptor(operation.kind()).requires {
        let index = match capability {
            Capability::Read => 0,
            Capability::Interact => 1,
            Capability::Write => 2,
            Capability::Execute => 3,
        };
        present[index] = true;
    }
}

/// Project operation availability under service and request restrictions.
pub fn project_availability(
    governance: &crate::governance::dispatch::Governance,
    restriction: Option<&crate::governance::overlay::SessionOverlay>,
    generation: u64,
) -> CatalogProjection {
    let operations = DESCRIPTORS
        .iter()
        .filter(|descriptor| {
            reachable(descriptor, governance.grants())
                && restriction.is_none_or(|restriction| reachable(descriptor, restriction.grants()))
        })
        .map(|descriptor| OperationAvailability {
            operation: descriptor.operation,
            workspace_use: descriptor.workspace_use,
        })
        .collect();
    CatalogProjection {
        generation,
        operations,
        restricted: restriction.is_some(),
    }
}

fn reachable(descriptor: &OperationDescriptor, grants: Option<&[Grant]>) -> bool {
    let Some(grants) = grants else {
        return true;
    };
    descriptor.requires.is_empty()
        || grants
            .iter()
            .any(|grant| capability_subset(descriptor.requires, &grant.allowed))
}

impl OperationDescriptor {
    /// Resolve the resource lease for this exact operation shape.
    ///
    /// URL navigation normally retains an existing tab surface. When the workspace has no
    /// current tab, the same operation creates its first tab and therefore runs on the topology
    /// lane. The distinction is part of the operation contract, not a browser-adapter workaround.
    pub fn scheduling_for(&self, operation: &Operation, arguments: &Value) -> Scheduling {
        if matches!(operation, Operation::BrowserNavigate(_)) && arguments.get("tab").is_none() {
            Scheduling::WORKSPACE_TOPOLOGY
        } else {
            self.scheduling
        }
    }

    /// Classify one acknowledged handler result without overstating its effect.
    pub fn success_disposition(&self, result: &Value) -> SuccessDisposition {
        if self.operation == OperationKind::BrowserWaitFor
            && crate::tool::wait::result_is_not_met(result)
        {
            return disposition(BrowserResultStatus::NotMet, OperationEffect::None, None);
        }
        if self.operation == OperationKind::BrowserHandleDialog
            && result
                .pointer("/structuredContent/resolution_not_met")
                .and_then(Value::as_bool)
                == Some(true)
            && result
                .pointer("/structuredContent/resolved")
                .and_then(Value::as_bool)
                == Some(false)
        {
            return disposition(BrowserResultStatus::NotMet, OperationEffect::None, None);
        }
        if self.operation == OperationKind::BrowserRunSequence {
            return sequence_disposition(result);
        }
        if self.post_dispatch == PostDispatch::NavigateLanding && is_error_result(result) {
            return disposition(
                BrowserResultStatus::Partial,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe),
            );
        }
        if matches!(
            self.operation,
            OperationKind::BrowserClick
                | OperationKind::BrowserHover
                | OperationKind::BrowserScrollToTarget
                | OperationKind::BrowserPressKey
                | OperationKind::BrowserDrag
        ) {
            if has_blocker(result, "expect_timeout")
                || has_blocker(result, "postcondition_paused")
                || has_blocker(result, "postcondition_interrupted")
            {
                return disposition(
                    BrowserResultStatus::Partial,
                    OperationEffect::Committed,
                    None,
                );
            }
            if has_blocker(result, "stale_ref")
                || has_blocker(result, "target_missing")
                || has_blocker(result, "covered_target")
            {
                return disposition(
                    BrowserResultStatus::Blocked,
                    OperationEffect::None,
                    Some(RetryDisposition::AfterStateChange),
                );
            }
            if has_blockers(result) || is_error_result(result) {
                return disposition(BrowserResultStatus::Blocked, OperationEffect::None, None);
            }
        }
        if self.operation == OperationKind::BrowserFillForm {
            let committed = result
                .pointer("/structuredContent/filled")
                .and_then(Value::as_array)
                .is_some_and(|filled| !filled.is_empty())
                || result
                    .pointer("/structuredContent/submitted")
                    .and_then(Value::as_bool)
                    == Some(true);
            if is_error_result(result) {
                return disposition(
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
            return disposition(
                BrowserResultStatus::Ok,
                if committed {
                    OperationEffect::Committed
                } else {
                    OperationEffect::None
                },
                None,
            );
        }
        if is_error_result(result) {
            disposition(BrowserResultStatus::Partial, OperationEffect::None, None)
        } else {
            disposition(BrowserResultStatus::Ok, self.success_effect, None)
        }
    }
}

const fn disposition(
    status: BrowserResultStatus,
    effect: OperationEffect,
    retry: Option<RetryDisposition>,
) -> SuccessDisposition {
    SuccessDisposition::new(status, effect, retry)
}

fn sequence_disposition(result: &Value) -> SuccessDisposition {
    let Some(data) = result.get("structuredContent") else {
        return disposition(
            BrowserResultStatus::Unavailable,
            OperationEffect::None,
            None,
        );
    };
    let Ok(flow) = serde_json::from_value::<FlowResultData>(data.clone()) else {
        return disposition(
            BrowserResultStatus::Unavailable,
            OperationEffect::None,
            None,
        );
    };
    if flow.steps.iter().any(|step| {
        matches!(
            step.result.effect,
            OperationEffect::Dispatched | OperationEffect::Unknown
        ) || step.status == FlowStepStatus::OutcomeUnknown
            || step.result.status == BrowserResultStatus::OutcomeUnknown
    }) {
        return disposition(
            BrowserResultStatus::OutcomeUnknown,
            OperationEffect::Unknown,
            Some(RetryDisposition::Unsafe),
        );
    }
    let committed = flow
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
        .filter_map(|step| aggregate_terminal_status(step.status, step.result.status))
        .max_by_key(|status| status_priority(*status));
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
    }
    .or(terminal);
    match (committed, typed_terminal) {
        (_, None) => disposition(BrowserResultStatus::Ok, effect, None),
        (true, Some(BrowserResultStatus::Cancelled | BrowserResultStatus::Blocked)) => disposition(
            typed_terminal.expect("matched terminal"),
            effect,
            Some(RetryDisposition::Unsafe),
        ),
        (true, Some(_)) => disposition(
            BrowserResultStatus::Partial,
            effect,
            Some(RetryDisposition::Unsafe),
        ),
        (false, Some(status)) => disposition(status, effect, None),
    }
}

const fn status_priority(status: BrowserResultStatus) -> u8 {
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
) -> Option<BrowserResultStatus> {
    match flow_status {
        FlowStepStatus::Ok => (result_status != BrowserResultStatus::Ok).then_some(result_status),
        FlowStepStatus::NotRun => None,
        FlowStepStatus::Partial => Some(BrowserResultStatus::Partial),
        FlowStepStatus::NotMet => Some(BrowserResultStatus::NotMet),
        FlowStepStatus::Blocked | FlowStepStatus::Denied | FlowStepStatus::WouldDeny => {
            Some(BrowserResultStatus::Blocked)
        }
        FlowStepStatus::Held => Some(BrowserResultStatus::Held),
        FlowStepStatus::AttentionRequired => Some(BrowserResultStatus::AttentionRequired),
        FlowStepStatus::Cancelled => Some(BrowserResultStatus::Cancelled),
        FlowStepStatus::NotDispatched => Some(BrowserResultStatus::NotDispatched),
        FlowStepStatus::OutcomeUnknown => Some(BrowserResultStatus::OutcomeUnknown),
        FlowStepStatus::Unavailable | FlowStepStatus::WouldAllow => {
            Some(BrowserResultStatus::Unavailable)
        }
    }
}

fn is_error_result(result: &Value) -> bool {
    result.get("isError").and_then(Value::as_bool) == Some(true)
}

fn blockers(result: &Value) -> Option<&[Value]> {
    result
        .pointer("/structuredContent/interactionReceipt/blockers")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
}

fn has_blockers(result: &Value) -> bool {
    blockers(result).is_some_and(|blockers| !blockers.is_empty())
}

fn has_blocker(result: &Value, expected: &str) -> bool {
    blockers(result).is_some_and(|blockers| {
        blockers
            .iter()
            .any(|blocker| blocker.get("kind").and_then(Value::as_str) == Some(expected))
    })
}

fn status_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move {
        crate::tool::outcome::ExecutionOutcome::Success {
            result: json!({
                "structuredContent": status_result(
                    Some(ctx.authority_snapshot),
                    ctx.browser.is_connected()
                )
            })
            .into(),
        }
    })
}

fn status_result(
    authority: Option<&crate::hub::authority::AuthoritySnapshot>,
    browser_connected: bool,
) -> Value {
    let capabilities = CAPABILITY_SEMANTICS
        .iter()
        .map(|(capability, semantics)| json!({"id":capability,"semantics":semantics}))
        .collect::<Vec<_>>();
    let operations = DESCRIPTORS
        .iter()
        .map(|descriptor| json!({"operation":descriptor.operation,"requires":descriptor.requires}))
        .collect::<Vec<_>>();
    let (policy_source, mode) = authority.map_or(("none", "open"), |authority| {
        use crate::governance::manifest::source::ManifestOrigin;
        let source = match authority.policy.origin {
            None => "none",
            Some(ManifestOrigin::UserFile | ManifestOrigin::UserEnv) => "user",
            Some(ManifestOrigin::OrgPolicyFile) => "machine",
            Some(ManifestOrigin::Managed) => "managed",
        };
        let mode = if authority.policy.manifest.is_none() {
            "open"
        } else {
            authority
                .policy
                .manifest
                .as_ref()
                .and_then(|manifest| manifest.mode)
                .unwrap_or_else(|| authority.config.governance_mode())
                .as_str()
        };
        (source, mode)
    });
    json!({
        "schema":CONTEXT_RESULT_SCHEMA,
        "capabilities":capabilities,
        "operations":operations,
        "authority":{"policySource":policy_source,"mode":mode},
        "browserConnected":browser_connected,
        "managedGovernance":managed_governance_context(),
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
    json!({
        "active":true,
        "organization":presentation.and_then(|value| value.org_name.as_deref()),
        "policySequence":status.seq,
        "freshness":freshness,
        "staleReason":stale_reason,
        "fetchedAt":fetched_at,
        "rationale":presentation.and_then(|value| value.rationale.as_deref()),
        "contact":presentation
            .and_then(|value| value.contacts.first())
            .map(|value| value.value.as_str()),
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

    #[test]
    fn registry_is_exact_and_unique() {
        assert_eq!(DESCRIPTORS.len(), OperationKind::ALL.len());
        for operation in OperationKind::ALL {
            assert_eq!(descriptor(*operation).operation, *operation);
        }
    }

    #[test]
    fn sequence_requirements_are_the_union_of_children() {
        use ghostlight_transport::operation::{
            ClickArguments, EmptyArguments, OperationTarget, RunSequenceArguments,
        };
        let sequence = Operation::BrowserRunSequence(RunSequenceArguments {
            tab: None,
            steps: vec![
                Operation::BrowserListTabs(EmptyArguments {}),
                Operation::BrowserClick(ClickArguments {
                    target: OperationTarget::parse("Save").expect("target"),
                    tab: None,
                    button: Default::default(),
                    clicks: 1,
                    modifiers: Vec::new(),
                }),
            ],
        });
        assert_eq!(
            requirements(&sequence),
            vec![Capability::Read, Capability::Interact]
        );
    }
}
