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
