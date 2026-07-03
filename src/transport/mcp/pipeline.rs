//! The generic ingest pipeline (ADR-0024 Decision 2): the `tools/call` dispatch chokepoint,
//! moved out of `transport::mcp::server` into its own module so `server.rs` keeps only the
//! JSON-RPC protocol loop and the composition root. Every per-tool `if name == ...` branch is
//! replaced by a read of the tool's [`crate::browser::directory::ToolDescriptor`] row
//! (ADR-0024 Decision 1); per-tool variance lives in the registry, not here.
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
//! 8. Free-action short-circuit (unchanged: keyed on the looked-up requires) and
//!    `Handler::Local` dispatch (`explain`), in the position pinned by stage 3.
//! 9. Governance authorization (ADR-0024 Decision 3), with resource resolution driven by the
//!    descriptor's resource shape and skipped entirely when ungoverned or requires is empty.
//! 10. Bounded first-call wait; dispatch via `Handler` (`ExtensionForward` -> `Browser::call`,
//!     unchanged contract).
//! 11. `PostDispatch::NavigateLanding`: the landing re-check and park-on-real-deny (never on
//!     shadow), driven by the marker instead of `name == "navigate"`.
//! 12. Audit completion (ADR-0024 Decision 3), then the `postprocess` hook and wait-note, then
//!     the JSON-RPC envelope.
//!
//! All-open byte-identity and the zero-cost paths are constraints on every stage: no per-call
//! fixture parse, no resource resolution under all-open, no frames for free actions, shadow
//! mode observably identical to allow.

use crate::browser::pattern::HostOutcome;
use crate::browser::{directory, pattern, resource, sacred};
use crate::governance::config::reload::ConfigStore;
use crate::governance::dispatch::{hold_message, Gate, Governance};
use crate::governance::ports::{Capability, Decision, Denial, EffectiveMode, GoverningResource};
use crate::transport::executor::Browser;
use crate::transport::mcp::types::{text_content, JsonRpcResponse};
use crate::ToolError;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The dispatch chokepoint (ADR-0024 Decision 2): every `tools/call` passes through here, in
/// the pinned stage order documented at the top of this module. `pub(crate)` so
/// `server::handle_line`'s `tools/call` arm can reach it.
pub(crate) async fn handle_tools_call(
    browser: &Browser,
    store: &Arc<ConfigStore>,
    governance: &Governance,
    id: Option<Value>,
    params: Option<&Value>,
) -> JsonRpcResponse {
    // One snapshot for the whole call, taken once at entry: a reload mid-call must not tear
    // the snapshot the call already started with.
    let config = store.current();

    let Some(name) = params.and_then(|p| p.get("name")).and_then(Value::as_str) else {
        return JsonRpcResponse::error(id, -32602, "tools/call requires a string 'name'");
    };
    let args = params
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or(Value::Null);

    // Unknown tool names are rejected before dispatch (and before waiting on the extension
    // channel at all): this is a client-request problem, not a browser/extension problem, and the
    // client should learn that instantly regardless of whether an extension is even connected.
    // The extension keeps its own `Unknown tool: ...` guard as a safety net (defense in depth);
    // this pre-check just means well-formed clients never round-trip to hit it. Stage 3
    // (ADR-0024 Decision 1): the registry lookup itself IS the validity check now
    // (`directory::descriptor`, replacing the transport layer's former per-call fixture
    // re-parse); a miss still returns the byte-identical "Unknown tool: {name}" result.
    let Some(descriptor) = directory::descriptor(name) else {
        let err = ToolError::invalid_request(format!("Unknown tool: {name}"))
            .next_step("call tools/list and use one of the advertised tool names");
        return JsonRpcResponse::success(id, error_result(err));
    };

    // The only tool-call argument ever read for audit purposes: the computer sub-action
    // (shared format doc section 6.2 sensitive-parameter omission; no other argument is read,
    // logged, or stored). Stage 4 (ADR-0024 Decision 1): the registry's `action_key` drives
    // this instead of a hardcoded `name == "computer"` check -- `computer` is the only
    // descriptor carrying one today.
    let action = descriptor
        .action_key
        .and_then(|key| args.get(key))
        .and_then(Value::as_str);

    // The single per-call action-directory lookup (ADR-0022 Decision 2, ADR-0024 Decision 3): a
    // pure static table scan, no I/O, performed ONCE and kept as the `Option` it is (a registry
    // miss is `None`, never coerced to an empty slice here): `governance.begin` and
    // `governance.authorize` both consume this SAME value, so there is exactly one lookup for
    // the whole call, feeding both the decision and the audit `capability` field.
    let lookup: Option<&'static [Capability]> = directory::requires(name, action);
    let mut audit = governance.begin(name, action, lookup);

    // Take-the-wheel hold (g10, ADR-0018 step 2): a user gesture, not a policy decision, so it
    // is checked before ANY dispatch machinery -- before governance.authorize, before the sacred
    // check, before any extension traffic. A held call is answered immediately with a
    // successful (never isError) text result and is never queued, deferred, or replayed;
    // resuming affects only future calls. Held calls still produce one audit record
    // (`decision: "allow"`, `held: true`, `duration_ms: 0`).
    if let Some(held_for) = browser.held_for() {
        audit.held();
        return JsonRpcResponse::success(id, text_content(hold_message(name, action, held_for)));
    }

    // ADR-0024 Decision 4: the sacred check and the grant path below share ONE lazily resolved,
    // memoized tab-URL probe per call, keyed on this call's own `tabId` argument, instead of two
    // different mechanisms (the sacred check's former internal `tabs_context_mcp` lookup,
    // deleted, and the grant path's `tab_url_request`). Nothing is probed until the first stage
    // that actually needs it calls `.get()` -- an all-open call, an ungoverned call, a free
    // action, or a call with no `tabId` at all issues zero frames.
    let mut tab_url = LazyTabUrl::new(browser, args.get("tabId").and_then(Value::as_i64));

    // The sacred-domains never-touch check (ADR-0018 step 2, g08): always enforced,
    // independent of governance.mode or manifest presence -- RECONCILIATION.md section 1's
    // "always-on carve-out", and ahead of grant evaluation below (g13: "if the sacred-domains
    // check has already landed, leave it in place and ahead of grant evaluation"). STEP A: an
    // empty list (every preset's default) is the byte-identical fast path -- no extension
    // traffic, no parsing, no allocation.
    let sacred_domains = config.sacred_domains();
    let SacredCheck { tab_domain, denial } = if sacred_domains.is_empty() {
        SacredCheck {
            tab_domain: None,
            denial: None,
        }
    } else {
        sacred_check(&mut tab_url, sacred_domains, descriptor.resource, &args).await
    };
    if let Some(denial) = denial {
        audit.sacred_deny(&denial, tab_domain.as_deref());
        return JsonRpcResponse::success(id, text_content(denial.message));
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
    // `computer` `wait`: requirement `[]`, yet it carries a `tabId`). `explain` (Decision 7,
    // ADR-0024 Decision 1's `Handler::Local`) is the one free action with a server-side body and
    // no extension action, so it is answered right here (no native-messaging frame is ever
    // produced for it); every other free action (`tabs_create_mcp`, `resize_window`,
    // `update_plan`, `computer` `wait`) falls through to an ordinary allowed dispatch below, and
    // to `governance.authorize`'s own free-action arm. All are audited as an allow with no grant
    // attribution and a real (not hardcoded) `duration_ms`.
    if let directory::Handler::Local(f) = descriptor.handler {
        let text = f();
        audit.complete();
        return JsonRpcResponse::success(id, text_content(text));
    }

    // Grant enforcement (g13, ADR-0018 step 3, ADR-0024 Decision 3): resolve the governing
    // resource for this call, then consult the single policy gate. Resource resolution stays
    // gated on being governed with a KNOWN, non-empty requirement set (a miss resolves nothing --
    // no wasted probe before its `unknown_action` denial; a free action was already allowed
    // above); `governance.authorize` itself is called for EVERY call that reaches this point
    // (governed or not, miss or not) -- its own precedence table makes the ungoverned/free/miss
    // arms cheap and correct, restoring ADR-0022's absent-means-DENY for a governed miss (the
    // ADR-0024 sanctioned delta this task owns) while leaving all-open and free-action behavior
    // byte-identical. Resolution itself is now shape-driven (ADR-0024 Decision 1's
    // `ResourceShape`) instead of a per-tool name match.
    let config_mode = config.governance_mode();
    let resolved = if governance.is_governed() && matches!(lookup, Some(r) if !r.is_empty()) {
        resolve_governing_resource(&mut tab_url, descriptor, &args).await
    } else {
        None
    };
    if let Some((_, domain)) = &resolved {
        audit.set_domain(domain.clone());
    }
    // The post-dispatch flag: only when the pre-check actually ran (a resolved resource) AND the
    // descriptor marks this tool for the navigate landing re-check (today: `navigate` only) --
    // preserving today's exact `name == "navigate"` gating via the marker instead.
    let navigate_post_check =
        resolved.is_some() && descriptor.post_dispatch == directory::PostDispatch::NavigateLanding;
    let resource = resolved.map(|(r, _)| r);
    match governance.authorize(&mut audit, resource, config_mode) {
        Gate::Deny { message } => return JsonRpcResponse::success(id, text_content(message)),
        Gate::Proceed => {}
    }

    // Bounded first-call wait: the first call of a session races the extension handshake.
    // Wait briefly for the channel instead of failing a healthy session (also covers calls
    // arriving during a mid-session reconnect). If the wait times out, `waited` stays `None` and
    // control falls through to `Browser::call` below, which fails fast with the canonical
    // "extension not connected" `ToolError` -- one hop-attributed message, not two to keep in sync.
    let mut waited: Option<Duration> = None;
    if !browser.is_connected() {
        let started = Instant::now();
        if browser
            .wait_connected(Duration::from_millis(config.first_call_wait_ms()))
            .await
        {
            waited = Some(started.elapsed());
        } else {
            tracing::warn!(
                tool = name,
                "tools/call failed: extension channel never came up"
            );
        }
    }

    let outcome = browser.call(name, &args).await;
    audit.dispatch_finished();

    // Point 5 (g13/g15): after a dispatched `navigate` succeeds, re-check the FINAL
    // (post-redirect) landing -- authoritative over the pre-dispatch verdict above for the
    // audit record, since a redirect can land somewhere the target itself never named. Only
    // reachable when governed and the pre-check above actually ran (skipped for an unparseable
    // target, per the fall-through comment above); a failed dispatch gets no post-check
    // (nothing landed).
    if navigate_post_check && outcome.is_ok() {
        if let Some(tab_id) = args.get("tabId").and_then(Value::as_i64) {
            let (landing, landing_domain) = post_navigate_landing_check(
                browser,
                governance,
                descriptor.tool,
                lookup.unwrap_or(&[]),
                tab_id,
                config_mode,
            )
            .await;
            match landing {
                Decision::Allow { grant_id } => {
                    audit.landing_allow(grant_id, landing_domain);
                }
                Decision::Deny(d) => {
                    audit.landing_deny(&d, landing_domain.as_deref());
                    return JsonRpcResponse::success(id, text_content(d.message));
                }
                Decision::ShadowDeny(d) => {
                    audit.landing_shadow_deny(d, landing_domain);
                }
            }
        }
    }

    audit.complete();

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
            JsonRpcResponse::success(id, result)
        }
        // A tool execution failure is an MCP tool error result (isError), not a JSON-RPC error.
        // The rendered text is exactly the hop-attributed ToolError Display: no "Error: " prefix.
        Err(e) => {
            let mut result = error_result(e);
            if let Some(waited) = waited {
                append_wait_note(&mut result, waited);
            }
            JsonRpcResponse::success(id, result)
        }
    }
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
/// (the target host) fires iff `resource_shape` is [`directory::ResourceShape::TargetArg`]
/// (today: `navigate` only, ADR-0024 Decision 1), even when STEP B could not resolve the tab,
/// since it is local and needs no extension. STEP B reads the tab's URL through the shared
/// `tab_url` cell (ADR-0024 Decision 4), the SAME probe the grant path below reuses, rather than
/// its own internal lookup.
async fn sacred_check(
    tab_url: &mut LazyTabUrl<'_>,
    sacred_domains: &[String],
    resource_shape: directory::ResourceShape,
    args: &Value,
) -> SacredCheck {
    let tab_host = match args.get("tabId").and_then(Value::as_i64) {
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

    if resource_shape == directory::ResourceShape::TargetArg {
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
/// (ADR-0024 Decision 1's [`directory::ResourceShape`]) instead of a per-tool name match. Only
/// called once [`Governance::is_governed`] is true. Returns `None` only for an unparseable
/// `TargetArg` (`navigate`) target: nothing to govern (section 4: "dispatch without pre- or
/// post-check"). Otherwise `Some((resource, domain))`, where `domain` is the resolved host for
/// the audit record's `domain` field when `resource` is [`GoverningResource::Resource`], `None`
/// otherwise (shared format doc section 6.1: never the denial message's `(unknown)` placeholder).
/// `TabScoped` resolution reads the tab's URL through the shared `tab_url` cell (ADR-0024
/// Decision 4), the SAME probe the sacred check above may already have resolved for this call.
async fn resolve_governing_resource(
    tab_url: &mut LazyTabUrl<'_>,
    descriptor: &directory::ToolDescriptor,
    args: &Value,
) -> Option<(GoverningResource, Option<String>)> {
    match descriptor.resource {
        directory::ResourceShape::DomainLess => Some((GoverningResource::None, None)),
        directory::ResourceShape::TargetArg => match args.get("url").and_then(Value::as_str) {
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
        },
        directory::ResourceShape::TabScoped => {
            if args.get("tabId").and_then(Value::as_i64).is_none() {
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

/// Point 5 (g13, SPEC 5.2 step 5; g15 shadow enforcement): after a dispatched `navigate`
/// succeeds, re-query tab `tab_id`'s FINAL (post-redirect) URL and re-run the SAME governed
/// decision the original call would get pre-dispatch (reusing [`Governance::decide`] rather than
/// duplicating grant logic), returning the full [`Decision`] plus the resolved landing host
/// (`None` for a non-host landing -- never the denial message's `(unknown)` placeholder). `tool`
/// is the descriptor's own tool name (ADR-0024 Decision 2: no hardcoded `"navigate"` literal in
/// the governance-core call), supplied by the only caller that reaches this function today
/// (`navigate`, via [`directory::PostDispatch::NavigateLanding`]). The caller decides what each
/// variant means for the response and the audit record; this function's own side effect is
/// limited to the best-effort `about:blank` park, and ONLY for an actual [`Decision::Deny`] -- a
/// [`Decision::ShadowDeny`] landing must leave the browser untouched (shadow mode is a fully
/// transparent pass-through; parking would be a visible, detectable side effect that gives away
/// a shadowed call, breaking g15's own truthfulness requirement that "the agent must not be able
/// to tell a shadowed call from a permitted one").
async fn post_navigate_landing_check(
    browser: &Browser,
    governance: &Governance,
    tool: &str,
    requires: &[Capability],
    tab_id: i64,
    config_mode: EffectiveMode,
) -> (Decision, Option<String>) {
    let resolved = match browser.tab_url(tab_id).await {
        Ok(Some(url)) => resource::resolved_url_resource(&url),
        Ok(None) | Err(_) => GoverningResource::Indeterminate,
    };
    let domain = match &resolved {
        GoverningResource::Resource(h) => Some(h.clone()),
        _ => None,
    };
    let decision = governance.decide(tool, None, requires, resolved, config_mode);
    if let Decision::Deny(_) = &decision {
        let _ = browser
            .call(
                "navigate",
                &json!({ "url": "about:blank", "tabId": tab_id }),
            )
            .await;
    }
    (decision, domain)
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
    tab_id: Option<i64>,
    resolved: Option<Option<String>>,
}

impl<'a> LazyTabUrl<'a> {
    fn new(browser: &'a Browser, tab_id: Option<i64>) -> Self {
        Self {
            browser,
            tab_id,
            resolved: None,
        }
    }

    /// Resolve (once, memoized for the lifetime of this cell -- one call) and return this call's
    /// tab URL, or `None` if there was no `tabId` to resolve or the resolution failed.
    async fn get(&mut self) -> Option<String> {
        if self.resolved.is_none() {
            let url = match self.tab_id {
                Some(tab_id) => match self.browser.tab_url(tab_id).await {
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

/// Build an MCP tool error result (`{ content: [...], isError: true }`) from a hop-attributed
/// [`ToolError`]. The result text is exactly the error's `Display`:
/// `[hop: <hop>] <message>. Next step: <next step>.`
fn error_result(err: ToolError) -> Value {
    let mut result = text_content(err.to_string());
    if let Some(obj) = result.as_object_mut() {
        obj.insert("isError".into(), json!(true));
    }
    result
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::polarity;
    use crate::governance::audit::Recorder;
    use crate::governance::config::layers::{self, LayerInputs};
    use crate::governance::config::{Config, CONTENT_SECURITY_SACRED_DOMAINS};
    use crate::governance::ports::AuditSink;
    use crate::transport::native::host;
    use std::sync::Mutex;
    use std::time::Duration as StdDuration;

    fn temp_audit_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-server-audit-test-{}-{tag}.jsonl",
            std::process::id()
        ))
    }

    fn read_lines(path: &std::path::Path) -> Vec<Value> {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .map(|l| serde_json::from_str(l).expect("each line is a JSON object"))
            .collect()
    }

    fn assert_wellformed_event_id_and_ts(rec: &Value) {
        let event_id = rec["event_id"].as_str().expect("event_id is a string");
        assert_eq!(event_id.len(), 36, "event_id: {event_id}");
        for offset in [8, 13, 18, 23] {
            assert_eq!(event_id.as_bytes()[offset], b'-', "event_id: {event_id}");
        }
        let ts = rec["ts"].as_str().expect("ts is a string");
        assert_eq!(ts.len(), 24, "ts: {ts}");
        assert!(ts.ends_with('Z'), "ts: {ts}");
        chrono::DateTime::parse_from_rfc3339(ts).expect("ts parses as rfc3339");
    }

    /// A `Config` whose `content.security.sacred_domains` resolves to exactly `patterns`,
    /// everything else at its Minimal default. Built through the real layered resolver (not a
    /// hand-built `Config`) so validation runs exactly as it would in production.
    fn config_with_sacred_domains(patterns: &[&str]) -> Config {
        let inputs = LayerInputs {
            user: serde_json::Map::from_iter([(
                CONTENT_SECURITY_SACRED_DOMAINS.to_string(),
                json!(patterns),
            )]),
            ..Default::default()
        };
        Config::from_resolution(&layers::resolve(&inputs))
    }

    async fn wait_connected(browser: &Browser) {
        for _ in 0..200 {
            if browser.is_connected() {
                return;
            }
            tokio::time::sleep(StdDuration::from_millis(5)).await;
        }
        panic!("browser never reported connected");
    }

    /// Attach a fake extension over an in-memory duplex pipe (the same pattern
    /// `transport::executor`'s own tests use). Answers a `tool_request` for any tool name found
    /// in `responses` with that canned result and records the tool names seen, in arrival order,
    /// into the returned `Arc<Mutex<Vec<String>>>`. Panics if a `tool_request` arrives for a
    /// tool not in `responses` -- tests use this to prove a denied call never reaches the real
    /// tool. No `tab_url_request` answers registered: a call that needs one (any tab-scoped
    /// sacred check or grant resolution, ADR-0024 Decision 4) panics; tests that need a tab-URL
    /// answer use [`attach_fake_extension_with_tab_urls`] instead.
    fn attach_fake_extension(
        browser: &Browser,
        responses: Vec<(&'static str, Value)>,
    ) -> (tokio::task::JoinHandle<()>, Arc<Mutex<Vec<String>>>) {
        attach_fake_extension_with_tab_urls(browser, responses, Vec::new())
    }

    /// Like [`attach_fake_extension`], plus a `tab_url_request` answer table (g13): `tab_urls`
    /// maps a `tabId` to the URL the fake extension reports for it (`None` for `url: null`, an
    /// unknown/closed tab). A `tab_url_request` for a `tabId` absent from the table panics, same
    /// posture as an unregistered `tool_request`. `seen` records a `"tab_url_request:<tabId>"`
    /// entry for each query, distinguishable from the tool names `tool_request` entries record.
    fn attach_fake_extension_with_tab_urls(
        browser: &Browser,
        responses: Vec<(&'static str, Value)>,
        tab_urls: Vec<(i64, Option<&'static str>)>,
    ) -> (tokio::task::JoinHandle<()>, Arc<Mutex<Vec<String>>>) {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let attached = browser.clone();
        tokio::spawn(async move {
            let _ = attached.attach(browser_side).await;
        });

        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_for_task = Arc::clone(&seen);
        let responses: std::collections::HashMap<&'static str, Value> =
            responses.into_iter().collect();
        let tab_urls: std::collections::HashMap<i64, Option<&'static str>> =
            tab_urls.into_iter().collect();
        let handle = tokio::spawn(async move {
            loop {
                let Some(req) = host::read_message(&mut ext_side).await.unwrap() else {
                    break;
                };
                let v: Value = serde_json::from_slice(&req).unwrap();
                if v["type"] == "tab_url_request" {
                    let tab_id = v["tabId"]
                        .as_i64()
                        .expect("tab_url_request carries a tabId");
                    seen_for_task
                        .lock()
                        .unwrap()
                        .push(format!("tab_url_request:{tab_id}"));
                    let url = *tab_urls
                        .get(&tab_id)
                        .unwrap_or_else(|| panic!("unexpected tab_url_request for tabId {tab_id}"));
                    let reply = json!({ "id": v["id"], "type": "tab_url_response", "result": { "url": url } });
                    host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                        .await
                        .unwrap();
                    continue;
                }
                let tool = v["tool"].as_str().unwrap().to_string();
                seen_for_task.lock().unwrap().push(tool.clone());
                let result = responses
                    .get(tool.as_str())
                    .cloned()
                    .unwrap_or_else(|| panic!("unexpected tool_request for '{tool}'"));
                let reply = json!({ "id": v["id"], "type": "tool_response", "result": result });
                host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                    .await
                    .unwrap();
            }
        });
        (handle, seen)
    }

    /// Test 6 (g08 spec section 6): a tab showing a sacred host denies every tool that carries
    /// its `tabId`, including `navigate` (navigating AWAY is denied too), and the extension
    /// never receives anything but the shared `tab_url_request` pre-flight (ADR-0024 Decision
    /// 4: the sacred check's former internal `tabs_context_mcp` pre-flight is gone; this test's
    /// `seen`-vector expectation is the sanctioned Decision 4 frame-traffic change, t05).
    #[tokio::test]
    async fn sacred_tab_denies_every_tool_and_never_runs_it() {
        let path = temp_audit_path("sacred-tab");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store = crate::governance::config::reload::ConfigStore::for_test_with_config(
            config_with_sacred_domains(&["*.mybank.com"]),
        );
        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![],
            vec![(5, Some("https://www.mybank.com/account"))],
        );
        wait_connected(&browser).await;

        let cases = [
            ("read_page", json!({ "tabId": 5 })),
            ("computer", json!({ "action": "screenshot", "tabId": 5 })),
            (
                "javascript_tool",
                json!({ "action": "javascript_exec", "text": "1", "tabId": 5 }),
            ),
            (
                "navigate",
                json!({ "url": "https://example.com", "tabId": 5 }),
            ),
        ];
        for (tool, args) in cases {
            let params = json!({ "name": tool, "arguments": args });
            let resp =
                handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params))
                    .await;
            let text = resp.result.as_ref().expect("tool result present")["content"][0]["text"]
                .as_str()
                .expect("text content block");
            assert!(
                text.starts_with("Denied (D-af6633ec)"),
                "{tool}: unexpected text: {text}"
            );
            assert!(text.contains("www.mybank.com"), "{tool}: {text}");
        }

        let lines = read_lines(&path);
        assert_eq!(lines.len(), 4, "exactly one deny record per denied call");
        for rec in &lines {
            assert_eq!(rec["decision"], "deny");
            assert_eq!(rec["denial_id"], "D-af6633ec");
            assert_eq!(rec["domain"], "www.mybank.com");
        }
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["tab_url_request:5"; 4],
            "the extension must never see anything but the tab_url_request pre-flight"
        );

        std::fs::remove_file(&path).ok();
    }

    /// Test 7 (g08 spec section 6): a `navigate` target matching a sacred pattern is denied
    /// even when the current tab is clean; a target that does not match is allowed.
    #[tokio::test]
    async fn navigate_target_denied_even_when_tab_is_clean() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store = crate::governance::config::reload::ConfigStore::for_test_with_config(
            config_with_sacred_domains(&["mybank.com"]),
        );
        let browser = Browser::new();
        let (_ext, _seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![(
                "navigate",
                json!({ "content": [{ "type": "text", "text": "navigated" }] }),
            )],
            vec![(5, Some("https://example.com/"))],
        );
        wait_connected(&browser).await;

        let denied_params = json!({
            "name": "navigate",
            "arguments": { "url": "mybank.com", "tabId": 5 },
        });
        let denied = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(1)),
            Some(&denied_params),
        )
        .await;
        let denied_text = denied.result.as_ref().expect("tool result present")["content"][0]
            ["text"]
            .as_str()
            .expect("text content block");
        assert!(
            denied_text.starts_with("Denied (D-171052e3)"),
            "{denied_text}"
        );
        assert!(denied_text.contains("mybank.com"));

        let allowed_params = json!({
            "name": "navigate",
            "arguments": { "url": "https://example.org", "tabId": 5 },
        });
        let allowed = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(2)),
            Some(&allowed_params),
        )
        .await;
        let allowed_text = allowed.result.as_ref().expect("tool result present")["content"][0]
            ["text"]
            .as_str()
            .expect("text content block");
        assert_eq!(allowed_text, "navigated");
    }

    /// Test 8 (g08 spec section 6): with the default (empty) sacred list, a call reaches the
    /// fake extension directly -- no `tabs_context_mcp` pre-flight ever -- and an unconnected
    /// browser still resolves the sacred check without any browser access.
    #[tokio::test]
    async fn empty_list_is_byte_identical() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        assert!(store.current().sacred_domains().is_empty());

        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension(
            &browser,
            vec![(
                "read_page",
                json!({ "content": [{ "type": "text", "text": "page text" }] }),
            )],
        );
        wait_connected(&browser).await;

        let params = json!({ "name": "read_page", "arguments": { "tabId": 5 } });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let text = resp.result.as_ref().expect("tool result present")["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert_eq!(text, "page text");
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["read_page"],
            "no tabs_context_mcp pre-flight ever, with an empty list"
        );

        // Allow resolves without touching the browser at all: an unconnected Browser still
        // reaches the ordinary not-connected error, never a sacred pre-flight attempt.
        let unconnected = Browser::new();
        let params2 = json!({ "name": "navigate", "arguments": {} });
        let resp2 = handle_tools_call(
            &unconnected,
            &store,
            &governance,
            Some(json!(2)),
            Some(&params2),
        )
        .await;
        let text2 = resp2.result.as_ref().expect("tool result present")["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert!(text2.contains("not connected"), "{text2}");
    }

    /// Test 9 (g08 spec section 6): a denied call writes exactly one audit record, and the
    /// internal tab-URL probe writes none.
    #[tokio::test]
    async fn denied_call_writes_one_deny_record() {
        let path = temp_audit_path("deny-record");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store = crate::governance::config::reload::ConfigStore::for_test_with_config(
            config_with_sacred_domains(&["*.mybank.com"]),
        );
        let browser = Browser::new();
        let (_ext, _seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![],
            vec![(5, Some("https://www.mybank.com/account"))],
        );
        wait_connected(&browser).await;

        let params = json!({ "name": "read_page", "arguments": { "tabId": 5 } });
        let _ =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;

        let lines = read_lines(&path);
        assert_eq!(
            lines.len(),
            1,
            "exactly one record: the tab-url probe writes none"
        );
        let rec = &lines[0];
        assert_eq!(rec["decision"], "deny");
        let denial_id = rec["denial_id"].as_str().expect("denial_id is a string");
        assert!(
            denial_id.starts_with("D-") && denial_id.len() == 10,
            "{denial_id}"
        );
        assert_eq!(rec["grant_id"], Value::Null);
        assert_eq!(rec["duration_ms"], 0);
        assert_eq!(rec["domain"], "www.mybank.com");

        std::fs::remove_file(&path).ok();
    }

    // --- t05 (ADR-0024 Decision 4): one tab-URL resolution per call ---

    /// A non-empty sacred list, a governed manifest, and a `TabScoped` call (`read_page`) on a
    /// clean, granted tab: the sacred check (STEP B) and the grant path's resource resolution
    /// share exactly ONE `tab_url_request` probe -- the pre-ADR-0024 code would show a
    /// `tabs_context_mcp` pre-flight (sacred) AND a `tab_url_request` (grant path); the unified
    /// code shows exactly one probe before the dispatched tool frame.
    #[tokio::test]
    async fn one_probe_serves_sacred_and_grants() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(governed_with_grants(
            vec![full_grant("g1", &["example.com"])],
            recorder as Arc<dyn AuditSink>,
        ));
        let store = crate::governance::config::reload::ConfigStore::for_test_with_config(
            config_with_sacred_domains(&["mybank.com"]),
        );
        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![(
                "read_page",
                json!({ "content": [{ "type": "text", "text": "ok" }] }),
            )],
            vec![(5, Some("https://example.com/"))],
        );
        wait_connected(&browser).await;

        let params = json!({ "name": "read_page", "arguments": { "tabId": 5 } });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let result = resp.result.as_ref().expect("tool result present");
        assert_ne!(
            result["isError"], true,
            "example.com is neither sacred nor ungranted: {result:?}"
        );

        assert_eq!(
            *seen.lock().unwrap(),
            vec!["tab_url_request:5", "read_page"],
            "exactly one tab-url probe serves both the sacred check and the grant path"
        );
    }

    /// A tab the extension cannot resolve (unknown, closed, or a channel failure): the shared
    /// probe answers `None`, which the sacred check reads as "no host to match" (the call is NOT
    /// sacred-denied) and the grant path reads as fail-closed `Indeterminate` (the call IS
    /// denied, with the same wording an unresolved tab id already produces today). Both
    /// conclusions are read from the SAME single probe.
    #[tokio::test]
    async fn unresolvable_tab_still_fails_closed_for_grants_and_skips_sacred() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(governed_with_grants(
            vec![full_grant("g1", &["example.com"])],
            recorder as Arc<dyn AuditSink>,
        ));
        let store = crate::governance::config::reload::ConfigStore::for_test_with_config(
            config_with_sacred_domains(&["mybank.com"]),
        );
        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension_with_tab_urls(&browser, vec![], vec![(5, None)]);
        wait_connected(&browser).await;

        let params = json!({ "name": "read_page", "arguments": { "tabId": 5 } });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let text = resp.result.as_ref().expect("tool result present")["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert!(text.starts_with("Denied (D-"), "{text}");
        assert!(
            text.contains("no grant covers (unknown)"),
            "an unresolvable tab fails closed to Indeterminate for the grant path: {text}"
        );

        assert_eq!(
            *seen.lock().unwrap(),
            vec!["tab_url_request:5"],
            "one probe serves both stages; the sacred check found no host to match, so it \
             never denied (only the grant path's fail-closed Indeterminate denies)"
        );
    }

    /// Test 10 (g06 spec section 6, adapted to the post-A3/A5 architecture): drives the real
    /// `handle_line` dispatch for `initialize` (proving `capture_client_info` is wired at the
    /// real chokepoint, not just callable in isolation) and `handle_tools_call` for a
    /// `navigate` call, then asserts the resulting audit line end to end.
    #[tokio::test]
    async fn tools_call_produces_one_audit_record_with_client_identity() {
        let path = temp_audit_path("basic");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        let (tx, _rx) =
            tokio::sync::mpsc::unbounded_channel::<crate::transport::mcp::server::Outbound>();

        let init_line = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "clientInfo": { "name": "test-client", "version": "9.9.9" } },
        })
        .to_string();
        crate::transport::mcp::server::handle_line(&browser, &store, &governance, &init_line, &tx)
            .await;

        let params = json!({ "name": "navigate", "arguments": {} });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(2)), Some(&params)).await;
        let text = resp.result.as_ref().expect("tool result present")["content"][0]["text"]
            .as_str()
            .expect("text content block")
            .to_string();
        assert!(text.contains("not connected"), "unexpected text: {text}");

        let lines = read_lines(&path);
        assert_eq!(lines.len(), 1, "exactly one audit record");
        let rec = &lines[0];
        assert_eq!(rec["tool"], "navigate");
        assert!(rec["action"].is_null());
        assert_eq!(rec["capability"], "read");
        assert_eq!(rec["decision"], "allow");
        assert_eq!(rec["client"]["name"], "test-client");
        assert_eq!(rec["client"]["version"], "9.9.9");
        for field in ["identity", "domain", "grant_id", "denial_id", "manifest"] {
            assert!(rec[field].is_null(), "{field} must be null");
        }
        assert_wellformed_event_id_and_ts(rec);

        std::fs::remove_file(&path).ok();
    }

    /// Test 11: a `computer` call with `action: "screenshot"` records that action and the
    /// `read` capability (ADR-0022 Decision 2: `computer screenshot` requires `read`).
    #[tokio::test]
    async fn computer_call_records_action_and_read_capability() {
        let path = temp_audit_path("computer");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();

        let params = json!({ "name": "computer", "arguments": { "action": "screenshot" } });
        let _ =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;

        let lines = read_lines(&path);
        assert_eq!(lines.len(), 1, "exactly one audit record");
        assert_eq!(lines[0]["action"], "screenshot");
        assert_eq!(lines[0]["capability"], "read");

        std::fs::remove_file(&path).ok();
    }

    /// Test 12: a `tools/call` whose params lack `name` returns the `-32602` error and never
    /// reaches the dispatch chokepoint, so no audit file is created.
    #[tokio::test]
    async fn invalid_tools_call_without_name_records_nothing() {
        let path = temp_audit_path("no-name");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();

        let params = json!({ "arguments": {} });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        assert_eq!(resp.error.as_ref().expect("error present")["code"], -32602);
        assert!(!path.exists(), "no audit file must be created");
    }

    /// Test 4 (g10 spec section 6): a held `Browser` with NO extension connected returns the
    /// `Paused:` text as a successful result (never `isError`), proving the hold check
    /// precedes the "extension not connected" failure path; with the hold released, the
    /// existing `isError` result is unchanged.
    #[tokio::test]
    async fn held_call_returns_the_pause_text_before_the_not_connected_error() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        browser.set_held(true);

        let params = json!({ "name": "computer", "arguments": { "action": "screenshot" } });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        assert!(resp.error.is_none(), "a held reply is a JSON-RPC success");
        let result = resp.result.as_ref().expect("tool result present");
        assert_ne!(
            result["isError"], true,
            "a held reply must never be isError"
        );
        let text = result["content"][0]["text"].as_str().expect("text block");
        assert!(text.starts_with("Paused:"), "{text}");
        assert!(text.contains("'computer (screenshot)' call"), "{text}");

        // ADR-0022 Decision 7: `explain` gets the ordinary pause text like any other tool
        // while held, even though its own directory requirement is `[]` -- the hold check
        // runs ahead of the `explain` server-side handler, same as every other pre-dispatch
        // outcome.
        let explain_params = json!({ "name": "explain", "arguments": {} });
        let explain_resp = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(3)),
            Some(&explain_params),
        )
        .await;
        let explain_result = explain_resp.result.as_ref().expect("tool result present");
        assert_ne!(
            explain_result["isError"], true,
            "a held reply is never isError"
        );
        let explain_text = explain_result["content"][0]["text"]
            .as_str()
            .expect("text block");
        assert!(explain_text.starts_with("Paused:"), "{explain_text}");
        assert!(explain_text.contains("'explain' call"), "{explain_text}");

        browser.set_held(false);
        let resp2 =
            handle_tools_call(&browser, &store, &governance, Some(json!(2)), Some(&params)).await;
        let result2 = resp2.result.as_ref().expect("tool result present");
        assert_eq!(
            result2["isError"], true,
            "with hold released, the not-connected path returns"
        );
        let text2 = result2["content"][0]["text"].as_str().expect("text block");
        assert!(text2.contains("not connected"), "{text2}");
    }

    /// Test 6 (g10 spec section 6): a held call writes one audit record with
    /// `decision: "allow"`, `held: true`, `duration_ms: 0`; a normal allowed call writes
    /// `held: false`.
    #[tokio::test]
    async fn held_call_marks_the_audit_record_and_normal_calls_do_not() {
        let path = temp_audit_path("held");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();

        browser.set_held(true);
        let held_params = json!({ "name": "navigate", "arguments": {} });
        let _ = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(1)),
            Some(&held_params),
        )
        .await;

        browser.set_held(false);
        let allowed_params = json!({ "name": "navigate", "arguments": {} });
        let _ = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(2)),
            Some(&allowed_params),
        )
        .await;

        let lines = read_lines(&path);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["decision"], "allow");
        assert_eq!(lines[0]["held"], true);
        assert_eq!(lines[0]["duration_ms"], 0);
        assert_eq!(lines[1]["held"], false);

        std::fs::remove_file(&path).ok();
    }

    // --- g13: grant enforcement, point 5 (navigate final-landing check) ---
    //
    // Every other g13 scenario (pre-dispatch domain/access/scheme/union-rule denials, the
    // all-open invariant, denial-id determinism) is covered end to end by the black-box
    // subprocess tests in `tests/tool_enforcement.rs`, which deliberately run with no extension
    // connected at all. Point 5 needs a dispatched `navigate` to actually succeed and then be
    // re-queried, which requires a connected (fake) extension; that is only practical here,
    // inline, using the same fake-extension pattern g08's sacred-domain tests above already
    // established.

    use crate::governance::enforcement::LocalPdp;
    use crate::governance::manifest::document::{Grant, HostRules};
    use crate::governance::ports::Capability;

    fn full_grant(id: &str, hosts: &[&str]) -> Grant {
        Grant {
            id: id.to_string(),
            hosts: HostRules {
                allow: hosts.iter().map(|d| d.to_string()).collect(),
                deny: Vec::new(),
            },
            allowed: vec![Capability::Read, Capability::Action, Capability::Write],
            description: None,
            mode: None,
        }
    }

    fn governed_with_grants(grants: Vec<Grant>, sink: Arc<dyn AuditSink>) -> Governance {
        governed_with_grants_and_mode(grants, sink, None)
    }

    fn governed_with_grants_and_mode(
        grants: Vec<Grant>,
        sink: Arc<dyn AuditSink>,
        manifest_mode: Option<crate::governance::ports::EffectiveMode>,
    ) -> Governance {
        Governance::governed(
            Box::new(LocalPdp::new(polarity::evaluate_host)),
            sink,
            grants,
            "test-hash".to_string(),
            manifest_mode,
        )
    }

    /// A landing that stays on-grant: the navigate result passes through unchanged, no denial.
    #[tokio::test]
    async fn point5_navigate_landing_on_grant_passes_through() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(governed_with_grants(
            vec![full_grant("g1", &["example.com"])],
            recorder as Arc<dyn AuditSink>,
        ));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![(
                "navigate",
                json!({ "content": [{ "type": "text", "text": "navigated" }] }),
            )],
            vec![(5, Some("https://example.com/"))],
        );
        wait_connected(&browser).await;

        let params = json!({
            "name": "navigate",
            "arguments": { "url": "https://example.com/", "tabId": 5 },
        });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let text = resp.result.as_ref().expect("tool result present")["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert_eq!(text, "navigated");
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["navigate", "tab_url_request:5"],
            "one dispatch, one point-5 re-query, no park"
        );
    }

    /// A landing that drifts off-grant (e.g. a redirect): the tab is best-effort parked on
    /// `about:blank`, the navigate result is replaced with a denial naming the FINAL host, and
    /// the audit record is a deny with the real elapsed duration (not the pre-dispatch `0`).
    #[tokio::test]
    async fn point5_navigate_landing_off_grant_parks_and_denies() {
        let path = temp_audit_path("point5-deny");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(governed_with_grants(
            vec![full_grant("g1", &["example.com"])],
            recorder as Arc<dyn AuditSink>,
        ));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![(
                "navigate",
                json!({ "content": [{ "type": "text", "text": "navigated" }] }),
            )],
            vec![(5, Some("https://evil.com/"))],
        );
        wait_connected(&browser).await;

        let params = json!({
            "name": "navigate",
            "arguments": { "url": "https://example.com/", "tabId": 5 },
        });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let text = resp.result.as_ref().expect("tool result present")["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert!(text.starts_with("Denied (D-"), "{text}");
        assert!(text.contains("evil.com"), "{text}");
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["navigate", "tab_url_request:5", "navigate"],
            "the original dispatch, the point-5 re-query, then the best-effort park"
        );

        let lines = read_lines(&path);
        assert_eq!(lines.len(), 1, "exactly one record for this call");
        assert_eq!(lines[0]["decision"], "deny");
        assert_eq!(lines[0]["domain"], "evil.com");
        assert_eq!(lines[0]["grant_id"], Value::Null);
        assert!(
            lines[0]["duration_ms"].as_u64().is_some(),
            "duration_ms present: {:?}",
            lines[0]["duration_ms"]
        );

        std::fs::remove_file(&path).ok();
    }

    /// ADR-0022 Decision 5 step 2 (free-action short-circuit): a governed "free action" -- one
    /// whose directory requirement is empty -- is allowed without resolving a governing resource,
    /// so no `tab_url` probe fires even when the call carries a `tabId` under an active manifest.
    /// `computer` `wait` is the sharp case: requirement `[]`, yet it carries a `tabId`, so before
    /// this short-circuit the grant path pointlessly probed the tab's URL. The fake extension
    /// registers NO `tab_url` answers, so a resource-resolution probe would panic; the explicit
    /// `seen` assertion gives a clearer failure if the short-circuit ever regresses.
    #[tokio::test]
    async fn governed_free_action_is_allowed_without_probing_the_tab_url() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(governed_with_grants(
            vec![full_grant("g1", &["example.com"])],
            recorder as Arc<dyn AuditSink>,
        ));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![(
                "computer",
                json!({ "content": [{ "type": "text", "text": "waited" }] }),
            )],
            vec![],
        );
        wait_connected(&browser).await;

        let params = json!({
            "name": "computer",
            "arguments": { "action": "wait", "tabId": 7, "duration": 1 },
        });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let result = resp.result.as_ref().expect("tool result present");
        assert_ne!(
            result["isError"], true,
            "a free action is allowed, never denied: {result:?}"
        );
        assert_eq!(
            result["content"][0]["text"], "waited",
            "the free action dispatched to the extension: {result:?}"
        );
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["computer"],
            "the free action dispatched once and NEVER probed a tab_url for grant resolution"
        );
    }

    /// t03 section 3 (ADR-0024 Decision 3): the pre-grant `audit.set_domain(tab_domain)` seeding
    /// that runs unconditionally right after the sacred check passes means an ALL-OPEN call on a
    /// resolvable, non-sacred tab still carries that tab's host on its allow record, even though
    /// all-open never resolves a GOVERNING resource at all (transcribes the pre-ADR-0024
    /// `audit_domain = tab_domain.clone()` pre-g13 seeding).
    #[tokio::test]
    async fn sacred_domain_seeding_survives_on_allow_records() {
        let path = temp_audit_path("sacred-seeding");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store = crate::governance::config::reload::ConfigStore::for_test_with_config(
            config_with_sacred_domains(&["*.mybank.com"]),
        );
        let browser = Browser::new();
        let (_ext, _seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![(
                "read_page",
                json!({ "content": [{ "type": "text", "text": "ok" }] }),
            )],
            vec![(5, Some("https://example.com/"))],
        );
        wait_connected(&browser).await;

        let params = json!({ "name": "read_page", "arguments": { "tabId": 5 } });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let result = resp.result.as_ref().expect("tool result present");
        assert_ne!(
            result["isError"], true,
            "example.com is not sacred: {result:?}"
        );

        let lines = read_lines(&path);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["decision"], "allow");
        assert_eq!(lines[0]["domain"], "example.com");

        std::fs::remove_file(&path).ok();
    }

    /// g15 constraint 9 (the sacred carve-out): a sacred-domain denial is ALWAYS a real
    /// `Deny`, never `ShadowDeny`, even when the active manifest's own mode is `observe`.
    /// Sacred denials never pass through `Decision`/`check_call` at all (a separate, always-on
    /// code path at the dispatch chokepoint, ahead of grant evaluation); this test pins the
    /// observable end-to-end behavior rather than relying on that structural fact alone.
    #[tokio::test]
    async fn sacred_domain_denies_even_under_an_observe_mode_manifest() {
        let path = temp_audit_path("sacred-under-observe");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(governed_with_grants_and_mode(
            vec![full_grant("g1", &["www.mybank.com"])],
            recorder as Arc<dyn AuditSink>,
            Some(crate::governance::ports::EffectiveMode::Observe),
        ));
        let store = crate::governance::config::reload::ConfigStore::for_test_with_config(
            config_with_sacred_domains(&["*.mybank.com"]),
        );
        let browser = Browser::new();
        let (_ext, _seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![],
            vec![(5, Some("https://www.mybank.com/account"))],
        );
        wait_connected(&browser).await;

        let params = json!({ "name": "read_page", "arguments": { "tabId": 5 } });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let text = resp.result.as_ref().expect("tool result present")["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert!(text.starts_with("Denied (D-"), "{text}");

        let lines = read_lines(&path);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0]["decision"], "deny",
            "a sacred denial is never shadow_deny, even under an observe-mode manifest"
        );

        std::fs::remove_file(&path).ok();
    }

    /// g15 required test 3/5 (non-sacred mode switch, inline variant): the SAME grant-based
    /// would-deny call, under an enforcing vs an observing manifest, yields `deny` (tool did
    /// not run) vs `shadow_deny` (tool ran, ordinary result, no `Denied (` text) with the
    /// IDENTICAL `grant_id`/`denial_id`. The subprocess-level equivalent
    /// (`tests/shadow_mode.rs`) additionally proves `duration_ms` truthfully differs (`0` vs
    /// real elapsed) using the real dispatch path with no extension connected; this inline
    /// version uses a fake extension so the observe-mode call can actually "execute". The
    /// would-deny call is `tabs_context_mcp` (domain-less, requires `read`, denied via the
    /// union rule) under a grant that permits `action`/`write` but not `read` (ADR-0022):
    /// `tabs_create_mcp`/`update_plan`/`resize_window` all require `[]` and short-circuit to
    /// Allow unconditionally, so they can no longer demonstrate a would-deny; `tabs_context_mcp`
    /// is the only domain-less tool with a non-empty capability requirement.
    #[tokio::test]
    async fn grant_shadow_deny_runs_the_tool_and_matches_the_enforce_denial_id() {
        let enforce_path = temp_audit_path("shadow-enforce");
        let observe_path = temp_audit_path("shadow-observe");
        let _ = std::fs::remove_file(&enforce_path);
        let _ = std::fs::remove_file(&observe_path);

        fn action_write_grant() -> Grant {
            let mut g = full_grant("r", &["example.com"]);
            g.allowed = vec![Capability::Action, Capability::Write];
            g
        }
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());

        // Enforce: the read-requiring call on a grant lacking 'read' is blocked outright.
        let enforce_recorder = Arc::new(Recorder::to_file(enforce_path.clone()));
        let enforce_governance = Arc::new(governed_with_grants_and_mode(
            vec![action_write_grant()],
            enforce_recorder as Arc<dyn AuditSink>,
            Some(crate::governance::ports::EffectiveMode::Enforce),
        ));
        let browser = Browser::new();
        let params = json!({ "name": "tabs_context_mcp", "arguments": {} });
        let enforce_resp = handle_tools_call(
            &browser,
            &store,
            &enforce_governance,
            Some(json!(1)),
            Some(&params),
        )
        .await;
        let enforce_text = enforce_resp.result.as_ref().expect("result")["content"][0]["text"]
            .as_str()
            .expect("text");
        assert!(enforce_text.starts_with("Denied (D-"), "{enforce_text}");
        let enforce_lines = read_lines(&enforce_path);
        assert_eq!(enforce_lines.len(), 1);
        assert_eq!(enforce_lines[0]["decision"], "deny");
        assert_eq!(enforce_lines[0]["duration_ms"], 0);

        // Observe: the identical call now dispatches (a fake extension answers it) and the
        // response carries no denial text at all.
        let observe_recorder = Arc::new(Recorder::to_file(observe_path.clone()));
        let observe_governance = Arc::new(governed_with_grants_and_mode(
            vec![action_write_grant()],
            observe_recorder as Arc<dyn AuditSink>,
            Some(crate::governance::ports::EffectiveMode::Observe),
        ));
        let observe_browser = Browser::new();
        let (_ext, _seen) = attach_fake_extension(
            &observe_browser,
            vec![(
                "tabs_context_mcp",
                json!({ "content": [{ "type": "text", "text": "created" }] }),
            )],
        );
        wait_connected(&observe_browser).await;
        let observe_resp = handle_tools_call(
            &observe_browser,
            &store,
            &observe_governance,
            Some(json!(1)),
            Some(&params),
        )
        .await;
        let observe_text = observe_resp.result.as_ref().expect("result")["content"][0]["text"]
            .as_str()
            .expect("text");
        assert_eq!(
            observe_text, "created",
            "shadow mode returns the ordinary tool result, no denial text: {observe_text}"
        );
        let observe_lines = read_lines(&observe_path);
        assert_eq!(observe_lines.len(), 1);
        assert_eq!(observe_lines[0]["decision"], "shadow_deny");
        assert!(
            observe_lines[0]["duration_ms"].as_u64().is_some(),
            "duration_ms present (a shadow-denied call ran, unlike an enforce deny's fixed 0): {:?}",
            observe_lines[0]["duration_ms"]
        );

        assert_eq!(
            enforce_lines[0]["grant_id"], observe_lines[0]["grant_id"],
            "enforce and observe must attribute the same grant"
        );
        assert_eq!(
            enforce_lines[0]["denial_id"], observe_lines[0]["denial_id"],
            "enforce and observe must derive the identical denial id"
        );

        std::fs::remove_file(&enforce_path).ok();
        std::fs::remove_file(&observe_path).ok();
    }

    // --- ADR-0022 Decision 7: the `explain` directory tool ---

    /// The full pinned `explain` response text, transcribed by hand from
    /// `browser::directory::REGISTRY` (26 variants) in fixture order. This is the ONE place the
    /// exact output is pinned; `directory::explain_text`'s own unit tests check only its
    /// structural shape.
    fn pinned_explain_text() -> String {
        [
            "Capabilities: read = retrieve and observe only; action = dispatch UI input whose \
             effect the page decides (this can trigger writes); write = declared \
             state-changing operations; execute = arbitrary code.",
            "",
            "tabs_context_mcp: requires read. List the MCP tab group: the ids, URLs, and \
             titles of the tabs this server controls.",
            "tabs_create_mcp: requires nothing. Open a new empty tab in the MCP tab group; \
             touches no page and no server.",
            "navigate: requires read. Load a URL in a tab, or go back or forward in its \
             history; a top-level GET.",
            "computer (left_click): requires action. Left-click at coordinates; commits an \
             activation whose effect the page decides.",
            "computer (right_click): requires action. Right-click at coordinates; commits an \
             activation.",
            "computer (type): requires action. Type text into the focused element; commits \
             data to page handlers.",
            "computer (screenshot): requires read. Capture a screenshot of the visible \
             viewport.",
            "computer (wait): requires nothing. Pause for a duration; touches no page and no \
             server.",
            "computer (scroll): requires read. Scroll the viewport; moves the view without \
             committing input to the page.",
            "computer (key): requires action. Press a key or key combination; commits input \
             to page handlers.",
            "computer (left_click_drag): requires action. Click and drag between two points; \
             commits pointer input to the page.",
            "computer (double_click): requires action. Double-click at coordinates; commits \
             an activation.",
            "computer (triple_click): requires action. Triple-click at coordinates; commits \
             an activation.",
            "computer (zoom): requires read. Capture a zoomed screenshot of a page region.",
            "computer (scroll_to): requires read. Scroll an element into view; moves the \
             viewport without committing input.",
            "computer (hover): requires read. Move the pointer over a point; commits no \
             activation and no data.",
            "find: requires read. Search the page for elements matching a natural-language \
             description.",
            "form_input: requires write. Fill or set values in form fields; a declared, \
             state-changing write.",
            "get_page_text: requires read. Extract the page's readable text content, \
             article-first, without HTML.",
            "javascript_tool: requires execute. Run arbitrary JavaScript in the page; \
             unbounded, and can bypass the UI entirely.",
            "read_console_messages: requires read. Read buffered browser console messages \
             from a tab.",
            "read_network_requests: requires read. Read buffered HTTP network requests \
             observed in a tab.",
            "read_page: requires read. Read the page as an accessibility tree of elements \
             with reference ids.",
            "resize_window: requires nothing. Resize the browser window; browser state only, \
             touches no page content.",
            "update_plan: requires nothing. Present a plan of intended actions to the user; \
             informational only.",
            "explain: requires nothing. Show every action available here and the capability \
             each one requires.",
        ]
        .join("\n")
    }

    /// `directory::explain_text` and the pinned expectation above must never drift apart: this
    /// is the tie between the hand-transcribed literal and the real implementation.
    #[test]
    fn pinned_explain_text_matches_the_real_directory_formatter() {
        assert_eq!(directory::explain_text(), pinned_explain_text());
    }

    /// The `explain` tool (ADR-0022 Decision 7) is handled entirely server-side: with NO
    /// extension attached at all, the call returns the exact pinned directory text and is
    /// audited as an ordinary allowed call with `capability: "none"`, `domain: null`, and a
    /// real (not hardcoded) `duration_ms`.
    #[tokio::test]
    async fn explain_returns_the_pinned_text_and_is_audited_as_allow_none() {
        let path = temp_audit_path("explain");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        // Deliberately never attached to any extension: a wrongly-dispatched `explain` would
        // hang out to the bounded handshake wait and fail with "not connected" instead of
        // returning instantly.
        assert!(!browser.is_connected());

        let params = json!({ "name": "explain", "arguments": {} });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let result = resp.result.as_ref().expect("tool result present");
        assert_ne!(result["isError"], true, "explain must never be isError");
        let text = result["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert_eq!(text, pinned_explain_text());

        let lines = read_lines(&path);
        assert_eq!(lines.len(), 1, "exactly one audit record");
        let rec = &lines[0];
        assert_eq!(rec["tool"], "explain");
        assert!(rec["action"].is_null());
        assert_eq!(rec["capability"], "none");
        assert_eq!(rec["decision"], "allow");
        assert!(rec["domain"].is_null());
        assert!(rec["grant_id"].is_null());
        assert!(rec["duration_ms"].as_u64().is_some(), "duration_ms present");

        std::fs::remove_file(&path).ok();
    }

    // --- t04 (ADR-0024 Decision 2): the generic ingest pipeline ---

    /// Test 2 (t04): a bogus tool name yields the exact current message and produces NO audit
    /// record; `explain` (a registry hit with a `Handler::Local`) still answers -- pinning that
    /// validity now comes from the registry (`directory::descriptor`), not a fixture re-parse.
    #[tokio::test]
    async fn unknown_tool_is_a_registry_miss() {
        let path = temp_audit_path("unknown-tool");
        let _ = std::fs::remove_file(&path);
        let recorder = Arc::new(Recorder::to_file(path.clone()));
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();

        let params = json!({ "name": "bogus_tool", "arguments": {} });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let result = resp.result.as_ref().expect("tool result present");
        assert_eq!(result["isError"], true, "unknown tool -> isError");
        let text = result["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert!(
            text.starts_with("[hop: invalid-request]"),
            "hop-attributed message: {text}"
        );
        assert!(text.contains("Unknown tool: bogus_tool"), "{text}");
        assert!(
            !path.exists(),
            "an unknown tool must produce no audit record"
        );

        let explain_params = json!({ "name": "explain", "arguments": {} });
        let explain_resp = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(2)),
            Some(&explain_params),
        )
        .await;
        let explain_result = explain_resp.result.as_ref().expect("tool result present");
        assert_ne!(
            explain_result["isError"], true,
            "explain is a registry hit and must never error"
        );
        let explain_text = explain_result["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert!(
            explain_text.starts_with("Capabilities: read = "),
            "{explain_text}"
        );

        std::fs::remove_file(&path).ok();
    }

    /// Test 3 (t04): a fake-extension `read_page` result containing a `secret_value=` marker is
    /// redacted (`descriptor.postprocess` fires); the identical payload via `find` (no
    /// `postprocess` hook on its descriptor row) is untouched. Marker/expected strings
    /// transcribed from `browser::redact`'s own fixture (`redact.rs`'s `LINE` const).
    #[tokio::test]
    async fn postprocess_fires_only_where_the_registry_says() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(Governance::all_open(recorder as Arc<dyn AuditSink>));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        let marked = "textbox \"Password\" [ref_3] secret_value=\"hunter2\" type=\"password\"\n\
                      textbox \"User\" [ref_2] value=\"alice\" type=\"text\"";
        let (_ext, _seen) = attach_fake_extension(
            &browser,
            vec![
                (
                    "read_page",
                    json!({ "content": [{ "type": "text", "text": marked }] }),
                ),
                (
                    "find",
                    json!({ "content": [{ "type": "text", "text": marked }] }),
                ),
            ],
        );
        wait_connected(&browser).await;

        let read_page_params = json!({ "name": "read_page", "arguments": { "tabId": 5 } });
        let read_page_resp = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(1)),
            Some(&read_page_params),
        )
        .await;
        let read_page_text = read_page_resp.result.as_ref().expect("tool result present")
            ["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert!(
            read_page_text.contains("value=\"[value redacted]\""),
            "{read_page_text}"
        );
        assert!(
            !read_page_text.contains("secret_value="),
            "{read_page_text}"
        );
        assert!(!read_page_text.contains("hunter2"), "{read_page_text}");
        assert!(
            read_page_text.contains("value=\"alice\""),
            "{read_page_text}"
        );

        let find_params = json!({ "name": "find", "arguments": { "tabId": 5 } });
        let find_resp = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(2)),
            Some(&find_params),
        )
        .await;
        let find_text = find_resp.result.as_ref().expect("tool result present")["content"][0]
            ["text"]
            .as_str()
            .expect("text content block");
        assert_eq!(
            find_text, marked,
            "find has no postprocess hook: the raw marker survives untouched"
        );
    }

    /// Test 4 (t04): with a governed store and a fake extension, `tabs_context_mcp`
    /// (`DomainLess`, requires `read`) resolves the union-rule path with NO `tab_url` probe; a
    /// `TabScoped` call (`read_page`) without a `tabId` denies fail-closed exactly as today
    /// (the `Indeterminate` resource's `unmatched_domain` denial over `"(unknown)"`).
    #[tokio::test]
    async fn resource_shape_drives_resolution() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(governed_with_grants(
            vec![full_grant("g1", &["example.com"])],
            recorder as Arc<dyn AuditSink>,
        ));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![(
                "tabs_context_mcp",
                json!({ "content": [{ "type": "text", "text": "ok" }] }),
            )],
            vec![], // no tab_url answers registered: a probe would panic
        );
        wait_connected(&browser).await;

        let params = json!({ "name": "tabs_context_mcp", "arguments": {} });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let result = resp.result.as_ref().expect("tool result present");
        assert_ne!(
            result["isError"], true,
            "the union rule allows via the g1 grant's read capability: {result:?}"
        );
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["tabs_context_mcp"],
            "a DomainLess resource never probes a tab_url"
        );

        let denied_params = json!({ "name": "read_page", "arguments": {} });
        let denied_resp = handle_tools_call(
            &browser,
            &store,
            &governance,
            Some(json!(2)),
            Some(&denied_params),
        )
        .await;
        let denied_text = denied_resp.result.as_ref().expect("tool result present")["content"][0]
            ["text"]
            .as_str()
            .expect("text content block");
        assert!(denied_text.starts_with("Denied (D-"), "{denied_text}");
        assert!(
            denied_text.contains("no grant covers (unknown)"),
            "a TabScoped call with no tabId fails closed via Indeterminate: {denied_text}"
        );
    }

    /// Verification pin (t04): a governed `navigate` with `{"url":"back","tabId":5}` consults
    /// the decision path with the union-rule resource (`GoverningResource::None`), same as a
    /// resolved host would -- the back/forward gloss in `resolve_governing_resource`'s
    /// `TargetArg` arm. The point-5 landing re-check still runs (the pre-check resolved
    /// `Some(...)`), so the final tab_url is probed too.
    #[tokio::test]
    async fn governed_navigate_back_consults_the_union_rule_resource() {
        let recorder = Arc::new(Recorder::disabled());
        let governance = Arc::new(governed_with_grants(
            vec![full_grant("g1", &["example.com"])],
            recorder as Arc<dyn AuditSink>,
        ));
        let store =
            crate::governance::config::reload::ConfigStore::for_test_with_config(Config::minimal());
        let browser = Browser::new();
        let (_ext, seen) = attach_fake_extension_with_tab_urls(
            &browser,
            vec![(
                "navigate",
                json!({ "content": [{ "type": "text", "text": "went back" }] }),
            )],
            vec![(5, Some("https://example.com/"))],
        );
        wait_connected(&browser).await;

        let params = json!({ "name": "navigate", "arguments": { "url": "back", "tabId": 5 } });
        let resp =
            handle_tools_call(&browser, &store, &governance, Some(json!(1)), Some(&params)).await;
        let text = resp.result.as_ref().expect("tool result present")["content"][0]["text"]
            .as_str()
            .expect("text content block");
        assert_eq!(text, "went back");
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["navigate", "tab_url_request:5"],
            "back/forward resolves the union-rule resource pre-dispatch (allowed by g1's read \
             capability), and the point-5 landing re-check still probes the final tab url"
        );
    }
}
