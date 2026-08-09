// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The pipeline's structured outcome and the async, context-bearing local-handler shape
//! (ADR-0035 Decision 6, PINS.md SS1 + SS2).
//!
//! Split from the data-only operation registry because [`LocalCtx`] must name
//! `Browser`/`ConfigStore`/`Governance`/`Config` to give a local handler what it needs to behave
//! like an ordinary dispatch. The registry can therefore remain a declarative operation
//! authority while `operation::registry::Handler::Local` points at these futures.
//!
//! [`CallOutcome`] is the pipeline's own honest account of what happened to one tool call,
//! BEFORE it is rendered into an MCP envelope. Compound operations consume
//! `CallOutcome` directly -- it is the only honest way to know whether a step was denied, held,
//! or genuinely ran, since a denial or hold is rendered as an ordinary successful MCP text
//! result on the wire (deliberately, so a model reads it), indistinguishable from real success
//! by envelope shape alone.

use crate::governance::config::reload::ConfigStore;
use crate::governance::config::Config;
use crate::governance::dispatch::Governance;
use crate::hub::authority::AuthoritySnapshot;
use crate::hub::authority::AuthorityStore;
use crate::hub::outbound::browser::{Browser, DeliveryFailure};
use crate::hub::scheduling::ExecutionContext;
use crate::operation::registry::SuccessDisposition;
use crate::work::{CancellationToken, WorkContext};
use crate::ToolError;
use ghostlight_transport::operation::{
    BrowserResult, Operation, OperationEffect, Readiness, TabFactRedaction,
};
use serde_json::Value;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// Whether an operation uses its descriptor's normal terminal meaning or an explicitly proven
/// exceptional terminal meaning.
#[derive(Debug, Clone, Copy)]
pub enum ExecutionDisposition {
    /// Use the operation descriptor to classify the acknowledged mechanism evidence.
    DescriptorDefault,
    /// Use one operation-owned classification established by orchestration or post-dispatch proof.
    Override(SuccessDisposition),
}

/// Navigation facts established while the operation retains its browser execution lease.
#[derive(Debug, Clone, Default)]
pub struct NavigationCompletion {
    /// Exact document-bound readiness, when the landing was verified.
    pub readiness: Option<Readiness>,
    /// Final authorized URL, when its exact landing identity was verified.
    pub final_url: Option<String>,
}

/// Resolved target facts owned by one operation.
#[derive(Debug, Clone)]
pub enum ResolvedTargets {
    /// The operation does not return target facts.
    None,
    /// One exact resolved target.
    One(Value),
    /// One exact source and destination pair.
    Drag { from: Value, to: Value },
}

/// Content-free facts retained only for operation audit correlation.
#[derive(Debug, Clone, Default)]
pub struct ExecutionAuditFacts {
    /// Correlation for one compound operation's audit children.
    pub batch_id: Option<String>,
    /// Content-free target assurance copied into audit only.
    pub target_assurance: Option<String>,
    /// Content-free interaction outcome copied into audit only.
    pub outcome_category: Option<String>,
}

/// Which pre-dispatch check produced a [`CallOutcome::Denied`] (PINS.md SS1): a governance
/// policy decision (a manifest grant, or the navigate landing re-check, which is also a policy
/// decision), or the always-on sacred-domains never-touch check.
///
/// `pub`, not `pub(crate)` (a deliberate, mechanically-forced widening from PINS.md SS1's
/// literal annotation): `operation::registry::Handler` is public and reachable outside this
/// crate (integration tests under `tests/`), and
/// `Handler::Local`'s function-pointer variant names [`CallOutcome`] (which itself carries this
/// type) directly. A `pub(crate)` `CallOutcome`/`DenialSource` behind a `pub enum Handler`
/// triggers rustc's `private_interfaces` lint, which `-D warnings` promotes to a hard failure.
pub enum DenialSource {
    Policy,
    Sacred,
}

/// Browser evidence retained for exactly one admitted Ghostlight operation.
///
/// The adapter payload is deliberately private mechanism evidence. Service-owned facts that are
/// needed to construct the final [`ghostlight_transport::operation::BrowserResult`] travel in
/// typed fields instead of being hidden inside that JSON under magic marker names. This execution
/// is consumed once by the operation result reducer.
#[derive(Debug, Clone)]
pub struct OperationExecution {
    value: Value,
    /// Exact browser-native tab affected by this operation, when proven.
    pub operation_tab: Option<i64>,
    /// Terminal meaning chosen by the operation lifecycle.
    pub disposition: ExecutionDisposition,
    /// Document-bound facts from navigation finalization.
    pub navigation: NavigationCompletion,
    /// Content-free audit correlation owned by this operation.
    pub audit: ExecutionAuditFacts,
    /// Exact resolved target facts owned by this operation.
    pub targets: ResolvedTargets,
}

/// One browser-native tab fact consumed only by workspace handle binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTabFact {
    /// Browser-native tab identity. It never crosses the owner bridge.
    pub tab_id: i64,
    /// Bounded page URL when the operation is allowed to expose it.
    pub url: Option<String>,
    /// Bounded page title when the operation is allowed to expose it.
    pub title: Option<String>,
    /// Service-authored reason page facts were withheld.
    pub redacted: Option<TabFactRedaction>,
}

/// Typed browser-topology evidence retained until opaque handle binding.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperationTopology {
    /// Exact native tab affected by the operation, when proven.
    pub affected_tab: Option<i64>,
    /// Bounded facts about possible addressed tabs.
    pub candidates: Vec<NativeTabFact>,
    /// Bounded inventory returned by a tab operation.
    pub inventory: Vec<NativeTabFact>,
    /// Native tabs conclusively closed by the operation.
    pub closed_tabs: Vec<i64>,
    /// Final authorized navigation URL, after all landing checks.
    pub final_navigation_url: Option<String>,
}

/// Operation-owned typed completion before workspace handles are bound.
#[derive(Debug, Clone)]
pub struct OperationCompletion {
    /// Closed typed terminal result constructed by the operation executor.
    pub result: BrowserResult,
    /// Typed topology facts consumed by the completion chokepoint.
    pub topology: OperationTopology,
}

impl OperationExecution {
    /// Wrap one policy-free adapter or local-handler payload.
    pub fn new(value: Value) -> Self {
        let operation_tab = value
            .pointer("/structuredContent/tabId")
            .and_then(Value::as_i64);
        Self {
            value,
            operation_tab,
            disposition: ExecutionDisposition::DescriptorDefault,
            navigation: NavigationCompletion::default(),
            audit: ExecutionAuditFacts::default(),
            targets: ResolvedTargets::None,
        }
    }

    /// Borrow the private adapter payload.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Mutably borrow the private adapter payload.
    pub fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    /// Consume the execution and return its private adapter payload.
    pub fn into_value(self) -> Value {
        self.value
    }
}

impl From<Value> for OperationExecution {
    fn from(value: Value) -> Self {
        Self::new(value)
    }
}

impl From<Value> for Box<OperationExecution> {
    fn from(value: Value) -> Self {
        Box::new(OperationExecution::new(value))
    }
}

impl Deref for OperationExecution {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for OperationExecution {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

/// The pipeline's structured account of one tool call's outcome (ADR-0035 Decision 6), before
/// MCP-envelope rendering. `Success`/`Failure` map to today's ordinary/`isError` results;
/// `Denied`/`Held` map to today's successful text-content results (a denial or hold is a
/// successful MCP reply carrying corrective text, never a transport-level error) -- see
/// the exact edge handlers for the revision-specific mappings. `pub`, not `pub(crate)`: see
/// [`DenialSource`]'s doc comment for why.
pub enum ExecutionOutcome {
    /// Exact operation-scoped browser evidence, consumed once into a typed Ghostlight result.
    Success { result: Box<OperationExecution> },
    /// A tool execution failure, rendered as an `isError` result at the edge.
    Failure { error: ToolError },
    /// Queue admission failed before browser dispatch. Retrying is safe when conditions change.
    NotDispatched { message: String },
    /// Bytes reached the browser but no conclusive terminal acknowledgement arrived.
    OutcomeUnknown { message: String },
    /// A pre-dispatch denial (governance or sacred): rendered as ordinary successful text.
    Denied {
        message: String,
        source: DenialSource,
    },
    /// A take-the-wheel pause, kept semantic until the selected edge profile renders it.
    Held { prolonged: bool },
    /// This workspace's denial circuit is open: rendered as ordinary successful text.
    AttentionRequired { message: String },
    /// Cooperative cancellation stopped the call at a typed physical-effect boundary.
    Cancelled {
        message: String,
        /// Proven effect at cancellation: none, committed, or unknown.
        effect: OperationEffect,
    },
}

/// Final account of one Ghostlight operation after its operation-owned result has been built.
///
/// Only this type may cross from the action pipeline into the owner bridge. Adapter evidence and
/// native browser identities are consumed before this boundary.
pub enum CallOutcome {
    /// Completed operation with one closed typed result.
    Success { result: Box<BrowserResult> },
    /// Operation execution failed conclusively.
    Failure { error: ToolError },
    /// Queue admission failed before browser dispatch.
    NotDispatched { message: String },
    /// Bytes reached the browser but no conclusive terminal acknowledgement arrived.
    OutcomeUnknown { message: String },
    /// Governance denied the operation before dispatch or refused a committed landing.
    Denied {
        message: String,
        source: DenialSource,
    },
    /// The user currently controls the browser.
    Held { prolonged: bool },
    /// The workspace requires user attention before more work.
    AttentionRequired { message: String },
    /// Cooperative cancellation reached a typed physical-effect boundary.
    Cancelled {
        message: String,
        effect: OperationEffect,
    },
}

/// Convert one browser-delivery failure into the protocol-neutral terminal disposition.
///
/// Compound handlers use this at the exact effectful sub-call so an ambiguous dispatch stops the
/// composition immediately and is never flattened into an ordinary tool failure.
pub(crate) fn delivery_failure_outcome(failure: DeliveryFailure) -> ExecutionOutcome {
    if failure.outcome_unknown {
        return ExecutionOutcome::OutcomeUnknown {
            message: format!(
                "The browser command may have completed, but Ghostlight did not receive a conclusive terminal acknowledgement. Do not retry automatically; inspect the tab first. ({})",
                failure.error
            ),
        };
    }
    match failure.error {
        ToolError::Held { prolonged } => ExecutionOutcome::Held { prolonged },
        ToolError::AttentionRequired { message } => ExecutionOutcome::AttentionRequired { message },
        error => ExecutionOutcome::Failure { error },
    }
}

/// Convert one conclusive browser mechanism error into its protocol-neutral terminal outcome.
///
/// Raw read and observation mechanisms do not carry delivery ambiguity because they cannot
/// commit the requested page effect. They can still be refused by the final hold or attention
/// admission check. Keep those refusals semantic instead of flattening them into an ordinary
/// extension failure.
pub(crate) fn tool_error_outcome(error: ToolError) -> ExecutionOutcome {
    match error {
        ToolError::Held { prolonged } => ExecutionOutcome::Held { prolonged },
        ToolError::AttentionRequired { message } => ExecutionOutcome::AttentionRequired { message },
        error => ExecutionOutcome::Failure { error },
    }
}

/// The context one [`crate::operation::registry::Handler::Local`] invocation receives (ADR-0035
/// Decision 6, PINS.md SS2): everything a local handler needs to behave like an ordinary
/// pipeline dispatch -- the browser handle, the live config store, the governance facade, this
/// call's own config snapshot, and its arguments. Deliberately carries no `CallAudit`: a local
/// handler never touches audit directly (PINS.md SS7's borrow-tangle note); the dispatching arm
/// in `pipeline.rs` stamps the record before and after the handler runs.
pub struct LocalCtx<'a> {
    pub browser: &'a Browser,
    pub store: &'a Arc<ConfigStore>,
    /// The complete authority slot used by orchestrated sub-steps.
    pub authority: &'a AuthorityStore,
    /// The immutable authority snapshot admitted for this compound call.
    pub authority_snapshot: &'a Arc<AuthoritySnapshot>,
    pub governance: &'a Governance,
    /// The workspace routing key. Browser wire keeps the compatibility spelling `guid`, so a local
    /// handler that re-enters the pipeline (`script`, `form_fill`) threads the SAME workspace onto
    /// its `Browser::call` envelopes.
    pub guid: &'a str,
    pub config: &'a Config,
    /// Ghostlight operation admitted by the registry for this handler invocation.
    pub operation: &'a Operation,
    /// Policy-free browser-mechanism input prepared from the typed operation.
    pub input: &'a Value,
    /// The admitted execution context retained by descriptor-gated compound handlers.
    pub execution: &'a ExecutionContext,
    /// This call's validated tighten-only policy restriction, when present, so a
    /// local handler that re-enters the pipeline (`script`, `form_fill`) subjects its OWN
    /// sub-steps to the same restriction -- an orchestrated sub-call can never escape the
    /// authority ceiling its parent call was bound by.
    pub overlay: Option<&'a crate::governance::overlay::SessionOverlay>,
    /// Immutable service work context for this operation.
    pub work: &'a WorkContext,
    /// Cooperative cancellation for the active bridge work item.
    pub cancellation: &'a CancellationToken,
    /// Workspace membership authority for neutral service work.
    pub workspaces: &'a crate::hub::workspace::WorkspaceRegistry,
}

/// A local operation handler's return type: a boxed, pinned future so the pipeline's
/// own async recursion (pipeline -> flow handler -> pipeline) can be stored behind an
/// ordinary `fn` pointer, since Rust has no native `async fn` pointer type.
pub type LocalFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = ExecutionOutcome> + Send + 'a>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conclusive_safety_refusals_remain_semantic() {
        assert!(matches!(
            tool_error_outcome(ToolError::held(true)),
            ExecutionOutcome::Held { prolonged: true }
        ));
        assert!(matches!(
            tool_error_outcome(ToolError::attention_required("review needed")),
            ExecutionOutcome::AttentionRequired { message } if message == "review needed"
        ));
        assert!(matches!(
            tool_error_outcome(ToolError::extension("offline")),
            ExecutionOutcome::Failure { .. }
        ));
    }
}
