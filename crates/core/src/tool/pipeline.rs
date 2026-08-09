// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The protocol-neutral work pipeline (ADR-0024 Decision 2 and ADR-0096). Every per-tool
//! surface branches are replaced by a read of the Ghostlight
//! [`crate::operation::registry::OperationDescriptor`] row; per-operation variance
//! lives in the registry, not here.
//!
//! The pipeline keeps the exact, test-pinned stage order the pre-move `handle_tools_call` had:
//!
//! 1. Config snapshot (one per call, torn never).
//! 2. Params extraction (name, arguments).
//! 3. Registry lookup. Miss: the "Unknown tool" invalid_request result, byte-identical.
//! 4. Action extraction via `descriptor.action_key` (no `name == "computer"`).
//! 5. Requires from the descriptor: THE one lookup per call, feeding both the decision and the
//!    audit `capability` field (ADR-0024 Decision 3).
//! 6. Hold check (unchanged position: before everything, including `Local` handlers).
//! 7. Sacred check: STEP B (current tab) is argument-driven (any call carrying a numeric
//!    `tabId`); STEP C (target host) fires iff the descriptor's resource shape is `TargetArg`.
//!    The empty-list fast path stays.
//! 8. Free-action short-circuit (unchanged: keyed on the looked-up requires) and free-action
//!    `Handler::Local` dispatch, in the position pinned by stage 3.
//! 9. Governance authorization (ADR-0024 Decision 3), with resource resolution driven by the
//!    descriptor's resource shape and skipped entirely when ungoverned or requires is empty.
//! 10. Bounded first-call wait; dispatch via `Handler` (`Mechanism` -> typed browser request).
//! 11. `PostDispatch::NavigateLanding`: the landing re-check and park-on-real-deny (never on
//!     shadow), driven by the marker instead of `name == "navigate"`.
//! 12. Audit completion (ADR-0024 Decision 3), then the `postprocess` hook and wait-note.
//!
//! All-open byte-identity and the zero-cost paths are constraints on every stage: no per-call
//! fixture parse, no resource resolution under all-open, no frames for free actions, shadow
//! mode observably identical to allow.

use crate::browser::mechanism::{
    compile_operation, BrowserAuxiliaryPurpose, MechanismId, MechanismRequest,
};
use crate::browser::pattern::HostOutcome;
use crate::browser::{pattern, resource, sacred};
use crate::governance::config::reload::ConfigStore;
use crate::governance::dispatch::{Gate, Governance, HOLD_HINT_AFTER};
use crate::governance::ports::{Capability, Decision, Denial, EffectiveMode, GoverningResource};
use crate::hub::authority::{AuthoritySnapshot, AuthorityStore};
use crate::hub::outbound::browser::Browser;
use crate::hub::scheduling::{ExecutionClass, ExecutionContext, ScheduleFailure};
use crate::hub::workspace::WorkspaceRegistry;
use crate::operation::registry::{
    self as operation_registry, Handler, OperationDescriptor, OperationResource, PostDispatch,
};
use crate::tool::navigation_readiness::{
    canonical_readiness, take_navigation_evidence, NavigationEvidence, NavigationReadinessPolicy,
    NavigationState,
};
use crate::tool::outcome::{
    delivery_failure_outcome, CallOutcome as CompletedOutcome, DenialSource, ExecutionDisposition,
    ExecutionOutcome as CallOutcome, LocalCtx, OperationExecution,
};
use crate::work::{CancellationToken, WorkContext};
use crate::ToolError;
use ghostlight_transport::operation::{Operation, OperationEffect, OperationKind, Readiness};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) fn schedule_failure_message(error: ScheduleFailure) -> String {
    match error {
        ScheduleFailure::TargetUnavailable { reason } => {
            format!("Browser command was not dispatched: {reason}")
        }
        ScheduleFailure::Overloaded { scope } => format!(
            "Browser command was not dispatched because the {scope} queue is full. Retry later."
        ),
        ScheduleFailure::QueueDeadline => {
            "Browser command was not dispatched before its queue deadline. Retry if it is still needed."
                .to_string()
        }
        ScheduleFailure::AuthorityChanged => {
            "Browser command was not dispatched because policy or configuration changed. Re-evaluate the current state before retrying."
                .to_string()
        }
        ScheduleFailure::Retired { reason } => format!(
            "Browser command was not dispatched because its queue was retired ({reason:?})."
        ),
        ScheduleFailure::ResourceUncertain { command_id } => format!(
            "Browser command was not dispatched because its browser resource is quarantined after command {command_id} had an unknown outcome. Inspect or recreate the affected browser state before continuing."
        ),
        ScheduleFailure::Cancelled => {
            "Browser command was cancelled before it was dispatched.".to_string()
        }
    }
}

fn schedule_failure_outcome(error: ScheduleFailure) -> CallOutcome {
    match error {
        ScheduleFailure::Cancelled => CallOutcome::Cancelled {
            message: schedule_failure_message(ScheduleFailure::Cancelled),
            effect: OperationEffect::None,
        },
        other => CallOutcome::NotDispatched {
            message: schedule_failure_message(other),
        },
    }
}

/// Append the org contact "door" line (ADR-0055 D9 / T6) to a denial message when managed
/// governance is active and the org published a contact. Reads the T2 status sidecar at the fixed
/// production paths; absent a managed bootstrap, sidecar, presentation, or contact the message is
/// returned byte-identical, so the all-open and non-managed denial streams are unchanged. This
/// lives OUTSIDE `src/governance/`, so the a7 arch rules do not constrain the sidecar read here.
/// The pure line renderer stays in [`crate::governance::denial::org_contact_line`].
fn with_org_contact_line(message: String) -> String {
    let paths = crate::governance::paths::GovernancePaths::production();
    if !paths.managed_bootstrap.exists() {
        return message;
    }
    let Some(cache_path) = paths.managed_cache.as_ref() else {
        return message;
    };
    let sidecar = crate::governance::managed::status::sidecar_path(cache_path);
    let Some(status) = crate::governance::managed::status::read_sidecar(&sidecar) else {
        return message;
    };
    let Some(presentation) = status.presentation.as_ref() else {
        return message;
    };
    let Some(contact) = presentation.contacts.first() else {
        return message;
    };
    let line = crate::governance::denial::org_contact_line(
        presentation.org_name.as_deref(),
        &contact.value,
    );
    format!("{message}\n{line}")
}

/// PINS.md SS7's `_batch_id` side channel: an orchestrator's own [`directory::Handler::Local`]
/// handler (`script`, C7) embeds its freshly minted batch id at its `Success` result's top
/// level under this key, because the handler has no way to reach the dispatching arm's own
/// `CallAudit` (SS7's borrow-tangle note). Removes the key in place and returns it so the
/// caller can stamp the PARENT record's `batch_id` before completing it; the client must never
/// see this key on the wire.
fn take_batch_id(outcome: &mut CallOutcome) -> Option<String> {
    let CallOutcome::Success { result } = outcome else {
        return None;
    };
    result.audit.batch_id.take()
}

/// Extract the content-free interaction vocabulary used by audit records. Private side-channel
/// fields are removed before the client sees the result; ordinary extension receipts are read
/// directly. No page text, accessible name, URL, value, selector, or coordinate is copied.
fn take_audit_signals(outcome: &mut CallOutcome) -> (Option<String>, Option<String>) {
    let CallOutcome::Success { result } = outcome else {
        return (None, None);
    };
    take_execution_audit_signals(result)
}

fn take_execution_audit_signals(
    result: &mut OperationExecution,
) -> (Option<String>, Option<String>) {
    let hidden_assurance = result.audit.target_assurance.take();
    let hidden_outcome = result.audit.outcome_category.take();
    let receipt = result.pointer("/structuredContent/interactionReceipt");
    let assurance = hidden_assurance.or_else(|| {
        receipt
            .and_then(|value| value.get("targetAssurance"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let outcome = hidden_outcome.or_else(|| receipt.and_then(derive_receipt_outcome));
    (assurance, outcome)
}

fn derive_receipt_outcome(receipt: &Value) -> Option<String> {
    let blockers = receipt
        .get("blockers")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    if blockers {
        return Some("blocked".to_string());
    }
    let observed = receipt.get("observedAfter")?;
    if observed.get("expectMet").and_then(Value::as_bool) == Some(true) {
        return Some("expect_met".to_string());
    }
    for (field, category) in [
        ("tabFocused", "tab_focused"),
        ("tabReloaded", "tab_reloaded"),
        ("tabClosed", "tab_closed"),
    ] {
        if observed.get(field).and_then(Value::as_bool) == Some(true) {
            return Some(category.to_string());
        }
    }
    let changed = observed
        .get("mutations")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0
        || observed.get("renderAdvanced").and_then(Value::as_bool) == Some(true)
        || observed.get("urlChanged").is_some()
        || observed.get("titleChanged").is_some()
        || observed.get("alertOrStatus").is_some()
        || observed
            .get("changedElements")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
    Some(if changed { "changed" } else { "unchanged" }.to_string())
}

fn stamp_audit_signals(
    audit: &mut crate::governance::dispatch::CallAudit,
    signals: (Option<String>, Option<String>),
) {
    if let Some(assurance) = signals.0 {
        audit.set_target_assurance(&assurance);
    }
    if let Some(outcome) = signals.1 {
        audit.set_outcome(&outcome);
    }
}

/// SS2's free-action dispatch guard: true for a [`directory::Handler::Local`] tool whose
/// `action: None` variant carries an EMPTY requirement set (today: `explain`; C7's `script`
/// joins it). `form_fill` (C10) declares `Read + Write` on its `action: None` variant, so it is
/// never free-local -- it always falls through to grant enforcement and dispatches at the
/// post-grant Local position instead, exactly like a direct mechanism operation.
fn is_free_local_action(
    descriptor: &OperationDescriptor,
    requirements: &[crate::governance::ports::Capability],
) -> bool {
    matches!(descriptor.handler, Handler::Local(_)) && requirements.is_empty()
}

fn completed_local_effect(
    descriptor: &OperationDescriptor,
    outcome: &CallOutcome,
) -> OperationEffect {
    match outcome {
        CallOutcome::Success { result } => {
            match result.disposition {
                ExecutionDisposition::DescriptorDefault => descriptor.success_disposition(result),
                ExecutionDisposition::Override(disposition) => disposition,
            }
            .effect
        }
        CallOutcome::NotDispatched { .. }
        | CallOutcome::Denied { .. }
        | CallOutcome::Held { .. }
        | CallOutcome::AttentionRequired { .. } => OperationEffect::None,
        CallOutcome::Failure { .. } | CallOutcome::OutcomeUnknown { .. } => {
            OperationEffect::Unknown
        }
        CallOutcome::Cancelled { effect, .. } => *effect,
    }
}

fn reentrant_authority_snapshot(
    execution: &ExecutionContext,
    inherited: Option<&Arc<AuthoritySnapshot>>,
    current: Arc<AuthoritySnapshot>,
) -> Result<Arc<AuthoritySnapshot>, ScheduleFailure> {
    if let Some(snapshot) =
        inherited.filter(|snapshot| execution.authority_epoch() == Some(snapshot.epoch))
    {
        return Ok((*snapshot).clone());
    }
    if execution.class() != ExecutionClass::Scheduled
        || execution.authority_epoch() == Some(current.epoch)
    {
        return Ok(current);
    }
    Err(ScheduleFailure::AuthorityChanged)
}

/// The dispatch chokepoint's core (ADR-0024 Decision 2, split out by ADR-0035 Decision 6):
/// everything from the registry lookup through post-dispatch -- per-call config snapshot,
/// schema validation, hold, sacred, free-action/grant enforcement, dispatch (extension or
/// local), landing re-check, postprocess, wait-note -- returning a [`CallOutcome`] instead of
/// rendering an envelope, so compound operations can consume the real outcome of a child without
/// parsing presentation text.
///
/// Stage order (unchanged from the pre-split `handle_tools_call`):
/// 1. Registry lookup. Miss: `CallOutcome::Failure` (the "Unknown tool" message, byte-identical).
/// 2. Typed operation validation. Failure: `CallOutcome::Failure`.
/// 3. The one descriptor lookup and `governance.begin`.
/// 4. Hold check: `CallOutcome::Held`.
/// 5. Protected-host check: `CallOutcome::Denied { source: Sacred }`.
/// 6. Free local-operation dispatch ([`is_free_local_action`]).
///    by ADR-0024 Decision 1 stage 3.
/// 7. Grant enforcement: `CallOutcome::Denied { source: Policy }` on `Gate::Deny`.
/// 8. Bounded first-call wait, then dispatch: `Handler::Local` (non-empty requires) at this
///    post-grant position, else `Browser::call`.
/// 9. `PostDispatch::NavigateLanding`: the landing re-check and park-on-real-deny.
/// 10. `descriptor.postprocess`, the wait-note, and `audit.complete()`.
///
// ADR-0047 D3 originally threaded the browser-wire `guid` through this dispatch seam. ADR-0096
// keeps that wire spelling for WorkspaceId while the neutral entry point receives a `WorkContext`.
/// Execute one Ghostlight operation through the service's single admission pipeline.
pub async fn run_work(
    browser: &Browser,
    store: &Arc<ConfigStore>,
    authority: &AuthorityStore,
    workspaces: &WorkspaceRegistry,
    work: &WorkContext,
    cancellation: &CancellationToken,
) -> CompletedOutcome {
    let outcome =
        run_work_execution(browser, store, authority, workspaces, work, cancellation).await;
    match outcome {
        CallOutcome::Success { result } => {
            let effect = match result.disposition {
                ExecutionDisposition::DescriptorDefault => {
                    operation_registry::descriptor(work.operation_kind())
                        .success_disposition(&result)
                }
                ExecutionDisposition::Override(disposition) => disposition,
            }
            .effect;
            match crate::operation::result::build_operation_completion(
                work.operation(),
                work.workspace().cloned(),
                *result,
            ) {
                Ok(completion) => {
                    let result = crate::hub::completion::bind_operation_completion(
                        work.operation(),
                        work.workspace().cloned(),
                        work.workspace().map(|_| workspaces),
                        completion,
                    );
                    match result.validate_semantics() {
                        Ok(()) => CompletedOutcome::Success {
                            result: Box::new(result),
                        },
                        Err(error) if effect == OperationEffect::None => {
                            CompletedOutcome::Failure {
                                error: ToolError::binary(format!(
                                    "operation result violated its typed contract: {error}"
                                )),
                            }
                        }
                        Err(error) => CompletedOutcome::OutcomeUnknown {
                            message: format!(
                                "The browser operation may have changed the page, but its typed result was invalid. Do not repeat it automatically. ({error})"
                            ),
                        },
                    }
                }
                Err(error) if effect == OperationEffect::None => CompletedOutcome::Failure {
                    error: ToolError::binary(format!(
                        "operation result could not be constructed: {error}"
                    )),
                },
                Err(error) => CompletedOutcome::OutcomeUnknown {
                    message: format!(
                        "The browser operation may have changed the page, but Ghostlight could not construct its typed result. Do not repeat it automatically. ({error})"
                    ),
                },
            }
        }
        CallOutcome::Failure { error } => CompletedOutcome::Failure { error },
        CallOutcome::NotDispatched { message } => CompletedOutcome::NotDispatched { message },
        CallOutcome::OutcomeUnknown { message } => CompletedOutcome::OutcomeUnknown { message },
        CallOutcome::Denied { message, source } => CompletedOutcome::Denied { message, source },
        CallOutcome::Held { prolonged } => CompletedOutcome::Held { prolonged },
        CallOutcome::AttentionRequired { message } => {
            CompletedOutcome::AttentionRequired { message }
        }
        CallOutcome::Cancelled { message, effect } => {
            CompletedOutcome::Cancelled { message, effect }
        }
    }
}

async fn run_work_execution(
    browser: &Browser,
    store: &Arc<ConfigStore>,
    authority: &AuthorityStore,
    workspaces: &WorkspaceRegistry,
    work: &WorkContext,
    cancellation: &CancellationToken,
) -> CallOutcome {
    if let ghostlight_transport::operation::Operation::BrowserRunSequence(arguments) =
        work.operation()
    {
        return crate::tool::flow::run_sequence(
            browser,
            store,
            authority,
            workspaces,
            work,
            cancellation,
            arguments,
        )
        .await;
    }
    let input = match crate::operation::preparation::prepare(
        workspaces,
        work.workspace(),
        work.operation(),
    ) {
        Ok(input) => input,
        Err(error) => {
            return CallOutcome::NotDispatched {
                message: error.to_string(),
            }
        }
    };
    let mut outcome = execute_operation(
        browser,
        store,
        authority,
        workspaces,
        work,
        cancellation,
        &input,
        None,
        None,
    )
    .await;

    if let CallOutcome::Denied { message, .. } = &mut outcome {
        *message = with_org_contact_line(std::mem::take(message));
    }
    outcome
}

#[allow(clippy::too_many_arguments)]
#[doc(hidden)]
async fn execute_operation(
    browser: &Browser,
    store: &Arc<ConfigStore>,
    authority: &AuthorityStore,
    workspaces: &WorkspaceRegistry,
    work: &WorkContext,
    cancellation: &CancellationToken,
    args: &Value,
    inherited_execution: Option<&ExecutionContext>,
    inherited_authority: Option<&Arc<AuthoritySnapshot>>,
) -> CallOutcome {
    let guid = work.routing_key();
    let overlay = work.restriction();
    let operation = work.operation();
    let operation_kind = operation.kind();
    let descriptor = operation_registry::descriptor(operation_kind);
    if cancellation.is_cancelled() {
        return CallOutcome::Cancelled {
            message: "The browser command was cancelled before dispatch.".to_string(),
            effect: OperationEffect::None,
        };
    }
    if let Err(error) = operation.validate() {
        return CallOutcome::Failure {
            error: ToolError::invalid_request(error.to_string()),
        };
    }
    let name = operation_kind.as_str();
    let action = None;
    let requirements = operation_registry::requirements(operation);
    let requirements = requirements.as_slice();
    let lookup = Some(requirements);

    if let (Some(workspace), Some(tab_id)) =
        (work.workspace(), args.get("tab").and_then(Value::as_i64))
    {
        if !workspaces.owns_tab(workspace, tab_id) {
            let snapshot = authority.current();
            let audit =
                snapshot
                    .governance
                    .begin_with_client(name, action, lookup, work.client().cloned());
            let rule = "cross_workspace/unowned_tab";
            let denial = Denial {
                rule: rule.to_string(),
                grant_id: None,
                denial_id: crate::governance::denial::denial_id("", "", rule),
                domain: String::new(),
                message: "unknown tab".to_string(),
            };
            audit.sacred_deny(&denial, None);
            return CallOutcome::Denied {
                message: denial.message,
                source: DenialSource::Sacred,
            };
        }
    }

    // Safety admission bypasses ordinary scheduling. A hold or attention pause is decided and
    // audited immediately; it never occupies a browser resource queue.
    if let Some(held_for) = browser.held_for() {
        let snapshot = authority.current();
        let audit =
            snapshot
                .governance
                .begin_with_client(name, action, lookup, work.client().cloned());
        audit.held();
        return CallOutcome::Held {
            prolonged: held_for >= HOLD_HINT_AFTER,
        };
    }
    let attention_exempt = matches!(
        operation_kind,
        OperationKind::BrowserGetStatus | OperationKind::BrowserRunSequence
    );
    if !attention_exempt {
        if let Some(message) = browser.attention_message(guid) {
            let snapshot = authority.current();
            let audit =
                snapshot
                    .governance
                    .begin_with_client(name, action, lookup, work.client().cloned());
            audit.attention_required();
            return CallOutcome::AttentionRequired { message };
        }
    }

    // ADR-0080: acquire the declared resource before the first URL or governing probe, then
    // capture config and governance together. If authority changes in the tiny interval between
    // admission and capture, release and retry under the new epoch. A retained compound lease
    // keeps the exact authority snapshot admitted for its parent operation.
    let (execution, authority_snapshot) = loop {
        let before = authority.current();
        let scheduling = descriptor.scheduling_for(operation, args);
        let admitted = browser
            .acquire_operation(
                scheduling,
                descriptor.workspace_use,
                guid,
                args,
                before.epoch,
                inherited_execution,
                Some(cancellation),
            )
            .await;
        let execution = match admitted {
            Ok(execution) => execution,
            Err(error) => return schedule_failure_outcome(error),
        };
        let reentrant = inherited_execution.is_some_and(|inherited| {
            execution.command_id().is_some() && execution.command_id() == inherited.command_id()
        });
        if reentrant {
            let snapshot = authority.current();
            match reentrant_authority_snapshot(&execution, inherited_authority, snapshot) {
                Ok(snapshot) => break (execution, snapshot),
                Err(error) => {
                    // A retained lease is immutable. Dropping this clone and reacquiring would
                    // return the same stale lease forever, so fail at this safe step boundary.
                    drop(execution);
                    return schedule_failure_outcome(error);
                }
            }
        }
        let snapshot = authority.current();
        if execution.class() != ExecutionClass::Scheduled
            || execution.authority_epoch() == Some(snapshot.epoch)
        {
            break (execution, snapshot);
        }
        drop(execution);
    };
    let config = authority_snapshot.config.clone();
    let governance = authority_snapshot.governance.as_ref();

    if cancellation.is_cancelled() {
        drop(execution);
        return CallOutcome::Cancelled {
            message: "The browser command was cancelled before dispatch.".to_string(),
            effect: OperationEffect::None,
        };
    }

    // The single per-call requirement lookup is a pure descriptor read. The same slice feeds
    // authorization, audit, attention, and landing checks.
    let resource_shape = descriptor.resource;
    let mut audit = governance.begin_with_client(name, action, lookup, work.client().cloned());

    // ADR-0024 Decision 4: the sacred check and the grant path below share ONE lazily resolved,
    // memoized tab-URL probe per call, keyed on this call's own `tabId` argument, instead of two
    // different mechanisms (the sacred check's former internal `tabs_context_mcp` lookup,
    // deleted, and the grant path's `tab_url_request`). Nothing is probed until the first stage
    // that actually needs it calls `.get()` -- an all-open call, an ungoverned call, a free
    // action, or a call with no `tabId` at all issues zero frames.
    let mut tab_url = LazyTabUrl::new(
        browser,
        guid,
        args.get("tab").and_then(Value::as_i64),
        &execution,
    );

    // The sacred-domains never-touch check (ADR-0018 step 2, g08): always enforced,
    // independent of governance.mode or manifest presence -- RECONCILIATION.md section 1's
    // "always-on carve-out", and ahead of grant evaluation below (g13: "if the sacred-domains
    // check has already landed, leave it in place and ahead of grant evaluation"). STEP A: an
    // empty list (every preset's default) is the byte-identical fast path -- no extension
    // traffic, no parsing, no allocation.
    //
    // ADR-0060/0096: a sacred deny-ceiling composes by UNION across tiers -- this request
    // restriction's own
    // sacred domains are checked alongside the service config's. The owned combined list is built
    // ONLY when the restriction actually contributes sacred entries, so a call with no restriction
    // (or one with no sacred list) keeps the exact borrow-and-fast-path above.
    let sacred_owned: Option<Vec<String>> = overlay
        .map(|o| o.sacred_domains())
        .filter(|s| !s.is_empty())
        .map(|extra| {
            config
                .sacred_domains()
                .iter()
                .chain(extra.iter())
                .cloned()
                .collect()
        });
    let sacred_domains: &[String] = match &sacred_owned {
        Some(v) => v,
        None => config.sacred_domains(),
    };
    let SacredCheck { tab_domain, denial } = if sacred_domains.is_empty() {
        SacredCheck {
            tab_domain: None,
            denial: None,
        }
    } else {
        sacred_check(&mut tab_url, sacred_domains, resource_shape, args).await
    };
    if let Some(denial) = denial {
        audit.sacred_deny(&denial, tab_domain.as_deref());
        let (title, description) = denial_notification("on the never-touch list", &denial.domain);
        let tab_id = args.get("tab").and_then(Value::as_i64);
        let present = browser.observe_denial(
            guid,
            tab_id,
            crate::governance::attention::DenialSignal {
                origin: tab_domain.clone().or_else(|| Some(denial.domain.clone())),
                capabilities: requirements.to_vec(),
                category: crate::governance::attention::DenialCategory::Sacred,
            },
        );
        if present {
            browser.notify(
                tab_id,
                "error",
                Some("lock"),
                &title,
                Some(&description),
                Some(&denial.denial_id),
            );
        }
        return CallOutcome::Denied {
            message: denial.message,
            source: DenialSource::Sacred,
        };
    }

    // Seed the audit domain from the sacred check's own tab resolution (the pre-grant default
    // for an ungoverned/free-action call) unconditionally, so an all-open or free-action allow
    // on a resolvable (non-sacred) tab still carries that tab's host on its record (shared
    // format doc section 6.1). Grant-stage resource resolution below overwrites this with its
    // own resolved host once governed (the two mechanisms resolve the tab independently and
    // deliberately, g08's sacred check and g13's grant check being out-of-scope-for-each-other
    // concerns; see RECONCILIATION.md section 1).
    audit.set_domain(tab_domain.clone());

    // Free actions (ADR-0022 Decision 5 step 2 and Decision 7): an action whose directory
    // requirement is empty provably touches no page and no server, so it is allowed
    // unconditionally -- no resource resolution and no grant scan. This runs AFTER the always-on
    // sacred check (step 1) and BEFORE grant enforcement, which the resource-resolution gate
    // below skips for these tools, so no `tab_url` probe ever fires for them (the sharp case is
    // `computer` `wait`: requirement `[]`, yet it carries a `tabId`). A `Handler::Local` tool
    // whose `action: None` variant requires nothing (`explain`, `update_plan`; C7's `script`) is
    // answered right here, with no extension frame ever produced; a `Handler::Local` tool that
    // does NOT qualify ([`is_free_local_action`] false -- C10's `form_fill`) falls through to
    // grant enforcement below and dispatches at the post-grant Local position instead. Every
    // OTHER free action (`tabs_create_mcp`, `resize_window`, `narrate`, `computer` `wait`) falls
    // through to an ordinary allowed typed mechanism dispatch below, and to
    // `governance.authorize`'s own free-action arm. All are audited as an allow with no grant
    // attribution and a real (not hardcoded) `duration_ms`.
    if is_free_local_action(descriptor, requirements) {
        let Handler::Local(f) = descriptor.handler else {
            unreachable!("is_free_local_action only returns true for Handler::Local");
        };
        let ctx = LocalCtx {
            browser,
            store,
            authority,
            authority_snapshot: &authority_snapshot,
            governance,
            guid,
            config: &config,
            operation,
            input: args,
            execution: &execution,
            overlay,
            work,
            cancellation,
            workspaces,
        };
        let mut outcome = f(ctx).await;
        if let Some(batch_id) = take_batch_id(&mut outcome) {
            audit.set_batch_id(&batch_id);
        }
        stamp_audit_signals(&mut audit, take_audit_signals(&mut outcome));
        audit.complete();
        return outcome;
    }

    // Grant enforcement (g13, ADR-0018 step 3, ADR-0024 Decision 3): resolve the governing
    // resource for this call, then consult the single policy gate. Resource resolution stays
    // gated on being governed with a known, non-empty requirement set; a free action was already
    // allowed above. `governance.authorize` itself is called for every live call that reaches this
    // point. Resolution is shape-driven (ADR-0024 Decision 1's `ResourceShape`) instead of a
    // per-tool name match.
    let config_mode = config.governance_mode();
    // ADR-0060/0096: a request restriction must be able to tighten even when the SERVICE is all-open, so
    // the resource (the tabId->host probe) is resolved when EITHER the service is governed OR an
    // restriction is present. A call with no restriction under an all-open service keeps the exact
    // zero-probe fast path.
    let resolved = if (governance.is_governed() || overlay.is_some()) && !requirements.is_empty() {
        resolve_governing_resource(&mut tab_url, resource_shape, args).await
    } else {
        None
    };
    // The resolved host at decision time, kept for the overlay deny's audit record below (the
    // service path stamps its own via `set_domain`; an overlay deny needs the same host string).
    let domain_str: Option<String> = resolved.as_ref().and_then(|(_, d)| d.clone());
    if let Some((_, domain)) = &resolved {
        audit.set_domain(domain.clone());
    }
    // The post-dispatch flag: only when the pre-check actually ran (a resolved resource) AND the
    // descriptor marks this tool for the navigate landing re-check (today: `navigate` only) --
    // preserving today's exact `name == "navigate"` gating via the marker instead.
    let navigate_post_check = descriptor.post_dispatch == PostDispatch::NavigateLanding
        && (operation_kind != OperationKind::BrowserOpenTab || args.get("url").is_some());
    let resource = resolved.map(|(r, _)| r);
    // ADR-0060/0096: the request restriction's decision for this call, evaluated against the SAME resolved
    // resource (a clone, so the service path still consumes the original). `None` when there is no
    // overlay or no resolved target -- the overlay abstains, leaving the service decision as-is.
    let overlay_decision = match (overlay, &resource) {
        (Some(ov), Some(res)) => {
            Some(ov.decide(name, action, requirements, res.clone(), config_mode))
        }
        _ => None,
    };
    let gate = governance.authorize(&mut audit, resource, config_mode);
    // ADR-0060/0096: intersect the request restriction (deny-overrides). A service Deny already stands and
    // is handled below; a service Proceed becomes Deny iff the overlay denies -- tighten-only,
    // never the reverse. Handled here as an early return (not a shared `match` arm) because
    // recording the deny CONSUMES the audit scope, so the Proceed continuation below must not be
    // reachable on this path. Mirrors the service Deny arm's notify + return.
    if let Gate::Proceed = gate {
        if let Some(crate::governance::ports::Decision::Deny(denial)) = overlay_decision {
            audit.sacred_deny(&denial, domain_str.as_deref());
            let (title, description) =
                denial_notification("outside the granted policy", &denial.domain);
            let tab_id = args.get("tab").and_then(Value::as_i64);
            let present = browser.observe_denial(
                guid,
                tab_id,
                crate::governance::attention::DenialSignal {
                    origin: domain_str.clone().or_else(|| Some(denial.domain.clone())),
                    capabilities: requirements.to_vec(),
                    category: crate::governance::attention::DenialCategory::Policy,
                },
            );
            if present {
                browser.notify(
                    tab_id,
                    "warn",
                    Some("shield"),
                    &title,
                    Some(&description),
                    Some(&denial.denial_id),
                );
            }
            return CallOutcome::Denied {
                message: denial.message,
                source: DenialSource::Policy,
            };
        }
    }
    match gate {
        Gate::Deny { denial } => {
            let (title, description) =
                denial_notification("outside the granted policy", &denial.domain);
            let tab_id = args.get("tab").and_then(Value::as_i64);
            let present = browser.observe_denial(
                guid,
                tab_id,
                crate::governance::attention::DenialSignal {
                    origin: domain_str.clone().or_else(|| Some(denial.domain.clone())),
                    capabilities: requirements.to_vec(),
                    category: crate::governance::attention::DenialCategory::Policy,
                },
            );
            if present {
                browser.notify(
                    tab_id,
                    "warn",
                    Some("shield"),
                    &title,
                    Some(&description),
                    Some(&denial.denial_id),
                );
            }
            return CallOutcome::Denied {
                message: denial.message,
                source: DenialSource::Policy,
            };
        }
        Gate::Proceed => {}
    }

    if cancellation.is_cancelled() {
        audit.complete();
        return CallOutcome::Cancelled {
            message: "The browser command was cancelled before dispatch.".to_string(),
            effect: OperationEffect::None,
        };
    }

    // Bounded first-call wait: a call may race the extension handshake.
    // Wait briefly for the channel instead of failing healthy work (also covers calls
    // arriving during a browser-link reconnect). If the wait times out, `waited` stays `None` and
    // control falls through to dispatch below, which fails fast with the Ghostlight
    // "extension not connected" `ToolError` -- one hop-attributed message, not two to keep in sync.
    let mut waited: Option<Duration> = None;
    if !browser.is_connected() {
        let started = Instant::now();
        let connected = tokio::select! {
            biased;
            _ = cancellation.cancelled() => false,
            connected = browser.wait_connected(Duration::from_millis(config.first_call_wait_ms())) => connected,
        };
        if cancellation.is_cancelled() {
            audit.complete();
            return CallOutcome::Cancelled {
                message: "The browser command was cancelled before dispatch.".to_string(),
                effect: OperationEffect::None,
            };
        }
        if connected {
            waited = Some(started.elapsed());
        } else {
            tracing::warn!(
                tool = name,
                "tools/call failed: extension channel never came up"
            );
        }
    }

    // Post-grant `Handler::Local` dispatch (ADR-0035 Decision 6, PINS.md SS2's second pinned
    // position): reachable only for a Local tool whose `action: None` variant requires
    // something ([`is_free_local_action`] already returned early for the empty-requires case
    // above), so by construction this is the ONLY remaining way a Local tool reaches here --
    // `form_fill` (C10) is the first user. Same audit/postprocess/wait-note flow as
    // typed mechanism dispatch below, minus the navigate-only landing re-check no Local operation
    // declares.
    if let Handler::Local(f) = descriptor.handler {
        let ctx = LocalCtx {
            browser,
            store,
            authority,
            authority_snapshot: &authority_snapshot,
            governance,
            guid,
            config: &config,
            operation,
            input: args,
            execution: &execution,
            overlay,
            work,
            cancellation,
            workspaces,
        };
        let mut outcome = f(ctx).await;
        audit.dispatch_finished();
        if let Some(batch_id) = take_batch_id(&mut outcome) {
            audit.set_batch_id(&batch_id);
        }
        stamp_audit_signals(&mut audit, take_audit_signals(&mut outcome));
        match &outcome {
            CallOutcome::Held { .. } => {
                audit.held();
                return outcome;
            }
            CallOutcome::AttentionRequired { .. } => {
                audit.attention_required();
                return outcome;
            }
            _ => {}
        }
        let operation_tab = match &mut outcome {
            CallOutcome::Success { result } => result
                .operation_tab
                .or_else(|| args.get("tab").and_then(Value::as_i64)),
            _ => None,
        };
        if navigate_post_check {
            let Some(tab_id) = operation_tab else {
                audit.complete();
                return CallOutcome::OutcomeUnknown {
                    message: "Navigation was dispatched, but Ghostlight could not identify its landing tab. Do not replay the navigation automatically."
                        .to_string(),
                };
            };
            if let CallOutcome::Success { result } = &mut outcome {
                match finalize_navigation(
                    browser,
                    governance,
                    overlay,
                    sacred_domains,
                    guid,
                    descriptor,
                    requirements,
                    tab_id,
                    config_mode,
                    &execution,
                    args,
                    result,
                    &mut audit,
                )
                .await
                {
                    NavigationFinalize::Continue => {}
                    NavigationFinalize::Denied {
                        denial,
                        domain,
                        source,
                    } => {
                        audit.landing_deny(&denial, domain.as_deref());
                        let sacred_source = matches!(source, DenialSource::Sacred);
                        let reason = if sacred_source {
                            "on the never-touch list"
                        } else {
                            "outside the granted policy"
                        };
                        let (title, description) = denial_notification(reason, &denial.domain);
                        let present = browser.observe_denial(
                            guid,
                            Some(tab_id),
                            crate::governance::attention::DenialSignal {
                                origin: domain.clone().or_else(|| Some(denial.domain.clone())),
                                capabilities: requirements.to_vec(),
                                category: if sacred_source {
                                    crate::governance::attention::DenialCategory::Sacred
                                } else {
                                    crate::governance::attention::DenialCategory::Policy
                                },
                            },
                        );
                        if present {
                            browser.notify(
                                Some(tab_id),
                                "warn",
                                Some("shield"),
                                &title,
                                Some(&description),
                                Some(&denial.denial_id),
                            );
                        }
                        let mut result = crate::tool::result::text_content(with_org_contact_line(
                            denial.message,
                        ));
                        if let Some(object) = result.as_object_mut() {
                            object.insert("isError".to_string(), Value::Bool(true));
                        }
                        let mut result = OperationExecution::new(result);
                        result.disposition = ExecutionDisposition::Override(
                            crate::operation::registry::SuccessDisposition::new(
                                ghostlight_transport::operation::BrowserResultStatus::Blocked,
                                OperationEffect::Committed,
                                Some(ghostlight_transport::operation::RetryDisposition::AfterStateChange),
                            ),
                        );
                        result.operation_tab = Some(tab_id);
                        return CallOutcome::Success {
                            result: Box::new(result),
                        };
                    }
                    NavigationFinalize::Terminal(terminal) => {
                        if let Err(terminal) = preserve_created_tab_completion(result, terminal) {
                            audit.complete();
                            return terminal;
                        }
                    }
                }
            }
        }
        if let CallOutcome::Success { result } = &mut outcome {
            if let Some(pp) = descriptor.postprocess {
                pp(result, config.secrets_redact());
            }
            if let Some(waited) = waited {
                append_wait_note(result, waited);
            }
            crate::tool::provenance::apply(result, descriptor.page_output, guid);
            append_tab_delta_note(result);
        }
        audit.complete();
        if cancellation.is_cancelled() && !matches!(outcome, CallOutcome::Cancelled { .. }) {
            return CallOutcome::Cancelled {
                message: "Cancellation was observed after local work reached a safe boundary; completed steps remain audited and are not replayed.".to_string(),
                effect: completed_local_effect(descriptor, &outcome),
            };
        }
        return outcome;
    }

    if cancellation.is_cancelled() {
        audit.complete();
        return CallOutcome::Cancelled {
            message: "The browser command was cancelled before dispatch.".to_string(),
            effect: OperationEffect::None,
        };
    }

    debug_assert!(matches!(descriptor.handler, Handler::Mechanism));
    let mut mechanism = match compile_operation(operation_kind, args) {
        Ok(Some(mechanism)) => mechanism,
        Ok(None) => {
            audit.complete();
            return CallOutcome::Failure {
                error: ToolError::invalid_request(format!(
                    "operation {} did not compile a required browser mechanism",
                    descriptor.operation
                )),
            };
        }
        Err(error) => {
            audit.complete();
            return CallOutcome::Failure { error };
        }
    };
    if matches!(
        work.operation(),
        ghostlight_transport::operation::Operation::BrowserNavigate(_)
            | ghostlight_transport::operation::Operation::BrowserGoBack(_)
            | ghostlight_transport::operation::Operation::BrowserGoForward(_)
            | ghostlight_transport::operation::Operation::BrowserReloadPage(_)
            | ghostlight_transport::operation::Operation::BrowserOpenTab(
                ghostlight_transport::operation::OpenTabArguments { url: Some(_), .. }
            )
    ) {
        mechanism.require_canonical_navigation_proof();
    }
    let mut outcome = browser
        .execute_mechanism_with_delivery_outcome(guid, &mechanism, &execution)
        .await
        .map(OperationExecution::new);
    audit.dispatch_finished();

    outcome = match outcome {
        Err(
            failure @ crate::hub::outbound::browser::DeliveryFailure {
                error: ToolError::Held { .. },
                ..
            },
        ) => {
            audit.held();
            return delivery_failure_outcome(failure);
        }
        Err(
            failure @ crate::hub::outbound::browser::DeliveryFailure {
                error: ToolError::AttentionRequired { .. },
                ..
            },
        ) => {
            audit.attention_required();
            return delivery_failure_outcome(failure);
        }
        outcome => outcome,
    };

    if operation_kind == OperationKind::BrowserListTabs {
        if let Ok(result) = &mut outcome {
            govern_tab_inventory_result(result, governance, overlay, sacred_domains, config_mode);
        }
    }

    // A navigation mechanism either returns exact document-bound readiness evidence or follows
    // the covered old-adapter path. Exact evidence is consumed under this retained surface lease
    // and immutable authority snapshot. Every committed document is authorized before its
    // readiness observation is accepted; the adapter's original deadline is never restarted.
    if let Ok(result) = &mut outcome {
        result.operation_tab = result
            .operation_tab
            .or_else(|| args.get("tab").and_then(Value::as_i64));
    }

    if navigate_post_check && outcome.is_ok() {
        if let Ok(result) = outcome.as_mut() {
            let Some(tab_id) = result.operation_tab else {
                audit.complete();
                return CallOutcome::OutcomeUnknown {
                    message: "Navigation was dispatched, but Ghostlight could not identify its landing tab. Do not replay the navigation automatically."
                        .to_string(),
                };
            };
            match finalize_navigation(
                browser,
                governance,
                overlay,
                sacred_domains,
                guid,
                descriptor,
                requirements,
                tab_id,
                config_mode,
                &execution,
                args,
                result,
                &mut audit,
            )
            .await
            {
                NavigationFinalize::Continue => {}
                NavigationFinalize::Denied {
                    denial,
                    domain,
                    source,
                } => {
                    audit.landing_deny(&denial, domain.as_deref());
                    let sacred_source = matches!(source, DenialSource::Sacred);
                    let reason = if sacred_source {
                        "on the never-touch list"
                    } else {
                        "outside the granted policy"
                    };
                    let (title, description) = denial_notification(reason, &denial.domain);
                    let present = browser.observe_denial(
                        guid,
                        Some(tab_id),
                        crate::governance::attention::DenialSignal {
                            origin: domain.clone().or_else(|| Some(denial.domain.clone())),
                            capabilities: requirements.to_vec(),
                            category: if sacred_source {
                                crate::governance::attention::DenialCategory::Sacred
                            } else {
                                crate::governance::attention::DenialCategory::Policy
                            },
                        },
                    );
                    if present {
                        browser.notify(
                            Some(tab_id),
                            "warn",
                            Some("shield"),
                            &title,
                            Some(&description),
                            Some(&denial.denial_id),
                        );
                    }
                    let message = with_org_contact_line(denial.message);
                    let mut result = crate::tool::result::text_content(message);
                    if let Some(object) = result.as_object_mut() {
                        object.insert("isError".to_string(), Value::Bool(true));
                    }
                    let mut result = OperationExecution::new(result);
                    result.disposition = ExecutionDisposition::Override(
                        crate::operation::registry::SuccessDisposition::new(
                            ghostlight_transport::operation::BrowserResultStatus::Blocked,
                            OperationEffect::Committed,
                            Some(
                                ghostlight_transport::operation::RetryDisposition::AfterStateChange,
                            ),
                        ),
                    );
                    result.operation_tab = Some(tab_id);
                    return CallOutcome::Success {
                        result: Box::new(result),
                    };
                }
                NavigationFinalize::Terminal(terminal) => {
                    if let Err(terminal) = preserve_created_tab_completion(result, terminal) {
                        audit.complete();
                        return terminal;
                    }
                }
            }
        }
    }

    if let Ok(result) = &mut outcome {
        stamp_required_dialog_resolution(operation_kind, args, result);
    }

    if let Ok(result) = &mut outcome {
        stamp_audit_signals(&mut audit, take_execution_audit_signals(result));
    }
    if let Ok(result) = &outcome {
        release_closed_workspace_tab(workspaces, work, operation, args, result);
    }
    audit.complete();

    if cancellation.is_cancelled() {
        let effect = match &outcome {
            Ok(result) => descriptor.success_disposition(result).effect,
            Err(_) => OperationEffect::Unknown,
        };
        return CallOutcome::Cancelled {
            message: if outcome
                .as_ref()
                .err()
                .is_some_and(|failure| failure.outcome_unknown)
            {
                "Cancellation arrived after browser dispatch. The effect may have completed; Ghostlight did not replay it and its available audit evidence was retained."
                    .to_string()
            } else {
                "Cancellation arrived after browser dispatch. The atomic operation drained and was audited; Ghostlight did not roll it back or replay it."
                    .to_string()
            },
            effect,
        };
    }

    match outcome {
        // The extension returns an MCP result object (`{ content: [...] }`). The engine is truthful:
        // read_page carries secret field values under a `secret_value=` marker; the governance
        // overlay rewrites that marker here (redacting per `content.security.secrets.redact`) before
        // the result leaves the binary. Other tools pass through untouched. Stage 12 (ADR-0024
        // Decision 1): `descriptor.postprocess` drives this now, replacing `name == "read_page"`.
        Ok(mut result) => {
            if let Some(f) = descriptor.postprocess {
                f(&mut result, config.secrets_redact());
            }
            if let Some(waited) = waited {
                append_wait_note(&mut result, waited);
            }
            crate::tool::provenance::apply(&mut result, descriptor.page_output, guid);
            append_tab_delta_note(&mut result);
            result.operation_tab = result
                .operation_tab
                .or_else(|| args.get("tab").and_then(Value::as_i64));
            CallOutcome::Success {
                result: Box::new(result),
            }
        }
        // A tool execution failure stays a typed engine outcome for the protocol edge to render.
        // The rendered text is exactly the hop-attributed ToolError Display: no "Error: " prefix.
        // NOTE (C2 deviation D-waitnote): today's code additionally appends the wait-note to an
        // error result when `waited` is `Some`; `CallOutcome::Failure` (PINS.md SS1) carries only
        // the bare `ToolError`, with no slot for a pre-rendered note. No test pins this exact
        // combination (extension connects within the handshake grace window, then the dispatched
        // call itself still errors); see LEDGER.md for the full note.
        Err(failure) => delivery_failure_outcome(failure),
    }
}

fn govern_tab_inventory_result(
    result: &mut OperationExecution,
    governance: &Governance,
    restriction: Option<&crate::governance::overlay::SessionOverlay>,
    protected_hosts: &[String],
    config_mode: EffectiveMode,
) {
    let Some(tabs) = result
        .pointer_mut("/structuredContent/tabs")
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for tab in tabs {
        let Some(tab) = tab.as_object_mut() else {
            continue;
        };
        let reason = tab
            .get("url")
            .and_then(Value::as_str)
            .map(|url| {
                tab_page_fact_redaction(url, governance, restriction, protected_hosts, config_mode)
            })
            .unwrap_or(Some("resource_indeterminate"));
        if let Some(reason) = reason {
            tab.remove("url");
            tab.remove("title");
            tab.insert("redacted".to_string(), Value::String(reason.to_string()));
        }
    }
}

fn tab_page_fact_redaction(
    url: &str,
    governance: &Governance,
    restriction: Option<&crate::governance::overlay::SessionOverlay>,
    protected_hosts: &[String],
    config_mode: EffectiveMode,
) -> Option<&'static str> {
    if url.eq_ignore_ascii_case("about:blank") {
        return None;
    }
    match pattern::host_for_matching(url) {
        HostOutcome::Host(host) => {
            if sacred::first_match(&host, protected_hosts).is_some() {
                return Some("protected_host");
            }
        }
        HostOutcome::NonHttpScheme(_) => return Some("resource_indeterminate"),
        HostOutcome::Unparseable => return Some("resource_indeterminate"),
    }

    let resource = resource::resolved_url_resource(url);
    if matches!(
        resource,
        GoverningResource::Indeterminate | GoverningResource::OutOfScope(_)
    ) {
        return Some("resource_indeterminate");
    }
    let requirements = [Capability::Read];
    if matches!(
        governance.decide(
            ghostlight_transport::operation::OperationKind::BrowserListTabs.as_str(),
            None,
            &requirements,
            resource.clone(),
            config_mode,
        ),
        Decision::Deny(_)
    ) {
        return Some("policy");
    }
    if restriction.is_some_and(|restriction| {
        matches!(
            restriction.decide(
                ghostlight_transport::operation::OperationKind::BrowserListTabs.as_str(),
                None,
                &requirements,
                resource,
                config_mode,
            ),
            Decision::Deny(_)
        )
    }) {
        return Some("request_restriction");
    }
    None
}

fn stamp_required_dialog_resolution(
    operation: OperationKind,
    arguments: &Value,
    result: &mut OperationExecution,
) {
    if operation != OperationKind::BrowserHandleDialog
        || arguments.get("require_resolution").and_then(Value::as_bool) != Some(true)
        || result
            .pointer("/structuredContent/open")
            .and_then(Value::as_bool)
            != Some(false)
        || result
            .pointer("/structuredContent/resolved")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return;
    }
    if let Some(structured) = result
        .get_mut("structuredContent")
        .and_then(Value::as_object_mut)
    {
        structured.insert("resolution_not_met".to_string(), Value::Bool(true));
    }
}

fn release_closed_workspace_tab(
    workspaces: &WorkspaceRegistry,
    work: &WorkContext,
    operation: &Operation,
    arguments: &Value,
    result: &Value,
) {
    let Some(workspace) = work.workspace() else {
        return;
    };

    let closed = operation.kind() == OperationKind::BrowserCloseTab
        && result
            .pointer("/structuredContent/interactionReceipt/observedAfter/tabClosed")
            .and_then(Value::as_bool)
            == Some(true);
    if closed {
        if let Some(tab_id) = arguments.get("tab").and_then(Value::as_i64) {
            workspaces.release_tab(workspace, tab_id);
        }
    }
}

/// The on-screen denial notification's title and description (SAPS PRES-HIGH-01): a domain-led
/// headline "Blocked - <domain>" (just "Blocked" when the denial carries no meaningful domain --
/// an unresolved/miss denial's placeholder, `enforcement.rs`'s `""` or `"(unknown)"`, never a
/// real host), paired with a short, direct reason line supplied by the caller. The domain leads
/// because it is the single most scannable fact -- what got blocked -- with the reason as
/// supporting detail. `reason` echoes the vocabulary of the real denial message (e.g. "on the
/// never-touch list", "outside the granted policy") rather than a generic phrase like "access is
/// denied", matching this project's own established denial-text voice.
fn denial_notification(reason: &str, domain: &str) -> (String, String) {
    let title = if domain.is_empty() || domain == "(unknown)" {
        "Blocked".to_string()
    } else {
        format!("Blocked - {domain}")
    };
    (title, reason.to_string())
}

/// Outcome of the sacred-domains check (shared format doc section 3.4, g08).
struct SacredCheck {
    /// The current tab's host at decision time (shared format doc section 6.1 `domain` field),
    /// resolved independently of whether a denial fired -- an allowed call on a clean tab still
    /// carries its `domain` through to the audit record.
    tab_domain: Option<String>,
    /// The denial, if the current tab (STEP B) or, for a `TargetArg`-shaped tool (`navigate`),
    /// the target (STEP C) matched a sacred pattern.
    denial: Option<Denial>,
}

/// STEPs B and C of the sacred-domains check. Only called when the list is non-empty (STEP A,
/// the caller's job). Always enforced, independent of `governance.mode` or manifest presence --
/// RECONCILIATION.md section 1's "always-on carve-out": this runs at the dispatch chokepoint
/// directly, bypassing the grant-based `PolicyDecisionPoint` machinery g12/g13 wire in later
/// (this rule predates and is exempt from that machinery by design, g08 constraint 9).
///
/// STEP B (current-tab check, any tool carrying a numeric `tabId`) runs first, so a sacred
/// current tab denies with the tab's host in the message even for `navigate` (never-touch means
/// the user, not the agent, moves that tab) -- this is ARGUMENT-driven, independent of
/// `resource_shape`, because tool arguments are not schema-validated and a never-touch check must
/// never be gated by a classification that could itself be wrong for a malformed call. STEP C
/// (the target host) fires iff `resource_shape` is [`ResourceShape::TargetArg`]
/// (today: `navigate` only, ADR-0024 Decision 1), even when STEP B could not resolve the tab,
/// since it is local and needs no extension. STEP B reads the tab's URL through the shared
/// `tab_url` cell (ADR-0024 Decision 4), the SAME probe the grant path below reuses, rather than
/// its own internal lookup.
async fn sacred_check(
    tab_url: &mut LazyTabUrl<'_>,
    sacred_domains: &[String],
    resource_shape: OperationResource,
    args: &Value,
) -> SacredCheck {
    let tab_host = match args.get("tab").and_then(Value::as_i64).filter(|_| {
        matches!(
            resource_shape,
            OperationResource::CurrentTab
                | OperationResource::CurrentAndLandings
                | OperationResource::TargetAndLandings
        )
    }) {
        Some(_) => tab_url
            .get()
            .await
            .and_then(|url| match pattern::host_for_matching(&url) {
                HostOutcome::Host(h) => Some(h),
                HostOutcome::NonHttpScheme(_) | HostOutcome::Unparseable => None,
            }),
        None => None,
    };
    let tab_domain = tab_host.as_ref().map(|h| h.as_str().to_string());

    if let Some(host) = &tab_host {
        if let Some(pattern) = sacred::first_match(host, sacred_domains) {
            return SacredCheck {
                tab_domain,
                denial: Some(sacred::sacred(host.as_str(), pattern)),
            };
        }
    }

    if matches!(
        resource_shape,
        OperationResource::OptionalTargetUrl | OperationResource::TargetAndLandings
    ) {
        if let Some(target_host) = args
            .get("url")
            .and_then(Value::as_str)
            .and_then(sacred::navigate_target_host)
        {
            if let Some(pattern) = sacred::first_match(&target_host, sacred_domains) {
                return SacredCheck {
                    tab_domain,
                    denial: Some(sacred::sacred(target_host.as_str(), pattern)),
                };
            }
        }
    }

    SacredCheck {
        tab_domain,
        denial: None,
    }
}

/// Resolve the g13 governing resource for one call (section 5's summary table), shape-driven
/// (ADR-0024 Decision 1's [`ResourceShape`]) instead of a per-tool name match. Only
/// called once [`Governance::is_governed`] is true. Returns `None` only for an unparseable
/// `TargetArg` (`navigate`) target: nothing to govern (section 4: "dispatch without pre- or
/// post-check"). Otherwise `Some((resource, domain))`, where `domain` is the resolved host for
/// the audit record's `domain` field when `resource` is [`GoverningResource::Resource`], `None`
/// otherwise (shared format doc section 6.1: never the denial message's `(unknown)` placeholder).
/// `TabScoped` resolution reads the tab's URL through the shared `tab_url` cell (ADR-0024
/// Decision 4), the SAME probe the sacred check above may already have resolved for this call.
async fn resolve_governing_resource(
    tab_url: &mut LazyTabUrl<'_>,
    resource_shape: OperationResource,
    args: &Value,
) -> Option<(GoverningResource, Option<String>)> {
    match resource_shape {
        OperationResource::None | OperationResource::TabInventory => {
            Some((GoverningResource::None, None))
        }
        OperationResource::OptionalTargetUrl | OperationResource::TargetAndLandings => {
            match args.get("url").and_then(Value::as_str) {
                // "back"/"forward" and a missing/non-string url argument have no target to check
                // pre-dispatch (point 5 covers the landing for "back"/"forward"; the extension's own
                // handling covers a missing url). The union rule (no host, tool/access still apply)
                // is the closest faithful fit: it is never more permissive than a resolved host would
                // be, and it does not require inventing a bypass-everything resource variant.
                Some("back") | Some("forward") | None => Some((GoverningResource::None, None)),
                Some(url_arg) => match resource::navigate_target_resource(url_arg) {
                    Some(GoverningResource::Resource(host)) => {
                        Some((GoverningResource::Resource(host.clone()), Some(host)))
                    }
                    Some(other) => Some((other, None)),
                    None => None,
                },
            }
        }
        OperationResource::CurrentTab | OperationResource::CurrentAndLandings => {
            if args.get("tab").and_then(Value::as_i64).is_none() {
                // Missing/non-integer tabId on a tab-scoped tool: fail closed (constraint 11).
                return Some((GoverningResource::Indeterminate, None));
            }
            let resolved = match tab_url.get().await {
                Some(url) => resource::resolved_url_resource(&url),
                None => GoverningResource::Indeterminate,
            };
            let domain = match &resolved {
                GoverningResource::Resource(h) => Some(h.clone()),
                _ => None,
            };
            Some((resolved, domain))
        }
    }
}

enum NavigationFinalize {
    Continue,
    Denied {
        denial: Denial,
        domain: Option<String>,
        source: DenialSource,
    },
    Terminal(CallOutcome),
}

struct LandingVerdict {
    decision: Decision,
    domain: Option<String>,
    source: DenialSource,
}

#[allow(clippy::too_many_arguments)]
async fn finalize_navigation(
    browser: &Browser,
    governance: &Governance,
    overlay: Option<&crate::governance::overlay::SessionOverlay>,
    sacred_domains: &[String],
    guid: &str,
    descriptor: &OperationDescriptor,
    requirements: &[Capability],
    tab_id: i64,
    config_mode: EffectiveMode,
    execution: &ExecutionContext,
    arguments: &Value,
    result: &mut OperationExecution,
    audit: &mut crate::governance::dispatch::CallAudit,
) -> NavigationFinalize {
    let policy = NavigationReadinessPolicy::from_arguments(arguments);
    debug_assert!(policy.min_ms <= policy.timeout_ms);
    debug_assert!((1..=30_000).contains(&policy.timeout_ms));
    let initial = match take_navigation_evidence(result) {
        Ok(initial) => initial,
        Err(reason) => {
            park_navigation_for_safety(browser, guid, tab_id).await;
            return NavigationFinalize::Terminal(CallOutcome::OutcomeUnknown {
                message: format!(
                    "Navigation was dispatched, but its committed-document evidence was invalid ({reason}). Ghostlight parked the tab; do not replay the navigation automatically."
                ),
            });
        }
    };

    let Some(mut evidence) = initial else {
        // Covered older adapters finish their one legacy load wait before replying. Do not stack
        // a second settle wait. Re-check the final URL, then report that exact-document readiness
        // evidence was unavailable.
        if governance.is_governed() || overlay.is_some() || !sacred_domains.is_empty() {
            let url = browser
                .tab_url(guid, tab_id, execution)
                .await
                .ok()
                .flatten();
            let verdict = landing_verdict(
                governance,
                overlay,
                sacred_domains,
                descriptor.operation,
                requirements,
                url.as_deref(),
                config_mode,
            );
            if let Some(denied) = apply_landing_verdict(browser, guid, tab_id, verdict, audit).await
            {
                return denied;
            }
        }
        let readiness = if policy.settle {
            canonical_readiness(NavigationState::Unavailable, 0)
        } else {
            canonical_readiness(NavigationState::NotRequested, 0)
        }
        .expect("terminal readiness state must convert");
        stamp_canonical_readiness(result, &readiness);
        return NavigationFinalize::Continue;
    };

    let transaction_token = evidence.navigation_token.clone();
    let deadline_at_ms = evidence.deadline_at_ms;
    let mut previous_elapsed_ms = 0;
    let mut committed_document: Option<String> = None;
    let mut committed_url: Option<String> = None;

    for _ in 0..32 {
        if evidence.navigation_token != transaction_token
            || evidence.deadline_at_ms != deadline_at_ms
            || evidence.elapsed_ms < previous_elapsed_ms
        {
            if committed_document.is_none() {
                park_navigation_for_safety(browser, guid, tab_id).await;
                return NavigationFinalize::Terminal(CallOutcome::OutcomeUnknown {
                    message: "Navigation was dispatched, but its readiness transaction changed identity before any committed document was proven. Ghostlight parked the tab; do not replay the navigation automatically."
                        .to_string(),
                });
            }
            return committed_navigation_integrity_partial(
                browser,
                guid,
                tab_id,
                result,
                "Navigation committed, but its readiness transaction changed identity. Do not replay the navigation automatically.",
            )
            .await;
        }
        previous_elapsed_ms = evidence.elapsed_ms;

        if evidence.elapsed_ms > policy.timeout_ms
            && matches!(
                evidence.state,
                NavigationState::Ready | NavigationState::NotRequested
            )
        {
            if committed_document.is_some() {
                return committed_navigation_integrity_partial(
                    browser,
                    guid,
                    tab_id,
                    result,
                    "Navigation readiness claimed a successful state after the original deadline. Do not replay the navigation automatically.",
                )
                .await;
            }
            park_navigation_for_safety(browser, guid, tab_id).await;
            return NavigationFinalize::Terminal(CallOutcome::OutcomeUnknown {
                message: "Navigation was dispatched, but its adapter reported impossible post-deadline evidence. Ghostlight parked the tab; do not replay it automatically."
                    .to_string(),
            });
        }

        let candidate_readiness = match evidence.state {
            NavigationState::LandingUnknown => {
                park_navigation_for_safety(browser, guid, tab_id).await;
                if committed_document.is_some() {
                    return committed_navigation_partial(
                        result,
                        None,
                        "Navigation committed, but a later landing could not be identified. Ghostlight parked the tab and will not replay the navigation automatically.",
                    );
                }
                return NavigationFinalize::Terminal(CallOutcome::OutcomeUnknown {
                    message: "Navigation was dispatched, but its committed landing could not be identified. Ghostlight parked the tab; do not replay the navigation automatically."
                        .to_string(),
                });
            }
            NavigationState::Committed => {
                let (Some(document), Some(url)) =
                    (evidence.document_handle.clone(), evidence.url.as_deref())
                else {
                    park_navigation_for_safety(browser, guid, tab_id).await;
                    return NavigationFinalize::Terminal(CallOutcome::OutcomeUnknown {
                        message: "Navigation was dispatched without a usable committed-document proof. Ghostlight parked the tab; do not replay it automatically."
                            .to_string(),
                    });
                };
                let verdict = landing_verdict(
                    governance,
                    overlay,
                    sacred_domains,
                    descriptor.operation,
                    requirements,
                    Some(url),
                    config_mode,
                );
                if let Some(denied) =
                    apply_landing_verdict(browser, guid, tab_id, verdict, audit).await
                {
                    return denied;
                }
                committed_document = Some(document);
                committed_url = Some(url.to_owned());
                if !policy.settle {
                    canonical_readiness(NavigationState::NotRequested, evidence.elapsed_ms)
                        .expect("not-requested readiness is terminal")
                } else {
                    evidence = match navigation_follow_up(
                        browser,
                        guid,
                        execution,
                        MechanismId::NavigationAwaitReadiness,
                        tab_id,
                        &evidence,
                    )
                    .await
                    {
                        Ok(next) => next,
                        Err(message) => {
                            return committed_navigation_integrity_partial(
                                browser, guid, tab_id, result, &message,
                            )
                            .await;
                        }
                    };
                    continue;
                }
            }
            NavigationState::Ready
            | NavigationState::TimedOut
            | NavigationState::Unavailable
            | NavigationState::NotRequested => {
                if committed_document.is_none() {
                    let (Some(document), Some(url)) =
                        (evidence.document_handle.clone(), evidence.url.as_deref())
                    else {
                        park_navigation_for_safety(browser, guid, tab_id).await;
                        return NavigationFinalize::Terminal(CallOutcome::OutcomeUnknown {
                            message: "Navigation reached its deadline without a proven committed document. Ghostlight parked the tab; do not replay it automatically."
                                .to_string(),
                        });
                    };
                    let verdict = landing_verdict(
                        governance,
                        overlay,
                        sacred_domains,
                        descriptor.operation,
                        requirements,
                        Some(url),
                        config_mode,
                    );
                    if let Some(denied) =
                        apply_landing_verdict(browser, guid, tab_id, verdict, audit).await
                    {
                        return denied;
                    }
                    committed_document = Some(document);
                    committed_url = Some(url.to_owned());
                } else if (evidence.document_handle.is_some() || evidence.url.is_some())
                    && (evidence.document_handle.as_deref() != committed_document.as_deref()
                        || evidence.url.as_deref() != committed_url.as_deref())
                {
                    return committed_navigation_integrity_partial(
                        browser,
                        guid,
                        tab_id,
                        result,
                        "Navigation readiness changed the authorized document identity without a committed transition. Do not replay the navigation automatically.",
                    )
                    .await;
                }
                canonical_readiness(evidence.state, evidence.elapsed_ms)
                    .expect("adapter terminal state must produce canonical readiness")
            }
            NavigationState::Same => {
                if committed_document.is_none() {
                    park_navigation_for_safety(browser, guid, tab_id).await;
                    return NavigationFinalize::Terminal(CallOutcome::OutcomeUnknown {
                        message: "Navigation returned document verification before any committed document was proven. Ghostlight parked the tab; do not replay the navigation automatically."
                            .to_string(),
                    });
                }
                return committed_navigation_integrity_partial(
                    browser,
                    guid,
                    tab_id,
                    result,
                    "Navigation returned an out-of-order document verification. Do not replay the navigation automatically.",
                )
                .await;
            }
        };

        let Some(document_handle) = committed_document.as_deref() else {
            park_navigation_for_safety(browser, guid, tab_id).await;
            return NavigationFinalize::Terminal(CallOutcome::OutcomeUnknown {
                message: "Navigation did not produce a committed document. Ghostlight parked the tab; do not replay it automatically."
                    .to_string(),
            });
        };
        let verify_seed = NavigationEvidence {
            state: NavigationState::Committed,
            navigation_token: transaction_token.clone(),
            document_handle: Some(document_handle.to_owned()),
            url: evidence.url.clone(),
            deadline_at_ms,
            elapsed_ms: previous_elapsed_ms,
        };
        let verified = match navigation_follow_up(
            browser,
            guid,
            execution,
            MechanismId::NavigationVerifyDocument,
            tab_id,
            &verify_seed,
        )
        .await
        {
            Ok(verified) => verified,
            Err(message) => {
                return committed_navigation_integrity_partial(
                    browser, guid, tab_id, result, &message,
                )
                .await
            }
        };
        if verified.navigation_token != transaction_token
            || verified.deadline_at_ms != deadline_at_ms
            || verified.elapsed_ms < previous_elapsed_ms
        {
            return committed_navigation_integrity_partial(
                browser,
                guid,
                tab_id,
                result,
                "Navigation verification changed transaction identity. Do not replay the navigation automatically.",
            )
            .await;
        }
        match verified.state {
            NavigationState::Same => {
                if verified.document_handle.as_deref() != Some(document_handle)
                    || verified.url.as_deref() != committed_url.as_deref()
                {
                    return committed_navigation_integrity_partial(
                        browser,
                        guid,
                        tab_id,
                        result,
                        "Navigation verification returned a different document identity without a commit. Do not replay the navigation automatically.",
                    )
                    .await;
                }
                stamp_canonical_navigation_final_url(result, committed_url.as_deref());
                stamp_canonical_readiness(result, &candidate_readiness);
                return NavigationFinalize::Continue;
            }
            NavigationState::Committed => {
                evidence = verified;
                continue;
            }
            NavigationState::Unavailable => {
                let readiness = canonical_readiness(
                    if policy.settle {
                        NavigationState::Unavailable
                    } else {
                        NavigationState::NotRequested
                    },
                    verified.elapsed_ms,
                )
                .expect("verification unavailability is terminal readiness");
                stamp_canonical_navigation_final_url(result, committed_url.as_deref());
                stamp_canonical_readiness(result, &readiness);
                return NavigationFinalize::Continue;
            }
            NavigationState::LandingUnknown => {
                park_navigation_for_safety(browser, guid, tab_id).await;
                return committed_navigation_partial(
                    result,
                    None,
                    "Navigation committed, but final document verification lost the actual landing. Ghostlight parked the tab and will not replay the navigation automatically.",
                );
            }
            _ => {
                return committed_navigation_integrity_partial(
                    browser,
                    guid,
                    tab_id,
                    result,
                    "Navigation verification returned an invalid terminal state. Do not replay the navigation automatically.",
                )
                .await;
            }
        }
    }

    committed_navigation_integrity_partial(
        browser,
        guid,
        tab_id,
        result,
        "Navigation exceeded the bounded committed-document journal. Do not replay it automatically.",
    )
    .await
}

fn committed_navigation_partial(
    result: &mut OperationExecution,
    final_url: Option<&str>,
    message: &str,
) -> NavigationFinalize {
    stamp_canonical_navigation_final_url(result, final_url);
    if final_url.is_none() {
        clear_unverified_navigation_page_facts(result);
    }
    if let Some(object) = result.as_object_mut() {
        object.insert("isError".into(), Value::Bool(true));
        if let Some(content) = object.get_mut("content").and_then(Value::as_array_mut) {
            content.push(json!({"type":"text","text":message}));
        }
    }
    result.disposition =
        ExecutionDisposition::Override(crate::operation::registry::SuccessDisposition::new(
            ghostlight_transport::operation::BrowserResultStatus::Partial,
            OperationEffect::Committed,
            Some(ghostlight_transport::operation::RetryDisposition::Unsafe),
        ));
    NavigationFinalize::Continue
}

fn preserve_created_tab_completion(
    result: &mut OperationExecution,
    terminal: CallOutcome,
) -> Result<(), CallOutcome> {
    let created = result
        .pointer("/structuredContent/created")
        .and_then(Value::as_bool)
        == Some(true);
    match (created, terminal) {
        (true, CallOutcome::OutcomeUnknown { message }) => {
            let _ = committed_navigation_partial(result, None, &format!(
                "The new tab was created, but its requested landing could not be verified. {message}"
            ));
            Ok(())
        }
        (_, terminal) => Err(terminal),
    }
}

async fn committed_navigation_integrity_partial(
    browser: &Browser,
    guid: &str,
    tab_id: i64,
    result: &mut OperationExecution,
    message: &str,
) -> NavigationFinalize {
    park_navigation_for_safety(browser, guid, tab_id).await;
    committed_navigation_partial(
        result,
        None,
        &format!(
            "{message} Ghostlight parked the tab because its actual landing could not be verified."
        ),
    )
}

fn clear_unverified_navigation_page_facts(result: &mut OperationExecution) {
    if let Some(data) = result
        .get_mut("structuredContent")
        .and_then(Value::as_object_mut)
    {
        data.remove("url");
        data.remove("title");
    }
    result.navigation.final_url = None;
}

fn stamp_canonical_navigation_final_url(result: &mut OperationExecution, final_url: Option<&str>) {
    result.navigation.final_url = final_url.map(str::to_owned);
}

fn stamp_canonical_readiness(result: &mut OperationExecution, readiness: &Readiness) {
    result.navigation.readiness = Some(readiness.clone());
}

async fn navigation_follow_up(
    browser: &Browser,
    guid: &str,
    execution: &ExecutionContext,
    mechanism_id: MechanismId,
    tab_id: i64,
    evidence: &NavigationEvidence,
) -> Result<NavigationEvidence, String> {
    let document_handle = evidence.document_handle.as_deref().ok_or_else(|| {
        "Navigation readiness follow-up lacked a committed document. Do not replay the navigation automatically."
            .to_string()
    })?;
    let request = MechanismRequest::for_auxiliary(
        BrowserAuxiliaryPurpose::NavigationReadiness,
        mechanism_id,
        json!({
            "tab": tab_id,
            "navigation_token": evidence.navigation_token,
            "document_handle": document_handle,
        }),
    )
    .expect("navigation readiness follow-up is declared by its auxiliary plan");
    let mut response = browser
        .execute_mechanism_with_delivery_outcome(guid, &request, execution)
        .await
        .map_err(|failure| {
            format!(
                "Navigation committed, but readiness observation could not complete ({}). Do not replay the navigation automatically.",
                failure.error
            )
        })?;
    take_navigation_evidence(&mut response)
        .map_err(|reason| format!("Navigation readiness evidence was invalid ({reason}). Do not replay the navigation automatically."))?
        .ok_or_else(|| {
            "The connected browser adapter does not retain this navigation readiness transaction. Do not replay the navigation automatically."
                .to_string()
        })
}

fn landing_verdict(
    governance: &Governance,
    overlay: Option<&crate::governance::overlay::SessionOverlay>,
    sacred_domains: &[String],
    operation: OperationKind,
    requires: &[Capability],
    url: Option<&str>,
    config_mode: EffectiveMode,
) -> LandingVerdict {
    let resolved = url
        .map(resource::resolved_url_resource)
        .unwrap_or(GoverningResource::Indeterminate);
    let domain = match &resolved {
        GoverningResource::Resource(host) => Some(host.clone()),
        _ => None,
    };
    let sacred_host = url.and_then(|url| match pattern::host_for_matching(url) {
        HostOutcome::Host(host) => Some(host),
        HostOutcome::NonHttpScheme(_) | HostOutcome::Unparseable => None,
    });
    if let Some(host) = sacred_host.as_ref() {
        if let Some(pattern) = sacred::first_match(host, sacred_domains) {
            return LandingVerdict {
                decision: Decision::Deny(sacred::sacred(host.as_str(), pattern)),
                domain,
                source: DenialSource::Sacred,
            };
        }
    }

    let service = governance.decide(
        operation.as_str(),
        None,
        requires,
        resolved.clone(),
        config_mode,
    );
    if matches!(service, Decision::Deny(_)) {
        return LandingVerdict {
            decision: service,
            domain,
            source: DenialSource::Policy,
        };
    }
    if let Some(overlay) = overlay {
        let restricted = overlay.decide(operation.as_str(), None, requires, resolved, config_mode);
        if matches!(restricted, Decision::Deny(_) | Decision::ShadowDeny(_)) {
            return LandingVerdict {
                decision: restricted,
                domain,
                source: DenialSource::Policy,
            };
        }
    }
    LandingVerdict {
        decision: service,
        domain,
        source: DenialSource::Policy,
    }
}

async fn apply_landing_verdict(
    browser: &Browser,
    guid: &str,
    tab_id: i64,
    verdict: LandingVerdict,
    audit: &mut crate::governance::dispatch::CallAudit,
) -> Option<NavigationFinalize> {
    match verdict.decision {
        Decision::Allow { grant_id } => {
            audit.landing_allow(grant_id, verdict.domain);
            None
        }
        Decision::ShadowDeny(denial) => {
            audit.landing_shadow_deny(denial, verdict.domain);
            None
        }
        Decision::Deny(denial) => {
            park_navigation_for_safety(browser, guid, tab_id).await;
            Some(NavigationFinalize::Denied {
                denial,
                domain: verdict.domain,
                source: verdict.source,
            })
        }
    }
}

async fn park_navigation_for_safety(browser: &Browser, guid: &str, tab_id: i64) {
    let park = MechanismRequest::for_auxiliary(
        BrowserAuxiliaryPurpose::SafetyPark,
        MechanismId::NavigateUrl,
        json!({ "url": "about:blank", "tab": tab_id }),
    )
    .expect("post-landing safety park must be declared by its auxiliary plan");
    let _ = browser
        .execute_mechanism(guid, &park, &ExecutionContext::safety_protocol())
        .await;
}

/// One lazily resolved, memoized tab-URL probe per call (ADR-0024 Decision 4): the sacred check
/// (STEP B, [`sacred_check`]) and the grant path's `TabScoped` resolution
/// ([`resolve_governing_resource`]) both read the SAME call's `tabId` argument, so they share
/// exactly one `tab_url_request` frame (the extension's own `Browser::tab_url`) instead of two
/// different mechanisms -- the sacred check's former internal `tabs_context_mcp` lookup (deleted
/// by this task) and the grant path's `tab_url_request`. Resolution happens at most once, on
/// whichever stage calls [`LazyTabUrl::get`] first; a call that never needs a tab URL (no
/// `tabId`, an empty sacred list plus all-open/ungoverned/free, etc.) never probes at all. `None`
/// means "no URL to resolve": either there was no `tabId` on this call, or the tab is unknown,
/// closed, or the channel failed -- callers apply their own meaning to that (the sacred check
/// finds no host to match, so it never denies from a `None`, g08 constraint 12; the grant path
/// fails closed to [`GoverningResource::Indeterminate`]).
struct LazyTabUrl<'a> {
    browser: &'a Browser,
    guid: &'a str,
    tab_id: Option<i64>,
    execution: &'a ExecutionContext,
    resolved: Option<Option<String>>,
}

impl<'a> LazyTabUrl<'a> {
    fn new(
        browser: &'a Browser,
        guid: &'a str,
        tab_id: Option<i64>,
        execution: &'a ExecutionContext,
    ) -> Self {
        Self {
            browser,
            guid,
            tab_id,
            execution,
            resolved: None,
        }
    }

    /// Resolve (once, memoized for the lifetime of this cell -- one call) and return this call's
    /// tab URL, or `None` if there was no `tabId` to resolve or the resolution failed.
    async fn get(&mut self) -> Option<String> {
        if self.resolved.is_none() {
            let url = match self.tab_id {
                Some(tab_id) => match self
                    .browser
                    .tab_url(self.guid, tab_id, self.execution)
                    .await
                {
                    Ok(Some(url)) => Some(url),
                    Ok(None) | Err(_) => None,
                },
                None => None,
            };
            self.resolved = Some(url);
        }
        self.resolved.clone().unwrap()
    }
}

/// Append the truthful handshake-wait note as a final text block on an MCP tool result.
fn append_wait_note(result: &mut Value, waited: Duration) {
    let note = format!(
        "(waited {:.1}s for browser extension handshake)",
        waited.as_secs_f64()
    );
    if let Some(content) = result.get_mut("content").and_then(Value::as_array_mut) {
        content.push(json!({ "type": "text", "text": note }));
    }
}

/// Add concise service-authored routing guidance for still-open tabs observed while the browser
/// call ran. This runs after page provenance so the guidance can never be wrapped or presented as
/// page-sourced content, and deliberately says "observed" rather than claiming action causality.
fn append_tab_delta_note(result: &mut Value) {
    let Some(opened) = result
        .pointer("/structuredContent/tabDelta/opened")
        .and_then(Value::as_array)
    else {
        return;
    };
    let closed = result
        .pointer("/structuredContent/tabDelta/closed")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let still_open = opened
        .iter()
        .filter_map(|item| item.get("tabId").and_then(Value::as_i64))
        .filter(|tab_id| !closed.iter().any(|closed| closed.as_i64() == Some(*tab_id)))
        .collect::<Vec<_>>();
    if still_open.is_empty() {
        return;
    }
    let active = result
        .pointer("/structuredContent/tabDelta/activeTabId")
        .and_then(Value::as_i64)
        .filter(|tab_id| still_open.contains(tab_id));
    let note = if let Some(tab_id) = active {
        format!(
            "Browser context changed while this call ran. Observed new active tab {tab_id}. Use that tabId for follow-up calls."
        )
    } else if still_open.len() == 1 {
        format!(
            "Browser context changed while this call ran. Observed new tab {}. Use that tabId for follow-up calls.",
            still_open[0]
        )
    } else {
        let ids = still_open
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Browser context changed while this call ran. Observed new tabs: {ids}. Use these tabIds for follow-up calls."
        )
    };
    if let Some(content) = result.get_mut("content").and_then(Value::as_array_mut) {
        content.push(json!({ "type": "text", "text": note }));
    }
}
