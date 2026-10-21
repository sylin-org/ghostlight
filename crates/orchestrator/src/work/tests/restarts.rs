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
    workspaces.apply_browser_disconnect(FAKE_BROWSER, None);
    browser.push(Ok(BrowserOutcome::Tabs { tabs: live }));
}

/// Whether any command after `before` reached the agent's old tab id by anything but a listing.
fn touched_old_id(browser: &FakeBrowser, before: usize) -> bool {
    browser.calls()[before..].iter().any(|call| {
        !matches!(call, BrowserCommand::ListTabs)
            && format!("{call:?}").contains(&format!("tab_id: {AGENT_TAB}"))
    })
}

/// The extension life the agent's tab was opened in.
const LIFE: &str = "life_first";

/// The life of the browser after it restarted, or after the extension reloaded.
const NEXT_LIFE: &str = "life_next";

/// The address a single-page app routed the agent's tab to, with no navigation the service saw.
const ROUTED_URL: &str = "https://example.com/agent/inbox";

/// The browser's connection, which named `ended` in its hello, ends, and a connection that names
/// `now` comes back listing `live`.
fn reconnect_listing(
    workspaces: &crate::workspace::WorkspaceStore,
    browser: &FakeBrowser,
    ended: Option<&str>,
    now: Option<&str>,
    live: Vec<PhysicalTab>,
) {
    workspaces.apply_browser_disconnect(FAKE_BROWSER, ended);
    browser.set_life(FAKE_BROWSER, now);
    browser.push(Ok(BrowserOutcome::Tabs { tabs: live }));
}

/// Read the agent's tab at `url` through `handle`.
fn read_routed(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
    handle: &str,
    url: &str,
) -> crate::work::result::InvocationResult {
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: AGENT_TAB,
        text: "Inbox".into(),
        truncated: false,
        title: "Inbox".into(),
        url: url.into(),
    }));
    executor.execute(
        workspace,
        "browser_read",
        json!({"tab":handle}),
        &CancellationToken::default(),
    )
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
        None,
    );
    (executor, browser, workspaces, workspace)
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

/// A browser port over the fake whose browser can go away, as a restarted browser or a
/// reconnecting native host does, and comes back the first time a call looks for it, so a call
/// that needs it has to wait. Armed, it goes away right after the next crossing.
struct Restarting {
    browser: Arc<FakeBrowser>,
    workspaces: WorkspaceStore,
    armed: Mutex<bool>,
    away: Mutex<bool>,
    /// How many commands the fake had recorded when it went away mid-call, once it has.
    restarted_at: Mutex<Option<usize>>,
}

impl Restarting {
    /// The browser's stream ends now; `gap` says whether the service saw it end.
    fn go_away(&self, gap: bool) {
        self.browser.connect(Vec::new());
        if gap {
            self.workspaces.apply_browser_disconnect(FAKE_BROWSER, None);
        }
        *self.away.lock().unwrap() = true;
    }

    fn restarted_at(&self) -> usize {
        self.restarted_at
            .lock()
            .unwrap()
            .expect("the browser restarted")
    }
}

impl BrowserPort for Restarting {
    fn call(
        &self,
        browser: &str,
        command: BrowserCommand,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.browser.call(browser, command, deadline, cancelled)
    }

    fn call_guarded(
        &self,
        browser: &str,
        command: BrowserCommand,
        dispatch: crate::browser::BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        let outcome = self.browser.call_guarded(browser, command, dispatch);
        let mut armed = self.armed.lock().unwrap();
        if *armed {
            *armed = false;
            *self.restarted_at.lock().unwrap() = Some(self.browser.calls().len());
            self.go_away(true);
        }
        outcome
    }

    /// Absent once after it went away, then back.
    fn browsers(&self) -> Vec<crate::browser::BrowserSummary> {
        let mut away = self.away.lock().unwrap();
        if *away {
            *away = false;
            self.browser.connect(vec![summary(FAKE_BROWSER, true)]);
            return Vec::new();
        }
        drop(away);
        self.browser.browsers()
    }

    fn publish_control_state(
        &self,
        state: crate::browser::adapter::RuntimeControlState,
    ) -> Result<(), BrowserError> {
        self.browser.publish_control_state(state)
    }
}

/// An executor whose browser is the fake behind [`Restarting`].
fn restartable() -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    Arc<Restarting>,
    WorkspaceStore,
    crate::workspace::WorkspaceId,
) {
    let browser = Arc::new(FakeBrowser::default());
    let workspaces = WorkspaceStore::default();
    let port = Arc::new(Restarting {
        browser: Arc::clone(&browser),
        workspaces: workspaces.clone(),
        armed: Mutex::new(false),
        away: Mutex::new(false),
        restarted_at: Mutex::new(None),
    });
    let (executor, workspace, _) =
        executor_on(GovernanceFacade::new(None), &workspaces, port.clone());
    (executor, browser, port, workspaces, workspace)
}

/// What a call on a handle the gap forgot is told.
fn forgotten() -> String {
    crate::language::outcome::WorkspaceReason::TabUnavailable.summary()
}

/// Read through `handle` in `workspace`.
fn read(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
    handle: &str,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_read",
        json!({"tab":handle}),
        &CancellationToken::default(),
    )
}

/// ACCEPTANCE Contract 19 and ADR-0203 Decision 1 (1d10bf74, 8a16343f, c4e382aa): a tab binding
/// kept across a gap in what the service knows of its browser is confirmed before any command
/// reaches it.
///
/// - A restarted browser that gives the person's tab the agent's old id: list, read, screenshot
///   and navigate all ask the browser first, never touch the old id or show the person's page;
///   the reads are told the handle names no tab, a list shows none, and a navigation reopens the
///   page in a fresh tab under the same handle.
/// - The same id at the same address is kept; within one extension life a page that routed
///   itself is kept and the binding takes its live address; a new life, or a hello or token with
///   no life, falls back to the address check.
/// - A resumed session's tab is confirmed by id and address before its first use and binds the
///   session's browser; a token from before a restart restores nothing; a tab whose browser never
///   returns is forgotten when named, left pending when not; opening in another browser forgets
///   the resumed tabs and says so.
/// - A gap inside a call: the next crossing asks first, never sends the old id, and keeps a tab
///   whose page is unchanged.
/// - A call that names no tab waits for its only tab's browser, resumed or not, and works in the
///   session's own tab.
#[test]
fn a_tab_binding_across_a_gap_is_confirmed_before_any_command_reaches_it() {
    // A restart that hands the agent's old id to the person's tab.
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
            "read" => read(&executor, &workspace, &handle),
            "screenshot" => executor.execute(
                &workspace,
                "browser_screenshot",
                json!({"tab":handle}),
                &CancellationToken::default(),
            ),
            _ => {
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
        assert!(!result.model_text().contains(PERSONS_URL), "{tool}");
        match tool {
            "list" => assert_eq!(result.facts["tabs"], json!([])),
            "navigate" => {
                assert_eq!(result.facts["tab"], json!(handle));
                assert_eq!(result.facts["recovered"], "new_tab");
            }
            _ => assert_eq!(result.summary, forgotten(), "{tool}"),
        }
    }

    // The same id at the same address is kept.
    let (executor, browser, workspaces, workspace, _) = fixture();
    let handle = open_agent_tab(&executor, &browser, &workspace);
    restart_listing(&workspaces, &browser, vec![tab(AGENT_TAB, AGENT_URL)]);
    browser.push(Ok(agent_text()));
    let kept = read(&executor, &workspace, &handle);
    assert_eq!(kept.status, Status::Succeeded, "{}", kept.summary);
    assert_eq!(kept.facts["tab"], json!(handle));
    let bindings = workspaces.tab_bindings(workspace.as_str());
    assert_eq!(bindings.len(), 1);
    assert!(!bindings[0].pending);

    // Lives: one life keeps a routed page; a new or unnamed life checks the address.
    // (the life the ended hello named, the life the new hello names, the address it lists)
    for (ended, now, listed, kept) in [
        (Some(LIFE), Some(LIFE), ROUTED_URL, true),
        (Some(LIFE), Some(NEXT_LIFE), ROUTED_URL, false),
        (Some(LIFE), Some(NEXT_LIFE), AGENT_URL, true),
        (None, Some(LIFE), ROUTED_URL, false),
        (Some(LIFE), None, ROUTED_URL, false),
        (None, None, ROUTED_URL, false),
    ] {
        let label = format!("{ended:?} then {now:?} at {listed}");
        let (executor, browser, workspaces, workspace, _) = fixture();
        browser.set_life(FAKE_BROWSER, ended);
        let handle = open_agent_tab(&executor, &browser, &workspace);
        reconnect_listing(
            &workspaces,
            &browser,
            ended,
            now,
            vec![tab(AGENT_TAB, listed)],
        );
        let before = browser.calls().len();
        let result = if kept {
            read_routed(&executor, &browser, &workspace, &handle, listed)
        } else {
            read(&executor, &workspace, &handle)
        };
        let bindings = workspaces.tab_bindings(workspace.as_str());
        if kept {
            assert_eq!(
                result.status,
                Status::Succeeded,
                "{label}: {}",
                result.summary
            );
            assert_eq!(result.facts["tab"], json!(handle), "{label}");
            assert!(!bindings[0].pending, "{label}");
            assert_eq!(
                bindings[0].url,
                crate::workspace::UrlDigest::of(listed),
                "{label}: the binding takes the live address"
            );
        } else {
            assert_eq!(result.summary, forgotten(), "{label}");
            assert!(
                !touched_old_id(&browser, before),
                "{label}: {:?}",
                browser.calls()
            );
            assert!(bindings.is_empty(), "{label}");
        }
    }

    // A resumed tab is confirmed by id and address first, and binds the session's browser.
    let (executor, browser, workspaces, workspace) = resumed_session(AGENT_URL);
    assert_eq!(workspaces.browser_of(workspace.as_str()), None);
    agent_documents(&browser);
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(3, PERSONS_URL), tab(AGENT_TAB, AGENT_URL)],
    }));
    browser.push(Ok(agent_text()));
    let resumed = read(&executor, &workspace, "tab_kept");
    assert_eq!(resumed.status, Status::Succeeded, "{}", resumed.summary);
    assert!(matches!(browser.calls()[0], BrowserCommand::ListTabs));
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some(FAKE_BROWSER)
    );

    // A token whose tab moved, or that names no life, restores nothing.
    for life in [None, Some(LIFE)] {
        let (executor, browser, workspaces, workspace) = resumed_session(AGENT_URL);
        browser.set_life(FAKE_BROWSER, life);
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![tab(
                AGENT_TAB,
                if life.is_some() {
                    ROUTED_URL
                } else {
                    PERSONS_URL
                },
            )],
        }));
        let result = read(&executor, &workspace, "tab_kept");
        assert_eq!(result.summary, forgotten(), "{life:?}");
        assert!(!touched_old_id(&browser, 0), "{:?}", browser.calls());
        assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());
        assert_eq!(workspaces.browser_of(workspace.as_str()), None);
    }

    // A browser that never returns: a call naming its tab forgets it, one that does not leaves it.
    let (executor, browser, workspaces, workspace) = resumed_session(AGENT_URL);
    browser.connect(Vec::new());
    let listed = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        &CancellationToken::default(),
    );
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert!(workspaces.tab_bindings(workspace.as_str())[0].pending);
    let named = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":"tab_kept","timeout_ms":1000}),
        &CancellationToken::default(),
    );
    assert_eq!(named.summary, forgotten());
    assert!(browser.calls().is_empty(), "{:?}", browser.calls());
    assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());

    // Opening in another browser forgets the resumed tabs and says so.
    let (executor, browser, workspaces, workspace, _) = fixture();
    workspaces.remember_tab(
        &workspace,
        "tab_away",
        "browser_away",
        AGENT_TAB,
        crate::workspace::UrlDigest::of(AGENT_URL),
        None,
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

    // A gap inside a call: the next crossing asks first.
    for listed in [PERSONS_URL, AGENT_URL] {
        let (executor, browser, port, workspaces, workspace) = restartable();
        let handle = open_agent_tab(&executor, &browser, &workspace);
        *port.armed.lock().unwrap() = true;
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![tab(AGENT_TAB, listed)],
        }));
        if listed == AGENT_URL {
            browser.push(Ok(agent_text()));
        }
        let result = read(&executor, &workspace, &handle);
        let restarted = port.restarted_at();
        assert!(
            !*port.away.lock().unwrap(),
            "{listed}: the call met the browser away"
        );
        assert!(
            matches!(
                browser.calls().get(restarted),
                Some(BrowserCommand::ListTabs)
            ),
            "{listed}: {:?}",
            browser.calls()
        );
        let bindings = workspaces.tab_bindings(workspace.as_str());
        if listed == AGENT_URL {
            assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
            assert!(!bindings[0].pending);
        } else {
            assert!(
                !touched_old_id(&browser, restarted),
                "{:?}",
                browser.calls()
            );
            assert_eq!(result.summary, forgotten());
            assert!(!result.model_text().contains(PERSONS_URL));
            assert!(bindings.is_empty());
        }
    }

    // A call that names no tab waits for its only tab's browser.
    for tool in ["read", "navigate", "resumed"] {
        let (executor, browser, port, workspaces, workspace) = restartable();
        let handle = if tool == "resumed" {
            workspaces.remember_tab(
                &workspace,
                "tab_kept",
                FAKE_BROWSER,
                AGENT_TAB,
                crate::workspace::UrlDigest::of(AGENT_URL),
                None,
            );
            agent_documents(&browser);
            port.go_away(false);
            "tab_kept".to_owned()
        } else {
            let handle = open_agent_tab(&executor, &browser, &workspace);
            port.go_away(true);
            handle
        };
        let before = browser.calls().len();
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![tab(AGENT_TAB, AGENT_URL)],
        }));
        let result = if tool == "navigate" {
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
        } else {
            browser.push(Ok(agent_text()));
            executor.execute(
                &workspace,
                "browser_read",
                json!({}),
                &CancellationToken::default(),
            )
        };
        assert_eq!(
            result.status,
            Status::Succeeded,
            "{tool}: {}",
            result.summary
        );
        assert!(
            !*port.away.lock().unwrap(),
            "{tool}: the call met the browser away"
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
