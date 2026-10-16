//! Tab reuse, tab ownership across browsers, and handle lifetime across same-document changes.
//!
//! Default reuse is the service's choice (ADR-0196 Decision 9): only a tab Ghostlight opened and
//! a released session of the same client left open is ever reused, and the adapter is always told
//! not to choose.

use super::*;

/// Open one fresh controlled tab and return its handle.
fn open_at(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
    tab_id: u64,
    url: &str,
) -> String {
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(tab_id, url),
        committed_urls: vec![url.into()],
    }));
    let opened = executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":url,"new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    opened.facts["tab"].as_str().unwrap().to_owned()
}

/// Release a workspace and answer each release close as the person's preserve-tabs setting
/// would, keeping the tab open. Returns the released physical ids.
fn release_kept_open(
    workspaces: &crate::workspace::WorkspaceStore,
    workspace: &crate::workspace::WorkspaceId,
) -> Vec<u64> {
    let released = workspaces.release(workspace);
    for &physical_id in &released.physical_ids {
        workspaces.settle_released(FAKE_BROWSER, physical_id, true);
    }
    released.physical_ids
}

/// Admit a later session of the client that `earlier` belongs to, the way a restarted agent or
/// the next run of a script returns. Call it before `earlier` is released.
fn later_session_of(
    workspaces: &crate::workspace::WorkspaceStore,
    earlier: &crate::workspace::WorkspaceId,
) -> crate::workspace::WorkspaceId {
    let client = workspaces.client_label(earlier).unwrap();
    workspaces.admit(client, IntakeChannel::Mcp)
}

fn navigate(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
    url: &str,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":url}),
        None,
        &CancellationToken::default(),
    )
}

#[test]
fn a_released_ghostlight_tab_is_reused_by_the_next_session_on_its_host() {
    let (executor, browser, workspaces, first, _) = fixture();
    open_at(&executor, &browser, &first, 7, "https://example.com/a");
    let next = later_session_of(&workspaces, &first);
    // The first caller goes away and the person's preserve-tabs choice keeps its tab open.
    assert_eq!(release_kept_open(&workspaces, &first), vec![7]);

    let before = browser.calls().len();
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![
            tab(7, "https://example.com/a"),
            tab(9, "https://example.com/the-persons-own"),
        ],
    }));
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.com/b"),
        committed_urls: vec!["https://example.com/b".into()],
    }));
    let reused = navigate(&executor, &next, "https://example.com/b");

    assert_eq!(reused.status, Status::Succeeded, "{}", reused.summary);
    assert_eq!(reused.summary, "Reused the example.com tab.");
    assert_eq!(reused.facts["reused"], true);
    assert_eq!(reused.facts["created"], false);
    assert_eq!(reused.facts["url"], "https://example.com/b");
    let calls = browser.calls()[before..].to_vec();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(matches!(calls[0], BrowserCommand::ListTabs));
    assert!(
        matches!(&calls[1], BrowserCommand::Navigate { tab_id: 7, url } if url == "https://example.com/b"),
        "the service names the exact tab it chose: {calls:?}"
    );

    // The tab now belongs to the new session, and nobody can reuse it a second time.
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(7, "https://example.com/b")],
    }));
    let listed = executor.execute(
        &next,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(listed.facts["tabs"][0]["tab"], reused.facts["tab"]);
    let client = workspaces.client_label(&next).unwrap();
    assert_eq!(
        workspaces.claim_released(FAKE_BROWSER, &client, "https://example.com/b"),
        None
    );
}

#[test]
fn a_released_tab_whose_cleanup_has_not_answered_is_not_reused() {
    let (executor, browser, workspaces, first, _) = fixture();
    open_at(&executor, &browser, &first, 7, "https://example.com/a");
    let next = later_session_of(&workspaces, &first);
    // The first caller goes away, but its release close has not answered yet. The browser may
    // still bind tab 7 to it, and that close could still act on the tab under a new owner.
    workspaces.release(&first);

    let before = browser.calls().len();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(12, "https://example.com/b"),
        committed_urls: vec!["https://example.com/b".into()],
    }));
    let opened = navigate(&executor, &next, "https://example.com/b");

    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(opened.facts["created"], true);
    let calls = browser.calls()[before..].to_vec();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        matches!(&calls[0], BrowserCommand::OpenTab { .. }),
        "{calls:?}"
    );
}

#[test]
fn a_tab_the_person_opened_on_the_same_host_is_never_reused() {
    let (executor, browser, workspaces, first, _) = fixture();
    // A released tab exists, but on another host, so this open has nothing of its own to reuse.
    open_at(&executor, &browser, &first, 3, "https://other.test/");
    let next = later_session_of(&workspaces, &first);
    release_kept_open(&workspaces, &first);

    let before = browser.calls().len();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(11, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = navigate(&executor, &next, "https://example.com/");

    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(opened.summary, "Opened example.com.");
    assert_eq!(opened.facts["created"], true);
    assert_eq!(opened.facts["reused"], false);
    // The person's own example.com tab is never looked for: the browser is only asked to open a
    // fresh tab.
    let calls = browser.calls()[before..].to_vec();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        matches!(&calls[0], BrowserCommand::OpenTab { .. }),
        "{calls:?}"
    );
}

#[test]
fn a_released_tab_the_person_took_back_or_closed_is_not_reused() {
    let (executor, browser, workspaces, first, _) = fixture();
    open_at(&executor, &browser, &first, 7, "https://example.com/a");
    open_at(&executor, &browser, &first, 8, "https://example.com/c");
    open_at(&executor, &browser, &first, 10, "https://example.com/d");
    let next = later_session_of(&workspaces, &first);
    release_kept_open(&workspaces, &first);
    // The person loaded another page in tab 7 after the release.
    workspaces.note_browser_landing(FAKE_BROWSER, 7, "https://example.com/elsewhere");

    let before = browser.calls().len();
    // The live browser shows tab 8 moved within its page, and tab 10 is gone.
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![
            tab(7, "https://example.com/elsewhere"),
            tab(8, "https://example.com/c#moved"),
        ],
    }));
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(12, "https://example.com/b"),
        committed_urls: vec!["https://example.com/b".into()],
    }));
    let opened = navigate(&executor, &next, "https://example.com/b");

    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(opened.facts["created"], true);
    let calls = browser.calls()[before..].to_vec();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(matches!(calls[0], BrowserCommand::ListTabs));
    assert!(
        matches!(&calls[1], BrowserCommand::OpenTab { .. }),
        "{calls:?}"
    );
    assert!(!calls
        .iter()
        .any(|call| matches!(call, BrowserCommand::Navigate { .. })));
    // Candidates that failed the live check are gone, so the next open does not ask again.
    let client = workspaces.client_label(&next).unwrap();
    assert_eq!(
        workspaces.claim_released(FAKE_BROWSER, &client, "https://example.com/b"),
        None
    );
}

#[test]
fn a_tab_another_client_released_is_not_reused_and_its_own_client_still_reuses_it() {
    let (executor, browser, workspaces, _, _) = fixture();
    // The sample scripts open example.com and exit. Preserve-tabs keeps their tab open, and it
    // stays in the "Ghostlight - ghostlight call" group.
    let scripts = workspaces.admit("ghostlight call".into(), IntakeChannel::Mcp);
    open_at(&executor, &browser, &scripts, 7, "https://example.com/a");
    let scripts_again = later_session_of(&workspaces, &scripts);
    assert_eq!(release_kept_open(&workspaces, &scripts), vec![7]);

    // A Claude Code session opening the same host gets a fresh tab in its own group. Reusing
    // tab 7 would leave a tab it controls under the group title of the scripts.
    let agent = workspaces.admit("Claude Code".into(), IntakeChannel::Mcp);
    let before = browser.calls().len();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(12, "https://example.com/b"),
        committed_urls: vec!["https://example.com/b".into()],
    }));
    let opened = navigate(&executor, &agent, "https://example.com/b");

    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(opened.summary, "Opened example.com.");
    assert_eq!(opened.facts["created"], true);
    assert_eq!(opened.facts["reused"], false);
    let calls = browser.calls()[before..].to_vec();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        matches!(
            &calls[0],
            BrowserCommand::OpenTab { group_title, .. }
                if group_title == "Ghostlight - Claude Code"
        ),
        "{calls:?}"
    );

    // The next run of the scripts still reuses their tab, which already sits in their group.
    let before = browser.calls().len();
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![
            tab(7, "https://example.com/a"),
            tab(12, "https://example.com/b"),
        ],
    }));
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.com/c"),
        committed_urls: vec!["https://example.com/c".into()],
    }));
    let reused = navigate(&executor, &scripts_again, "https://example.com/c");

    assert_eq!(reused.status, Status::Succeeded, "{}", reused.summary);
    assert_eq!(reused.summary, "Reused the example.com tab.");
    assert_eq!(reused.facts["reused"], true);
    let calls = browser.calls()[before..].to_vec();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(matches!(calls[0], BrowserCommand::ListTabs));
    assert!(
        matches!(&calls[1], BrowserCommand::Navigate { tab_id: 7, url } if url == "https://example.com/c"),
        "{calls:?}"
    );
}

#[test]
fn chrome_and_edge_tabs_with_the_same_id_are_different_tabs() {
    let (executor, browser, workspaces, chrome, _) = fixture();
    browser.connect(vec![
        summary("browser_chrome", true),
        summary("browser_edge", true),
    ]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(5, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let in_chrome = executor.execute(
        &chrome,
        "browser_navigate",
        json!({"url":"https://example.com/","new_tab":true,"browser":"browser_chrome"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(in_chrome.status, Status::Succeeded, "{}", in_chrome.summary);

    let edge = workspaces.admit("edge".into(), IntakeChannel::Mcp);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(5, "https://example.org/"),
        committed_urls: vec!["https://example.org/".into()],
    }));
    let in_edge = executor.execute(
        &edge,
        "browser_navigate",
        json!({"url":"https://example.org/","new_tab":true,"browser":"browser_edge"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(
        in_edge.status,
        Status::Succeeded,
        "Edge's tab 5 is not Chrome's tab 5: {}",
        in_edge.summary
    );
    assert_eq!(browser.routed(), vec!["browser_chrome", "browser_edge"]);
    assert_ne!(in_chrome.facts["tab"], in_edge.facts["tab"]);
}

#[test]
fn a_tab_refused_after_the_browser_opened_it_reports_the_effect() {
    let (executor, browser, workspaces, owner, _) = fixture();
    open_at(&executor, &browser, &owner, 5, "https://example.com/");

    // An adapter hands back a tab another session in the same browser already controls.
    let other = workspaces.admit("other".into(), IntakeChannel::Mcp);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(5, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let refused = executor.execute(
        &other,
        "browser_navigate",
        json!({"url":"https://example.com/","new_tab":true}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(refused.status, Status::Failed, "{}", refused.summary);
    assert_eq!(refused.facts["reason"], "ownership_mismatch");
    assert_eq!(
        refused.effect,
        Effect::Applied,
        "the browser already opened the page"
    );
    assert!(!refused.repeat_safe, "repeating would open another tab");
}

#[test]
fn a_same_document_route_keeps_handles_and_a_new_origin_expires_them() {
    let (executor, browser, _, workspace, _) = fixture();
    let tab_handle = open_at(&executor, &browser, &workspace, 7, "https://example.com/");
    let save = ObservedTarget {
        locator: "btn-1".into(),
        role: "button".into(),
        name: "Save".into(),
        state: vec![],
        credential_class: false,
        ..Default::default()
    };
    browser.push(Ok(BrowserOutcome::targets(7, vec![save.clone()])));
    let inspected = executor.execute(
        &workspace,
        "browser_inspect",
        json!({"tab":tab_handle,"scope":"controls","max_items":10}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(inspected.status, Status::Succeeded, "{}", inspected.summary);
    let generation = inspected.facts["document_generation"].clone();
    let target = inspected.facts["items"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();

    // An anchor route: the address moves and no document commits.
    browser.push(Ok(BrowserOutcome::ScriptEvaluated {
        tab: tab(7, "https://example.com/#settings"),
        value: "null".into(),
        truncated: false,
        form: crate::browser::adapter::ScriptForm::AsWritten,
        committed_urls: vec![],
    }));
    let anchored = executor.execute(
        &workspace,
        "browser_execute",
        json!({"script":"location.hash = 'settings'"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(anchored.status, Status::Succeeded, "{}", anchored.summary);
    assert_eq!(
        anchored.facts["landing"]["url"],
        "https://example.com/#settings"
    );
    assert_eq!(anchored.facts["landing"]["document_generation"], generation);

    // A history.pushState route from a click, the way single-page apps move.
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/settings"),
        subject: None,
        committed_urls: vec![],
    }));
    let routed = executor.execute(
        &workspace,
        "browser_click",
        json!({"target":target}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(
        routed.status,
        Status::Succeeded,
        "a handle from before the route still works: {}",
        routed.summary
    );
    assert_eq!(routed.facts["landing"]["document_generation"], generation);

    // The same handle works again after the route, and the tab follows the new address.
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/settings"),
        subject: None,
        committed_urls: vec![],
    }));
    let again = executor.execute(
        &workspace,
        "browser_click",
        json!({"target":target}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(again.status, Status::Succeeded, "{}", again.summary);
    assert!(again.facts.get("landing").is_none());

    // An address in another origin is a new document even when no commit was reported.
    browser.push(Ok(BrowserOutcome::ScriptEvaluated {
        tab: tab(7, "https://example.org/"),
        value: "null".into(),
        truncated: false,
        form: crate::browser::adapter::ScriptForm::AsWritten,
        committed_urls: vec![],
    }));
    let moved = executor.execute(
        &workspace,
        "browser_execute",
        json!({"script":"location.assign('https://example.org/')"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(moved.status, Status::Succeeded, "{}", moved.summary);
    assert_ne!(moved.facts["landing"]["document_generation"], generation);
    let stale = executor.execute(
        &workspace,
        "browser_click",
        json!({"target":target}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(stale.status, Status::Failed, "{}", stale.summary);
    assert_eq!(stale.facts["reason"], "stale_target");
}

#[test]
fn a_committed_document_in_the_same_origin_still_expires_handles() {
    let (executor, browser, _, workspace, _) = fixture();
    let tab_handle = open_at(&executor, &browser, &workspace, 7, "https://example.com/");
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![ObservedTarget {
            locator: "link-1".into(),
            role: "link".into(),
            name: "Next".into(),
            state: vec![],
            credential_class: false,
            ..Default::default()
        }],
    )));
    let inspected = executor.execute(
        &workspace,
        "browser_inspect",
        json!({"tab":tab_handle,"scope":"controls","max_items":10}),
        None,
        &CancellationToken::default(),
    );
    let target = inspected.facts["items"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();

    // A link that loads a new document reports its commit, so the generation moves.
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/next"),
        subject: None,
        committed_urls: vec!["https://example.com/next".into()],
    }));
    let followed = executor.execute(
        &workspace,
        "browser_click",
        json!({"target":target}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(followed.status, Status::Succeeded, "{}", followed.summary);
    assert_ne!(
        followed.facts["landing"]["document_generation"],
        inspected.facts["document_generation"]
    );
    let stale = executor.execute(
        &workspace,
        "browser_click",
        json!({"target":target}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(stale.facts["reason"], "stale_target");
}
