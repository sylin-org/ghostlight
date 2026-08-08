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
//! BEFORE it is rendered into an MCP envelope: the canonical pipeline returns this; the exact
//! date-named handlers in `ghostlight-mcp-connector` map the neutral terminal variant into their own
//! envelopes. Orchestrators (`script`, `form_fill`) consume
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
use crate::work::{CancellationToken, WorkContext};
use crate::ToolError;
use ghostlight_transport::operation::{BrowserOperation, OperationEffect};
use serde_json::Value;
use std::sync::Arc;

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

/// The pipeline's structured account of one tool call's outcome (ADR-0035 Decision 6), before
/// MCP-envelope rendering. `Success`/`Failure` map to today's ordinary/`isError` results;
/// `Denied`/`Held` map to today's successful text-content results (a denial or hold is a
/// successful MCP reply carrying corrective text, never a transport-level error) -- see
/// the exact edge handlers for the revision-specific mappings. `pub`, not `pub(crate)`: see
/// [`DenialSource`]'s doc comment for why.
pub enum CallOutcome {
    /// The MCP result object (the extension's `{content:[...]}` shape, or a locally built one),
    /// post-processed and wait-note appended. May carry `structuredContent` (ADR-0038).
    Success { result: Value },
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
    /// A take-the-wheel pause: rendered as ordinary successful text.
    Held { message: String },
    /// This workspace's denial circuit is open: rendered as ordinary successful text.
    AttentionRequired { message: String },
    /// Cooperative cancellation stopped the call at a typed physical-effect boundary.
    Cancelled {
        message: String,
        /// Proven effect at cancellation: none, committed, or unknown.
        effect: OperationEffect,
    },
}

/// Convert one browser-delivery failure into the protocol-neutral terminal disposition.
///
/// Compound handlers use this at the exact effectful sub-call so an ambiguous dispatch stops the
/// composition immediately and is never flattened into an ordinary tool failure.
pub(crate) fn delivery_failure_outcome(failure: DeliveryFailure) -> CallOutcome {
    if failure.outcome_unknown {
        return CallOutcome::OutcomeUnknown {
            message: format!(
                "The browser command may have completed, but Ghostlight did not receive a conclusive terminal acknowledgement. Do not retry automatically; inspect the tab first. ({})",
                failure.error
            ),
        };
    }
    match failure.error {
        ToolError::Held { message } => CallOutcome::Held { message },
        ToolError::AttentionRequired { message } => CallOutcome::AttentionRequired { message },
        error => CallOutcome::Failure { error },
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
    /// Canonical operation admitted by the registry for this local handler invocation.
    ///
    /// Compound handlers use this identity and its semantic arguments directly. `args` remains
    /// the bounded legacy implementation shape for local handlers not yet migrated in R1.
    pub operation: &'a BrowserOperation,
    pub args: &'a Value,
    /// The admitted execution context retained by descriptor-gated compound handlers.
    pub execution: &'a ExecutionContext,
    /// This call's validated tighten-only policy restriction, when present, so a
    /// local handler that re-enters the pipeline (`script`, `form_fill`) subjects its OWN
    /// sub-steps to the same restriction -- an orchestrated sub-call can never escape the
    /// authority ceiling its parent call was bound by.
    pub overlay: Option<&'a crate::governance::overlay::SessionOverlay>,
    /// Immutable service work context for protocol-edge calls. Absent only on legacy in-process
    /// compatibility paths during the ADR-0096 cutover.
    pub work: Option<&'a WorkContext>,
    /// Cooperative cancellation for the active bridge work item.
    pub cancellation: Option<&'a CancellationToken>,
    /// Workspace membership authority for neutral service work.
    pub workspaces: Option<&'a crate::hub::workspace::WorkspaceRegistry>,
}

/// A canonical local operation handler's return type: a boxed, pinned future so the pipeline's
/// own async recursion (pipeline -> flow handler -> pipeline) can be stored behind an
/// ordinary `fn` pointer, since Rust has no native `async fn` pointer type.
pub type LocalFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = CallOutcome> + Send + 'a>>;
