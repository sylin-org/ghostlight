//! Integration tests for G13 per-call grant enforcement: end to end over stdio, no extension
//! connected. A permitted call reaches dispatch and returns the familiar `not connected`
//! execution error; a denied call never reaches dispatch and returns a `Denied (D-...)` text
//! result instead -- that contrast is the test signal (mirrors `tests/mcp_protocol.rs`'s own
//! subprocess pattern, one unique `BROWSER_MCP_ENDPOINT` per spawn).
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

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// Build the `file://` source-string form `governance::manifest::source::parse_source_string`
/// expects, on either platform: three slashes after the scheme, with the Windows drive-letter
/// convenience (`file:///C:/...`) or a bare Unix absolute path (`file:///tmp/...`).
fn file_uri(path: &Path) -> String {
    let forward = path.to_string_lossy().replace('\\', "/");
    match forward.strip_prefix('/') {
        Some(rest) => format!("file:///{rest}"),
        None => format!("file:///{forward}"),
    }
}

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
        "browser-mcp-tool-enforcement-{}-{tag}-{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn write_manifest(tag: &str, value: &Value) -> PathBuf {
    let path = temp_path(&format!("{tag}-manifest")).with_extension("json");
    std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

fn read_audit_lines(path: &Path) -> Vec<Value> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content
        .lines()
        .map(|l| serde_json::from_str(l).expect("each audit line is a JSON object"))
        .collect()
}

/// Spawn the binary with `--manifest file://<manifest_path>` (or no `--manifest` at all when
/// `manifest_path` is `None`, the all-open case), drive `requests` over stdio, and collect the
/// response lines.
fn drive(manifest_path: Option<&Path>, requests: &[Value]) -> Vec<Value> {
    let endpoint = format!(
        "browser-mcp-ge-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_browser-mcp"));
    cmd.env("BROWSER_MCP_ENDPOINT", &endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(path) = manifest_path {
        cmd.arg("--manifest").arg(file_uri(path));
    }
    let mut child = cmd.spawn().expect("spawn browser-mcp");

    let mut stdin = child.stdin.take().expect("stdin");
    for req in requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    drop(stdin); // EOF -> the server loop ends

    let stdout = child.stdout.take().expect("stdout");
    let responses: Vec<Value> = BufReader::new(stdout)
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).expect("each stdout line is JSON"))
        .collect();
    child.wait().expect("wait for child");
    responses
}

fn text_of(resp: &Value) -> &str {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content block in {resp:?}"))
}

/// Extract the `D-xxxxxxxx` id out of a `"... (D-xxxxxxxx): ..."`-shaped denial message.
fn extract_denial_id(text: &str) -> &str {
    let start = text.find("(D-").expect("a denial id in parens") + 1;
    let end = start + text[start..].find(')').expect("closing paren");
    &text[start..end]
}

/// `tools/call` runs concurrently (each spawns its own task; see `transport::mcp::server`'s
/// module doc), so response order does not track request order -- a denied call (near-instant)
/// can and does finish before a permitted one still waiting out the bounded extension-handshake
/// window. Every test driving more than one `tools/call` must look responses up by `id`, never
/// by position.
fn by_id(responses: &[Value], id: i64) -> &Value {
    responses
        .iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("no response with id {id} in {responses:?}"))
}

fn init_and_call(name: &str, arguments: Value) -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
    ]
}

const EXAMPLE_FULL_AND_RESEARCH_READ: &str = r#"[
    { "id": "example-full", "hosts": {"allow": ["example.com", "*.example.com"]}, "allowed": ["read", "action", "write"] },
    { "id": "research-read", "hosts": {"allow": ["research.example.org"]}, "allowed": ["read"] }
]"#;

/// Test 1 + test 2 + test 7 (g13 doc "Integration tests"): a permitted call passes policy (it
/// reaches dispatch and gets the ordinary `not connected` execution error, never `Denied (`); a
/// denied domain never reaches dispatch and returns `Denied (D-...` naming `no grant covers`;
/// the audit file records both outcomes with the right `grant_id`/`denial_id`/`duration_ms`.
#[test]
fn permitted_call_passes_and_denied_domain_is_denied_with_matching_audit() {
    let audit_path = temp_path("case12-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = write_manifest("case12", &manifest_value("case12", grants, &audit_path));

    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://example.com/","tabId":1}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://evil.com/","tabId":1}}}),
    ];
    let responses = drive(Some(&manifest), &requests);
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
    std::fs::remove_file(&manifest).ok();
}

/// Test 3: a call requiring `read` (`tabs_context_mcp`, domain-less, via the union rule) under a
/// grant that permits `action`/`write` but not `read` denies `capability`, naming the grant and
/// the missing capability. `tabs_create_mcp` no longer serves as the would-deny example here: it
/// requires `[]` under ADR-0022 and short-circuits to Allow unconditionally, so it can no longer
/// demonstrate a union-rule denial (see `navigate_is_permitted_on_a_read_only_grant` for the
/// analogous `navigate` reclassification). `EXAMPLE_FULL_AND_RESEARCH_READ` is deliberately not
/// reused here: its all-access `example-full` grant would satisfy the union rule and mask the
/// denial.
#[test]
fn denied_capability_names_the_grant_and_the_missing_capability() {
    let audit_path = temp_path("case3-audit");
    let grants = json!([{
        "id": "research-write",
        "hosts": {"allow": ["research.example.org"]},
        "allowed": ["action", "write"]
    }]);
    let manifest = write_manifest("case3", &manifest_value("case3", grants, &audit_path));

    let responses = drive(
        Some(&manifest),
        &init_and_call("tabs_context_mcp", json!({})),
    );
    let denied_text = text_of(&responses[1]);
    assert!(denied_text.starts_with("Denied (D-"), "{denied_text}");
    assert!(denied_text.contains("research-write"), "{denied_text}");
    assert!(
        denied_text.contains("needs the 'read' capability"),
        "{denied_text}"
    );

    std::fs::remove_file(&audit_path).ok();
    std::fs::remove_file(&manifest).ok();
}

/// The s01 bugfix pin: `navigate` on a read-only grant's own domain is now PERMITTED (it is
/// provably a GET; ADR-0022 Context + Decision 2), reaching dispatch (the ordinary no-extension
/// `not connected` execution error) instead of a `Denied (` text result, and the audit line
/// records `decision: allow`, the covering grant, and `capability: read`.
#[test]
fn navigate_is_permitted_on_a_read_only_grant() {
    let audit_path = temp_path("case-navigate-read-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = write_manifest(
        "case-navigate-read",
        &manifest_value("case-navigate-read", grants, &audit_path),
    );

    let responses = drive(
        Some(&manifest),
        &init_and_call(
            "navigate",
            json!({"url": "https://research.example.org/", "tabId": 1}),
        ),
    );
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
    std::fs::remove_file(&manifest).ok();
}

/// Test 4: a non-http(s) target the extension leaves untouched (`chrome:`, one of the four
/// allowlisted prefixes) denies with the scheme wording. See the module doc for why this uses
/// `chrome://settings` rather than the task doc's own `file:///etc/passwd` example.
#[test]
fn denied_scheme_names_the_scheme() {
    let audit_path = temp_path("case4-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = write_manifest("case4", &manifest_value("case4", grants, &audit_path));

    let responses = drive(
        Some(&manifest),
        &init_and_call("navigate", json!({"url": "chrome://settings/", "tabId": 1})),
    );
    let denied_text = text_of(&responses[1]);
    assert!(denied_text.starts_with("Denied (D-"), "{denied_text}");
    assert!(denied_text.contains("'chrome:'"), "{denied_text}");

    std::fs::remove_file(&audit_path).ok();
    std::fs::remove_file(&manifest).ok();
}

/// Test 5: a tab-scoped call whose tab URL cannot be resolved (no extension connected) fails
/// closed -- a denial, never the `not connected` execution error.
#[test]
fn fail_closed_when_tab_url_is_unknowable() {
    let audit_path = temp_path("case5-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = write_manifest("case5", &manifest_value("case5", grants, &audit_path));

    let responses = drive(
        Some(&manifest),
        &init_and_call("read_page", json!({"tabId": 1})),
    );
    let text = text_of(&responses[1]);
    assert!(text.starts_with("Denied (D-"), "{text}");
    assert!(!text.contains("not connected"), "{text}");

    std::fs::remove_file(&audit_path).ok();
    std::fs::remove_file(&manifest).ok();
}

/// Test 6: the `NoPage` union rule, end to end. `tabs_context_mcp` (the only domain-less tool
/// with a non-empty capability requirement under ADR-0022: `tabs_create_mcp`/`update_plan`/
/// `resize_window` all require `[]` and short-circuit to Allow unconditionally) is allowed
/// (reaches `not connected`) under a grant that includes `read`; the same call is denied under a
/// grant that omits it.
#[test]
fn union_rule_end_to_end() {
    let all_audit = temp_path("case6-all-audit");
    let all_grants = json!([{
        "id": "g-all",
        "hosts": {"allow": ["example.com"]},
        "allowed": ["read", "action", "write"]
    }]);
    let all_manifest = write_manifest(
        "case6-all",
        &manifest_value("case6-all", all_grants, &all_audit),
    );
    let responses = drive(
        Some(&all_manifest),
        &init_and_call("tabs_context_mcp", json!({})),
    );
    let allowed_text = text_of(&responses[1]);
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
    let write_manifest_path = write_manifest(
        "case6-write",
        &manifest_value("case6-write", write_grants, &write_audit),
    );
    let responses = drive(
        Some(&write_manifest_path),
        &init_and_call("tabs_context_mcp", json!({})),
    );
    let denied_text = text_of(&responses[1]);
    assert!(denied_text.starts_with("Denied (D-"), "{denied_text}");

    for p in [all_audit, all_manifest, write_audit, write_manifest_path] {
        std::fs::remove_file(p).ok();
    }
}

/// Test 8: the all-open invariant. With no `--manifest` at all, behavior is byte-identical to
/// today (14 tools -- the 13 trained tools plus the ADR-0022 Decision 7 `explain` addition --
/// fixture identity, `not connected` execution error) and no `Denied (` text ever appears.
#[test]
fn all_open_invariant_no_manifest_means_no_denials() {
    let responses = drive(
        None,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{}}}),
        ],
    );
    assert_eq!(responses.len(), 3, "got {responses:?}");

    let tools = responses[1]["result"]["tools"]
        .as_array()
        .expect("tools array");
    assert_eq!(tools.len(), 14, "13 trained tools plus explain");
    let fixture: Value = serde_json::from_str(browser_mcp::mcp::tools::TOOLS_JSON).unwrap();
    assert_eq!(responses[1]["result"], fixture, "byte-identical tools/list");

    let call_text = text_of(&responses[2]);
    assert_eq!(
        responses[2]["result"]["isError"], true,
        "no extension -> isError"
    );
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
/// again across a second spawn against the SAME manifest file, gets the identical `D-...` id
/// every time.
#[test]
fn denial_id_is_deterministic_within_and_across_sessions() {
    let audit_path = temp_path("case-determinism-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = write_manifest(
        "case-determinism",
        &manifest_value("case-determinism", grants, &audit_path),
    );

    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://evil.com/","tabId":1}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://evil.com/","tabId":1}}}),
    ];
    let first_run = drive(Some(&manifest), &requests);
    let id_a = extract_denial_id(text_of(by_id(&first_run, 2))).to_string();
    let id_b = extract_denial_id(text_of(by_id(&first_run, 3))).to_string();
    assert_eq!(id_a, id_b, "same id within one session");

    let second_run = drive(Some(&manifest), &requests[..2]);
    let id_c = extract_denial_id(text_of(by_id(&second_run, 2))).to_string();
    assert_eq!(
        id_a, id_c,
        "same id across a fresh spawn of the same manifest file"
    );

    std::fs::remove_file(&audit_path).ok();
    std::fs::remove_file(&manifest).ok();
}

/// ADR-0022 Decision 2/5: a call whose bound requirement set is empty (`tabs_create_mcp`,
/// `requires: []`) short-circuits to allow BEFORE any grant scan -- proven here under a
/// manifest with an empty `grants` array (nothing could possibly cover it if grants were
/// consulted). The call reaches ordinary dispatch (the familiar `not connected` execution
/// error), never a `Denied (` text result, and the single audit line records `capability:
/// "none"`, no attributed grant.
#[test]
fn requires_empty_call_records_capability_none() {
    let audit_path = temp_path("case-requires-empty-audit");
    let manifest = write_manifest(
        "case-requires-empty",
        &manifest_value("case-requires-empty", json!([]), &audit_path),
    );

    let responses = drive(
        Some(&manifest),
        &init_and_call("tabs_create_mcp", json!({})),
    );
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
    std::fs::remove_file(&manifest).ok();
}

/// ADR-0024 Decision 3, the sanctioned delta this task owns: a GOVERNED directory miss (a known
/// tool, unknown sub-action) is DENIED with `unknown_action` instead of dispatching ungoverned --
/// the deliberate fix of the `b4b2faf` fail-open regression, restoring ADR-0022's
/// absent-means-DENY. `computer` with an unrecognized `action` string is the concrete case: the
/// extension's own schema would reject it too, but governance must not rely on that, and the
/// denial fires before any extension traffic (no probe, no dispatch).
#[test]
fn governed_unknown_computer_action_is_denied_unknown_action() {
    let audit_path = temp_path("case-unknown-action-audit");
    let grants: Value = serde_json::from_str(EXAMPLE_FULL_AND_RESEARCH_READ).unwrap();
    let manifest = write_manifest(
        "case-unknown-action",
        &manifest_value("case-unknown-action", grants, &audit_path),
    );

    let responses = drive(
        Some(&manifest),
        &init_and_call("computer", json!({ "action": "bogus_action", "tabId": 1 })),
    );
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
    std::fs::remove_file(&manifest).ok();
}
