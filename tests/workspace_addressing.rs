// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Opaque workspace/tab addressing at the owner bridge boundary.

use ghostlight::governance::config::reload::PolicySource;
use ghostlight::governance::manifest::source::LoadedPolicy;
use ghostlight::hub::bridge::serve_bridge;
use ghostlight::hub::outbound::browser::Browser;
use ghostlight::hub::peer::{PeerCred, PeerUser};
use ghostlight::hub::ServiceContext;
use ghostlight::native::host;
use ghostlight::observability::DebugSink;
use ghostlight_transport::bridge::{
    read_service_message, write_edge_message, BridgeError, BridgeErrorKind, BridgeSequence,
    ClientPresentation, EdgeMessage, RequestContext, ServiceMessage, TerminalOutcome, BRIDGE_MAJOR,
};
use ghostlight_transport::operation::{
    BrowserResultStatus, InspectPageArguments, NavigateArguments, OpenTabArguments, Operation,
    OperationEffect, OperationResult, OptionalTabArguments, RequiredTabArguments,
    RunSequenceArguments, TabHandle,
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::DuplexStream;
use tokio::task::JoinHandle;

const BROWSER_ID: &str = "workspace-addressing";

fn build_context(browser: Browser) -> ServiceContext {
    ServiceContext::from_startup(
        browser,
        DebugSink::disabled(),
        LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        },
        PolicySource::SourceString { user_source: None },
        None,
    )
    .expect("build service context")
}

async fn attach_browser(browser: &Browser) -> Arc<Mutex<Vec<Value>>> {
    attach_browser_with_navigation_error(browser, false).await
}

async fn attach_browser_with_navigation_error(
    browser: &Browser,
    reject_navigation: bool,
) -> Arc<Mutex<Vec<Value>>> {
    let (browser_side, mut extension_side) = tokio::io::duplex(64 * 1024);
    let attached = browser.clone();
    tokio::spawn(async move {
        let _ = attached.attach(browser_side).await;
    });
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_task = Arc::clone(&seen);
    let next_created_tab = Arc::new(AtomicI64::new(5));
    let next_created_tab_for_task = Arc::clone(&next_created_tab);
    tokio::spawn(async move {
        let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
        host::write_message(&mut extension_side, &hello)
            .await
            .expect("write browser hello");
        let identity = serde_json::to_vec(&json!({
            "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
            ghostlight_transport::handshake::BROWSER_ID_FIELD: BROWSER_ID,
            "features": ["mechanismRequestV1", "navigationReadinessV1", "atomicTabOpenV1"],
        }))
        .expect("serialize browser identity");
        host::write_message(&mut extension_side, &identity)
            .await
            .expect("write browser identity");

        while let Ok(Some(frame)) = host::read_message(&mut extension_side).await {
            let request: Value = serde_json::from_slice(&frame).expect("parse browser request");
            seen_for_task.lock().unwrap().push(request.clone());
            let Some(id) = request.get("id") else {
                continue;
            };
            let mechanism = request
                .get("mechanism")
                .or_else(|| request.get("tool"))
                .and_then(Value::as_str);
            let reply = if mechanism == Some("navigation.verify_document") {
                json!({
                    "id": id,
                    "type": "tool_response",
                    "result": {
                        "content": [{"type": "text", "text": "same"}],
                        "structuredContent": {"navigation": {
                            "state":"same",
                            "navigation_token":request.pointer("/input/navigation_token"),
                            "document_handle":request.pointer("/input/document_handle"),
                            "url":"https://example.com/",
                            "deadline_at_ms":10_000,
                            "elapsed_ms":20
                        }}
                    }
                })
            } else if mechanism == Some("navigation.await_readiness") {
                json!({
                    "id": id,
                    "type": "tool_response",
                    "result": {
                        "content": [{"type": "text", "text": "ready"}],
                        "structuredContent": {"navigation": {
                            "state":"ready",
                            "navigation_token":request.pointer("/input/navigation_token"),
                            "document_handle":request.pointer("/input/document_handle"),
                            "url":"https://example.com/",
                            "deadline_at_ms":10_000,
                            "elapsed_ms":20
                        }}
                    }
                })
            } else if mechanism == Some("workspace.tab.open") {
                let created = next_created_tab_for_task.fetch_add(1, Ordering::SeqCst);
                let navigation = if reject_navigation {
                    json!({
                        "state":"landing_unknown",
                        "navigation_token":"n_12345678",
                        "deadline_at_ms":10_000,
                        "elapsed_ms":10_000
                    })
                } else {
                    json!({
                        "state":"ready",
                        "navigation_token":"n_12345678",
                        "document_handle":"d_12345678",
                        "url":"https://example.com/",
                        "deadline_at_ms":10_000,
                        "elapsed_ms":10
                    })
                };
                json!({
                    "id": id,
                    "type": "tool_response",
                    "result": {
                        "content": [{"type": "text", "text": "opened"}],
                        "structuredContent": {
                            "tabId": created,
                            "tabs": [{
                                "tabId": created,
                                "title": "Created",
                                "url": "https://example.com/"
                            }],
                            "created": true,
                            "navigated": !reject_navigation,
                            "navigation": navigation
                        }
                    }
                })
            } else if request.get("type").and_then(Value::as_str) == Some("tab_url_request")
                || request.get("mechanism").and_then(Value::as_str) == Some("tab.url_query")
            {
                json!({
                    "id": id,
                    "type": "tab_url_response",
                    "result": {"url": "https://example.com/"}
                })
            } else {
                let tab_id = request
                    .pointer("/args/tabId")
                    .and_then(Value::as_i64)
                    .or_else(|| request.pointer("/input/tab").and_then(Value::as_i64));
                let structured = if matches!(
                    mechanism,
                    Some("workspace.tab.create" | "tabs_create_mcp")
                ) {
                    let created = next_created_tab_for_task.fetch_add(1, Ordering::SeqCst);
                    json!({
                        "tabId": created,
                        "created": true,
                        "tabs": [{
                            "tabId": created,
                            "title": "Created",
                            "url": "https://example.com/"
                        }]
                    })
                } else if matches!(mechanism, Some("tab.close" | "tab_control"))
                    && (mechanism == Some("tab.close")
                        || request.pointer("/args/action").and_then(Value::as_str) == Some("close"))
                {
                    json!({
                        "interactionReceipt": {
                            "targetAssurance": "none",
                            "action": "close",
                            "observedAfter": {"tabClosed": true},
                            "blockers": [],
                            "page": {"tabId": tab_id},
                            "more": false
                        }
                    })
                } else if matches!(mechanism, Some("page.snapshot" | "read_page")) {
                    json!({
                        "tabId": tab_id,
                        "targets": [{"ref":"ref_1","role":"button","name":"Save","visible":true,"enabled":true,"mechanicalActions":["left_click"]}],
                        "more": false
                    })
                } else if matches!(mechanism, Some("dialog.inspect" | "dialog")) {
                    json!({"tabId":tab_id,"open":false})
                } else {
                    tab_id.map_or_else(|| json!({}), |tab_id| json!({"tabId": tab_id}))
                };
                json!({
                    "id": id,
                    "type": "tool_response",
                    "result": {
                        "content": [{"type": "text", "text": "ok"}],
                        "structuredContent": structured
                    }
                })
            };
            host::write_message(
                &mut extension_side,
                &serde_json::to_vec(&reply).expect("serialize browser reply"),
            )
            .await
            .expect("write browser reply");
        }
    });

    for _ in 0..200 {
        if browser.is_connected() {
            return seen;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("browser fixture did not connect")
}

fn composite_tab(browser: &Browser, native_tab: i64) -> i64 {
    let slot = browser
        .slot_of(BROWSER_ID)
        .expect("browser fixture has an assigned slot");
    ghostlight_core::constants::tab_id::encode(slot, native_tab)
}

async fn open_bridge(
    context: ServiceContext,
    peer: PeerCred,
) -> (DuplexStream, JoinHandle<ghostlight::Result<()>>) {
    let (service_side, mut edge_side) = tokio::io::duplex(64 * 1024);
    let service = tokio::spawn(serve_bridge(service_side, context, peer));
    write_edge_message(
        &mut edge_side,
        &EdgeMessage::Hello {
            bridge_major: BRIDGE_MAJOR,
        },
    )
    .await
    .expect("write bridge hello");
    assert_eq!(
        read_service_message(&mut edge_side)
            .await
            .expect("read bridge hello"),
        Some(ServiceMessage::Hello {
            bridge_major: BRIDGE_MAJOR,
        })
    );
    (edge_side, service)
}

fn request_context() -> RequestContext {
    RequestContext {
        client: Some(ClientPresentation {
            name: "self-asserted-native-client".to_string(),
            version: "999.0".to_string(),
        }),
        restriction: None,
    }
}

async fn send_start(
    edge: &mut DuplexStream,
    sequence: u64,
    operation: Operation,
    workspace: Option<ghostlight_transport::workspace_id::WorkspaceId>,
) {
    write_edge_message(
        edge,
        &EdgeMessage::Start {
            sequence: BridgeSequence(sequence),
            operation,
            workspace,
            context: request_context(),
        },
    )
    .await
    .expect("write start");
}

async fn read_rejection(edge: &mut DuplexStream) -> BridgeError {
    match read_service_message(edge)
        .await
        .expect("read rejection")
        .expect("service response")
    {
        ServiceMessage::Rejected { error, .. } => error,
        other => panic!("expected pre-start rejection, got {other:?}"),
    }
}

fn test_tab(tab: Value) -> TabHandle {
    TabHandle::parse(tab.as_str().expect("opaque tab string")).expect("valid tab handle")
}

fn snapshot(tab: Value) -> Operation {
    Operation::BrowserInspectPage(InspectPageArguments {
        cursor: None,
        tab: Some(test_tab(tab)),
        query: None,
        target: None,
        include: Default::default(),
    })
}

fn close(tab: Value) -> Operation {
    Operation::BrowserCloseTab(RequiredTabArguments { tab: test_tab(tab) })
}

fn flow(step: Operation) -> Operation {
    let tab = match &step {
        Operation::BrowserInspectPage(arguments) => arguments.tab.clone(),
        Operation::BrowserCloseTab(arguments) => Some(arguments.tab.clone()),
        _ => None,
    };
    let tail = Operation::BrowserGetDialog(OptionalTabArguments { tab: tab.clone() });
    Operation::BrowserRunSequence(RunSequenceArguments {
        tab,
        steps: vec![step, tail],
    })
}

#[tokio::test]
async fn native_tab_handle_becomes_numeric_before_browser_dispatch_and_enriches_result() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let composite = composite_tab(&browser, 5);
    let context = build_context(browser);
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 1,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&workspace, composite);
    let handle = context
        .workspaces
        .tab_handle(&workspace, composite)
        .unwrap();
    let raw = handle.as_str().to_string();
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(
        &mut edge,
        1,
        snapshot(Value::String(raw.clone())),
        Some(workspace.clone()),
    )
    .await;
    assert!(matches!(
        read_service_message(&mut edge).await.unwrap(),
        Some(ServiceMessage::Started {
            sequence: BridgeSequence(1),
            ..
        })
    ));
    let completed = read_service_message(&mut edge)
        .await
        .unwrap()
        .expect("completed result");
    let ServiceMessage::Completed {
        outcome: TerminalOutcome { result },
        ..
    } = completed
    else {
        panic!(
            "expected successful canonical result: {completed:?}; frames={:?}",
            seen.lock().unwrap()
        );
    };
    let Some(OperationResult::BrowserInspectPage { targets, more, .. }) = result.result.as_ref()
    else {
        panic!("expected typed inspection result")
    };
    assert_eq!(targets[0].r#ref, "r_1");
    assert!(!more);
    assert_eq!(result.workspace.as_ref(), Some(&workspace));
    assert_eq!(result.tab.as_ref().map(|tab| &tab.id), Some(&handle));

    {
        let requests = seen.lock().unwrap();
        let dispatched = requests
            .iter()
            .find(|request| {
                request.get("type").and_then(Value::as_str) == Some("mechanism_request")
            })
            .expect("browser received the operation");
        assert_eq!(dispatched["input"]["tab"], 5);
        assert!(!serde_json::to_string(dispatched).unwrap().contains(&raw));
    }
    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn creator_inventory_issues_singular_and_plural_opaque_results() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let composite = composite_tab(&browser, 5);
    let context = build_context(browser);
    let workspaces = context.workspaces.clone();
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 11,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    let operation = Operation::BrowserOpenTab(OpenTabArguments::default());
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(&mut edge, 1, operation, Some(workspace.clone())).await;
    assert!(matches!(
        read_service_message(&mut edge).await.unwrap(),
        Some(ServiceMessage::Started { .. })
    ));
    let completed = read_service_message(&mut edge)
        .await
        .unwrap()
        .expect("completed creator result");
    let ServiceMessage::Completed {
        outcome: TerminalOutcome { result },
        ..
    } = completed
    else {
        panic!("expected successful creator result");
    };
    assert_eq!(
        result.result,
        Some(OperationResult::BrowserOpenTab {
            created: true,
            navigated: None,
        })
    );
    let tab = result
        .tab
        .as_ref()
        .expect("creator returns one relevant tab");
    assert_eq!(workspaces.resolve_tab(&workspace, &tab.id), Some(composite));
    assert_eq!(result.tabs.len(), 1);
    assert_eq!(result.tabs[0].id, tab.id);

    {
        let requests = seen.lock().unwrap();
        assert!(requests.iter().any(|request| {
            request.get("mechanism").and_then(Value::as_str) == Some("workspace.tab.create")
        }));
    }
    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn open_url_is_one_physical_transaction_with_no_blank_intermediate() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let context = build_context(browser);
    let workspaces = context.workspaces.clone();
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 112,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    let operation = Operation::BrowserOpenTab(OpenTabArguments {
        url: Some("https://example.com/".into()),
    });
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(&mut edge, 1, operation, Some(workspace.clone())).await;
    assert!(matches!(
        read_service_message(&mut edge).await.unwrap(),
        Some(ServiceMessage::Started { .. })
    ));
    let ServiceMessage::Completed { outcome, .. } = read_service_message(&mut edge)
        .await
        .unwrap()
        .expect("completed open result")
    else {
        panic!("expected completed open result");
    };
    assert_eq!(outcome.result.status, BrowserResultStatus::Ok);
    assert_eq!(outcome.result.effect, OperationEffect::Committed);
    assert_eq!(
        outcome.result.result,
        Some(OperationResult::BrowserOpenTab {
            created: true,
            navigated: Some(true),
        })
    );
    let tab = outcome.result.tab.expect("new tab has an opaque handle");
    assert!(workspaces.resolve_tab(&workspace, &tab.id).is_some());
    assert!(outcome.result.readiness.is_some());

    {
        let requests = seen.lock().unwrap();
        let opens = requests
            .iter()
            .filter(|request| {
                request.get("mechanism").and_then(Value::as_str) == Some("workspace.tab.open")
            })
            .collect::<Vec<_>>();
        assert_eq!(opens.len(), 1);
        assert_eq!(opens[0]["input"]["url"], "https://example.com/");
        assert_eq!(opens[0]["input"]["readiness"]["settle"], true);
        assert!(!requests.iter().any(|request| {
            matches!(
                request.get("mechanism").and_then(Value::as_str),
                Some("workspace.tab.create" | "navigate.url")
            )
        }));
    }
    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn zero_state_navigate_uses_the_same_single_open_transaction() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let context = build_context(browser);
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 113,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    let operation = Operation::BrowserNavigate(NavigateArguments {
        url: "https://example.com/".into(),
        tab: None,
    });
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(&mut edge, 1, operation, Some(workspace)).await;
    assert!(matches!(
        read_service_message(&mut edge).await.unwrap(),
        Some(ServiceMessage::Started { .. })
    ));
    let ServiceMessage::Completed { outcome, .. } = read_service_message(&mut edge)
        .await
        .unwrap()
        .expect("completed navigation result")
    else {
        panic!("expected completed navigation result");
    };
    assert_eq!(
        outcome.result.result,
        Some(OperationResult::BrowserNavigate { landed: true }),
        "{:#?}",
        outcome.result
    );
    assert!(outcome.result.tab.is_some());

    {
        let requests = seen.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.get("mechanism").and_then(Value::as_str)
                    == Some("workspace.tab.open"))
                .count(),
            1
        );
        assert!(!requests.iter().any(|request| {
            matches!(
                request.get("mechanism").and_then(Value::as_str),
                Some("workspace.tab.create" | "navigate.url")
            )
        }));
    }
    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn created_tab_survives_an_unverified_initial_landing() {
    let browser = Browser::new();
    let seen = attach_browser_with_navigation_error(&browser, true).await;
    let composite = composite_tab(&browser, 5);
    let context = build_context(browser);
    let workspaces = context.workspaces.clone();
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 111,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    let operation = Operation::BrowserOpenTab(OpenTabArguments {
        url: Some("https://example.com/".into()),
    });
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(&mut edge, 1, operation, Some(workspace.clone())).await;
    assert!(matches!(
        read_service_message(&mut edge).await.unwrap(),
        Some(ServiceMessage::Started { .. })
    ));
    let completed = read_service_message(&mut edge)
        .await
        .unwrap()
        .expect("completed open-tab result");
    let ServiceMessage::Completed {
        outcome: TerminalOutcome { result },
        ..
    } = completed
    else {
        panic!("expected a canonical partial result: {completed:?}");
    };

    assert_eq!(result.status, BrowserResultStatus::Partial);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_eq!(
        result.result,
        Some(OperationResult::BrowserOpenTab {
            created: true,
            navigated: Some(false),
        })
    );
    let tab = result
        .tab
        .as_ref()
        .expect("the committed creation keeps its exact opaque tab");
    assert_eq!(workspaces.resolve_tab(&workspace, &tab.id), Some(composite));
    assert_eq!(result.repeat.as_str(), "unsafe");

    {
        let requests = seen.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.get("mechanism").and_then(Value::as_str)
                    == Some("workspace.tab.open"))
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.get("mechanism").and_then(Value::as_str)
                    == Some("workspace.tab.create"))
                .count(),
            0
        );
    }
    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_creators_keep_their_exact_operation_scoped_tab_results() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let existing = composite_tab(&browser, 3);
    let first_created = composite_tab(&browser, 5);
    let second_created = composite_tab(&browser, 6);
    let context = build_context(browser);
    let workspaces = context.workspaces.clone();
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 12,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&workspace, existing);
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(
        &mut edge,
        1,
        Operation::BrowserOpenTab(OpenTabArguments::default()),
        Some(workspace.clone()),
    )
    .await;
    send_start(
        &mut edge,
        2,
        Operation::BrowserOpenTab(OpenTabArguments::default()),
        Some(workspace.clone()),
    )
    .await;

    let mut completed = Vec::new();
    while completed.len() < 2 {
        match read_service_message(&mut edge)
            .await
            .expect("read creator response")
            .expect("creator response")
        {
            ServiceMessage::Started { .. } => {}
            ServiceMessage::Completed { outcome, .. } => completed.push(outcome),
            other => panic!("unexpected creator response: {other:?}"),
        }
    }

    let mut resolved = completed
        .iter()
        .map(|outcome| {
            let tab = outcome
                .result
                .tab
                .as_ref()
                .expect("each creator returns its exact tab");
            workspaces
                .resolve_tab(&workspace, &tab.id)
                .expect("creator tab remains owned")
        })
        .collect::<Vec<_>>();
    resolved.sort_unstable();
    assert_eq!(resolved, vec![first_created, second_created]);
    assert!(!resolved.contains(&existing));
    assert_eq!(
        seen.lock()
            .unwrap()
            .iter()
            .filter(|request| request.get("mechanism").and_then(Value::as_str)
                == Some("workspace.tab.create"))
            .count(),
        2
    );

    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn presentation_cannot_replace_missing_workspace_authority() {
    let context = build_context(Browser::new());
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 2,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&workspace, 5);
    let handle = context.workspaces.tab_handle(&workspace, 5).unwrap();
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(
        &mut edge,
        1,
        snapshot(Value::String(handle.as_str().to_string())),
        None,
    )
    .await;
    let error = read_rejection(&mut edge).await;
    assert_eq!(error.kind, BridgeErrorKind::InvalidWorkspace);
    assert!(!error.message.contains(handle.as_str()));

    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn presentation_cannot_cross_the_os_owner_boundary() {
    let context = build_context(Browser::new());
    let owner = PeerUser("owner".into());
    let workspace = context.workspaces.mint(&owner, false).unwrap();
    context.workspaces.claim_tab(&workspace, 5);
    let handle = context.workspaces.tab_handle(&workspace, 5).unwrap();
    let peer = PeerCred {
        user: PeerUser("other-owner".into()),
        pid: 22,
    };
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(
        &mut edge,
        1,
        snapshot(Value::String(handle.as_str().to_string())),
        Some(workspace),
    )
    .await;
    let error = read_rejection(&mut edge).await;
    assert_eq!(error.kind, BridgeErrorKind::InvalidWorkspace);
    assert!(!error.message.contains(handle.as_str()));

    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn unknown_cross_workspace_and_released_handles_share_one_leak_free_rejection() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let context = build_context(browser);
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 3,
    };
    let first = context.workspaces.mint(&peer.user, false).unwrap();
    let second = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tabs(&first, &[5, 6]);
    let cross = context.workspaces.tab_handle(&first, 5).unwrap();
    let released = context.workspaces.tab_handle(&first, 6).unwrap();
    assert!(context.workspaces.release_tab(&first, 6));
    let unknown = TabHandle::parse("t_unknown_but_bounded").unwrap();
    let (mut edge, service) = open_bridge(context, peer).await;

    let mut errors = Vec::new();
    for (sequence, handle) in [(1, cross), (2, released), (3, unknown)] {
        send_start(
            &mut edge,
            sequence,
            snapshot(Value::String(handle.as_str().to_string())),
            Some(second.clone()),
        )
        .await;
        let error = read_rejection(&mut edge).await;
        assert!(!error.message.contains(handle.as_str()));
        errors.push(error);
    }
    assert!(errors.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(errors[0].kind, BridgeErrorKind::InvalidRequest);
    assert_eq!(errors[0].message, "unknown tab");
    assert!(seen.lock().unwrap().is_empty());

    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn flow_step_handle_is_normalized_and_nested_result_is_enriched() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let composite = composite_tab(&browser, 5);
    let context = build_context(browser);
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 4,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&workspace, composite);
    let handle = context
        .workspaces
        .tab_handle(&workspace, composite)
        .unwrap();
    let raw = handle.as_str().to_string();
    let operation = flow(snapshot(Value::String(raw.clone())));
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(&mut edge, 1, operation, Some(workspace.clone())).await;
    assert!(matches!(
        read_service_message(&mut edge).await.unwrap(),
        Some(ServiceMessage::Started { .. })
    ));
    let completed = read_service_message(&mut edge)
        .await
        .unwrap()
        .expect("completed flow result");
    let ServiceMessage::Completed {
        outcome: TerminalOutcome { result },
        ..
    } = completed
    else {
        panic!("expected successful flow result: {completed:?}");
    };
    let Some(OperationResult::BrowserRunSequence(flow)) = result.result.as_ref() else {
        panic!("expected typed sequence result: {result:#?}")
    };
    assert_eq!(flow.termination.reason.as_str(), "completed");
    let Some(OperationResult::BrowserInspectPage { targets, .. }) =
        flow.steps[0].result.result.as_ref()
    else {
        panic!("expected typed child inspection result")
    };
    assert_eq!(targets[0].r#ref, "r_1");
    assert_eq!(
        flow.steps[0].result.tab.as_ref().map(|tab| tab.id.as_str()),
        Some(handle.as_str())
    );

    {
        let requests = seen.lock().unwrap();
        let dispatched = requests
            .iter()
            .find(|request| {
                request.get("type").and_then(Value::as_str) == Some("mechanism_request")
            })
            .expect("flow step reached browser");
        assert_eq!(dispatched["input"]["tab"], 5);
        assert!(!serde_json::to_string(dispatched).unwrap().contains(&raw));
    }
    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_close_commits_without_a_result_handle_and_retires_the_old_handle() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let composite = composite_tab(&browser, 5);
    let context = build_context(browser);
    let workspaces = context.workspaces.clone();
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 41,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&workspace, composite);
    let handle = context
        .workspaces
        .tab_handle(&workspace, composite)
        .unwrap();
    let raw = handle.as_str().to_string();
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(
        &mut edge,
        1,
        close(Value::String(raw.clone())),
        Some(workspace.clone()),
    )
    .await;
    assert!(matches!(
        read_service_message(&mut edge).await.unwrap(),
        Some(ServiceMessage::Started {
            sequence: BridgeSequence(1),
            ..
        })
    ));
    let completed = read_service_message(&mut edge)
        .await
        .unwrap()
        .expect("completed close result");
    let ServiceMessage::Completed {
        outcome: TerminalOutcome { result },
        ..
    } = completed
    else {
        panic!("expected successful close result");
    };
    assert_eq!(result.status, BrowserResultStatus::Ok);
    assert_eq!(result.effect, OperationEffect::Committed);
    assert_eq!(
        result.result,
        Some(OperationResult::BrowserCloseTab { closed: true })
    );
    assert!(result.tab.is_none());
    assert!(result.tabs.is_empty());
    assert_eq!(workspaces.resolve_tab(&workspace, &handle), None);

    send_start(
        &mut edge,
        2,
        snapshot(Value::String(raw.clone())),
        Some(workspace),
    )
    .await;
    let rejected = read_rejection(&mut edge).await;
    assert_eq!(rejected.kind, BridgeErrorKind::InvalidRequest);
    assert_eq!(rejected.message, "unknown tab");
    assert!(!rejected.message.contains(&raw));
    assert_eq!(
        seen.lock()
            .unwrap()
            .iter()
            .filter(|request| request.get("mechanism").and_then(Value::as_str) == Some("tab.close"))
            .count(),
        1,
        "the retired handle was rejected before another browser dispatch"
    );

    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn sequence_rejects_tab_management_before_dispatch() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let composite = composite_tab(&browser, 5);
    let context = build_context(browser);
    let workspaces = context.workspaces.clone();
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 42,
    };
    let workspace = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&workspace, composite);
    let handle = context
        .workspaces
        .tab_handle(&workspace, composite)
        .unwrap();
    let raw = handle.as_str().to_string();
    let operation = flow(close(Value::String(raw.clone())));
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(&mut edge, 1, operation, Some(workspace.clone())).await;
    let rejected = read_rejection(&mut edge).await;
    assert_eq!(rejected.kind, BridgeErrorKind::InvalidRequest);
    assert!(rejected.message.contains("sequence"));
    assert_eq!(workspaces.resolve_tab(&workspace, &handle), Some(composite));
    assert!(seen.lock().unwrap().is_empty());
    assert!(!rejected.message.contains(&raw));
    drop(edge);
    service.await.unwrap().unwrap();
}

#[tokio::test]
async fn cross_workspace_flow_handle_and_nested_flow_are_rejected_before_dispatch() {
    let browser = Browser::new();
    let seen = attach_browser(&browser).await;
    let context = build_context(browser);
    let peer = PeerCred {
        user: PeerUser("owner".into()),
        pid: 5,
    };
    let first = context.workspaces.mint(&peer.user, false).unwrap();
    let second = context.workspaces.mint(&peer.user, false).unwrap();
    context.workspaces.claim_tab(&first, 5);
    let handle = context.workspaces.tab_handle(&first, 5).unwrap();
    let (mut edge, service) = open_bridge(context, peer).await;

    send_start(
        &mut edge,
        1,
        flow(snapshot(Value::String(handle.as_str().to_string()))),
        Some(second.clone()),
    )
    .await;
    let cross = read_rejection(&mut edge).await;
    assert_eq!(cross.kind, BridgeErrorKind::InvalidRequest);
    assert_eq!(cross.message, "unknown tab");

    let nested = flow(flow(snapshot(Value::String(handle.as_str().to_string()))));
    send_start(&mut edge, 2, nested, Some(second)).await;
    let nested = read_rejection(&mut edge).await;
    assert_eq!(nested.kind, BridgeErrorKind::InvalidRequest);
    assert!(nested.message.contains("sequence must contain"));
    assert!(seen.lock().unwrap().is_empty());

    drop(edge);
    service.await.unwrap().unwrap();
}
