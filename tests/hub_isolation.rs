// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Workspace ownership gates browser handles before any browser probe or dispatch.

use ghostlight::governance::manifest::source::LoadedPolicy;
use ghostlight::hub::outbound::browser::Browser;
use ghostlight::hub::peer::PeerUser;
use ghostlight::hub::ServiceContext;
use ghostlight::native::host;
use ghostlight::observability::DebugSink;
use ghostlight::tool::outcome::CallOutcome;
use ghostlight::tool::pipeline::run_work;
use ghostlight::work::{CancellationToken, WorkContext};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn build_ctx(browser: Browser) -> ServiceContext {
    ServiceContext::from_startup(
        browser,
        DebugSink::disabled(),
        LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        },
        ghostlight::governance::config::reload::PolicySource::SourceString { user_source: None },
        None,
    )
    .expect("build neutral service context")
}

async fn attach_observer(browser: &Browser) -> Arc<Mutex<Vec<String>>> {
    let (browser_side, mut extension_side) = tokio::io::duplex(64 * 1024);
    let attached = browser.clone();
    tokio::spawn(async move {
        let _ = attached.attach(browser_side).await;
    });
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_task = Arc::clone(&seen);
    tokio::spawn(async move {
        let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
        host::write_message(&mut extension_side, &hello)
            .await
            .unwrap();
        let identity = serde_json::to_vec(&json!({
            "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
            ghostlight_transport::handshake::BROWSER_ID_FIELD: "workspace-isolation",
        }))
        .unwrap();
        host::write_message(&mut extension_side, &identity)
            .await
            .unwrap();
        while let Ok(Some(frame)) = host::read_message(&mut extension_side).await {
            let request: Value = serde_json::from_slice(&frame).unwrap();
            seen_for_task.lock().unwrap().push(
                request
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
            );
            if let Some(id) = request.get("id") {
                let reply = json!({
                    "id": id,
                    "type": "tool_response",
                    "result": {
                        "content": [{"type": "text", "text": "ok"}],
                        "structuredContent": {}
                    }
                });
                host::write_message(&mut extension_side, &serde_json::to_vec(&reply).unwrap())
                    .await
                    .unwrap();
            }
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

async fn denied_for(
    ctx: &ServiceContext,
    workspace: ghostlight_transport::workspace_id::WorkspaceId,
    tab_id: i64,
) -> String {
    let work = WorkContext::new(Some(workspace), "read_page", None, None);
    match run_work(
        &ctx.browser,
        &ctx.store,
        &ctx.authority,
        &ctx.workspaces,
        &work,
        &CancellationToken::new(),
        &json!({ "tabId": tab_id }),
    )
    .await
    {
        CallOutcome::Denied { message, .. } => message,
        _ => panic!("unknown or cross-workspace tab must be denied"),
    }
}

#[tokio::test]
async fn cross_workspace_tab_is_refused_before_any_browser_probe() {
    let browser = Browser::new();
    let seen = attach_observer(&browser).await;
    let ctx = build_ctx(browser);
    let owner = PeerUser("owner".into());
    let first = ctx.workspaces.mint(&owner, false).unwrap();
    let second = ctx.workspaces.mint(&owner, false).unwrap();
    assert_eq!(
        ctx.workspaces.claim_tab(&first, 5),
        ghostlight::hub::workspace::TabClaim::Adopted
    );

    assert_eq!(denied_for(&ctx, second, 5).await, "unknown tab");
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn genuinely_unowned_tab_is_refused_without_adoption_or_browser_frames() {
    let browser = Browser::new();
    let seen = attach_observer(&browser).await;
    let ctx = build_ctx(browser);
    let owner = PeerUser("owner".into());
    let workspace = ctx.workspaces.mint(&owner, false).unwrap();

    assert_eq!(denied_for(&ctx, workspace.clone(), 77).await, "unknown tab");
    assert!(ctx.workspaces.owned_tabs(&workspace).is_empty());
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn already_owned_tab_dispatches_only_the_requested_tool() {
    let browser = Browser::new();
    let seen = attach_observer(&browser).await;
    let ctx = build_ctx(browser);
    let owner = PeerUser("owner".into());
    let workspace = ctx.workspaces.mint(&owner, false).unwrap();
    assert_eq!(
        ctx.workspaces.claim_tab(&workspace, 5),
        ghostlight::hub::workspace::TabClaim::Adopted
    );
    let work = WorkContext::new(Some(workspace), "read_page", None, None);

    let outcome = run_work(
        &ctx.browser,
        &ctx.store,
        &ctx.authority,
        &ctx.workspaces,
        &work,
        &CancellationToken::new(),
        &json!({"tabId": 5}),
    )
    .await;

    assert!(matches!(outcome, CallOutcome::Success { .. }));
    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|kind| kind == "tool_request"));
    assert_eq!(
        seen.iter().filter(|kind| *kind == "tool_request").count(),
        1
    );
}

#[tokio::test]
async fn unknown_tab_result_leaks_no_host_or_existence() {
    let ctx = build_ctx(Browser::new());
    let owner = PeerUser("owner".into());
    let first = ctx.workspaces.mint(&owner, false).unwrap();
    let second = ctx.workspaces.mint(&owner, false).unwrap();
    ctx.workspaces.claim_tab(&first, 5);
    ctx.workspaces.claim_tab(&first, 999);

    let existing = denied_for(&ctx, second.clone(), 5).await;
    let nonexistent = denied_for(&ctx, second, 999).await;
    assert_eq!(existing, nonexistent);
    assert_eq!(existing, "unknown tab");
    assert!(!existing.contains("secret-host"));
}
