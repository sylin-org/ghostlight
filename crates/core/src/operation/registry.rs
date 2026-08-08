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
use ghostlight_transport::operation::FlowTermination;
use ghostlight_transport::operation::{
    BrowserOperation, BrowserResultStatus, FlowResultData, FlowStepStatus, FlowTerminationReason,
    IntentId, OperationEffect, OperationId, OperationKey, RetryDisposition,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

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
    /// Forward through the browser mechanism compatibility serializer.
    ExtensionForward,
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
    legacy_dispatch_tool: Option<&'static str>,
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

    /// Return the current extension command alias during the bounded R1-R3 migration.
    pub fn legacy_dispatch_tool(&self) -> Option<&'static str> {
        self.legacy_dispatch_tool
    }

    /// Serialize canonical arguments for the bounded pre-mechanism compatibility implementation.
    ///
    /// The returned action discriminator is derived from [`OperationDescriptor::key`]. A caller
    /// cannot alter physical behavior by smuggling a surface action string through arguments.
    pub fn legacy_arguments(&self, arguments: &Value) -> Result<Value, ToolError> {
        encode_legacy_arguments(self.key, arguments)
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

        if self.key.id == OperationId::BrowserAct {
            if has_interaction_blocker(result, "expect_timeout") {
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
            if is_error_success(result) {
                return SuccessDisposition::new(
                    BrowserResultStatus::Blocked,
                    OperationEffect::None,
                    None,
                );
            }
            let committed = result
                .pointer("/structuredContent/filled")
                .and_then(Value::as_array)
                .is_some_and(|filled| !filled.is_empty())
                || result
                    .pointer("/structuredContent/submitted")
                    .and_then(Value::as_bool)
                    == Some(true);
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
    legacy_dispatch_tool: Option<&'static str>,
}

macro_rules! seed {
    ($id:ident, $intent:ident, $schema:expr, $workspace:ident, $requires:expr,
     $resource:ident, $scheduling:expr, $handler:expr, $postprocess:expr,
     $page:ident, $post_dispatch:ident, $effect:ident, $validation:expr, $wire:expr) => {
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
            legacy_dispatch_tool: $wire,
        }
    };
}

const REDACT: Option<fn(&mut Value, bool)> = Some(crate::browser::redact::apply_to_result);

const SEEDS: &[Seed] = &[
    seed!(
        BrowserTabs,
        TabsList,
        0,
        Creates,
        &[Capability::Read],
        DomainLess,
        Scheduling::WORKSPACE_TOPOLOGY,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        None,
        ValidationRule::LegacySchema,
        Some("tabs_context_mcp")
    ),
    seed!(
        BrowserTabs,
        TabsNew,
        1,
        Creates,
        &[],
        DomainLess,
        Scheduling::WORKSPACE_TOPOLOGY,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        Committed,
        ValidationRule::LegacySchema,
        Some("tabs_create_mcp")
    ),
    seed!(
        BrowserTabs,
        TabsFocus,
        19,
        Uses,
        &[],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        None,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "focus"
        },
        Some("tab_control")
    ),
    seed!(
        BrowserTabs,
        TabsClose,
        19,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        None,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "close"
        },
        Some("tab_control")
    ),
    seed!(
        BrowserNavigate,
        NavigateUrl,
        2,
        Uses,
        &[Capability::Read],
        TargetArg,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        NavigateLanding,
        Committed,
        ValidationRule::LegacySchema,
        Some("navigate")
    ),
    seed!(
        BrowserNavigate,
        NavigateBack,
        2,
        Uses,
        &[Capability::Read],
        TargetArg,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        NavigateLanding,
        Committed,
        ValidationRule::ExpectedString {
            field: "url",
            value: "back"
        },
        Some("navigate")
    ),
    seed!(
        BrowserNavigate,
        NavigateForward,
        2,
        Uses,
        &[Capability::Read],
        TargetArg,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        NavigateLanding,
        Committed,
        ValidationRule::ExpectedString {
            field: "url",
            value: "forward"
        },
        Some("navigate")
    ),
    seed!(
        BrowserNavigate,
        NavigateReload,
        19,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        None,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "reload"
        },
        Some("tab_control")
    ),
    seed!(
        BrowserSnapshot,
        SnapshotCapture,
        10,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Text,
        None,
        None,
        ValidationRule::LegacySchema,
        Some("read_page")
    ),
    seed!(
        BrowserRead,
        ReadText,
        6,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        None,
        ValidationRule::LegacySchema,
        Some("get_page_text")
    ),
    seed!(
        BrowserFind,
        FindQuery,
        4,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Text,
        None,
        None,
        ValidationRule::LegacySchema,
        Some("find")
    ),
    seed!(
        BrowserScreenshot,
        ScreenshotViewport,
        3,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        None,
        ValidationRule::ExpectedString {
            field: "action",
            value: "screenshot"
        },
        Some("computer")
    ),
    seed!(
        BrowserScreenshot,
        ScreenshotRegion,
        3,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        None,
        ValidationRule::ExpectedString {
            field: "action",
            value: "zoom"
        },
        Some("computer")
    ),
    seed!(
        BrowserAct,
        ActClick,
        17,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::CanonicalAct {
            action: "left_click"
        },
        None
    ),
    seed!(
        BrowserAct,
        ActRightClick,
        17,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::CanonicalAct {
            action: "right_click"
        },
        None
    ),
    seed!(
        BrowserAct,
        ActDoubleClick,
        17,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::CanonicalAct {
            action: "double_click"
        },
        None
    ),
    seed!(
        BrowserAct,
        ActTripleClick,
        17,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::CanonicalAct {
            action: "triple_click"
        },
        None
    ),
    seed!(
        BrowserAct,
        ActHover,
        17,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::CanonicalAct { action: "hover" },
        None
    ),
    seed!(
        BrowserAct,
        ActScrollIntoView,
        17,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::CanonicalAct {
            action: "scroll_to"
        },
        None
    ),
    seed!(
        BrowserAct,
        ActSetValue,
        17,
        Uses,
        &[Capability::Write],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::act_on::act_on_handler),
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::CanonicalAct {
            action: "set_value"
        },
        None
    ),
    seed!(
        BrowserFill,
        FillField,
        5,
        Uses,
        &[Capability::Write],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::LegacySchema,
        Some("form_input")
    ),
    seed!(
        BrowserFill,
        FillFields,
        16,
        Uses,
        &[Capability::Read, Capability::Write],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::form_fill::form_fill_handler),
        None,
        Structured,
        None,
        Committed,
        ValidationRule::ExpectedBoolean {
            field: "submit",
            value: false
        },
        None
    ),
    seed!(
        BrowserFill,
        FillFieldsAndSubmit,
        16,
        Uses,
        &[Capability::Read, Capability::Write, Capability::Action],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::form_fill::form_fill_handler),
        None,
        Structured,
        None,
        Committed,
        ValidationRule::ExpectedBoolean {
            field: "submit",
            value: true
        },
        None
    ),
    seed!(
        BrowserWait,
        WaitDelay,
        3,
        Uses,
        &[],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        None,
        ValidationRule::ExpectedString {
            field: "action",
            value: "wait"
        },
        Some("computer")
    ),
    seed!(
        BrowserWait,
        WaitUntil,
        14,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        None,
        ValidationRule::LegacySchema,
        Some("wait_for")
    ),
    seed!(
        BrowserFlow,
        FlowExecute,
        15,
        Uses,
        &[],
        DomainLess,
        Scheduling::COMPOSITION,
        Handler::Local(crate::tool::flow::flow_handler),
        None,
        None,
        None,
        None,
        ValidationRule::CanonicalFlow,
        None
    ),
    seed!(
        BrowserFlow,
        FlowPreflight,
        15,
        Uses,
        &[],
        DomainLess,
        Scheduling::COMPOSITION,
        Handler::Local(crate::tool::flow::flow_handler),
        None,
        None,
        None,
        None,
        ValidationRule::CanonicalFlow,
        None
    ),
    seed!(
        BrowserDialog,
        DialogStatus,
        18,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Text,
        None,
        None,
        ValidationRule::ExpectedString {
            field: "action",
            value: "status"
        },
        Some("dialog")
    ),
    seed!(
        BrowserDialog,
        DialogAccept,
        18,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Text,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "accept"
        },
        Some("dialog")
    ),
    seed!(
        BrowserDialog,
        DialogDismiss,
        18,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Text,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "dismiss"
        },
        Some("dialog")
    ),
    seed!(
        BrowserDialog,
        DialogRespond,
        18,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Text,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "respond"
        },
        Some("dialog")
    ),
    seed!(
        BrowserInput,
        InputPointerClick,
        3,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "left_click"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputPointerRightClick,
        3,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "right_click"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputPointerDoubleClick,
        3,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "double_click"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputPointerTripleClick,
        3,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "triple_click"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputPointerHover,
        3,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "hover"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputPointerDrag,
        3,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "left_click_drag"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputTypeText,
        3,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "type"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputPressKey,
        3,
        Uses,
        &[Capability::Action],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "key"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputWheel,
        3,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "scroll"
        },
        Some("computer")
    ),
    seed!(
        BrowserInput,
        InputScrollToOffset,
        3,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "scroll_to"
        },
        Some("computer")
    ),
    seed!(
        BrowserViewport,
        ViewportResizeWindow,
        11,
        Uses,
        &[],
        TabScoped,
        Scheduling::BROWSER,
        Handler::ExtensionForward,
        None,
        None,
        None,
        Committed,
        ValidationRule::LegacySchema,
        Some("resize_window")
    ),
    seed!(
        BrowserUpload,
        UploadClientFiles,
        20,
        Uses,
        &[Capability::Write],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::LegacySchema,
        Some("file_upload")
    ),
    seed!(
        BrowserUpload,
        UploadCapturedArtifact,
        22,
        Uses,
        &[Capability::Write],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::upload_image::upload_image_handler),
        REDACT,
        Receipt,
        None,
        Committed,
        ValidationRule::LegacySchema,
        None
    ),
    seed!(
        BrowserConsole,
        ConsoleRead,
        8,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        None,
        ValidationRule::ExpectedBoolean {
            field: "clear",
            value: false
        },
        Some("read_console_messages")
    ),
    seed!(
        BrowserConsole,
        ConsoleReadAndClear,
        8,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        Committed,
        ValidationRule::ExpectedBoolean {
            field: "clear",
            value: true
        },
        Some("read_console_messages")
    ),
    seed!(
        BrowserNetwork,
        NetworkRead,
        9,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        None,
        ValidationRule::ExpectedBoolean {
            field: "clear",
            value: false
        },
        Some("read_network_requests")
    ),
    seed!(
        BrowserNetwork,
        NetworkReadAndClear,
        9,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        Committed,
        ValidationRule::ExpectedBoolean {
            field: "clear",
            value: true
        },
        Some("read_network_requests")
    ),
    seed!(
        BrowserEvaluate,
        EvaluateJavascript,
        7,
        Uses,
        &[Capability::Execute],
        TabScoped,
        Scheduling::SURFACE,
        Handler::ExtensionForward,
        None,
        Text,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "javascript_exec"
        },
        Some("javascript_tool")
    ),
    seed!(
        BrowserRecord,
        RecordStart,
        23,
        Uses,
        &[Capability::Read],
        TabScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "start_recording"
        },
        None
    ),
    seed!(
        BrowserRecord,
        RecordStop,
        23,
        Uses,
        &[],
        RecordingScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "stop_recording"
        },
        None
    ),
    seed!(
        BrowserRecord,
        RecordStatus,
        23,
        Uses,
        &[],
        RecordingScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        None,
        ValidationRule::ExpectedString {
            field: "action",
            value: "status"
        },
        None
    ),
    seed!(
        BrowserRecord,
        RecordClear,
        23,
        Uses,
        &[],
        RecordingScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "clear"
        },
        None
    ),
    seed!(
        BrowserRecord,
        RecordExport,
        23,
        Uses,
        &[Capability::Read],
        RecordingScoped,
        Scheduling::RETAIN_SURFACE,
        Handler::Local(crate::tool::gif_creator::gif_creator_handler),
        None,
        Structured,
        None,
        Committed,
        ValidationRule::ExpectedString {
            field: "action",
            value: "export"
        },
        None
    ),
    seed!(
        BrowserPresent,
        PresentNarrate,
        13,
        Uses,
        &[],
        DomainLess,
        Scheduling::PRESENTATION,
        Handler::ExtensionForward,
        None,
        None,
        None,
        Committed,
        ValidationRule::LegacySchema,
        Some("narrate")
    ),
    seed!(
        WorkflowPlan,
        PlanUpdate,
        12,
        Independent,
        &[],
        DomainLess,
        Scheduling::LOCAL,
        Handler::Local(crate::tool::update_plan::update_plan_handler),
        None,
        None,
        None,
        None,
        ValidationRule::LegacySchema,
        None
    ),
    seed!(
        BrowserContext,
        ContextDescribe,
        24,
        Independent,
        &[],
        DomainLess,
        Scheduling::LOCAL,
        Handler::Local(explain_handler),
        None,
        None,
        None,
        None,
        ValidationRule::LegacySchema,
        None
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
                legacy_dispatch_tool: seed.legacy_dispatch_tool,
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

/// Temporarily normalize a frozen in-process legacy call into the canonical vocabulary.
///
/// Protocol edges own this translation long term. This compatibility entry remains only until
/// service-local orchestrators stop invoking model-facing tool names.
pub fn decode_legacy_call(name: &str, arguments: &Value) -> Result<BrowserOperation, ToolError> {
    crate::operation::legacy::decode_call(name, arguments)
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
                "tab": { "type": "number" }, "target": target, "value": {}
            }, "required": ["tab", "target", "value"], "additionalProperties": false
        }),
        (OperationId::BrowserFill, IntentId::FillFields | IntentId::FillFieldsAndSubmit) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" },
                "fields": {
                    "type": "array", "minItems": 1,
                    "items": {
                        "type": "object", "properties": {
                            "target": target, "value": {}
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
                "tab": { "type": "number" }, "point": point, "target": target,
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
                "tab": { "type": "number" }, "target": target,
                "files": { "type": "array" }, "paths": { "type": "array" }
            }, "required": ["tab", "target"], "additionalProperties": false
        }),
        (OperationId::BrowserUpload, IntentId::UploadCapturedArtifact) => json!({
            "type": "object", "properties": {
                "tab": { "type": "number" }, "artifact": { "type": "string" },
                "target": target, "point": point, "filename": { "type": "string" }
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
                "tab": { "type": "number" }, "target": target, "point": point,
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
    if key.id == OperationId::BrowserUpload {
        let ref_target = arguments
            .pointer("/target/ref")
            .and_then(Value::as_str)
            .is_some();
        if key.intent == IntentId::UploadClientFiles && !ref_target {
            return Err(ToolError::invalid_request(
                "upload.client_files requires target.ref",
            ));
        }
        if key.intent == IntentId::UploadCapturedArtifact {
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

fn operation_action(intent: IntentId) -> Option<&'static str> {
    match intent {
        IntentId::TabsFocus => Some("focus"),
        IntentId::TabsClose => Some("close"),
        IntentId::NavigateReload => Some("reload"),
        IntentId::ScreenshotViewport => Some("screenshot"),
        IntentId::ScreenshotRegion => Some("zoom"),
        IntentId::ActClick | IntentId::InputPointerClick => Some("left_click"),
        IntentId::ActRightClick | IntentId::InputPointerRightClick => Some("right_click"),
        IntentId::ActDoubleClick | IntentId::InputPointerDoubleClick => Some("double_click"),
        IntentId::ActTripleClick | IntentId::InputPointerTripleClick => Some("triple_click"),
        IntentId::ActHover | IntentId::InputPointerHover => Some("hover"),
        IntentId::ActScrollIntoView | IntentId::InputScrollToOffset => Some("scroll_to"),
        IntentId::ActSetValue => Some("set_value"),
        IntentId::WaitDelay => Some("wait"),
        IntentId::DialogStatus => Some("status"),
        IntentId::DialogAccept => Some("accept"),
        IntentId::DialogDismiss => Some("dismiss"),
        IntentId::DialogRespond => Some("respond"),
        IntentId::InputPointerDrag => Some("left_click_drag"),
        IntentId::InputTypeText => Some("type"),
        IntentId::InputPressKey => Some("key"),
        IntentId::InputWheel => Some("scroll"),
        IntentId::EvaluateJavascript => Some("javascript_exec"),
        IntentId::RecordStart => Some("start_recording"),
        IntentId::RecordStop => Some("stop_recording"),
        IntentId::RecordStatus => Some("status"),
        IntentId::RecordClear => Some("clear"),
        IntentId::RecordExport => Some("export"),
        _ => None,
    }
}

fn rename(object: &mut serde_json::Map<String, Value>, from: &str, to: &str) {
    if let Some(value) = object.remove(from) {
        object.insert(to.to_string(), value);
    }
}

fn target_ref(arguments: &Value) -> Option<Value> {
    arguments.pointer("/target/ref").cloned()
}

fn canonical_fields_to_legacy(
    object: &mut serde_json::Map<String, Value>,
) -> Result<(), ToolError> {
    let fields = object
        .remove("fields")
        .ok_or_else(|| ToolError::invalid_request("canonical fill requires fields"))?;
    if is_deferred_flow_reference(&fields) {
        object.insert("fields".into(), fields);
        return Ok(());
    }
    let fields = fields
        .as_array()
        .cloned()
        .ok_or_else(|| ToolError::invalid_request("canonical fill fields must be an array"))?;
    let mut legacy = serde_json::Map::new();
    for field in fields {
        let query = field
            .pointer("/target/query")
            .and_then(Value::as_str)
            .filter(|query| !query.is_empty())
            .ok_or_else(|| ToolError::invalid_request("fill fields require target.query"))?;
        let value = field
            .get("value")
            .cloned()
            .ok_or_else(|| ToolError::invalid_request("fill fields require value"))?;
        if legacy.insert(query.to_owned(), value).is_some() {
            return Err(ToolError::invalid_request(
                "fill fields cannot contain duplicate target.query values",
            ));
        }
    }
    object.insert("fields".into(), Value::Object(legacy));
    Ok(())
}

fn encode_legacy_arguments(key: OperationKey, arguments: &Value) -> Result<Value, ToolError> {
    let mut object = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| ToolError::invalid_request("canonical arguments must be an object"))?;
    rename(&mut object, "tab", "tabId");
    rename(&mut object, "create_if_empty", "createIfEmpty");
    rename(&mut object, "scope_ref", "ref_id");
    rename(&mut object, "only_errors", "onlyErrors");
    rename(&mut object, "url_pattern", "urlPattern");
    rename(&mut object, "image_id", "imageId");

    match (key.id, key.intent) {
        (OperationId::BrowserNavigate, IntentId::NavigateBack) => {
            object.insert("url".into(), json!("back"));
        }
        (OperationId::BrowserNavigate, IntentId::NavigateForward) => {
            object.insert("url".into(), json!("forward"));
        }
        (OperationId::BrowserAct, _) => {
            object.insert(
                "action".into(),
                json!(operation_action(key.intent).expect("act intent has action")),
            );
        }
        (OperationId::BrowserFill, IntentId::FillField) => {
            if let Some(reference) = target_ref(arguments) {
                object.insert("ref".into(), reference);
            }
            object.remove("target");
        }
        (OperationId::BrowserFill, IntentId::FillFields) => {
            canonical_fields_to_legacy(&mut object)?;
            object.insert("submit".into(), json!(false));
        }
        (OperationId::BrowserFill, IntentId::FillFieldsAndSubmit) => {
            canonical_fields_to_legacy(&mut object)?;
            object.insert("submit".into(), json!(true));
        }
        (OperationId::BrowserWait, IntentId::WaitDelay) => {
            rename(&mut object, "seconds", "duration");
            object.insert("action".into(), json!("wait"));
        }
        (OperationId::BrowserFlow, _) => return encode_legacy_flow(key, arguments),
        (OperationId::BrowserInput, IntentId::InputPointerDrag) => {
            rename(&mut object, "from", "start_coordinate");
            rename(&mut object, "to", "coordinate");
        }
        (OperationId::BrowserInput, IntentId::InputPressKey) => {
            rename(&mut object, "key", "text");
        }
        (OperationId::BrowserInput, IntentId::InputWheel) => {
            rename(&mut object, "point", "coordinate");
            rename(&mut object, "direction", "scroll_direction");
            rename(&mut object, "amount", "scroll_amount");
            if let Some(reference) = target_ref(arguments) {
                object.insert("ref".into(), reference);
            }
            object.remove("target");
        }
        (OperationId::BrowserInput, _) => {
            rename(&mut object, "point", "coordinate");
            if let Some(reference) = target_ref(arguments) {
                object.insert("ref".into(), reference);
            }
            object.remove("target");
        }
        (OperationId::BrowserUpload, IntentId::UploadClientFiles) => {
            if let Some(reference) = target_ref(arguments) {
                object.insert("ref".into(), reference);
            }
            object.remove("target");
            if let Some(Value::Array(files)) = object.get_mut("files") {
                for file in files {
                    if let Some(file) = file.as_object_mut() {
                        rename(file, "mime_type", "mimeType");
                    }
                }
            }
        }
        (OperationId::BrowserUpload, IntentId::UploadCapturedArtifact) => {
            rename(&mut object, "artifact", "imageId");
            rename(&mut object, "point", "coordinate");
            if let Some(reference) = target_ref(arguments) {
                object.insert("ref".into(), reference);
            }
            object.remove("target");
        }
        (OperationId::BrowserEvaluate, IntentId::EvaluateJavascript) => {
            rename(&mut object, "script", "text");
        }
        (OperationId::BrowserRecord, IntentId::RecordExport) => {
            rename(&mut object, "point", "coordinate");
            if let Some(reference) = target_ref(arguments) {
                object.insert("ref".into(), reference);
            }
            object.remove("target");
        }
        _ => {}
    }
    if let Some(action) = operation_action(key.intent) {
        object.insert("action".into(), json!(action));
    }
    if key.intent == IntentId::ConsoleReadAndClear || key.intent == IntentId::NetworkReadAndClear {
        object.insert("clear".into(), json!(true));
    }
    Ok(Value::Object(object))
}

fn legacy_surface_tool(key: OperationKey) -> Option<&'static str> {
    match key.id {
        OperationId::BrowserContext => Some("explain"),
        OperationId::BrowserTabs => Some(match key.intent {
            IntentId::TabsList => "tabs_context_mcp",
            IntentId::TabsNew => "tabs_create_mcp",
            _ => "tab_control",
        }),
        OperationId::BrowserNavigate => Some(if key.intent == IntentId::NavigateReload {
            "tab_control"
        } else {
            "navigate"
        }),
        OperationId::BrowserSnapshot => Some("read_page"),
        OperationId::BrowserRead => Some("get_page_text"),
        OperationId::BrowserFind => Some("find"),
        OperationId::BrowserScreenshot | OperationId::BrowserInput => Some("computer"),
        OperationId::BrowserWait => Some(if key.intent == IntentId::WaitDelay {
            "computer"
        } else {
            "wait_for"
        }),
        OperationId::BrowserAct => Some("act_on"),
        OperationId::BrowserFill => Some(if key.intent == IntentId::FillField {
            "form_input"
        } else {
            "form_fill"
        }),
        OperationId::BrowserDialog => Some("dialog"),
        OperationId::BrowserViewport => Some("resize_window"),
        OperationId::BrowserUpload => Some(if key.intent == IntentId::UploadClientFiles {
            "file_upload"
        } else {
            "upload_image"
        }),
        OperationId::BrowserConsole => Some("read_console_messages"),
        OperationId::BrowserNetwork => Some("read_network_requests"),
        OperationId::BrowserEvaluate => Some("javascript_tool"),
        OperationId::BrowserRecord => Some("gif_creator"),
        OperationId::BrowserPresent => Some("narrate"),
        OperationId::WorkflowPlan => Some("update_plan"),
        _ => None,
    }
}

fn encode_legacy_flow(key: OperationKey, arguments: &Value) -> Result<Value, ToolError> {
    let mut steps = Vec::new();
    for step in arguments["steps"]
        .as_array()
        .expect("canonical flow was validated")
    {
        let operation: ghostlight_transport::operation::BrowserOperation =
            serde_json::from_value(step.clone())
                .map_err(|error| ToolError::invalid_request(error.to_string()))?;
        let descriptor = descriptor(operation.key()).ok_or_else(|| {
            ToolError::invalid_request("canonical flow step uses an unavailable operation")
        })?;
        let tool = legacy_surface_tool(operation.key()).ok_or_else(|| {
            ToolError::invalid_request("canonical flow step has no compatibility executor")
        })?;
        steps.push(json!({
            "tool": tool,
            "args": descriptor.legacy_arguments(&operation.arguments)?
        }));
    }
    let mut result = json!({
        "steps": steps,
        "dry_run": key.intent == IntentId::FlowPreflight
    });
    if let Some(on_error) = arguments.get("on_error") {
        result["onError"] = on_error.clone();
    }
    if let Some(tab) = arguments.get("tab") {
        result["tabId"] = tab.clone();
    }
    if let Some(budget) = arguments.get("budget_ms") {
        result["budget_ms"] = budget.clone();
    }
    Ok(result)
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
        let mut text = crate::browser::directory::explain_text();
        let paths = crate::governance::paths::GovernancePaths::production();
        if paths.managed_bootstrap.exists() {
            if let Some(cache_path) = paths.managed_cache.as_ref() {
                let sidecar = crate::governance::managed::status::sidecar_path(cache_path);
                if let Some(status) = crate::governance::managed::status::read_sidecar(&sidecar) {
                    text.push('\n');
                    text.push_str(&crate::governance::explain::managed_passport(&status));
                }
            }
        }
        crate::tool::outcome::CallOutcome::Success {
            result: json!({ "content": [ { "type": "text", "text": text } ] }),
        }
    })
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

        let fill = super::descriptor(OperationKey::new(
            OperationId::BrowserFill,
            IntentId::FillFields,
        ))
        .expect("fill descriptor");
        assert_eq!(
            fill.legacy_arguments(&json!({"tab":1,"fields":"$prev.fields"}))
                .expect("deferred fields serialize"),
            json!({"tabId":1,"fields":"$prev.fields","submit":false})
        );
    }

    fn representative_legacy_arguments(tool: &str, action: Option<&str>) -> Value {
        match tool {
            "tabs_context_mcp" => json!({"createIfEmpty": false}),
            "tabs_create_mcp" | "explain" => json!({}),
            "navigate" => json!({"tabId": 1, "url": "https://example.com"}),
            "computer" => match action.expect("computer action") {
                "left_click" | "right_click" | "double_click" | "triple_click" | "hover"
                | "scroll_to" => json!({"tabId": 1, "action": action, "coordinate": [1, 2]}),
                "type" => json!({"tabId": 1, "action": action, "text": "hello"}),
                "screenshot" => json!({"tabId": 1, "action": action}),
                "wait" => json!({"tabId": 1, "action": action, "duration": 0.1}),
                "scroll" => json!({
                    "tabId": 1, "action": action, "coordinate": [1, 2],
                    "scroll_direction": "down", "scroll_amount": 1
                }),
                "key" => json!({"tabId": 1, "action": action, "text": "Enter"}),
                "left_click_drag" => json!({
                    "tabId": 1, "action": action,
                    "start_coordinate": [1, 2], "coordinate": [3, 4]
                }),
                "zoom" => json!({"tabId": 1, "action": action, "region": [0, 0, 10, 10]}),
                other => panic!("unhandled computer action {other}"),
            },
            "find" => json!({"tabId": 1, "query": "Save"}),
            "form_input" => json!({"tabId": 1, "ref": "ref_1", "value": "x"}),
            "get_page_text" | "read_page" => json!({"tabId": 1}),
            "javascript_tool" => {
                json!({"tabId": 1, "action": "javascript_exec", "text": "1"})
            }
            "read_console_messages" | "read_network_requests" => json!({"tabId": 1}),
            "resize_window" => json!({"tabId": 1, "width": 800, "height": 600}),
            "update_plan" => json!({"domains": [], "approach": ["inspect"]}),
            "narrate" => json!({"tabId": 1, "text": "Working"}),
            "wait_for" => json!({"tabId": 1}),
            "script" => json!({
                "steps": [{"tool": "find", "args": {"tabId": 1, "query": "Save"}}]
            }),
            "form_fill" => json!({
                "tabId": 1, "fields": {"Email": "a@example.com"},
                "submit": action.is_some()
            }),
            "act_on" => {
                let mut args = json!({
                    "tabId": 1, "action": action, "target": {"ref": "ref_1"}
                });
                if action == Some("set_value") {
                    args["value"] = json!("x");
                }
                args
            }
            "dialog" => {
                let mut args = json!({"tabId": 1, "action": action});
                if action == Some("respond") {
                    args["text"] = json!("yes");
                }
                args
            }
            "tab_control" => json!({"tabId": 1, "action": action}),
            "file_upload" => json!({"tabId": 1, "ref": "ref_1"}),
            "browser_batch" => json!({
                "actions": [{"name": "find", "input": {"tabId": 1, "query": "Save"}}]
            }),
            "upload_image" => json!({"tabId": 1, "imageId": "img_1", "ref": "ref_1"}),
            "gif_creator" => {
                let mut args = json!({"tabId": 1, "action": action});
                if action == Some("export") {
                    args["download"] = json!(true);
                }
                args
            }
            other => panic!("unhandled legacy tool {other}"),
        }
    }

    #[test]
    fn all_52_legacy_variants_decode_to_their_canonical_registry_rows() {
        let mut count = 0;
        for tool in crate::browser::directory::REGISTRY {
            for variant in tool.variants {
                count += 1;
                let legacy = representative_legacy_arguments(tool.tool, variant.action);
                let operation = decode_legacy_call(tool.tool, &legacy).unwrap_or_else(|error| {
                    panic!(
                        "{} {:?} failed to decode: {error}",
                        tool.tool, variant.action
                    )
                });
                assert_eq!(
                    operation.key(),
                    OperationKey::new(variant.operation, variant.intent),
                    "{} {:?} canonical key drifted",
                    tool.tool,
                    variant.action
                );
                let operation_descriptor = super::descriptor(operation.key())
                    .expect("every decoded operation has one registry row");
                operation_descriptor
                    .validate(&operation.arguments)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} {:?} canonical validation failed: {error}",
                            tool.tool, variant.action
                        )
                    });
                if tool.tool != "browser_batch" {
                    let encoded = operation_descriptor
                        .legacy_arguments(&operation.arguments)
                        .expect("compatibility serialization");
                    let decoded = decode_legacy_call(tool.tool, &encoded)
                        .expect("serialized arguments decode again");
                    assert_eq!(
                        decoded, operation,
                        "{} {:?} round trip",
                        tool.tool, variant.action
                    );
                }
            }
        }
        assert_eq!(count, 52);
    }

    #[test]
    fn legacy_ref_computer_actions_converge_on_semantic_act_rows() {
        let cases = [
            ("left_click", IntentId::ActClick),
            ("right_click", IntentId::ActRightClick),
            ("double_click", IntentId::ActDoubleClick),
            ("triple_click", IntentId::ActTripleClick),
            ("hover", IntentId::ActHover),
            ("scroll_to", IntentId::ActScrollIntoView),
        ];
        for (action, intent) in cases {
            let operation = decode_legacy_call(
                "computer",
                &json!({"tabId": 1, "action": action, "ref": "ref_1"}),
            )
            .expect("decode ref action");
            assert_eq!(
                operation.key(),
                OperationKey::new(OperationId::BrowserAct, intent)
            );
            assert_eq!(
                operation.arguments,
                json!({"tab": 1, "target": {"ref": "ref_1"}})
            );
        }
    }

    #[test]
    fn ordinary_success_uses_the_descriptor_effect_and_other_errors_do_not_commit() {
        let descriptor = super::descriptor(OperationKey::new(
            OperationId::BrowserNavigate,
            IntentId::NavigateUrl,
        ))
        .expect("navigate descriptor");
        assert_eq!(
            descriptor.success_disposition_for(&json!({"content": []})),
            SuccessDisposition::new(BrowserResultStatus::Ok, OperationEffect::Committed, None)
        );
        assert_eq!(
            descriptor.success_disposition_for(&json!({"content": [], "isError": true})),
            SuccessDisposition::new(BrowserResultStatus::Partial, OperationEffect::None, None)
        );
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
}
