// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Integration tests for G13 per-call grant enforcement: no extension connected. A permitted call
//! reaches dispatch and returns the familiar `not connected` execution error; a denied call never
//! reaches dispatch and returns a `Denied (D-...)` text result instead -- that contrast is the test
//! signal.
//!
//! ADR-0051 Phase 4 (P4.2): all but one of these migrated from spawn-a-service-plus-adapter onto
//! the in-process `support::inproc::Harness`, which drives the SAME serve_session -> governance
//! decide -> dispatch path with no OS process. Governed cases carry their `audit.*` config in the
//! manifest pointing at a temp file; the harness's user-layer config resolution writes there
//! exactly as a `--manifest file://` spawn would, so every audit read is unchanged. The one
//! exception is `form_fill_without_extension_fails_with_parent_audit` (see its own doc): it needs
//! an ALL-OPEN session with audit enabled via the USER CONFIG FILE layer, which the in-process
//! harness does not override (that would take a process-global `GHOSTLIGHT_USER_CONFIG_DIR` env var,
//! racing every parallel in-process test in this binary), so it stays a spawn test and belongs to
//! the P4.3 quarantined end-to-end tier.
//!
//! `file:///etc/passwd` is deliberately NOT used for the scheme-denial scenario (unlike the g13
//! task doc's own worked example): the extension's `navigate` normalization -- which the g13
//! pre-dispatch check mirrors exactly, and which `browser::sacred::navigate_target_host`'s own
//! test suite already pins for the analogous `ftp://mybank.com/` case -- strips ANY
//! non-allowlisted scheme's `scheme:/+` prefix and retries the remainder as an `https://` host,
//! so `file:///etc/passwd` normalizes to `https://etc/passwd` (host `etc`), not a `file` scheme
//! denial. `chrome://settings` is used instead: `chrome:` is one of the four prefixes
//! (`about`/`chrome`/`edge`/`brave`) the extension recognizes and leaves untouched, so it reaches
//! the matcher as a genuine non-http(s) scheme.

mod support;

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use support::inproc::{by_id, init_and_call, manifest_from_value, text_of, Harness};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// A schema-3 manifest with `grants`, audit enabled and pointed at `audit_path` (so tests can
/// read back what was recorded), all at the user config layer (`level` is downgraded from
/// `mandatory` for a user-sourced manifest regardless, per `manifest::source`).
fn manifest_value(name: &str, grants: Value, audit_path: &Path) -> Value {
    json!({
        "schema": 3,
        "name": name,
        "version": "1",
        "grants": grants,
        "config": [
            { "key": "audit.enabled", "value": true, "level": "mandatory" },
            { "key": "audit.destination", "value": "file", "level": "mandatory" },
            { "key": "audit.file.path", "value": audit_path.to_string_lossy(), "level": "mandatory" },
        ],
    })
}

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-tool-enforcement-{}-{tag}-{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn read_audit_lines(path: &Path) -> Vec<Value> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content
        .lines()
        .map(|l| serde_json::from_str(l).expect("each audit line is a JSON object"))
        .collect()
}

/// Drive `requests` through an in-process session: governed by `manifest` (a manifest JSON `Value`
/// carrying its own audit config) when `Some`, all-open when `None`. Builds a FRESH
/// `support::inproc::Harness` per call, so two calls with the same manifest model two independent
/// sessions of the same policy version (the denial-id determinism test relies on this).
async fn drive(manifest: Option<&Value>, requests: &[Value]) -> Vec<Value> {
    let harness = match manifest {
        Some(value) => Harness::governed(manifest_from_value(value)),
        None => Harness::all_open(),
    };
    harness.drive(requests).await
}

/// Extract the `D-xxxxxxxx` id out of a `"... (D-xxxxxxxx): ..."`-shaped denial message.
fn extract_denial_id(text: &str) -> &str {
    let start = text.find("(D-").expect("a denial id in parens") + 1;
    let end = start + text[start..].find(')').expect("closing paren");
    &text[start..end]
}

const EXAMPLE_FULL_AND_RESEARCH_READ: &str = r#"[
    { "id": "example-full", "hosts": {"allow": ["example.com", "*.example.com"]}, "allowed": ["read", "action", "write"] },
    { "id": "research-read", "hosts": {"allow": ["research.example.org"]}, "allowed": ["read"] }
]"#;

/// Test 1 + test 2 + test 7 (g13 doc "Integration tests"): a permitted call passes policy (it
/// reaches dispatch and gets the ordinary `not connected` execution error, never `Denied (`); a
/// denied domain never reaches dispatch and returns `Denied (D-...` naming `no grant covers`;
/// the audit file records both outcomes with the right `grant_id`/`denial_id`/`duration_ms`.
#[tokio::test]
async fn permitted_call_passes_and_denied_domain_is_denied_with_matching_audit() {
    let audit_path = temp_path("case12-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = manifest_value("case12", grants, &audit_path);

    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://example.com/","tabId":1}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://evil.com/","tabId":1}}}),
    ];
    let responses = drive(Some(&manifest), &requests).await;
    assert_eq!(responses.len(), 3, "got {responses:?}");

    let permitted = by_id(&responses, 2);
    assert_eq!(
        permitted["result"]["isError"], true,
        "the permitted call reaches dispatch and fails at execution (no extension): {permitted:?}"
    );
    let permitted_text = text_of(permitted);
    assert!(
        permitted_text.starts_with("[hop: extension]") && permitted_text.contains("not connected"),
        "policy allowed it through to dispatch: {permitted_text}"
    );

    let denied = by_id(&responses, 3);
    assert_ne!(denied["result"]["isError"], true, "a denial is not isError");
    let denied_text = text_of(denied);
    assert!(denied_text.starts_with("Denied (D-"), "{denied_text}");
    assert!(
        denied_text.contains("no grant covers evil.com"),
        "{denied_text}"
    );
    let denial_id = extract_denial_id(denied_text).to_string();

    // Calls dispatch concurrently, so the deny record (near-instant) can land in the file before
    // the allow record (which waits out the extension-handshake window); find each by outcome,
    // never by position.
    let lines = read_audit_lines(&audit_path);
    assert_eq!(lines.len(), 2, "one record per call: {lines:?}");
    let allow_line = lines
        .iter()
        .find(|l| l["decision"] == "allow")
        .unwrap_or_else(|| panic!("no allow record in {lines:?}"));
    let deny_line = lines
        .iter()
        .find(|l| l["decision"] == "deny")
        .unwrap_or_else(|| panic!("no deny record in {lines:?}"));
    assert_eq!(allow_line["grant_id"], "example-full");
    assert_eq!(allow_line["capability"], "read");
    assert_eq!(deny_line["grant_id"], Value::Null);
    assert_eq!(deny_line["capability"], "read");
    assert_eq!(deny_line["duration_ms"], 0);
    assert_eq!(deny_line["denial_id"], denial_id);

    std::fs::remove_file(&audit_path).ok();
}

/// Test 3: a call requiring `read` (`tabs_context_mcp`, domain-less, via the union rule) under a
/// grant that permits `action`/`write` but not `read` denies `capability`, naming the grant and
/// the missing capability. `tabs_create_mcp` no longer serves as the would-deny example here: it
/// requires `[]` under ADR-0022 and short-circuits to Allow unconditionally, so it can no longer
/// demonstrate a union-rule denial (see `navigate_is_permitted_on_a_read_only_grant` for the
/// analogous `navigate` reclassification). `EXAMPLE_FULL_AND_RESEARCH_READ` is deliberately not
/// reused here: its all-access `example-full` grant would satisfy the union rule and mask the
/// denial.
#[tokio::test]
async fn denied_capability_names_the_grant_and_the_missing_capability() {
    let audit_path = temp_path("case3-audit");
    let grants = json!([{
        "id": "research-write",
        "hosts": {"allow": ["research.example.org"]},
        "allowed": ["action", "write"]
    }]);
    let manifest = manifest_value("case3", grants, &audit_path);

    let responses = drive(
        Some(&manifest),
        &init_and_call("tabs_context_mcp", json!({})),
    )
    .await;
    let denied_text = text_of(by_id(&responses, 2));
    assert!(denied_text.starts_with("Denied (D-"), "{denied_text}");
    assert!(denied_text.contains("research-write"), "{denied_text}");
    assert!(
        denied_text.contains("needs the 'read' capability"),
        "{denied_text}"
    );

    std::fs::remove_file(&audit_path).ok();
}

/// The s01 bugfix pin: `navigate` on a read-only grant's own domain is now PERMITTED (it is
/// provably a GET; ADR-0022 Context + Decision 2), reaching dispatch (the ordinary no-extension
/// `not connected` execution error) instead of a `Denied (` text result, and the audit line
/// records `decision: allow`, the covering grant, and `capability: read`.
#[tokio::test]
async fn navigate_is_permitted_on_a_read_only_grant() {
    let audit_path = temp_path("case-navigate-read-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = manifest_value("case-navigate-read", grants, &audit_path);

    let responses = drive(
        Some(&manifest),
        &init_and_call(
            "navigate",
            json!({"url": "https://research.example.org/", "tabId": 1}),
        ),
    )
    .await;
    let permitted = by_id(&responses, 2);
    assert_eq!(
        permitted["result"]["isError"], true,
        "reaches dispatch and fails at execution (no extension): {permitted:?}"
    );
    let permitted_text = text_of(permitted);
    assert!(
        permitted_text.starts_with("[hop: extension]") && permitted_text.contains("not connected"),
        "policy allowed it through to dispatch: {permitted_text}"
    );
    assert!(!permitted_text.starts_with("Denied ("), "{permitted_text}");

    let lines = read_audit_lines(&audit_path);
    assert_eq!(lines.len(), 1, "one record for the call: {lines:?}");
    assert_eq!(lines[0]["decision"], "allow");
    assert_eq!(lines[0]["grant_id"], "research-read");
    assert_eq!(lines[0]["capability"], "read");
    assert_eq!(lines[0]["domain"], "research.example.org");

    std::fs::remove_file(&audit_path).ok();
}

/// Test 4: a non-http(s) target the extension leaves untouched (`chrome:`, one of the four
/// allowlisted prefixes) denies with the scheme wording. See the module doc for why this uses
/// `chrome://settings` rather than the task doc's own `file:///etc/passwd` example.
#[tokio::test]
async fn denied_scheme_names_the_scheme() {
    let audit_path = temp_path("case4-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = manifest_value("case4", grants, &audit_path);

    let responses = drive(
        Some(&manifest),
        &init_and_call("navigate", json!({"url": "chrome://settings/", "tabId": 1})),
    )
    .await;
    let denied_text = text_of(by_id(&responses, 2));
    assert!(denied_text.starts_with("Denied (D-"), "{denied_text}");
    assert!(denied_text.contains("'chrome:'"), "{denied_text}");

    std::fs::remove_file(&audit_path).ok();
}

/// Test 5: a tab-scoped call whose tab URL cannot be resolved (no extension connected) fails
/// closed -- a denial, never the `not connected` execution error.
#[tokio::test]
async fn fail_closed_when_tab_url_is_unknowable() {
    let audit_path = temp_path("case5-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = manifest_value("case5", grants, &audit_path);

    let responses = drive(
        Some(&manifest),
        &init_and_call("read_page", json!({"tabId": 1})),
    )
    .await;
    let text = text_of(by_id(&responses, 2));
    assert!(text.starts_with("Denied (D-"), "{text}");
    assert!(!text.contains("not connected"), "{text}");

    std::fs::remove_file(&audit_path).ok();
}

/// Test 6: the `NoPage` union rule, end to end. `tabs_context_mcp` (the only domain-less tool
/// with a non-empty capability requirement under ADR-0022: `tabs_create_mcp`/`update_plan`/
/// `resize_window` all require `[]` and short-circuit to Allow unconditionally) is allowed
/// (reaches `not connected`) under a grant that includes `read`; the same call is denied under a
/// grant that omits it.
#[tokio::test]
async fn union_rule_end_to_end() {
    let all_audit = temp_path("case6-all-audit");
    let all_grants = json!([{
        "id": "g-all",
        "hosts": {"allow": ["example.com"]},
        "allowed": ["read", "action", "write"]
    }]);
    let all_manifest = manifest_value("case6-all", all_grants, &all_audit);
    let responses = drive(
        Some(&all_manifest),
        &init_and_call("tabs_context_mcp", json!({})),
    )
    .await;
    let allowed_text = text_of(by_id(&responses, 2));
    assert!(
        allowed_text.starts_with("[hop: extension]") && allowed_text.contains("not connected"),
        "allowed under a grant that includes read: {allowed_text}"
    );

    let write_audit = temp_path("case6-write-audit");
    let write_grants = json!([{
        "id": "g-write",
        "hosts": {"allow": ["example.com"]},
        "allowed": ["action", "write"]
    }]);
    let write_manifest = manifest_value("case6-write", write_grants, &write_audit);
    let responses = drive(
        Some(&write_manifest),
        &init_and_call("tabs_context_mcp", json!({})),
    )
    .await;
    let denied_text = text_of(by_id(&responses, 2));
    assert!(denied_text.starts_with("Denied (D-"), "{denied_text}");

    for p in [all_audit, write_audit] {
        std::fs::remove_file(p).ok();
    }
}

/// Test 8: the all-open invariant. With no `--manifest` at all, behavior is byte-identical to
/// today (21 tools -- the 13 trained tools plus `wait_for`, `script`, `form_fill`, `file_upload`,
/// `browser_batch`, `upload_image`, `gif_creator`, and the ADR-0022 Decision 7 `explain` addition -- fixture
/// identity, `not connected` execution error) and no `Denied (` text ever appears (the count itself
/// derives from `directory::advertised_tool_count()`, so this narration is descriptive, not a pin).
#[tokio::test]
async fn all_open_invariant_no_manifest_means_no_denials() {
    let responses = drive(
        None,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            // o04: inputSchema validation now runs before dispatch; navigate needs url + tabId.
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://example.com","tabId":1}}}),
        ],
    )
    .await;
    assert_eq!(responses.len(), 3, "got {responses:?}");

    let list = by_id(&responses, 2);
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        ghostlight::browser::directory::advertised_tool_count(),
        "the wire advertises the full REGISTRY surface (see directory::advertised_tool_names)"
    );
    let fixture = ghostlight::mcp::tools::advertised_tools_json();
    assert_eq!(list["result"], fixture, "byte-identical tools/list");

    let call = by_id(&responses, 3);
    let call_text = text_of(call);
    assert_eq!(call["result"]["isError"], true, "no extension -> isError");
    assert!(call_text.contains("not connected"), "{call_text}");

    for resp in &responses {
        let text = resp["result"]["content"]
            .as_array()
            .and_then(|c| c.first())
            .and_then(|c| c["text"].as_str())
            .unwrap_or("");
        assert!(
            !text.starts_with("Denied ("),
            "no denial under all-open: {text}"
        );
    }
}

/// Denial-id determinism (ADR-0020): the same denied call, driven twice within one session and
/// again in a second independent session against the SAME manifest, gets the identical `D-...` id
/// every time (the id derives from the manifest's content hash, not from any per-session state).
#[tokio::test]
async fn denial_id_is_deterministic_within_and_across_sessions() {
    let audit_path = temp_path("case-determinism-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = manifest_value("case-determinism", grants, &audit_path);

    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://evil.com/","tabId":1}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://evil.com/","tabId":1}}}),
    ];
    let first_run = drive(Some(&manifest), &requests).await;
    let id_a = extract_denial_id(text_of(by_id(&first_run, 2))).to_string();
    let id_b = extract_denial_id(text_of(by_id(&first_run, 3))).to_string();
    assert_eq!(id_a, id_b, "same id within one session");

    let second_run = drive(Some(&manifest), &requests[..2]).await;
    let id_c = extract_denial_id(text_of(by_id(&second_run, 2))).to_string();
    assert_eq!(
        id_a, id_c,
        "same id across a fresh session of the same manifest"
    );

    std::fs::remove_file(&audit_path).ok();
}

/// ADR-0022 Decision 2/5: a call whose bound requirement set is empty (`tabs_create_mcp`,
/// `requires: []`) short-circuits to allow BEFORE any grant scan -- proven here under a
/// manifest with an empty `grants` array (nothing could possibly cover it if grants were
/// consulted). The call reaches ordinary dispatch (the familiar `not connected` execution
/// error), never a `Denied (` text result, and the single audit line records `capability:
/// "none"`, no attributed grant.
#[tokio::test]
async fn requires_empty_call_records_capability_none() {
    let audit_path = temp_path("case-requires-empty-audit");
    let manifest = manifest_value("case-requires-empty", json!([]), &audit_path);

    let responses = drive(
        Some(&manifest),
        &init_and_call("tabs_create_mcp", json!({})),
    )
    .await;
    let resp = by_id(&responses, 2);
    assert_eq!(
        resp["result"]["isError"], true,
        "reaches dispatch and fails at execution (no extension): {resp:?}"
    );
    let text = text_of(resp);
    assert!(text.contains("not connected"), "{text}");
    assert!(!text.starts_with("Denied ("), "{text}");

    let lines = read_audit_lines(&audit_path);
    assert_eq!(lines.len(), 1, "one record for the call: {lines:?}");
    assert_eq!(lines[0]["decision"], "allow");
    assert_eq!(lines[0]["capability"], "none");
    assert_eq!(lines[0]["grant_id"], Value::Null);
    assert_eq!(lines[0]["held"], false);

    std::fs::remove_file(&audit_path).ok();
}

/// ADR-0024 Decision 3, the sanctioned delta this task owns: a GOVERNED directory miss (a known
/// tool, unknown sub-action) is DENIED with `unknown_action` instead of dispatching ungoverned --
/// the deliberate fix of the `b4b2faf` fail-open regression, restoring ADR-0022's
/// absent-means-DENY. `computer` with an unrecognized `action` string is the concrete case: the
/// extension's own schema would reject it too, but governance must not rely on that, and the
/// denial fires before any extension traffic (no probe, no dispatch).
#[tokio::test]
async fn governed_unknown_computer_action_is_denied_unknown_action() {
    let audit_path = temp_path("case-unknown-action-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = manifest_value("case-unknown-action", grants, &audit_path);

    let responses = drive(
        Some(&manifest),
        &init_and_call("computer", json!({ "action": "bogus_action", "tabId": 1 })),
    )
    .await;
    let resp = by_id(&responses, 2);
    assert_ne!(
        resp["result"]["isError"], true,
        "a denial is not isError: {resp:?}"
    );
    let text = text_of(resp);
    assert!(text.starts_with("Denied (D-"), "{text}");
    assert!(
        text.contains("computer (bogus_action)"),
        "the label must name the tool and the unknown action: {text}"
    );
    let denial_id = extract_denial_id(text).to_string();

    let lines = read_audit_lines(&audit_path);
    assert_eq!(lines.len(), 1, "one record for the call: {lines:?}");
    assert_eq!(lines[0]["decision"], "deny");
    assert_eq!(lines[0]["capability"], "none");
    assert_eq!(lines[0]["denial_id"], denial_id);

    std::fs::remove_file(&audit_path).ok();
}

// --- C10 (ADR-0036, PINS.md SS13): form_fill's parent-level governance decision ---

/// ADR-0036 Decision 4: `form_fill`'s ONE governance decision covers the whole interaction, at
/// the parent call, before anything dispatches. A manifest whose only grant lacks `write` (the
/// capability `form_fill`'s default variant requires alongside `read`) denies the parent call --
/// exactly one denial, and (proven by the audit log) no internal execution ever started: no
/// `form_structure`/`form_input` record exists alongside it.
#[tokio::test]
async fn form_fill_denied_upfront_under_write_deny() {
    let audit_path = temp_path("case-form-fill-write-deny-audit");
    let grants = json!([{
        "id": "read-action-only",
        "hosts": {"allow": ["example.com"]},
        "allowed": ["read", "action"]
    }]);
    let manifest = manifest_value("case-form-fill-write-deny", grants, &audit_path);

    let responses = drive(
        Some(&manifest),
        &init_and_call(
            "form_fill",
            json!({"tabId": 1, "fields": {"Email": "a@b.c"}}),
        ),
    )
    .await;
    let resp = by_id(&responses, 2);
    assert_ne!(
        resp["result"]["isError"], true,
        "a denial is not isError: {resp:?}"
    );
    let text = text_of(resp);
    assert!(text.starts_with("Denied (D-"), "{text}");

    let lines = read_audit_lines(&audit_path);
    assert_eq!(
        lines.len(),
        1,
        "one denial record, no partial-fill internals: {lines:?}"
    );
    assert_eq!(lines[0]["tool"], "form_fill");
    assert_eq!(lines[0]["decision"], "deny");
    assert!(
        !lines
            .iter()
            .any(|l| l["tool"] == "form_structure" || l["tool"] == "form_input"),
        "no internal execution ever started: {lines:?}"
    );

    std::fs::remove_file(&audit_path).ok();
}

/// ADR-0036 Decision 5/7: under ALL-OPEN (no manifest at all -- `form_fill` is `TabScoped`, so
/// under any GOVERNED manifest a call with no extension connected fails closed before the
/// handler ever runs, since the tab's URL cannot be resolved; this scenario needs the grant gate
/// skipped entirely), the parent call reaches its handler, which dispatches the dedicated
/// `form_structure` internal read; with no extension connected that dispatch itself fails, and
/// the result is an isError text naming the extension hop. The correlated audit still carries
/// BOTH the parent `form_fill` record (batch_id set, no action, capability from the `action: None`
/// variant) and the `form_structure` step record (orchestrator `form_fill`, same batch_id, step
/// 1, a real -- not hardcoded -- duration_ms, since the failed internal read still completes its
/// own scope).
///
/// STAYS a spawn test (ADR-0051 Phase 4, quarantined E2E tier): audit cannot be turned on via a
/// manifest here (any manifest at all makes the session governed, which would deny `form_fill`
/// upfront per the note above), so it drives the USER CONFIG FILE layer
/// (`GHOSTLIGHT_USER_CONFIG_DIR`) to enable audit while leaving the session manifest-less
/// (`Governance::all_open`). The in-process harness deliberately does not override that env var
/// (it is process-global and would race every parallel in-process test in this binary), so this
/// one case keeps the spawn-a-service pattern.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn form_fill_without_extension_fails_with_parent_audit() {
    let pid = std::process::id();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let audit_path = temp_path("case-form-fill-no-ext-audit");
    let _ = std::fs::remove_file(&audit_path);

    let user_config_dir = std::env::temp_dir().join(format!(
        "ghostlight-tool-enforcement-no-ext-config-{pid}-{seq}"
    ));
    std::fs::create_dir_all(user_config_dir.join("ghostlight")).unwrap();
    std::fs::write(
        user_config_dir.join("ghostlight").join("config.json"),
        serde_json::to_vec(&json!({
            "config": {
                "audit.enabled": true,
                "audit.destination": "file",
                "audit.file.path": audit_path.to_string_lossy(),
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let endpoint = format!("ghostlight-ge-noext-{pid}-{seq}");
    // The web API listener is unused by this test, but
    // `spawn_service_with_user_config_dir_and_webapi_port` is the one existing support helper that
    // spawns an ALL-OPEN service (no `--manifest`) with an isolated `GHOSTLIGHT_USER_CONFIG_DIR`;
    // it binds an OS-assigned port, which this test simply ignores.
    let (mut service, _port) =
        support::spawn_service_with_user_config_dir_and_webapi_port(&endpoint, &user_config_dir);
    let mut adapter = support::spawn_adapter(&endpoint);

    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    let requests = init_and_call(
        "form_fill",
        json!({"tabId": 0, "fields": {"Email": "a@b.c"}}),
    );
    for req in &requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    let expected = requests.iter().filter(|r| r.get("id").is_some()).count();
    let stdout = adapter.stdout.take().expect("adapter stdout");
    let mut lines_reader = BufReader::new(stdout).lines();
    let responses: Vec<Value> = (0..expected)
        .map(|_| {
            let line = lines_reader
                .next()
                .expect("the adapter's stdout closed before every expected reply arrived")
                .unwrap();
            serde_json::from_str(&line).expect("each stdout line is JSON")
        })
        .collect();

    drop(stdin);
    let _ = adapter.wait();
    let _ = service.kill();
    let _ = service.wait();

    let call = by_id(&responses, 2);
    assert_eq!(
        call["result"]["isError"], true,
        "no extension -> isError: {call:?}"
    );
    let text = text_of(call);
    assert!(text.contains("extension"), "{text}");

    let audit_lines = read_audit_lines(&audit_path);
    let parent = audit_lines
        .iter()
        .find(|l| l["tool"] == "form_fill")
        .unwrap_or_else(|| panic!("no form_fill parent record in {audit_lines:?}"));
    assert!(parent["batch_id"].is_string(), "parent batch_id set");
    assert!(parent["action"].is_null(), "parent action is null");
    assert_eq!(
        parent["capability"], "read",
        "capability from the action:None variant [read, write]"
    );
    let batch_id = parent["batch_id"].as_str().unwrap();

    let structure = audit_lines
        .iter()
        .find(|l| l["tool"] == "form_structure")
        .unwrap_or_else(|| panic!("no form_structure step record in {audit_lines:?}"));
    assert_eq!(structure["orchestrator"], "form_fill");
    assert_eq!(structure["batch_id"], batch_id);
    assert_eq!(structure["step"], 1);
    assert!(
        structure["duration_ms"].is_u64(),
        "a real duration_ms, not hardcoded: {structure:?}"
    );

    std::fs::remove_file(&audit_path).ok();
    std::fs::remove_dir_all(&user_config_dir).ok();
}
