// SPDX-License-Identifier: Apache-2.0 OR MIT
//! One-deadline navigation readiness across the typed owner bridge and browser wire.

use ghostlight::browser::pattern::is_valid_pattern;
use ghostlight::governance::config::reload::PolicySource;
use ghostlight::governance::manifest::document::parse_manifest;
use ghostlight::governance::manifest::source::LoadedPolicy;
use ghostlight::governance::manifest::source::ManifestOrigin;
use ghostlight::hub::bridge::serve_bridge;
use ghostlight::hub::outbound::browser::Browser;
use ghostlight::hub::peer::{PeerCred, PeerUser};
use ghostlight::hub::ServiceContext;
use ghostlight::native::host;
use ghostlight::observability::DebugSink;
use ghostlight_transport::bridge::{
    read_service_message, write_edge_message, BridgeSequence, ClientPresentation, EdgeMessage,
    RequestContext, ServiceMessage, TerminalOutcome, BRIDGE_MAJOR,
};
use ghostlight_transport::handshake::{
    browser_hello_bytes, BROWSER_ID_FIELD, EXTENSION_IDENTITY_TYPE,
};
use ghostlight_transport::operation::{
    BrowserResult, BrowserResultStatus, NavigateArguments, Operation, OperationEffect,
    OperationKind, ReadinessStatus, RetryDisposition, SettlementStatus,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn context(browser: Browser) -> ServiceContext {
    context_with_policy(
        browser,
        LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        },
    )
}

fn context_with_policy(browser: Browser, policy: LoadedPolicy) -> ServiceContext {
    ServiceContext::from_startup(
        browser,
        DebugSink::disabled(),
        policy,
        PolicySource::SourceString { user_source: None },
        None,
    )
    .expect("build service context")
}

fn loaded_policy(value: Value) -> LoadedPolicy {
    let manifest = parse_manifest(&value.to_string(), "readiness-contract", is_valid_pattern)
        .expect("readiness policy parses");
    LoadedPolicy {
        manifest: Some(manifest),
        origin: Some(ManifestOrigin::UserFile),
        user_manifest_ignored: false,
    }
}

async fn attach_scripted_adapter(
    browser: &Browser,
    features: &[&str],
    evidence: Vec<Value>,
) -> Arc<Mutex<Vec<Value>>> {
    let (browser_side, mut extension_side) = tokio::io::duplex(128 * 1024);
    let attached = browser.clone();
    tokio::spawn(async move {
        let _ = attached.attach(browser_side).await;
    });
    let frames = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&frames);
    let navigation_readiness_v1 = features.contains(&"navigationReadinessV1");
    let features = features
        .iter()
        .map(|feature| Value::String((*feature).to_owned()))
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        host::write_message(&mut extension_side, &browser_hello_bytes(1, None))
            .await
            .unwrap();
        host::write_message(
            &mut extension_side,
            &serde_json::to_vec(&json!({
                "type": EXTENSION_IDENTITY_TYPE,
                BROWSER_ID_FIELD: "readiness-contract",
                "features": features,
            }))
            .unwrap(),
        )
        .await
        .unwrap();
        let mut evidence = VecDeque::from(evidence);
        while let Ok(Some(bytes)) = host::read_message(&mut extension_side).await {
            let request: Value = serde_json::from_slice(&bytes).unwrap();
            captured.lock().unwrap().push(request.clone());
            let Some(id) = request.get("id").cloned() else {
                continue;
            };
            let is_tab_url_query = request.get("type").and_then(Value::as_str)
                == Some("tab_url_request")
                || request.get("mechanism").and_then(Value::as_str) == Some("tab.url_query");
            if is_tab_url_query {
                host::write_message(
                    &mut extension_side,
                    &serde_json::to_vec(&json!({
                        "id": id,
                        "type": "tab_url_response",
                        "result": {"url": "https://example.com/"},
                    }))
                    .unwrap(),
                )
                .await
                .unwrap();
                continue;
            }
            let mechanism = request.get("mechanism").and_then(Value::as_str);
            let uses_readiness = navigation_readiness_v1
                && request.get("type").and_then(Value::as_str) == Some("mechanism_request")
                && (request.pointer("/input/readiness").is_some()
                    || matches!(
                        mechanism,
                        Some("navigation.await_readiness" | "navigation.verify_document")
                    ));
            let result = if uses_readiness {
                let mut navigation = evidence.pop_front().expect("scripted navigation evidence");
                let delay_ms = navigation
                    .as_object_mut()
                    .and_then(|object| object.remove("_delay_ms"))
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
                if delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
                if let Some(error) = navigation.get("tool_error").and_then(Value::as_str) {
                    host::write_message(
                        &mut extension_side,
                        &serde_json::to_vec(&json!({
                            "id": id,
                            "type": "tool_error",
                            "error": error,
                            "hop": "navigation",
                        }))
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                    continue;
                }
                if mechanism.is_some_and(|mechanism| mechanism.starts_with("navigate.")) {
                    json!({
                        "content":[{"type":"text","text":"Navigated to https://example.com/."}],
                        "structuredContent": {
                            "tabId": 5,
                            "url": "https://example.com/",
                            "title": "Example",
                            "navigation": navigation,
                        }
                    })
                } else {
                    json!({"structuredContent":{"navigation":navigation}})
                }
            } else {
                json!({
                    "content":[{"type":"text","text":"Navigated to https://example.com/."}],
                    "structuredContent":{"tabId":5,"url":"https://example.com/","title":"Example"}
                })
            };
            host::write_message(
                &mut extension_side,
                &serde_json::to_vec(&json!({
                    "id": id,
                    "type": "tool_response",
                    "result": result,
                }))
                .unwrap(),
            )
            .await
            .unwrap();
        }
    });
    for _ in 0..200 {
        if browser.is_connected() {
            return frames;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("browser adapter did not connect")
}

fn evidence(state: &str, document: Option<&str>, url: Option<&str>, elapsed_ms: u64) -> Value {
    let mut value = json!({
        "state": state,
        "navigation_token": "n_contract",
        "deadline_at_ms": 10_000,
        "elapsed_ms": elapsed_ms,
    });
    if let Some(document) = document {
        value["document_handle"] = Value::String(document.to_owned());
    }
    if let Some(url) = url {
        value["url"] = Value::String(url.to_owned());
    }
    value
}

async fn execute(
    features: &[&str],
    readiness: Value,
    evidence_script: Vec<Value>,
) -> (TerminalOutcome, Vec<Value>) {
    execute_url(features, readiness, evidence_script, "https://example.com/").await
}

async fn execute_url(
    features: &[&str],
    readiness: Value,
    evidence_script: Vec<Value>,
    url: &str,
) -> (TerminalOutcome, Vec<Value>) {
    execute_configured(features, readiness, evidence_script, url, None, None).await
}

async fn execute_configured(
    features: &[&str],
    _readiness: Value,
    evidence_script: Vec<Value>,
    url: &str,
    policy: Option<LoadedPolicy>,
    restriction: Option<String>,
) -> (TerminalOutcome, Vec<Value>) {
    let browser = Browser::new();
    let frames = attach_scripted_adapter(&browser, features, evidence_script).await;
    let context = match policy {
        Some(policy) => context_with_policy(browser, policy),
        None => context(browser),
    };
    let peer = PeerCred {
        user: PeerUser("readiness-owner".into()),
        pid: 500,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&workspace, 5);
    let tab = context
        .workspaces
        .tab_handle(&workspace, 5)
        .expect("opaque tab handle");
    let (service_side, mut edge_side) = tokio::io::duplex(128 * 1024);
    let service = tokio::spawn(serve_bridge(service_side, context, peer));
    write_edge_message(
        &mut edge_side,
        &EdgeMessage::Hello {
            bridge_major: BRIDGE_MAJOR,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        read_service_message(&mut edge_side).await.unwrap(),
        Some(ServiceMessage::Hello { .. })
    ));
    let operation = Operation::BrowserNavigate(NavigateArguments {
        url: url.to_owned(),
        tab: Some(tab),
    });
    write_edge_message(
        &mut edge_side,
        &EdgeMessage::Start {
            sequence: BridgeSequence(1),
            operation,
            workspace: Some(workspace),
            context: RequestContext {
                client: Some(ClientPresentation {
                    name: "native-contract-test".into(),
                    version: "1".into(),
                }),
                restriction,
            },
        },
    )
    .await
    .unwrap();
    let outcome = match read_service_message(&mut edge_side)
        .await
        .unwrap()
        .expect("start resolution")
    {
        ServiceMessage::Started { .. } => match read_service_message(&mut edge_side)
            .await
            .unwrap()
            .expect("completed outcome")
        {
            ServiceMessage::Completed { outcome, .. } => outcome,
            other => panic!("expected completion, got {other:?}"),
        },
        ServiceMessage::Rejected { error, .. } => {
            let mut result = BrowserResult::new(
                OperationKind::BrowserNavigate,
                BrowserResultStatus::NotDispatched,
                OperationEffect::None,
            );
            result.problem.as_mut().expect("rejection problem").message = error.message;
            TerminalOutcome {
                result: Box::new(result),
            }
        }
        other => panic!("expected start resolution, got {other:?}"),
    };
    drop(edge_side);
    service.await.unwrap().unwrap();
    let frames = frames.lock().unwrap().clone();
    (outcome, frames)
}

#[tokio::test]
async fn invalid_navigation_url_is_rejected_before_every_adapter_grammar() {
    for features in [
        &[][..],
        &["mechanismRequestV1"][..],
        &["mechanismRequestV1", "navigationReadinessV1"][..],
    ] {
        let (outcome, frames) = execute_url(
            features,
            json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
            Vec::new(),
            "https://",
        )
        .await;
        assert_eq!(outcome.result.status, BrowserResultStatus::NotDispatched);
        assert!(
            frames.is_empty(),
            "invalid URL reached an adapter with features {features:?}: {frames:?}"
        );
    }
}

fn success(outcome: TerminalOutcome) -> BrowserResult {
    *outcome.result
}

fn assert_safety_park(frames: &[Value]) {
    let navigations = frames
        .iter()
        .filter(|frame| frame.get("mechanism").and_then(Value::as_str) == Some("navigate.url"))
        .collect::<Vec<_>>();
    assert_eq!(navigations.len(), 2, "original navigation plus safety park");
    assert_eq!(
        navigations[1].pointer("/input/url").and_then(Value::as_str),
        Some("about:blank")
    );
}

#[tokio::test]
async fn explicit_pre_dispatch_navigation_error_is_not_soft_success() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![json!({"tool_error":"Invalid URL"})],
    )
    .await;
    assert_eq!(outcome.result.status, BrowserResultStatus::OutcomeUnknown);
    assert_eq!(outcome.result.effect, OperationEffect::Unknown);
    assert!(frames
        .iter()
        .any(|frame| { frame.get("mechanism").and_then(Value::as_str) == Some("navigate.url") }));
    assert!(!frames.iter().any(|frame| {
        matches!(
            frame.get("mechanism").and_then(Value::as_str),
            Some("navigation.await_readiness" | "navigation.verify_document")
        )
    }));
}

#[tokio::test]
async fn malformed_initial_commit_evidence_is_outcome_unknown_and_safety_parked() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![json!({"state":"committed"})],
    )
    .await;
    assert_eq!(outcome.result.status, BrowserResultStatus::OutcomeUnknown);
    assert_eq!(outcome.result.effect, OperationEffect::Unknown);
    let navigations = frames
        .iter()
        .filter(|frame| frame.get("mechanism").and_then(Value::as_str) == Some("navigate.url"))
        .collect::<Vec<_>>();
    assert_eq!(navigations.len(), 2, "original navigation plus safety park");
    assert_eq!(
        navigations[1].pointer("/input/url").and_then(Value::as_str),
        Some("about:blank")
    );
}

#[tokio::test]
async fn changed_follow_up_transaction_identity_is_partial_and_safety_parked() {
    let mut changed = evidence("ready", Some("d_one"), Some("https://example.com/"), 600);
    changed["navigation_token"] = json!("n_other");
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            changed,
        ],
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Partial);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_eq!(result.repeat, RetryDisposition::Unsafe);
    let tab = result.tab.expect("addressed tab handle remains available");
    assert!(tab.url.is_none());
    assert!(tab.title.is_none());
    assert_safety_park(&frames);
}

#[tokio::test]
async fn settled_navigation_keeps_one_token_deadline_and_verifies_the_document() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence(
                "committed",
                Some("d_first"),
                Some("https://example.com/"),
                10,
            ),
            evidence("ready", Some("d_first"), Some("https://example.com/"), 650),
            evidence("same", Some("d_first"), Some("https://example.com/"), 651),
        ],
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Ok);
    assert_eq!(result.effect, OperationEffect::Committed);
    let readiness = result.readiness.expect("readiness evidence");
    assert_eq!(readiness.status, ReadinessStatus::Ready);
    assert_eq!(
        readiness.settlement.unwrap().status,
        SettlementStatus::Settled
    );
    let mechanisms = frames
        .iter()
        .filter_map(|frame| frame.get("mechanism").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        mechanisms,
        [
            "tab.url_query",
            "navigate.url",
            "navigation.await_readiness",
            "navigation.verify_document",
        ]
    );
    assert!(frames.iter().all(|frame| {
        frame
            .pointer("/input/readiness/timeout_ms")
            .is_none_or(|value| value == 10_000)
    }));
}

#[tokio::test]
async fn soft_timeout_and_unavailable_remain_successful_commits() {
    for (state, expected_status, expected_settlement) in [
        (
            "timed_out",
            ReadinessStatus::TimedOut,
            Some(SettlementStatus::NotSettled),
        ),
        (
            "unavailable",
            ReadinessStatus::Unavailable,
            Some(SettlementStatus::Unavailable),
        ),
    ] {
        let script = vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            evidence(state, Some("d_one"), Some("https://example.com/"), 10_000),
            evidence("same", Some("d_one"), Some("https://example.com/"), 10_000),
        ];
        let (outcome, frames) = execute(
            &["mechanismRequestV1", "navigationReadinessV1"],
            json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
            script,
        )
        .await;
        let result = success(outcome);
        assert_eq!(
            result.status,
            BrowserResultStatus::Ok,
            "state {state}: {result:?}"
        );
        assert_eq!(result.effect, OperationEffect::Committed);
        let readiness = result.readiness.unwrap();
        assert_eq!(readiness.status, expected_status);
        assert_eq!(
            readiness.settlement.map(|settlement| settlement.status),
            expected_settlement
        );
        assert!(frames.iter().any(|frame| {
            frame.get("mechanism").and_then(Value::as_str) == Some("navigation.await_readiness")
        }));
    }
}

#[tokio::test]
async fn document_change_discards_old_readiness_and_authorizes_the_latest_commit() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            evidence(
                "committed",
                Some("d_two"),
                Some("https://example.com/two"),
                50,
            ),
            evidence("ready", Some("d_two"), Some("https://example.com/two"), 700),
            evidence("same", Some("d_two"), Some("https://example.com/two"), 701),
        ],
    )
    .await;
    assert_eq!(
        success(outcome).readiness.unwrap().status,
        ReadinessStatus::Ready
    );
    assert_eq!(
        frames
            .iter()
            .filter(|frame| {
                frame.get("mechanism").and_then(Value::as_str) == Some("navigation.await_readiness")
            })
            .count(),
        2
    );
}

fn navigation_policy(mode: &str, hosts: &[&str], config: Vec<Value>) -> LoadedPolicy {
    loaded_policy(json!({
        "schema":3,
        "name":"readiness-policy",
        "version":"1",
        "mode":mode,
        "grants":[{
            "id":"navigate-grant",
            "hosts":{"allow":hosts},
            "allowed":["read","action","write"]
        }],
        "config":config
    }))
}

fn overlay(mode: &str, hosts: &[&str]) -> String {
    overlay_with_config(mode, hosts, Vec::new())
}

fn overlay_with_config(mode: &str, hosts: &[&str], config: Vec<Value>) -> String {
    json!({
        "schema":3,
        "name":"readiness-overlay",
        "version":"1",
        "mode":mode,
        "grants":[{
            "id":"overlay-grant",
            "hosts":{"allow":hosts},
            "allowed":["read","action","write"]
        }],
        "config":config
    })
    .to_string()
}

#[tokio::test]
async fn every_committed_redirect_is_checked_by_policy_sacred_and_request_overlay() {
    let cases = [
        (
            Some(navigation_policy("enforce", &["example.com"], Vec::new())),
            None,
            "https://outside.example/",
        ),
        (
            Some(navigation_policy(
                "observe",
                &["example.com", "sacred.example"],
                Vec::new(),
            )),
            Some(overlay_with_config(
                "observe",
                &["example.com", "sacred.example"],
                vec![json!({
                    "key":"content.security.sacred_domains",
                    "value":["sacred.example"],
                    "level":"mandatory"
                })],
            )),
            "https://sacred.example/",
        ),
        (
            None,
            Some(overlay("enforce", &["example.com"])),
            "https://overlay-denied.example/",
        ),
    ];

    for (policy, restriction, denied_url) in cases {
        let (outcome, frames) = execute_configured(
            &["mechanismRequestV1", "navigationReadinessV1"],
            json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
            vec![
                evidence(
                    "committed",
                    Some("d_allowed"),
                    Some("https://example.com/"),
                    1,
                ),
                evidence("committed", Some("d_denied"), Some(denied_url), 2),
            ],
            "https://example.com/",
            policy,
            restriction,
        )
        .await;
        let result = success(outcome);
        assert_eq!(result.status, BrowserResultStatus::Blocked, "{denied_url}");
        assert_eq!(result.effect, OperationEffect::Committed, "{denied_url}");
        assert_eq!(
            result.repeat,
            RetryDisposition::AfterStateChange,
            "{denied_url}"
        );
        assert!(frames.iter().any(|frame| {
            frame.get("mechanism").and_then(Value::as_str) == Some("navigate.url")
                && frame.pointer("/input/url").and_then(Value::as_str) == Some("about:blank")
        }));
        assert!(frames.iter().all(|frame| {
            frame.get("mechanism").and_then(Value::as_str) != Some("navigation.verify_document")
        }));
    }
}

#[tokio::test]
async fn shadow_denied_redirect_remains_audited_after_a_later_allowed_commit() {
    let audit_path = std::env::temp_dir().join(format!(
        "ghostlight-readiness-shadow-{}.jsonl",
        std::process::id()
    ));
    std::fs::remove_file(&audit_path).ok();
    let policy = navigation_policy(
        "observe",
        &["example.com", "final.example"],
        vec![
            json!({"key":"audit.enabled","value":true,"level":"mandatory"}),
            json!({"key":"audit.destination","value":"file","level":"mandatory"}),
            json!({
                "key":"audit.file.path",
                "value":audit_path.to_string_lossy(),
                "level":"mandatory"
            }),
        ],
    );
    let (outcome, frames) = execute_configured(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            evidence(
                "committed",
                Some("d_shadow"),
                Some("https://shadow.example/"),
                2,
            ),
            evidence(
                "committed",
                Some("d_final"),
                Some("https://final.example/"),
                3,
            ),
            evidence(
                "ready",
                Some("d_final"),
                Some("https://final.example/"),
                650,
            ),
            evidence("same", Some("d_final"), Some("https://final.example/"), 651),
        ],
        "https://example.com/",
        Some(policy),
        None,
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Ok);
    assert_eq!(result.readiness.unwrap().status, ReadinessStatus::Ready);
    assert!(frames.iter().all(|frame| {
        frame.pointer("/input/url").and_then(Value::as_str) != Some("about:blank")
    }));

    let records = std::fs::read_to_string(&audit_path).expect("shadow audit exists");
    let record: Value =
        serde_json::from_str(records.lines().last().expect("one shadow audit record"))
            .expect("audit record parses");
    assert_eq!(record["decision"], "shadow_deny");
    assert_eq!(record["domain"], "shadow.example");
    assert!(record["denial_id"].as_str().is_some());
    std::fs::remove_file(audit_path).ok();
}

#[tokio::test]
async fn changed_terminal_document_identity_fails_closed() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            evidence("ready", Some("d_two"), Some("https://example.com/two"), 700),
        ],
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Partial);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_ne!(
        result.readiness.as_ref().map(|readiness| readiness.status),
        Some(ReadinessStatus::Ready)
    );
    assert!(frames.iter().all(|frame| {
        frame.get("mechanism").and_then(Value::as_str) != Some("navigation.verify_document")
    }));
    let tab = result.tab.expect("addressed tab handle remains available");
    assert!(tab.url.is_none());
    assert!(tab.title.is_none());
    assert_safety_park(&frames);
}

#[tokio::test]
async fn malformed_follow_up_after_commit_is_partial_and_safety_parked() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            json!({"state":"ready"}),
        ],
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Partial);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_eq!(result.repeat, RetryDisposition::Unsafe);
    assert!(result.readiness.is_none());
    let tab = result.tab.expect("addressed tab handle remains available");
    assert!(tab.url.is_none());
    assert!(tab.title.is_none());
    let navigations = frames
        .iter()
        .filter(|frame| frame.get("mechanism").and_then(Value::as_str) == Some("navigate.url"))
        .collect::<Vec<_>>();
    assert_eq!(navigations.len(), 2, "original navigation plus safety park");
    assert_eq!(
        navigations[1].pointer("/input/url").and_then(Value::as_str),
        Some("about:blank")
    );
}

#[tokio::test]
async fn impossible_ready_after_the_original_deadline_fails_closed() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            evidence("ready", Some("d_one"), Some("https://example.com/"), 10_001),
        ],
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Partial);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert!(result.readiness.is_none());
    assert_safety_park(&frames);
}

#[tokio::test]
async fn mismatched_verification_identity_is_partial_and_safety_parked() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            evidence("ready", Some("d_one"), Some("https://example.com/"), 600),
            evidence("same", Some("d_other"), Some("https://other.example/"), 601),
        ],
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Partial);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_eq!(result.repeat, RetryDisposition::Unsafe);
    let tab = result.tab.expect("addressed tab handle remains available");
    assert!(tab.url.is_none());
    assert!(tab.title.is_none());
    assert_safety_park(&frames);
}

#[tokio::test]
async fn bounded_commit_loop_exhaustion_is_partial_and_safety_parked() {
    let evidence_script = (0..33)
        .map(|index| {
            evidence(
                "committed",
                Some(&format!("d_{index}")),
                Some(&format!("https://redirect-{index}.example/")),
                index + 1,
            )
        })
        .collect::<Vec<_>>();
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        evidence_script,
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Partial);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_eq!(result.repeat, RetryDisposition::Unsafe);
    let tab = result.tab.expect("addressed tab handle remains available");
    assert!(tab.url.is_none());
    assert!(tab.title.is_none());
    assert_safety_park(&frames);
}

#[tokio::test]
async fn late_redirect_is_authorized_and_becomes_the_final_timed_out_landing() {
    let (outcome, _) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            evidence(
                "committed",
                Some("d_two"),
                Some("https://final.example/"),
                10_001,
            ),
            evidence(
                "timed_out",
                Some("d_two"),
                Some("https://final.example/"),
                10_001,
            ),
            evidence(
                "same",
                Some("d_two"),
                Some("https://final.example/"),
                10_002,
            ),
        ],
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Ok);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_eq!(
        result.readiness.as_ref().map(|readiness| readiness.status),
        Some(ReadinessStatus::TimedOut)
    );
    let tab = result.tab.expect("addressed final tab");
    assert_eq!(tab.url.as_deref(), Some("https://final.example/"));
    assert!(tab.title.is_none());
}

#[tokio::test]
async fn unknown_landing_after_a_commit_is_parked_and_never_reports_a_stale_page() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![
            evidence("committed", Some("d_one"), Some("https://example.com/"), 1),
            evidence("landing_unknown", None, None, 2),
        ],
    )
    .await;
    let result = success(outcome);
    assert_eq!(result.status, BrowserResultStatus::Partial);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_eq!(result.repeat, RetryDisposition::Unsafe);
    assert!(result.readiness.is_none());
    let tab = result.tab.expect("addressed tab handle remains available");
    assert!(tab.url.is_none());
    assert!(tab.title.is_none());

    let navigations = frames
        .iter()
        .filter(|frame| frame.get("mechanism").and_then(Value::as_str) == Some("navigate.url"))
        .collect::<Vec<_>>();
    assert_eq!(navigations.len(), 2, "original navigation plus safety park");
    assert_eq!(
        navigations[1].pointer("/input/url").and_then(Value::as_str),
        Some("about:blank")
    );
}

#[tokio::test]
async fn cancellation_during_readiness_drains_the_commit_and_reports_committed_effect() {
    let browser = Browser::new();
    let mut delayed_ready = evidence("ready", Some("d_cancel"), Some("https://example.com/"), 500);
    delayed_ready["_delay_ms"] = json!(200);
    let frames = attach_scripted_adapter(
        &browser,
        &["mechanismRequestV1", "navigationReadinessV1"],
        vec![
            evidence(
                "committed",
                Some("d_cancel"),
                Some("https://example.com/"),
                1,
            ),
            delayed_ready,
            evidence("same", Some("d_cancel"), Some("https://example.com/"), 501),
        ],
    )
    .await;
    let context = context(browser);
    let peer = PeerCred {
        user: PeerUser("readiness-cancel-owner".into()),
        pid: 501,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&workspace, 5);
    let tab = context
        .workspaces
        .tab_handle(&workspace, 5)
        .expect("opaque tab handle");
    let (service_side, mut edge_side) = tokio::io::duplex(128 * 1024);
    let service = tokio::spawn(serve_bridge(service_side, context, peer));
    write_edge_message(
        &mut edge_side,
        &EdgeMessage::Hello {
            bridge_major: BRIDGE_MAJOR,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        read_service_message(&mut edge_side).await.unwrap(),
        Some(ServiceMessage::Hello { .. })
    ));
    write_edge_message(
        &mut edge_side,
        &EdgeMessage::Start {
            sequence: BridgeSequence(1),
            operation: Operation::BrowserNavigate(NavigateArguments {
                url: "https://example.com/".to_owned(),
                tab: Some(tab),
            }),
            workspace: Some(workspace),
            context: RequestContext::default(),
        },
    )
    .await
    .unwrap();
    let work_id = match read_service_message(&mut edge_side)
        .await
        .unwrap()
        .expect("started")
    {
        ServiceMessage::Started { work_id, .. } => work_id,
        other => panic!("expected started, got {other:?}"),
    };
    for _ in 0..200 {
        if frames.lock().unwrap().iter().any(|frame| {
            frame.get("mechanism").and_then(Value::as_str) == Some("navigation.await_readiness")
        }) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert!(frames.lock().unwrap().iter().any(|frame| {
        frame.get("mechanism").and_then(Value::as_str) == Some("navigation.await_readiness")
    }));
    write_edge_message(&mut edge_side, &EdgeMessage::Cancel { work_id })
        .await
        .unwrap();
    let outcome = match read_service_message(&mut edge_side)
        .await
        .unwrap()
        .expect("completed cancellation")
    {
        ServiceMessage::Completed { outcome, .. } => outcome,
        other => panic!("expected completed, got {other:?}"),
    };
    assert_eq!(outcome.result.status, BrowserResultStatus::Cancelled);
    assert_eq!(outcome.result.effect, OperationEffect::Committed);
    drop(edge_side);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn no_proven_commit_is_outcome_unknown_and_never_soft_success() {
    let (outcome, frames) = execute(
        &["mechanismRequestV1", "navigationReadinessV1"],
        json!({"settle":true,"timeout_ms":10000,"min_ms":0}),
        vec![evidence("timed_out", None, None, 10_000)],
    )
    .await;
    assert_eq!(outcome.result.status, BrowserResultStatus::OutcomeUnknown);
    assert_safety_park(&frames);
}
