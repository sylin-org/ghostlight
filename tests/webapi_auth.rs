// SPDX-License-Identifier: Apache-2.0 OR MIT
//! H8 (`docs/tasks/hub/H8-web-api-loopback-policy.md`, ADR-0030 Decision 5/9): the web adapter's
//! builtin bind default, the policy-driven (never code-gated) remote-open path, and the
//! anonymous-loopback principal under all-open.

use ghostlight::governance::channels::ChannelsPdp;
use ghostlight::governance::dispatch::Governance;
use ghostlight::governance::ports::{
    AuditSink, Decision, DecisionRequest, EffectiveMode, GoverningResource, PolicyDecisionPoint,
};
use ghostlight::hub::webapi::{
    builtin_webapi_from, resolve_bind, DEFAULT_WEBAPI_BIND, REMOTE_WEBAPI_BIND,
};
use serde_json::Value;
use std::sync::Arc;

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-webapi-auth-test-{}-{tag}.jsonl",
        std::process::id()
    ))
}

#[test]
fn webapi_builtin_default_is_loopback_only_with_no_overlay() {
    // With NO user/org overlay, the resolved channels.webapi.from equals the web adapter's
    // builtin fragment (PINS.md SS7: `[allow: "localhost"]`).
    let resolved = builtin_webapi_from();
    assert_eq!(resolved, vec!["localhost".to_string()]);

    // ADR-0030 Decision 9 (verbatim, transcribed): "the web adapter's builtin default is
    // loopback (127.0.0.1, bound explicitly, never 0.0.0.0)".
    let bind = resolve_bind(&resolved);
    assert_eq!(bind, "127.0.0.1");
    assert_eq!(bind, DEFAULT_WEBAPI_BIND);
    assert_ne!(bind, "0.0.0.0");
}

#[test]
fn enabling_remote_is_a_user_policy_change_not_a_code_gate() {
    // Apply a USER-layer policy over the builtin fragment: the resolved allowlist now contains
    // "*". This is an ordinary policy/config write (ADR-0030 Decision 5), never a code gate --
    // modeled here exactly as the resolved value that layering would produce, since `resolve_bind`
    // takes ONLY the resolved allowlist and nothing else.
    let resolved = vec!["*".to_string()];
    assert!(resolved.contains(&"*".to_string()));

    // The SAME pure function as the builtin-default test above; its ONLY input is the resolved
    // allowlist -- there is no separate boolean/flag/env parameter to pass, so remote is
    // reachable ONLY because the policy layer changed.
    let bind = resolve_bind(&resolved);
    assert_ne!(bind, DEFAULT_WEBAPI_BIND);
    assert_eq!(bind, REMOTE_WEBAPI_BIND);

    // The builtin default, unaffected by this overlay's own resolved value, is still loopback --
    // proving the two are decided independently by the same one-argument function.
    assert_eq!(resolve_bind(&builtin_webapi_from()), DEFAULT_WEBAPI_BIND);
}

#[test]
fn anonymous_is_a_valid_principal_under_all_open() {
    // Under a lone all-open session (no manifest) with the builtin loopback fragment, an
    // anonymous (no-token) loopback connection is AUTHORIZED: the channels decision returns
    // Decision::Allow for the anonymous subject on a loopback source, with no authentication step
    // invoked (ADR-0030 Decision 5: "Anonymous is a first-class principal. Loopback + anonymous
    // is zero-friction, no token.").
    let pdp = ChannelsPdp::new(builtin_webapi_from());
    let req = DecisionRequest {
        grants: Vec::new(),
        tool: String::new(),
        action: None,
        requires: Vec::new(),
        resource: GoverningResource::None,
        manifest_mode: None,
        config_mode: EffectiveMode::Enforce,
        manifest_hash: String::new(),
        channel_source: Some("localhost".to_string()),
    };
    assert_eq!(pdp.decide(&req), Decision::Allow { grant_id: None });
    assert!(
        !matches!(pdp.decide(&req), Decision::Deny(_)),
        "no denial for the anonymous-loopback case"
    );

    // Additionally: a lone all-open MCP-stdio session's audit bytes are unchanged. The
    // subject representation chosen for Required behavior item 6 is the EXISTING `identity`
    // field (PINS.md SS2) -- never a 15th key -- so this is byte-identical to
    // `tests/audit_recorder.rs`'s own pinned 14-key assertion, reproduced here directly (no
    // listener, no web session involved) to prove H8 introduced no drift.
    let path = temp_path("anonymous-all-open");
    let _ = std::fs::remove_file(&path);
    let recorder = ghostlight::governance::audit::Recorder::to_file(path.clone());
    let governance = Governance::all_open(Arc::new(recorder) as Arc<dyn AuditSink>);
    governance.set_client("web-api-test-client", "0.0.0");
    let mut audit = governance.begin(
        "navigate",
        None,
        ghostlight::browser::directory::requires("navigate", None),
    );
    audit.dispatch_finished();
    audit.complete();

    let content = std::fs::read_to_string(&path).expect("audit file exists");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1);
    let rec: Value = serde_json::from_str(lines[0]).expect("line is a JSON object");
    let keys: Vec<&String> = rec
        .as_object()
        .expect("record is an object")
        .keys()
        .collect();
    assert_eq!(
        keys,
        vec![
            "event_id",
            "ts",
            "identity",
            "client",
            "tool",
            "action",
            "capability",
            "domain",
            "decision",
            "grant_id",
            "denial_id",
            "duration_ms",
            "manifest",
            "held"
        ],
        "the 14-key AuditRecord order is unchanged"
    );
    assert!(
        rec["identity"].is_null(),
        "anonymous/all-open resolves identity to None, byte-identical to today"
    );

    std::fs::remove_file(&path).ok();
}
