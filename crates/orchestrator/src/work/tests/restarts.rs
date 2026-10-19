//! A browser that restarts hands its tab ids out again. A binding kept across a gap in what the
//! service knows of its browser is pending until that browser lists a tab with its id at the URL
//! the workspace last knew, and the call that needs it asks first (ADR-0203 Decision 1).
//!
//! The fake browser here restarts and gives the person's own tab the agent's old id.

use super::*;

/// The page the agent's tab showed before the gap.
const AGENT_URL: &str = "https://example.com/agent";

/// The person's page that the restarted browser shows under the agent's old id.
const PERSONS_URL: &str = "https://bank.example/the-persons-own";

/// The agent's tab id before the browser restarted.
const AGENT_TAB: u64 = 7;

/// Open the agent's tab at [`AGENT_URL`] and return its handle.
fn open_agent_tab(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
) -> String {
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(AGENT_TAB, AGENT_URL),
        committed_urls: vec![AGENT_URL.into()],
    }));
    let opened = executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":AGENT_URL,"new_tab":true}),
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    opened.facts["tab"].as_str().unwrap().to_owned()
}

/// The browser's stream ends and it comes back listing `live`.
fn restart_listing(
    workspaces: &crate::workspace::WorkspaceStore,
    browser: &FakeBrowser,
    live: Vec<PhysicalTab>,
) {
    workspaces.apply_browser_disconnect(FAKE_BROWSER);
    browser.push(Ok(BrowserOutcome::Tabs { tabs: live }));
}

/// Whether any command after `before` reached the agent's old tab id by anything but a listing.
fn touched_old_id(browser: &FakeBrowser, before: usize) -> bool {
    browser.calls()[before..].iter().any(|call| {
        !matches!(call, BrowserCommand::ListTabs)
            && format!("{call:?}").contains(&format!("tab_id: {AGENT_TAB}"))
    })
}

#[test]
fn a_browser_restart_never_hands_an_agent_handle_the_persons_tab() {
    for tool in ["list", "read", "screenshot", "navigate"] {
        let (executor, browser, workspaces, workspace, _) = fixture();
        let handle = open_agent_tab(&executor, &browser, &workspace);
        restart_listing(&workspaces, &browser, vec![tab(AGENT_TAB, PERSONS_URL)]);
        let before = browser.calls().len();
        let result = match tool {
            "list" => executor.execute(
                &workspace,
                "browser_tabs",
                json!({"action":"list"}),
                &CancellationToken::default(),
            ),
            "read" => executor.execute(
                &workspace,
                "browser_read",
                json!({"tab":handle}),
                &CancellationToken::default(),
            ),
            "screenshot" => executor.execute(
                &workspace,
                "browser_screenshot",
                json!({"tab":handle}),
                &CancellationToken::default(),
            ),
            _ => {
                // A navigation of the forgotten handle reopens the page in a fresh tab.
                browser.push(Ok(BrowserOutcome::TabOpened {
                    tab: tab(9, AGENT_URL),
                    committed_urls: vec![AGENT_URL.into()],
                }));
                executor.execute(
                    &workspace,
                    "browser_navigate",
                    json!({"tab":handle,"url":AGENT_URL}),
                    &CancellationToken::default(),
                )
            }
        };
        assert!(
            matches!(browser.calls()[before], BrowserCommand::ListTabs),
            "{tool}: the call asks the browser first: {:?}",
            browser.calls()
        );
        assert!(
            !touched_old_id(&browser, before),
            "{tool}: {:?}",
            browser.calls()
        );
        assert!(
            !result.model_text().contains(PERSONS_URL),
            "{tool}: {}",
            result.model_text()
        );
        match tool {
            "list" => {
                assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
                assert_eq!(result.facts["tabs"], json!([]));
            }
            "navigate" => {
                assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
                assert_eq!(result.facts["tab"], json!(handle));
                assert_eq!(result.facts["recovered"], "new_tab");
            }
            _ => {
                assert_ne!(result.status, Status::Succeeded, "{tool}");
                assert_eq!(result.summary, "That tab is no longer open.", "{tool}");
            }
        }
    }
}

#[test]
fn a_port_restart_keeps_a_tab_whose_page_is_unchanged() {
    let (executor, browser, workspaces, workspace, _) = fixture();
    let handle = open_agent_tab(&executor, &browser, &workspace);
    restart_listing(&workspaces, &browser, vec![tab(AGENT_TAB, AGENT_URL)]);
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: AGENT_TAB,
        text: "Agent page".into(),
        truncated: false,
        title: "Example".into(),
        url: AGENT_URL.into(),
    }));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":handle}),
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Succeeded, "{}", read.summary);
    assert_eq!(read.facts["tab"], json!(handle));
    let bindings = workspaces.tab_bindings(workspace.as_str());
    assert_eq!(bindings.len(), 1);
    assert!(!bindings[0].pending);
}

/// Give the agent's tab its page's document, as a browser that opened it in this run knows it.
fn agent_documents(browser: &FakeBrowser) {
    browser.set_documents(
        AGENT_TAB,
        crate::browser::adapter::documents::DocumentInventory {
            documents: vec![crate::browser::adapter::documents::PhysicalDocument {
                id: "top".into(),
                url: AGENT_URL.into(),
                parent: None,
                supported: true,
            }],
            ..Default::default()
        },
    );
}

/// A resumed session's tab, brought back from a token remembered at `url`.
fn resumed_session(
    url: &str,
) -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    WorkspaceStore,
    crate::workspace::WorkspaceId,
) {
    let (executor, browser, workspaces, workspace, _) = fixture();
    workspaces.remember_tab(
        &workspace,
        "tab_kept",
        FAKE_BROWSER,
        AGENT_TAB,
        crate::workspace::UrlDigest::of(url),
    );
    (executor, browser, workspaces, workspace)
}

#[test]
fn a_resumed_tab_is_confirmed_by_id_and_url_before_its_first_use() {
    let (executor, browser, workspaces, workspace) = resumed_session(AGENT_URL);
    assert_eq!(workspaces.browser_of(workspace.as_str()), None);
    agent_documents(&browser);
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(3, PERSONS_URL), tab(AGENT_TAB, AGENT_URL)],
    }));
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: AGENT_TAB,
        text: "Agent page".into(),
        truncated: false,
        title: "Example".into(),
        url: AGENT_URL.into(),
    }));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":"tab_kept"}),
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Succeeded, "{}", read.summary);
    assert!(matches!(browser.calls()[0], BrowserCommand::ListTabs));
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some(FAKE_BROWSER),
        "the confirmed tab binds the session's browser"
    );
}

#[test]
fn a_token_from_before_a_browser_restart_restores_nothing() {
    let (executor, browser, workspaces, workspace) = resumed_session(AGENT_URL);
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(AGENT_TAB, PERSONS_URL)],
    }));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":"tab_kept"}),
        &CancellationToken::default(),
    );
    assert_eq!(read.summary, "That tab is no longer open.");
    assert!(!touched_old_id(&browser, 0), "{:?}", browser.calls());
    assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());
    assert_eq!(workspaces.browser_of(workspace.as_str()), None);
}

#[test]
fn a_call_on_a_tab_whose_browser_never_returns_forgets_it() {
    let (executor, browser, workspaces, workspace) = resumed_session(AGENT_URL);
    browser.connect(Vec::new());
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":"tab_kept","timeout_ms":1000}),
        &CancellationToken::default(),
    );
    assert_eq!(read.summary, "That tab is no longer open.");
    assert!(browser.calls().is_empty(), "{:?}", browser.calls());
    assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());
}

#[test]
fn a_call_that_names_no_pending_tab_leaves_it_pending_while_its_browser_is_away() {
    let (executor, browser, workspaces, workspace) = resumed_session(AGENT_URL);
    browser.connect(Vec::new());
    let listed = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        &CancellationToken::default(),
    );
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert!(browser.calls().is_empty(), "{:?}", browser.calls());
    let bindings = workspaces.tab_bindings(workspace.as_str());
    assert_eq!(bindings.len(), 1);
    assert!(bindings[0].pending);
}

#[test]
fn opening_in_another_browser_forgets_the_resumed_tabs_and_says_so() {
    let (executor, browser, workspaces, workspace, _) = fixture();
    workspaces.remember_tab(
        &workspace,
        "tab_away",
        "browser_away",
        AGENT_TAB,
        crate::workspace::UrlDigest::of(AGENT_URL),
    );
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(3, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/","new_tab":true}),
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(opened.facts["forgotten_tabs"], json!(["tab_away"]));
    let bindings = workspaces.tab_bindings(workspace.as_str());
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].browser, FAKE_BROWSER);
}

/// How long the restarted browser takes to connect again, well inside a call's return grace.
const RECONNECT_AFTER: Duration = Duration::from_millis(150);

/// Connect the fake browser again after [`RECONNECT_AFTER`], as a restarted browser or a
/// reconnecting native host does.
fn reconnect_soon(browser: &Arc<FakeBrowser>) {
    let returning = Arc::clone(browser);
    std::thread::spawn(move || {
        std::thread::sleep(RECONNECT_AFTER);
        returning.connect(vec![summary(FAKE_BROWSER, true)]);
    });
}

/// End the fake browser's stream now and connect it again soon. What it lists when it is back
/// is queued by the test.
fn drop_and_reconnect(workspaces: &WorkspaceStore, browser: &Arc<FakeBrowser>) {
    browser.connect(Vec::new());
    workspaces.apply_browser_disconnect(FAKE_BROWSER);
    reconnect_soon(browser);
}

/// A browser port whose browser restarts after the first crossing it carries once armed: the
/// stream ends there, and the browser connects again shortly after.
struct RestartsAfterOneCrossing {
    browser: Arc<FakeBrowser>,
    workspaces: WorkspaceStore,
    armed: Mutex<bool>,
    /// How many commands the fake had recorded when it restarted, once it has.
    restarted_at: Mutex<Option<usize>>,
}

impl RestartsAfterOneCrossing {
    fn after_crossing(&self) {
        let mut armed = self.armed.lock().unwrap();
        if *armed {
            *armed = false;
            *self.restarted_at.lock().unwrap() = Some(self.browser.calls().len());
            drop_and_reconnect(&self.workspaces, &self.browser);
        }
    }

    fn restarted_at(&self) -> usize {
        self.restarted_at
            .lock()
            .unwrap()
            .expect("the browser restarted")
    }
}

impl BrowserPort for RestartsAfterOneCrossing {
    fn call(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.browser
            .call(browser, workspace, command, deadline, cancelled)
    }

    fn call_guarded(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        dispatch: crate::browser::BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        let outcome = self
            .browser
            .call_guarded(browser, workspace, command, dispatch);
        self.after_crossing();
        outcome
    }

    fn browsers(&self) -> Vec<crate::browser::BrowserSummary> {
        self.browser.browsers()
    }

    fn publish_control_state(
        &self,
        state: crate::browser::adapter::RuntimeControlState,
    ) -> Result<(), BrowserError> {
        self.browser.publish_control_state(state)
    }
}

/// The agent's tab open at [`AGENT_URL`], in an executor whose browser restarts after the next
/// call's first crossing. Returns the executor, the fake, the port, the store, the workspace and
/// the tab's handle.
fn restarting_mid_call() -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    Arc<RestartsAfterOneCrossing>,
    WorkspaceStore,
    crate::workspace::WorkspaceId,
    String,
) {
    let browser = Arc::new(FakeBrowser::default());
    let workspaces = WorkspaceStore::default();
    let port = Arc::new(RestartsAfterOneCrossing {
        browser: Arc::clone(&browser),
        workspaces: workspaces.clone(),
        armed: Mutex::new(false),
        restarted_at: Mutex::new(None),
    });
    let (executor, workspace, _) =
        executor_on(GovernanceFacade::new(None, None), &workspaces, port.clone());
    let handle = open_agent_tab(&executor, &browser, &workspace);
    *port.armed.lock().unwrap() = true;
    (executor, browser, port, workspaces, workspace, handle)
}

/// The text the agent's page reads as.
fn agent_text() -> BrowserOutcome {
    BrowserOutcome::Text {
        tab_id: AGENT_TAB,
        text: "Agent page".into(),
        truncated: false,
        title: "Example".into(),
        url: AGENT_URL.into(),
    }
}

#[test]
fn a_gap_inside_a_call_never_sends_the_old_id_to_the_restarted_browser() {
    let (executor, browser, port, workspaces, workspace, handle) = restarting_mid_call();
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(AGENT_TAB, PERSONS_URL)],
    }));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":handle}),
        &CancellationToken::default(),
    );
    let restarted = port.restarted_at();
    assert!(
        matches!(
            browser.calls().get(restarted),
            Some(BrowserCommand::ListTabs)
        ),
        "the crossing after the gap asks the browser first: {:?}",
        browser.calls()
    );
    assert!(
        !touched_old_id(&browser, restarted),
        "{:?}",
        browser.calls()
    );
    assert_eq!(read.summary, "That tab is no longer open.");
    assert!(
        !read.model_text().contains(PERSONS_URL),
        "{}",
        read.model_text()
    );
    assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());
}

#[test]
fn a_gap_inside_a_call_keeps_a_tab_whose_page_is_unchanged() {
    let (executor, browser, port, workspaces, workspace, handle) = restarting_mid_call();
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(AGENT_TAB, AGENT_URL)],
    }));
    browser.push(Ok(agent_text()));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":handle}),
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Succeeded, "{}", read.summary);
    assert!(matches!(
        browser.calls().get(port.restarted_at()),
        Some(BrowserCommand::ListTabs)
    ));
    let bindings = workspaces.tab_bindings(workspace.as_str());
    assert_eq!(bindings.len(), 1);
    assert!(!bindings[0].pending);
}

#[test]
fn a_call_that_names_no_tab_waits_for_its_only_tabs_browser() {
    for tool in ["read", "navigate"] {
        let (executor, browser, workspaces, workspace, _) = fixture();
        let handle = open_agent_tab(&executor, &browser, &workspace);
        drop_and_reconnect(&workspaces, &browser);
        let before = browser.calls().len();
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![tab(AGENT_TAB, AGENT_URL)],
        }));
        let result = if tool == "read" {
            browser.push(Ok(agent_text()));
            executor.execute(
                &workspace,
                "browser_read",
                json!({}),
                &CancellationToken::default(),
            )
        } else {
            browser.push(Ok(BrowserOutcome::Navigated {
                tab: tab(AGENT_TAB, "https://example.com/next"),
                committed_urls: vec!["https://example.com/next".into()],
            }));
            executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":"https://example.com/next"}),
                &CancellationToken::default(),
            )
        };
        assert_eq!(
            result.status,
            Status::Succeeded,
            "{tool}: {}",
            result.summary
        );
        assert_eq!(result.facts["tab"], json!(handle), "{tool}");
        let calls = browser.calls()[before..].to_vec();
        assert!(
            matches!(calls.first(), Some(BrowserCommand::ListTabs)),
            "{tool}: {calls:?}"
        );
        assert!(
            !calls
                .iter()
                .any(|call| matches!(call, BrowserCommand::OpenTab { .. })),
            "{tool}: the call works in the session's own tab: {calls:?}"
        );
    }
}

#[test]
fn a_resumed_session_that_names_no_tab_waits_for_its_tabs_browser() {
    let (executor, browser, _, workspace) = resumed_session(AGENT_URL);
    agent_documents(&browser);
    browser.connect(Vec::new());
    reconnect_soon(&browser);
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(AGENT_TAB, AGENT_URL)],
    }));
    browser.push(Ok(agent_text()));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({}),
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Succeeded, "{}", read.summary);
    assert_eq!(read.facts["tab"], "tab_kept");
}
