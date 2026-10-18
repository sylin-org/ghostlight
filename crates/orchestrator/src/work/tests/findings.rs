//! What a frame learns lives with the frame (ADR-0202 Decision 9 to 12): a flow and each of its
//! steps record only their own checks, coverage and browser, and two calls in one workspace never
//! share what they learned.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Instant;

use super::documents::{inventory, open, policy_file};
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
    let (executor, browser, _, workspace, audit) = fixture();
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
