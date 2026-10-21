//! Documents at the application boundary: content and effects come only from admitted documents
//! (ADR-0158).

use super::*;
use crate::browser::adapter::documents::{DocumentInventory, PhysicalDocument};
use crate::scratch::Scratch;
use crate::workbench::WorkbenchFacade;
use crate::workspace::WorkspaceId;

pub(super) fn inventory(child_supported: bool) -> DocumentInventory {
    DocumentInventory {
        documents: vec![
            PhysicalDocument {
                id: "top".into(),
                url: "https://example.com/".into(),
                parent: None,
                supported: true,
            },
            PhysicalDocument {
                id: "child".into(),
                url: "https://private-frame.invalid/HIDDEN_PATH".into(),
                parent: Some("top".into()),
                supported: child_supported,
            },
        ],
        ..DocumentInventory::default()
    }
}

pub(super) fn policy_file(child_read: bool) -> Scratch {
    let path = Scratch::file("documents-policy.json");
    let mut grants = vec![
        json!({"id":"main","hosts":{"allow":["example.com"]},"allowed":["read","action","write","execute"]}),
    ];
    if child_read {
        grants.push(json!({"id":"embedded_read","hosts":{"allow":["private-frame.invalid"]},"allowed":["read"]}));
    }
    fs::write(
        &path,
        serde_json::to_vec(
            &json!({"schema":4,"name":"H6","version":"1","grants":grants,"config":[]}),
        )
        .unwrap(),
    )
    .unwrap();
    path
}

pub(super) fn open(executor: &ApplicationExecutor, browser: &FakeBrowser, workspace: &WorkspaceId) {
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    assert_eq!(
        executor
            .execute(
                workspace,
                "browser_navigate",
                json!({"url":"https://example.com/","new_tab":true}),
                &CancellationToken::default()
            )
            .status,
        Status::Succeeded
    );
}

pub(super) fn captured_image() -> BrowserOutcome {
    BrowserOutcome::Screenshot {
        tab_id: 7,
        mime_type: "image/jpeg".into(),
        data: "aW1hZ2U=".into(),
        width: 1280,
        height: 720,
        viewport: ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 0.0,
            page_y: 0.0,
            css_width: 1280.0,
            css_height: 720.0,
            visual_page_x: 0.0,
            visual_page_y: 0.0,
            visual_css_width: 1280.0,
            visual_css_height: 720.0,
            device_scale: 1.0,
            zoom: 1.0,
            output_scale: 1.0,
        },
    }
}

/// A session under the H6 policy, with a tab open on example.com.
fn governed(
    child_read: bool,
) -> (
    Scratch,
    ApplicationExecutor,
    Arc<FakeBrowser>,
    WorkspaceId,
    Arc<MemoryAudit>,
) {
    let path = policy_file(child_read);
    let (executor, browser, _, workspace, audit) =
        fixture_with_governance(GovernanceFacade::new(Some(path.to_path_buf())));
    open(&executor, &browser, &workspace);
    (path, executor, browser, workspace, audit)
}

/// The handle `browser_inspect` gives the one control `target`.
fn handle_of(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &WorkspaceId,
    target: ObservedTarget,
) -> String {
    browser.push(Ok(BrowserOutcome::targets(7, vec![target])));
    let observed = executor.execute(
        workspace,
        "browser_inspect",
        json!({}),
        &CancellationToken::default(),
    );
    observed.facts["items"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// ADR-0158 items 1-3 and 5: content and effects come only from admitted documents.
///
/// - An embedded document the policy excludes is filtered out before extraction; its host never
///   reaches the model or the audit, while the person's coverage details name it and announce it.
/// - A permitted target survives an exclusion elsewhere on the page, and its coverage counts the
///   page's exclusions apart from its own.
/// - A field in a document admitted only for Read is not writable, and the fill never starts.
/// - A document Ghostlight could not see cannot prove absence, a script cannot cross an exclusion,
///   and a read counts the unseen document as unavailable.
/// - Restricted unbounded work refuses before dispatch while a placeholder is unseen, and runs
///   strictly scoped once it is seen; all-open work runs with no document restriction.
/// - A recording is saved nowhere unless every page it captured is still admitted.
/// - Screenshots are pixels, never filtered by document; a policy that disables them refuses
///   every capture form before dispatch.
#[test]
fn content_and_effects_come_only_from_admitted_documents() {
    // An excluded document is filtered before extraction and named only to the person.
    let (_policy, executor, browser, workspace, audit) = governed(false);
    browser.set_documents(7, inventory(true));
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "Permitted Sylin fixture content".into(),
        title: "Page".into(),
        url: "https://example.com/".into(),
        truncated: false,
    }));
    let result = executor.execute(
        &workspace,
        "browser_read",
        json!({}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert_eq!(result.facts["coverage"]["excluded_documents"], 1);
    assert_eq!(browser.scopes().last().unwrap().allowed, ["top"]);
    let model = serde_json::to_string(&result).unwrap();
    let retained = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    for hidden in ["private-frame.invalid", "HIDDEN_PATH"] {
        assert!(!model.contains(hidden));
        assert!(!retained.contains(hidden));
    }
    assert!(audit.0.lock().unwrap().last().unwrap().coverage.is_some());
    let facade = WorkbenchFacade::new(
        executor.workbench.clone(),
        executor.workspaces.clone(),
        executor.governance.clone(),
        Arc::new(crate::browser::RelayBrowserPort::new(
            "test".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let snapshot = facade.snapshot();
    let details = snapshot
        .document_coverage
        .iter()
        .find(|item| item.invocation == result.invocation)
        .unwrap();
    assert_eq!(details.excluded_hosts, ["private-frame.invalid"]);
    assert!(
        details.proactive,
        "an exclusion that affects the work is announced"
    );

    // A permitted target survives an unrelated exclusion.
    for child_supported in [true, false] {
        let (_policy, executor, browser, workspace, _) = governed(false);
        let target = handle_of(
            &executor,
            &browser,
            &workspace,
            ObservedTarget {
                locator: "button".into(),
                role: "button".into(),
                name: "Continue".into(),
                ..Default::default()
            },
        );
        let mut documents = inventory(child_supported);
        documents.subjects = vec!["top".into()];
        browser.set_documents(7, documents);
        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            subject: None,
            committed_urls: vec![],
        }));
        let result = executor.execute(
            &workspace,
            "browser_click",
            json!({"target":target}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
        assert_eq!(result.facts["coverage"]["excluded_documents"], 0);
        assert_eq!(
            result.facts["coverage"]["page_excluded_documents"],
            usize::from(child_supported)
        );
    }

    // A field readable but not writable: the fill never starts.
    let (_policy, executor, browser, workspace, _) = governed(true);
    browser.set_documents(7, inventory(true));
    let target = handle_of(
        &executor,
        &browser,
        &workspace,
        ObservedTarget {
            locator: "field".into(),
            role: "textbox".into(),
            name: "Name".into(),
            ..Default::default()
        },
    );
    let mut documents = inventory(true);
    documents.subjects = vec!["child".into()];
    browser.set_documents(7, documents);
    let before = browser.calls().len();
    let result = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"fields":[{"target":target,"value":"NOT_SENT"}]}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked, "{}", result.summary);
    assert_eq!(result.effect, Effect::None);
    assert_eq!(browser.calls().len(), before);

    // An unseen document cannot prove absence, and a script cannot cross an exclusion.
    let (_policy, executor, browser, workspace, _) = governed(false);
    browser.set_documents(7, inventory(true));
    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 0,
        readiness: BrowserReadiness::Complete,
    }));
    let absent = executor.execute(
        &workspace,
        "browser_wait",
        json!({"condition":"text_absent","value":"private"}),
        &CancellationToken::default(),
    );
    assert_eq!(absent.status, Status::Failed, "{}", absent.summary);
    assert_eq!(absent.facts["coverage"]["excluded_documents"], 1);
    let script = executor.execute(
        &workspace,
        "browser_execute",
        json!({"script":"window.effect = 1"}),
        &CancellationToken::default(),
    );
    assert_eq!(script.status, Status::Blocked);
    assert_eq!(script.effect, Effect::None);
    assert_eq!(browser.calls().len(), 2, "the open and the wait");
    browser.set_documents(7, inventory(false));
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "Permitted".into(),
        title: "Page".into(),
        url: "https://example.com/".into(),
        truncated: false,
    }));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({}),
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Succeeded);
    assert_eq!(read.facts["coverage"]["unavailable_documents"], 1);
    assert_eq!(read.facts["coverage"]["excluded_documents"], 0);

    // An unseen placeholder: restricted unbounded work refuses, then runs strictly once seen.
    for supported in [false, true] {
        let (_policy, executor, browser, workspace, audit) = governed(false);
        browser.set_documents(
            7,
            DocumentInventory {
                documents: vec![
                    PhysicalDocument {
                        id: "T.t1".into(),
                        url: "https://example.com/".into(),
                        parent: None,
                        supported: true,
                    },
                    PhysicalDocument {
                        id: "unseen.0".into(),
                        url: "https://example.com/embedded".into(),
                        parent: Some("T.t1".into()),
                        supported,
                    },
                ],
                ..DocumentInventory::default()
            },
        );
        if supported {
            browser.push(Ok(script_ran()));
        }
        let result = executor.execute(
            &workspace,
            "browser_execute",
            json!({"script":"1+1"}),
            &CancellationToken::default(),
        );
        if supported {
            assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
            let scopes = browser.scopes();
            assert_eq!(scopes.len(), 1);
            assert!(scopes[0].strict_tree);
        } else {
            assert_eq!(result.status, Status::Failed, "{}", result.summary);
            assert_eq!(result.effect, Effect::None);
            assert_eq!(result.facts["coverage"]["unavailable_documents"], 1);
            assert!(browser.scopes().is_empty(), "no scoped work is sent");
            assert_eq!(browser.calls().len(), 1, "only the navigation ran");
            assert_eq!(
                audit.0.lock().unwrap().last().unwrap().refusal_facts,
                Some(crate::language::audit::AuditRefusal::DocumentUnavailable)
            );
        }
    }

    // All-open work keeps its browser behavior, with no document restriction.
    let (executor, browser, _, workspace, _) = fixture_with_governance(GovernanceFacade::new(None));
    open(&executor, &browser, &workspace);
    let mut documents = inventory(false);
    documents.documents[1].url = "about:blank".into();
    browser.set_documents(7, documents);
    browser.push(Ok(script_ran()));
    let result = executor.execute(
        &workspace,
        "browser_execute",
        json!({"script":"1+1"}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    let scope = browser.scopes().last().unwrap().clone();
    assert!(!scope.strict_tree && !scope.watch_changes);

    // A recording is saved nowhere unless every captured page is still admitted.
    for destination in [
        json!({}),
        json!({"download":true}),
        json!({"target":"target_fixture"}),
    ] {
        let path = policy_file(false);
        let (executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(path.to_path_buf())));
        holding_recording(&executor, &workspace);
        open(&executor, &browser, &workspace);
        let mut summary = recording_summary(RecordingState::Frozen, "https://example.com/");
        summary
            .source_urls
            .push("https://private-frame.invalid/".into());
        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary,
            changed: false,
        }));
        let mut arguments = destination;
        arguments["action"] = json!("save");
        let result = executor.execute(
            &workspace,
            "browser_record",
            arguments,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked, "{}", result.summary);
        assert_eq!(
            browser.calls().len(),
            2,
            "no encoder or destination receives captured content"
        );
    }

    // Screenshots are never filtered by document, and a policy can disable every form of them.
    for child_read in [None, Some(false)] {
        let path = child_read.map(policy_file);
        let (executor, browser, _, workspace, _) = fixture_with_governance(GovernanceFacade::new(
            path.as_ref().map(|path| path.to_path_buf()),
        ));
        open(&executor, &browser, &workspace);
        browser.set_documents(7, DocumentInventory::default());
        for arguments in [json!({}), json!({"full_page":true})] {
            browser.push(Ok(captured_image()));
            let result = executor.execute(
                &workspace,
                "browser_screenshot",
                arguments,
                &CancellationToken::default(),
            );
            assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
            assert!(result.facts.get("coverage").is_none());
            assert!(
                browser.scopes().is_empty(),
                "capture never enters document filtering"
            );
        }
    }
    let (path, executor, browser, workspace, audit) = governed(true);
    let target = handle_of(
        &executor,
        &browser,
        &workspace,
        ObservedTarget {
            locator: "heading".into(),
            role: "heading".into(),
            name: "Page".into(),
            ..Default::default()
        },
    );
    browser.push(Ok(captured_image()));
    let image = executor.execute(
        &workspace,
        "browser_screenshot",
        json!({}),
        &CancellationToken::default(),
    );
    let view = image.facts["view"].as_str().unwrap();
    let mut policy: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    policy["config"].as_array_mut().unwrap().push(json!({
        "key":crate::governance::manifest::PolicySetting::ScreenshotsEnabled.key(),"value":false,"level":"mandatory",
    }));
    fs::write(&path, serde_json::to_vec(&policy).unwrap()).unwrap();
    executor.governance.refresh_tick();
    let prior = browser.calls().len();
    for arguments in [
        json!({}),
        json!({"full_page":true}),
        json!({"target":target}),
        json!({"view":view,"x":0,"y":0,"width":100,"height":100}),
    ] {
        let result = executor.execute(
            &workspace,
            "browser_screenshot",
            arguments,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked, "{}", result.summary);
        assert_eq!(result.effect, Effect::None);
        assert_eq!(browser.calls().len(), prior, "no capture command is sent");
        assert_eq!(
            audit.0.lock().unwrap().last().unwrap().reason,
            crate::governance::ReasonCode::ScreenshotDenied
        );
    }
}

/// A script the browser ran, answering 2.
fn script_ran() -> BrowserOutcome {
    BrowserOutcome::ScriptEvaluated {
        tab: tab(7, "https://example.com/"),
        value: "2".into(),
        truncated: false,
        form: crate::browser::adapter::ScriptForm::AsWritten,
        committed_urls: vec![],
    }
}
