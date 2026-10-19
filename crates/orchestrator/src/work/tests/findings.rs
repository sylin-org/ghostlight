//! What a frame learns lives with the frame (ADR-0202 Decision 9 to 13): a flow and each of its
//! steps record only their own checks, coverage and browser, a step answers for its own browser,
//! tab and target as the same call outside a flow would, and two calls in one workspace never
//! share what they learned.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Instant;

use serde_json::Value;

use super::documents::{inventory, open, policy_file};
use super::execution::workspace_with_a_target;
use super::replacement::{one_same, removed};
use super::*;
use crate::browser::adapter::BrowserCommand as Work;
use crate::browser::{BrowserPort, BrowserSummary};
use crate::governance::CapabilitySet;

#[test]
fn a_flow_record_keeps_its_own_admission_check() {
    let (executor, _, _, workspace, audit) = fixture();
    let result = executor.execute(
        &workspace,
        "browser_flow",
        json!({"steps":[{"id":"policy","tool":"policy_explain","arguments":{}}]}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    let records = audit.0.lock().unwrap();
    let tools: Vec<_> = records.iter().map(|record| record.tool.as_str()).collect();
    assert_eq!(tools, ["policy_explain", "browser_flow"]);
    for record in records.iter() {
        let checks = &record.permissions.checks;
        assert_eq!(checks.len(), 1, "{}: {checks:?}", record.tool);
        assert_eq!(checks[0].requirements, CapabilitySet::EMPTY);
        assert_eq!(checks[0].host, None);
        assert!(checks[0].allowed);
    }
}

#[test]
fn each_flow_step_record_holds_only_its_own_checks_and_coverage() {
    let path = policy_file(false);
    let (executor, browser, _, workspace, audit) =
        fixture_with_governance(GovernanceFacade::new(Some(path.to_path_buf()), None));
    open(&executor, &browser, &workspace);
    browser.set_documents(7, inventory(true));
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "Permitted Sylin fixture content".into(),
        title: "Page".into(),
        url: "https://example.com/".into(),
        truncated: false,
    }));
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(7, "https://example.com/")],
    }));
    let result = executor.execute(
        &workspace,
        "browser_flow",
        json!({"steps":[
            {"id":"read","tool":"browser_read","arguments":{}},
            {"id":"tabs","tool":"browser_tabs","arguments":{"action":"list"}}
        ]}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert_eq!(
        result
            .summary
            .matches("Content excluded by policy.")
            .count(),
        1,
        "the flow's summary is qualified once: {}",
        result.summary
    );
    let records = audit.0.lock().unwrap();
    let tools: Vec<_> = records.iter().map(|record| record.tool.as_str()).collect();
    assert_eq!(
        tools,
        [
            "browser_navigate",
            "browser_read",
            "browser_tabs",
            "browser_flow"
        ]
    );
    let (read, tabs, flow) = (&records[1], &records[2], &records[3]);
    assert_eq!(
        read.coverage
            .as_ref()
            .map(|coverage| coverage.excluded_documents),
        Some(1)
    );
    assert!(read
        .permissions
        .checks
        .iter()
        .any(|check| check.host.as_deref() == Some("example.com")));
    assert_eq!(tabs.coverage, None);
    assert!(
        tabs.permissions
            .checks
            .iter()
            .all(|check| check.host.is_none()),
        "the tabs step holds none of the read's checks: {:?}",
        tabs.permissions.checks
    );
    assert_eq!(flow.coverage, None);
    assert_eq!(flow.permissions.checks.len(), 1);
    assert_eq!(
        flow.permissions.checks[0].requirements,
        CapabilitySet::EMPTY
    );
}

#[test]
fn a_flow_record_names_the_browser_its_last_step_reached() {
    let (executor, browser, workspaces, workspace, audit) = fixture();
    // The session holds a tab, so its list asks the browser.
    workspaces
        .acquire(&workspace)
        .unwrap()
        .add_tab(FAKE_BROWSER, &tab(7, "https://example.com/"))
        .unwrap();
    browser.push(Ok(BrowserOutcome::Tabs { tabs: vec![] }));
    let result = executor.execute(
        &workspace,
        "browser_flow",
        json!({"on_error":"continue","steps":[
            {"id":"before","tool":"policy_explain","arguments":{}},
            {"id":"tabs","tool":"browser_tabs","arguments":{"action":"list"}},
            {"id":"after","tool":"policy_explain","arguments":{}}
        ]}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    let named: Vec<_> = audit
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|record| (record.tool.clone(), record.browser.clone()))
        .collect();
    let fake = Some(summary(FAKE_BROWSER, true).display_name());
    assert_eq!(
        named,
        [
            ("policy_explain".to_owned(), None),
            ("browser_tabs".to_owned(), fake.clone()),
            ("policy_explain".to_owned(), None),
            ("browser_flow".to_owned(), fake),
        ]
    );
}

#[test]
fn an_invalid_request_records_no_capabilities() {
    let (executor, browser, _, workspace, audit) = fixture();
    let refused = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"nonsense":true}),
        &CancellationToken::default(),
    );
    assert_eq!(refused.facts["reason"], "invalid_input");
    assert!(browser.calls().is_empty());
    let records = audit.0.lock().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].capabilities, CapabilitySet::EMPTY);
    assert!(records[0].permissions.checks.is_empty());
    assert_eq!(records[0].browser, None);
}

/// A fake browser that holds the first scoped read until the test lets it go, so a second call
/// can run in the same workspace while the read is in flight.
struct HeldRead {
    fake: Arc<FakeBrowser>,
    entered: Mutex<Option<mpsc::Sender<()>>>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

impl BrowserPort for HeldRead {
    fn call(
        &self,
        browser: &str,
        workspace: &str,
        command: Work,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        if matches!(command, Work::InDocuments { .. }) {
            if let Some(entered) = self.entered.lock().unwrap().take() {
                entered.send(()).unwrap();
                let release = self.release.lock().unwrap().take().unwrap();
                release.recv().unwrap();
            }
        }
        self.fake
            .call(browser, workspace, command, deadline, cancelled)
    }

    fn browsers(&self) -> Vec<BrowserSummary> {
        self.fake.browsers()
    }
}

#[test]
fn two_calls_in_one_workspace_keep_their_own_findings() {
    let path = policy_file(false);
    let (mut executor, fake, _, workspace, audit) =
        fixture_with_governance(GovernanceFacade::new(Some(path.to_path_buf()), None));
    open(&executor, &fake, &workspace);
    fake.set_documents(7, inventory(true));
    fake.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "Permitted Sylin fixture content".into(),
        title: "Page".into(),
        url: "https://example.com/".into(),
        truncated: false,
    }));
    let (entered, read_entered) = mpsc::channel();
    let (release_read, release) = mpsc::channel();
    executor.browser = Arc::new(HeldRead {
        fake: Arc::clone(&fake),
        entered: Mutex::new(Some(entered)),
        release: Mutex::new(Some(release)),
    });
    let executor = &executor;
    let workspace = &workspace;
    let (read, explained) = std::thread::scope(|scope| {
        let read = scope.spawn(move || {
            executor.execute(
                workspace,
                "browser_read",
                json!({}),
                &CancellationToken::default(),
            )
        });
        read_entered.recv().unwrap();
        // The independent lane runs beside the leased read while the read is in flight.
        let explained = executor.execute(
            workspace,
            "policy_explain",
            json!({}),
            &CancellationToken::default(),
        );
        release_read.send(()).unwrap();
        (read.join().unwrap(), explained)
    });
    assert_eq!(read.status, Status::Succeeded, "{}", read.summary);
    assert_eq!(explained.status, Status::Succeeded, "{}", explained.summary);
    let records = audit.0.lock().unwrap();
    let record_of = |invocation: &crate::work::InvocationId| {
        records
            .iter()
            .find(|record| record.invocation == invocation.as_str())
            .unwrap()
    };
    let explained = record_of(&explained.invocation);
    assert_eq!(explained.coverage, None);
    assert_eq!(explained.browser, None);
    assert_eq!(explained.permissions.checks.len(), 1);
    assert_eq!(explained.permissions.checks[0].host, None);
    let read = record_of(&read.invocation);
    assert!(read.coverage.is_some());
    assert!(read.browser.is_some());
    let checks = &read.permissions.checks;
    assert!(checks
        .iter()
        .any(|check| check.host.as_deref() == Some("example.com")));
    assert!(
        checks
            .iter()
            .all(|check| check.requirements != CapabilitySet::EMPTY),
        "the read holds none of the explanation's checks: {checks:?}"
    );
}

/// The result of the step at `index` of a flow's result.
fn step_result(flow: &crate::work::result::InvocationResult, index: usize) -> &Value {
    &flow.facts["steps"][index]["result"]
}

#[test]
fn a_flow_step_opens_in_the_browser_it_names() {
    let (executor, browser, workspaces, workspace, _) = fixture();
    browser.connect(vec![
        summary("browser_chrome", false),
        summary("browser_edge", true),
    ]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let flow = executor.execute(
        &workspace,
        "browser_flow",
        json!({"on_error":"continue","steps":[
            {"id":"chrome","tool":"browser_navigate",
                "arguments":{"url":"https://example.com","new_tab":true,"browser":"browser_chrome"}},
            {"id":"edge","tool":"browser_navigate",
                "arguments":{"url":"https://example.com","new_tab":true,"browser":"browser_edge"}}
        ]}),
        &CancellationToken::default(),
    );
    assert_eq!(
        step_result(&flow, 0)["status"],
        "succeeded",
        "{}",
        flow.facts
    );
    assert_eq!(browser.routed(), vec!["browser_chrome"]);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some("browser_chrome")
    );
    // The second step names another browser than the one the session now works in, and meets
    // the refusal the same call meets outside a flow.
    let pinned = step_result(&flow, 1);
    assert_eq!(pinned["status"], "failed", "{pinned}");
    assert_eq!(pinned["facts"]["reason"], "browser_pinned");
    assert_eq!(browser.routed(), vec!["browser_chrome"]);
}

#[test]
fn a_flow_step_whose_target_the_page_removed_is_marked_and_offered_its_replacement() {
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Err(removed(one_same())));
    let flow = executor.execute(
        &workspace,
        "browser_flow",
        json!({"steps":[{"id":"click","tool":"browser_click",
            "arguments":{"tab":tab_handle,"target":target}}]}),
        &CancellationToken::default(),
    );
    let refused = step_result(&flow, 0);
    assert_eq!(refused["facts"]["reason"], "stale_target", "{refused}");
    assert_eq!(refused["facts"]["stale"], "removed");
    let replacement = refused["facts"]["replacement"]
        .as_str()
        .expect("the step names the proven replacement");
    assert_ne!(replacement, target);
    assert_eq!(
        refused["next_steps"],
        json!([format!("Repeat browser_click with target {replacement}.")])
    );

    // The step marked the handle removed, so its next use refuses before any action reaches
    // the page.
    let before = browser.calls().len();
    let again = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target}),
        &CancellationToken::default(),
    );
    assert_eq!(again.facts["reason"], "stale_target");
    assert_eq!(again.facts["stale"], "removed");
    assert!(browser.calls()[before..]
        .iter()
        .all(|call| matches!(call, Work::QuerySemantic { .. })));
}

#[test]
fn a_flow_step_refused_for_another_sessions_tab_names_that_session() {
    let (executor, browser, _, workspace_a, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    let workspace_b = executor
        .workspaces
        .admit("Workspace B".into(), IntakeChannel::Mcp);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(42, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace_b,
        "browser_navigate",
        json!({"url": "https://example.com", "new_tab": true}),
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded);
    let tab_b = opened.facts["tab"].as_str().unwrap().to_owned();
    let dispatched = browser.calls().len();

    let flow = executor.execute(
        &workspace_a,
        "browser_flow",
        json!({"steps":[{"id":"read","tool":"browser_read","arguments":{"tab":tab_b}}]}),
        &CancellationToken::default(),
    );
    let refused = step_result(&flow, 0);
    assert_eq!(
        refused["facts"]["reason"], "ownership_mismatch",
        "{refused}"
    );
    assert_eq!(refused["facts"]["owner_workspace"], "Workspace B");
    assert_eq!(
        refused["summary"],
        "That tab handle belongs to Workspace B."
    );
    assert_eq!(browser.calls().len(), dispatched);
}
