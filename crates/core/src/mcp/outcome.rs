// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The pipeline's structured outcome and the async, context-bearing local-handler shape
//! (ADR-0035 Decision 6, PINS.md SS1 + SS2).
//!
//! Split into its own module rather than folded into `browser::directory` (PINS.md SS2's
//! sanctioned fallback placement): `directory.rs` declares itself a PURE module with no
//! dependencies beyond `core`/`std`/`serde_json::Value`, and [`LocalCtx`] must name
//! `Browser`/`ConfigStore`/`Governance`/`Config` to give a local handler what it needs to behave
//! like an ordinary dispatch. Living here instead keeps that purity claim true while still
//! letting `directory::Handler::Local`'s function-pointer variant name these types.
//!
//! [`CallOutcome`] is the pipeline's own honest account of what happened to one tool call,
//! BEFORE it is rendered into an MCP envelope: `pipeline::run_tool_call` returns this; the MCP
//! edge renderer (`pipeline::render_outcome`) maps each variant into today's envelopes
//! byte-identically (PINS.md SS1's table). Orchestrators (`script`, `form_fill`) consume
//! `CallOutcome` directly -- it is the only honest way to know whether a step was denied, held,
//! or genuinely ran, since a denial or hold is rendered as an ordinary successful MCP text
//! result on the wire (deliberately, so a model reads it), indistinguishable from real success
//! by envelope shape alone.

use crate::governance::config::reload::ConfigStore;
use crate::governance::config::Config;
use crate::governance::dispatch::Governance;
use crate::hub::outbound::browser::Browser;
use crate::ToolError;
use serde_json::Value;
use std::sync::Arc;

/// Which pre-dispatch check produced a [`CallOutcome::Denied`] (PINS.md SS1): a governance
/// policy decision (a manifest grant, or the navigate landing re-check, which is also a policy
/// decision), or the always-on sacred-domains never-touch check.
///
/// `pub`, not `pub(crate)` (a deliberate, mechanically-forced widening from PINS.md SS1's
/// literal annotation): `directory::Handler`, `ToolDescriptor`, and `REGISTRY` are already fully
/// `pub` and reachable outside this crate (integration tests under `tests/`), and
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
/// `pipeline::render_outcome` for the byte-identical mapping table. `pub`, not `pub(crate)`: see
/// [`DenialSource`]'s doc comment for why.
pub enum CallOutcome {
    /// The MCP result object (the extension's `{content:[...]}` shape, or a locally built one),
    /// post-processed and wait-note appended. May carry `structuredContent` (ADR-0038).
    Success { result: Value },
    /// A tool execution failure, rendered as an `isError` result at the edge.
    Failure { error: ToolError },
    /// A pre-dispatch denial (governance or sacred): rendered as ordinary successful text.
    Denied {
        message: String,
        source: DenialSource,
    },
    /// A take-the-wheel pause: rendered as ordinary successful text.
    Held { message: String },
}

/// The context one [`crate::browser::directory::Handler::Local`] invocation receives (ADR-0035
/// Decision 6, PINS.md SS2): everything a local handler needs to behave like an ordinary
/// pipeline dispatch -- the browser handle, the live config store, the governance facade, this
/// call's own config snapshot, and its arguments. Deliberately carries no `CallAudit`: a local
/// handler never touches audit directly (PINS.md SS7's borrow-tangle note); the dispatching arm
/// in `pipeline.rs` stamps the record before and after the handler runs.
pub struct LocalCtx<'a> {
    pub browser: &'a Browser,
    pub store: &'a Arc<ConfigStore>,
    pub governance: &'a Governance,
    /// The calling session's guid (ADR-0047 D3), so a local handler that re-enters the pipeline
    /// (`script`, `form_fill`) threads the SAME session identity onto its `Browser::call` envelopes.
    pub guid: &'a str,
    pub config: &'a Config,
    pub args: &'a Value,
    /// The calling session's tighten-only policy overlay (ADR-0060), if it declared one, so a
    /// local handler that re-enters the pipeline (`script`, `form_fill`) subjects its OWN
    /// sub-steps to the same session tier -- an orchestrated sub-call can never escape the
    /// overlay its parent call was bound by.
    pub overlay: Option<&'a crate::governance::overlay::SessionOverlay>,
}

/// A [`crate::browser::directory::Handler::Local`] handler's return type: a boxed, pinned
/// future so the pipeline's own async recursion (pipeline -> local handler -> pipeline, e.g.
/// `script`'s interpreter re-entering `run_tool_call` per step) can be stored behind an
/// ordinary `fn` pointer, since Rust has no native `async fn` pointer type.
pub type LocalFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = CallOutcome> + Send + 'a>>;
