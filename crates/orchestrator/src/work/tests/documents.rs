//! H6 authority, scope, privacy, and truthful-negative executor regressions.

use super::*;
use crate::workbench::WorkbenchFacade;
use crate::workspace::WorkspaceId;
use ghostlight_bridge::browser::documents::{DocumentInventory, PhysicalDocument};
use uuid::Uuid;

fn inventory(child_supported: bool) -> DocumentInventory {
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

fn policy_file(mode: &str, notice: &str, child_read: bool) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ghostlight-h6-{}.json", Uuid::new_v4()));
    let mut grants = vec![
        json!({"id":"main","hosts":{"allow":["example.com"]},"allowed":["read","action","write","execute"]}),
    ];
    if child_read {
        grants.push(json!({"id":"embedded_read","hosts":{"allow":["private-frame.invalid"]},"allowed":["read"]}));
    }
    fs::write(
        &path,
        serde_json::to_vec(
            &json!({"schema":3,"name":"H6","version":"1","grants":grants,"config":[
                {"key":"content.frames.handling","value":mode,"level":"mandatory"},
                {"key":"content.frames.notice","value":notice,"level":"mandatory"}
            ]}),
        )
        .unwrap(),
    )
    .unwrap();
    path
}

fn open(executor: &ApplicationExecutor, browser: &FakeBrowser, workspace: &WorkspaceId) {
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    assert_eq!(
        executor
            .execute(
                workspace,
                "browser_navigate",
                json!({"url":"https://example.com/","new_tab":true}),
                None,
                &CancellationToken::default()
            )
            .status,
        Status::Succeeded
    );
}

#[test]
fn page_modes_filter_before_extraction_and_keep_hosts_only_in_human_details() {
    for mode in ["permitted_content", "complete_operation", "complete_page"] {
        for notice in ["on_demand", "when_affected", "when_excluded"] {
            let path = policy_file(mode, notice, false);
            let (executor, browser, _, workspace, audit) =
                fixture_with_governance(GovernanceFacade::new(Some(path.clone()), None));
            open(&executor, &browser, &workspace);
            browser.set_documents(7, inventory(true));
            if mode == "permitted_content" {
                browser.push(Ok(BrowserOutcome::Text {
                    tab_id: 7,
                    text: "Permitted Sylin fixture content".into(),
                    title: "Page".into(),
                    url: "https://example.com/".into(),
                    truncated: false,
                }));
            }
            let result = executor.execute(
                &workspace,
                "browser_read",
                json!({}),
                None,
                &CancellationToken::default(),
            );
            assert_eq!(
                result.status,
                if mode == "permitted_content" {
                    Status::Succeeded
                } else {
                    Status::Blocked
                },
                "{}",
                result.summary
            );
            assert_eq!(result.facts["coverage"]["excluded_documents"], 1);
            if mode == "permitted_content" {
                assert_eq!(browser.scopes().last().unwrap().allowed, ["top"]);
            } else {
                assert_eq!(browser.calls().len(), 1, "no content primitive may run");
            }
            let model = serde_json::to_string(&result).unwrap();
            let retained = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
            for hidden in ["private-frame.invalid", "HIDDEN_PATH"] {
                assert!(!model.contains(hidden));
                assert!(!retained.contains(hidden));
            }
            let facade = WorkbenchFacade::new(
                executor.workbench.clone(),
                executor.workspaces.clone(),
                executor.governance.clone(),
                Arc::new(crate::browser::RelayBrowserPort::new("test".into())),
                crate::diagnostics::DiagnosticsHub::for_tests(),
            );
            let snapshot = facade.snapshot();
            let details = snapshot
                .document_coverage
                .iter()
                .find(|item| item.invocation == result.invocation)
                .unwrap();
            assert_eq!(details.excluded_hosts, ["private-frame.invalid"]);
            assert_eq!(details.proactive, notice != "on_demand");
            assert!(audit.0.lock().unwrap().last().unwrap().coverage.is_some());
            fs::remove_file(path).unwrap();
        }
    }
}

#[test]
fn permitted_target_survives_unrelated_exclusion_except_under_complete_page() {
    for mode in ["permitted_content", "complete_operation", "complete_page"] {
        for child_supported in [true, false] {
            let path = policy_file(mode, "when_affected", false);
            let (executor, browser, _, workspace, _) =
                fixture_with_governance(GovernanceFacade::new(Some(path.clone()), None));
            open(&executor, &browser, &workspace);
            browser.push(Ok(BrowserOutcome::Targets {
                tab_id: 7,
                targets: vec![ObservedTarget {
                    locator: "button".into(),
                    role: "button".into(),
                    name: "Continue".into(),
                    state: vec![],
                    credential_class: false,
                }],
            }));
            let observed = executor.execute(
                &workspace,
                "browser_inspect",
                json!({}),
                None,
                &CancellationToken::default(),
            );
            let target = observed.facts["items"][0]["target"].as_str().unwrap();
            let mut documents = inventory(child_supported);
            documents.subjects = vec!["top".into()];
            browser.set_documents(7, documents);
            if mode != "complete_page" {
                browser.push(Ok(BrowserOutcome::Activated {
                    tab: tab(7, "https://example.com/"),
                    subject: None,
                    committed_urls: vec![],
                }));
            }
            let result = executor.execute(
                &workspace,
                "browser_click",
                json!({"target":target}),
                None,
                &CancellationToken::default(),
            );
            assert_eq!(
                result.status,
                if mode == "complete_page" {
                    if child_supported {
                        Status::Blocked
                    } else {
                        Status::Failed
                    }
                } else {
                    Status::Succeeded
                },
                "{}",
                result.summary
            );
            assert_eq!(result.facts["coverage"]["excluded_documents"], 0);
            assert_eq!(
                result.facts["coverage"]["page_excluded_documents"],
                usize::from(child_supported)
            );
            fs::remove_file(path).unwrap();
        }
    }
}

#[test]
fn readable_embedded_field_is_not_writable_and_batch_never_starts() {
    let path = policy_file("permitted_content", "when_affected", true);
    let (executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(path.clone()), None));
    open(&executor, &browser, &workspace);
    browser.set_documents(7, inventory(true));
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "field".into(),
            role: "textbox".into(),
            name: "Name".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
    let observed = executor.execute(
        &workspace,
        "browser_inspect",
        json!({}),
        None,
        &CancellationToken::default(),
    );
    let target = observed.facts["items"][0]["target"].as_str().unwrap();
    let mut documents = inventory(true);
    documents.subjects = vec!["child".into()];
    browser.set_documents(7, documents);
    let result = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"fields":[{"target":target,"value":"NOT_SENT"}]}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked, "{}", result.summary);
    assert_eq!(result.effect, Effect::None);
    assert_eq!(browser.calls().len(), 2);
    fs::remove_file(path).unwrap();
}

#[test]
fn unseen_documents_cannot_prove_absence_and_scripts_do_not_cross_exclusions() {
    let path = policy_file("permitted_content", "on_demand", false);
    let (executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(path.clone()), None));
    open(&executor, &browser, &workspace);
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
        None,
        &CancellationToken::default(),
    );
    assert_eq!(absent.status, Status::Failed, "{}", absent.summary);
    assert_eq!(absent.facts["coverage"]["excluded_documents"], 1);
    let script = executor.execute(
        &workspace,
        "browser_execute",
        json!({"script":"window.effect = 1"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(script.status, Status::Blocked);
    assert_eq!(script.effect, Effect::None);
    assert_eq!(browser.calls().len(), 2);
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
        None,
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Succeeded);
    assert_eq!(read.facts["coverage"]["unavailable_documents"], 1);
    assert_eq!(read.facts["coverage"]["excluded_documents"], 0);
    fs::remove_file(path).unwrap();
}

#[test]
fn all_open_scripts_keep_their_browser_behavior_without_document_host_restrictions() {
    let (executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(None, None));
    open(&executor, &browser, &workspace);
    let mut documents = inventory(false);
    documents.documents[1].url = "about:blank".into();
    browser.set_documents(7, documents);
    browser.push(Ok(BrowserOutcome::ScriptEvaluated {
        tab: tab(7, "https://example.com/"),
        value: "2".into(),
        truncated: false,
        committed_urls: vec![],
    }));
    let result = executor.execute(
        &workspace,
        "browser_execute",
        json!({"script":"1+1"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert!(!browser.scopes().last().unwrap().watch_changes);
    browser.push(Ok(BrowserOutcome::EffectUnknown {
        reason: "lost acknowledgement".into(),
    }));
    let unknown = executor.execute(
        &workspace,
        "browser_execute",
        json!({"script":"window.effect=1"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(unknown.status, Status::Unknown);
    assert_eq!(unknown.effect, Effect::Unknown);
    assert!(!unknown.repeat_safe);
}

#[test]
fn every_recording_destination_requires_current_access_to_all_captured_sources() {
    for destination in [
        json!({}),
        json!({"download":true}),
        json!({"target":"target_fixture"}),
    ] {
        let path = policy_file("permitted_content", "on_demand", false);
        let (executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(path.clone()), None));
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
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked, "{}", result.summary);
        assert_eq!(
            browser.calls().len(),
            2,
            "no encoder or destination receives captured content"
        );
        fs::remove_file(path).unwrap();
    }
}
