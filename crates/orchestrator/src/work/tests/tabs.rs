//! Tab reuse, tab ownership across browsers, handle lifetime across same-document changes, and
//! the two ways out of a held tab.
//!
//! Default reuse is the service's choice (ADR-0196 Decision 9): only a tab Ghostlight opened and
//! a released session of the same agent left open is ever reused, and the adapter is always told
//! not to choose. The agent is the one the opener's claim names (ADR-0203 Decision 9), and the tab
//! group is titled for the agent the window names. A held tab refuses reading and acting, and only a navigation to an allowed page
//! or a close may select it (ADR-0203 Decision 2.4). Closing a tab needs Action on its host, a
//! held tab's close and the undoing of a refused landing need only the tab-close setting, a refused
//! landing that could not be undone holds its tab, and a listing names a tab on a refused host by
//! its host alone (Decision 7.6).

use super::*;
use crate::provenance::{Attribution, ConnectionEvidence, Source};
use std::sync::Arc;

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
    let opener = workspaces.single_connection(earlier).unwrap();
    workspaces.connect(
        Arc::new(ConnectionEvidence::new(
            Attribution::accepted(opener.attribution().channel),
            opener.claim(),
        )),
        None,
    )
}

/// Admit a session on `channel` whose client claimed `claim`, started by the app of the catalog
/// agent `source`, or by a program that is no agent's app when `source` is none.
fn session_of(
    workspaces: &crate::workspace::WorkspaceStore,
    channel: IntakeChannel,
    claim: Option<&str>,
    source: Option<&str>,
) -> crate::workspace::WorkspaceId {
    let source = Source {
        agent: source.map(str::to_owned),
        image: "program.exe".into(),
    };
    workspaces.connect(
        Arc::new(ConnectionEvidence::new(
            Attribution {
                source: Some(source),
                ..Attribution::accepted(channel)
            },
            claim,
        )),
        None,
    )
}

/// The group title of the one tab-opening command among `calls`.
fn opened_group(calls: &[BrowserCommand]) -> String {
    let titles: Vec<&String> = calls
        .iter()
        .filter_map(|call| match call {
            BrowserCommand::OpenTab { group_title, .. }
            | BrowserCommand::AdoptTab { group_title, .. } => Some(group_title),
            _ => None,
        })
        .collect();
    assert_eq!(titles.len(), 1, "{calls:?}");
    titles[0].clone()
}

/// Open `url` in `workspace` as an open that may reuse a kept tab, answered as a fresh tab
/// `tab_id`, and return the commands it sent.
fn open_fresh(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
    tab_id: u64,
    url: &str,
) -> Vec<BrowserCommand> {
    let before = browser.calls().len();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(tab_id, url),
        committed_urls: vec![url.into()],
    }));
    let opened = navigate(executor, workspace, url);
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(opened.facts["created"], true, "{}", opened.summary);
    browser.calls()[before..].to_vec()
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
        &CancellationToken::default(),
    )
}

/// ACCEPTANCE Browser journey 25 and ADR-0196 Decision 9: default reuse takes only a tab Ghostlight
/// kept for the same agent, still at its address and released cleanly.
///
/// A released tab is reused by the next session of the same client on its host; not while its
/// cleanup has not answered, not a tab the person opened, took back or closed, not one another
/// client released (its own client still reuses it), not by a client that names nothing or a
/// claimless one started by another agent; command-line sessions share their own kept tabs only.
/// Chrome and Edge tabs with the same id are different tabs.
#[test]
fn default_reuse_takes_only_a_kept_tab_of_the_same_agent_still_at_its_address() {
    // a released ghostlight tab is reused by the next session on its host.
    {
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
        browser.push(Ok(BrowserOutcome::TabOpened {
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
            matches!(
                &calls[1],
                BrowserCommand::AdoptTab { tab_id: 7, url, group_title, focus_window: true }
                    if url == "https://example.com/b" && group_title == "\u{1F47B} test"
            ),
            "the service names the exact tab it chose and puts it back in the group: {calls:?}"
        );

        // The tab now belongs to the new session, and nobody can reuse it a second time.
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![tab(7, "https://example.com/b")],
        }));
        let listed = executor.execute(
            &next,
            "browser_tabs",
            json!({"action":"list"}),
            &CancellationToken::default(),
        );
        assert_eq!(listed.facts["tabs"][0]["tab"], reused.facts["tab"]);
        let key = workspaces.opener(&next).unwrap().reuse_key.unwrap();
        assert_eq!(
            workspaces.claim_released(FAKE_BROWSER, &key, "https://example.com/b"),
            None
        );
    }

    // a released tab whose cleanup has not answered is not reused.
    {
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

    // a tab the person opened on the same host is never reused.
    {
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

    // a released tab the person took back or closed is not reused.
    {
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
        let key = workspaces.opener(&next).unwrap().reuse_key.unwrap();
        assert_eq!(
            workspaces.claim_released(FAKE_BROWSER, &key, "https://example.com/b"),
            None
        );
    }

    // a tab another client released is not reused and its own client still reuses it.
    {
        let (executor, browser, workspaces, _, _) = fixture();
        // A script client opens example.com and exits. Preserve-tabs keeps its tab open, out of the
        // "ghostlight call" group (ADR-0201).
        let scripts = workspaces.admit("ghostlight call".into(), IntakeChannel::Mcp);
        open_at(&executor, &browser, &scripts, 7, "https://example.com/a");
        let scripts_again = later_session_of(&workspaces, &scripts);
        assert_eq!(release_kept_open(&workspaces, &scripts), vec![7]);

        // A Claude Code session opening the same host gets a fresh tab in its own group: a client
        // reuses only the tabs its own sessions kept.
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
                    if group_title == "\u{1F47B} Claude Code"
            ),
            "{calls:?}"
        );

        // The next run of the scripts still reuses their tab and puts it back in their group.
        let before = browser.calls().len();
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![
                tab(7, "https://example.com/a"),
                tab(12, "https://example.com/b"),
            ],
        }));
        browser.push(Ok(BrowserOutcome::TabOpened {
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
            matches!(
                &calls[1],
                BrowserCommand::AdoptTab { tab_id: 7, url, group_title, .. }
                    if url == "https://example.com/c" && group_title == "\u{1F47B} ghostlight call"
            ),
            "{calls:?}"
        );
    }

    // a client that names nothing reuses no kept tab.
    {
        let (executor, browser, workspaces, _, _) = fixture();
        let first = workspaces.admit(String::new(), IntakeChannel::Mcp);
        open_at(&executor, &browser, &first, 7, "https://example.com/a");
        let next = workspaces.admit(String::new(), IntakeChannel::Mcp);
        assert_eq!(release_kept_open(&workspaces, &first), vec![7]);

        let calls = open_fresh(&executor, &browser, &next, 12, "https://example.com/b");
        assert_eq!(calls.len(), 1, "no kept tab is even looked for: {calls:?}");
        assert!(
            matches!(calls[0], BrowserCommand::OpenTab { .. }),
            "{calls:?}"
        );

        // Nor does a named client reuse the tab a client that named nothing kept.
        let named = workspaces.admit("test".into(), IntakeChannel::Mcp);
        let calls = open_fresh(&executor, &browser, &named, 13, "https://example.com/c");
        assert_eq!(calls.len(), 1, "{calls:?}");
    }

    // a claimless client started by claude code reuses no claude code tab.
    {
        let (executor, browser, workspaces, _, _) = fixture();
        let claude_code = session_of(&workspaces, IntakeChannel::Mcp, Some("claude-code"), None);
        open_at(
            &executor,
            &browser,
            &claude_code,
            7,
            "https://example.com/a",
        );
        let next_claude_code = later_session_of(&workspaces, &claude_code);
        assert_eq!(release_kept_open(&workspaces, &claude_code), vec![7]);

        // The window names this session Claude Code from its source, and its group says so, but its
        // client claimed nothing, so it reuses nothing.
        let claimless = session_of(&workspaces, IntakeChannel::Mcp, None, Some("claude-code"));
        let calls = open_fresh(&executor, &browser, &claimless, 12, "https://example.com/b");
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(opened_group(&calls), "\u{1F47B} Claude Code");

        // The kept tab still waits for a session whose client claims Claude Code.
        let before = browser.calls().len();
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![tab(7, "https://example.com/a")],
        }));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/c"),
            committed_urls: vec!["https://example.com/c".into()],
        }));
        let reused = navigate(&executor, &next_claude_code, "https://example.com/c");
        assert_eq!(reused.facts["reused"], true, "{}", reused.summary);
        assert_eq!(
            opened_group(&browser.calls()[before..]),
            "\u{1F47B} Claude Code"
        );
    }

    // a command line session reuses a tab another command line session kept.
    {
        let (executor, browser, workspaces, _, _) = fixture();
        let script = session_of(
            &workspaces,
            IntakeChannel::Cli,
            Some("ghostlight call"),
            None,
        );
        open_at(&executor, &browser, &script, 7, "https://example.com/a");
        let another = session_of(
            &workspaces,
            IntakeChannel::Cli,
            Some("nightly report"),
            None,
        );
        assert_eq!(release_kept_open(&workspaces, &script), vec![7]);

        let before = browser.calls().len();
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![tab(7, "https://example.com/a")],
        }));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/b"),
            committed_urls: vec!["https://example.com/b".into()],
        }));
        let reused = navigate(&executor, &another, "https://example.com/b");

        assert_eq!(reused.status, Status::Succeeded, "{}", reused.summary);
        assert_eq!(reused.facts["reused"], true);
        let calls = browser.calls()[before..].to_vec();
        assert!(
            matches!(
                &calls[..],
                [BrowserCommand::ListTabs, BrowserCommand::AdoptTab { tab_id: 7, group_title, .. }]
                    if group_title == "\u{1F47B} Command line"
            ),
            "{calls:?}"
        );
    }

    // a tab a command line session kept is not reused by claude code.
    {
        let (executor, browser, workspaces, _, _) = fixture();
        let script = session_of(&workspaces, IntakeChannel::Cli, None, Some("claude-code"));
        open_at(&executor, &browser, &script, 7, "https://example.com/a");
        assert_eq!(release_kept_open(&workspaces, &script), vec![7]);

        let claude_code = session_of(
            &workspaces,
            IntakeChannel::Mcp,
            Some("claude-code"),
            Some("claude-code"),
        );
        let calls = open_fresh(
            &executor,
            &browser,
            &claude_code,
            12,
            "https://example.com/b",
        );
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(opened_group(&calls), "\u{1F47B} Claude Code");
    }

    // chrome and edge tabs with the same id are different tabs.
    {
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
}

/// ACCEPTANCE Extension 8: a tab group is titled for its opener as the window names it, with the
/// ghost mark, and an opener the catalog cannot name is titled unknown agent.
#[test]
fn a_group_is_titled_for_its_opener_as_the_window_names_it() {
    // a group is titled for the agent the window names.
    {
        let (executor, browser, workspaces, _, _) = fixture();
        for (tab_id, claim, source, agent) in [
            (20, Some("claude-code"), None, "Claude Code"),
            (21, Some(" Claude-Code "), Some("cursor"), "Claude Code"),
            (22, None, Some("claude-desktop"), "Claude Desktop"),
            (23, Some("my agent"), Some("claude-desktop"), "my agent"),
        ] {
            let session = session_of(&workspaces, IntakeChannel::Mcp, claim, source);
            let window = workspaces
                .summaries()
                .into_iter()
                .find(|summary| summary.id == session.as_str())
                .unwrap()
                .opener
                .agent
                .name;
            assert_eq!(window, agent, "{claim:?} {source:?}");
            let calls = open_fresh(
                &executor,
                &browser,
                &session,
                tab_id,
                "https://example.com/",
            );
            assert_eq!(
                opened_group(&calls),
                format!("\u{1F47B} {window}"),
                "{claim:?} {source:?}"
            );
        }
        let command_line = session_of(
            &workspaces,
            IntakeChannel::Cli,
            Some("ghostlight call"),
            None,
        );
        let calls = open_fresh(
            &executor,
            &browser,
            &command_line,
            24,
            "https://example.com/",
        );
        assert_eq!(opened_group(&calls), "\u{1F47B} Command line");
    }

    // an opener the catalog cannot name is titled unknown agent.
    {
        let (executor, browser, workspaces, _, _) = fixture();
        let by_program = session_of(&workspaces, IntakeChannel::Mcp, None, None);
        let calls = open_fresh(&executor, &browser, &by_program, 20, "https://example.com/");
        assert_eq!(opened_group(&calls), "\u{1F47B} Unknown agent");
        for (tab_id, unnamed) in [(21, ""), (22, "  "), (23, "Unknown agent")] {
            let unseen = workspaces.admit(unnamed.into(), IntakeChannel::Mcp);
            let calls = open_fresh(&executor, &browser, &unseen, tab_id, "https://example.org/");
            assert_eq!(
                opened_group(&calls),
                "\u{1F47B} Unknown agent",
                "{unnamed:?}"
            );
        }
    }
}

/// ADR-0203 Decisions 2.4 and 5.5 (b9c2da60): a held tab refuses with its landing's decision.
///
/// A tab refused after the browser opened it reports the effect. A held tab still refuses reads,
/// actions, history, focus and flow steps, names the refused host and the calls that clear it, is
/// audited with its landing denial rather than a pause, and credits the policy that refused it
/// after a republish; navigating it to an allowed page releases it. Listing withholds the title and
/// address of a tab on a refused host and names the host. Typing that landed stays applied when the
/// landing denies the check that would follow it, and the tab is not held for it.
#[test]
fn a_held_tab_refuses_with_its_landings_decision() {
    // a tab refused after the browser opened it reports the effect.
    {
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

    // a held tab can be navigated to an allowed page and is released.
    {
        for named in [true, false] {
            let policy = example_com_only();
            let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
            let (handle, _) = held_tab(&executor, &browser, &workspace);

            // The destination is authorized on its own, so a refused one sends nothing.
            let before = browser.calls().len();
            let refused = executor.execute(
                &workspace,
                "browser_navigate",
                json!({"tab":handle,"url":"https://elsewhere.test/again"}),
                &CancellationToken::default(),
            );
            assert_eq!(refused.status, Status::Blocked);
            assert_eq!(refused.effect, Effect::None);
            assert_eq!(browser.calls().len(), before);

            browser.push(Ok(BrowserOutcome::Navigated {
                tab: tab(7, "https://example.com/back"),
                committed_urls: vec!["https://example.com/back".into()],
            }));
            let input = if named {
                json!({"tab":handle,"url":"https://example.com/back"})
            } else {
                json!({"url":"https://example.com/back"})
            };
            let left = executor.execute(
                &workspace,
                "browser_navigate",
                input,
                &CancellationToken::default(),
            );
            assert_eq!(left.status, Status::Succeeded, "{}", left.summary);
            assert_eq!(left.facts["tab"], handle.as_str());
            assert!(matches!(
                browser.calls().last(),
                Some(BrowserCommand::Navigate { tab_id: 7, .. })
            ));

            browser.push(Ok(BrowserOutcome::Text {
                tab_id: 7,
                text: "Back".into(),
                truncated: false,
                title: "Back".into(),
                url: "https://example.com/back".into(),
            }));
            let read = executor.execute(
                &workspace,
                "browser_read",
                json!({"tab":handle}),
                &CancellationToken::default(),
            );
            assert_eq!(
                read.status,
                Status::Succeeded,
                "the allowed landing released the hold: {}",
                read.summary
            );
        }
    }

    // listing withholds title and url of a tab on a refused host and names the host.
    {
        let policy = example_com_only();
        let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
        let (held, landed) = held_tab(&executor, &browser, &workspace);
        let allowed = open_at(&executor, &browser, &workspace, 8, "https://example.com/");

        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![
                PhysicalTab {
                    tab_id: 7,
                    title: "Private ledger".into(),
                    url: "https://elsewhere.test/accounts/secret".into(),
                    active: false,
                    readiness: BrowserReadiness::Complete,
                },
                PhysicalTab {
                    tab_id: 8,
                    title: "Example Domain".into(),
                    url: "https://example.com/".into(),
                    active: true,
                    readiness: BrowserReadiness::Complete,
                },
            ],
        }));
        let listed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            &CancellationToken::default(),
        );
        assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
        let rows = listed.facts["tabs"].as_array().unwrap();
        let row = |handle: &str| {
            rows.iter()
                .find(|row| row["tab"] == handle)
                .cloned()
                .unwrap_or_else(|| panic!("{handle} is listed: {rows:?}"))
        };
        assert_eq!(
            row(&held),
            json!({
                "tab":held,
                "host":"elsewhere.test",
                "reason":landed.facts["reason"],
                "active":false,
                "readiness":"complete"
            })
        );
        assert_eq!(
            row(&allowed),
            json!({
                "tab":allowed,
                "title":"Example Domain",
                "url":"https://example.com/",
                "active":true,
                "readiness":"complete"
            })
        );
        let line = serde_json::to_string(audit.0.lock().unwrap().last().unwrap()).unwrap();
        for text in [listed.summary.as_str(), &listed.facts.to_string(), &line] {
            assert!(!text.contains("Private ledger"), "{text}");
            assert!(!text.contains("secret"), "{text}");
        }
    }

    // a held tab still refuses reads actions history and focus.
    {
        let policy = example_com_only();
        let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
        let (handle, _) = held_tab(&executor, &browser, &workspace);
        let before = browser.calls().len();
        for (tool, input) in [
            ("browser_read", json!({"tab":handle})),
            ("browser_screenshot", json!({"tab":handle})),
            ("browser_inspect", json!({"tab":handle})),
            ("browser_press_key", json!({"tab":handle,"key":"Enter"})),
            ("browser_execute", json!({"tab":handle,"script":"1"})),
            ("browser_history", json!({"tab":handle,"action":"back"})),
            ("browser_history", json!({"tab":handle,"action":"reload"})),
            ("browser_tabs", json!({"action":"focus","tab":handle})),
        ] {
            let refused = executor.execute(&workspace, tool, input, &CancellationToken::default());
            assert_eq!(
                refused.status,
                Status::Blocked,
                "{tool}: {}",
                refused.summary
            );
            assert_eq!(refused.facts["reason"], "tab_held", "{tool}");
            assert_eq!(refused.effect, Effect::None, "{tool}");
        }
        assert_eq!(
            browser.calls().len(),
            before,
            "nothing reached the held tab"
        );
    }

    // a held tab refusal names the refused host and the calls that clear it.
    {
        let policy = example_com_only();
        let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
        let (handle, landed) = held_tab(&executor, &browser, &workspace);
        for input in [json!({"tab":handle}), json!({})] {
            let refused = executor.execute(
                &workspace,
                "browser_read",
                input.clone(),
                &CancellationToken::default(),
            );
            assert_eq!(refused.status, Status::Blocked, "{input}");
            assert_eq!(refused.facts["reason"], "tab_held");
            assert_eq!(
                refused.summary,
                "That tab shows elsewhere.test, which this session may not use, so Ghostlight will not read or act on it there."
            );
            assert_eq!(
                refused.next_steps,
                vec!["Call browser_navigate with the tab and an allowed url to leave the page, or browser_tabs with action close to close the tab."]
            );
            // The refusal explains itself the way the landing that held the tab did.
            assert_eq!(refused.facts["denial_id"], landed.facts["denial_id"]);
            assert_eq!(refused.facts["policy_rule"], landed.facts["policy_rule"]);
        }
    }

    // a held tab is audited with its landing denial not a pause.
    {
        let policy = example_com_only();
        let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
        let (handle, landed) = held_tab(&executor, &browser, &workspace);
        let refused = executor.execute(
            &workspace,
            "browser_press_key",
            json!({"tab":handle,"key":"Enter"}),
            &CancellationToken::default(),
        );
        assert_eq!(refused.status, Status::Blocked, "{}", refused.summary);
        let records = audit.0.lock().unwrap();
        let record_of = |invocation: String| {
            records
                .iter()
                .find(|record| record.invocation == invocation)
                .cloned()
                .expect("every call is audited")
        };
        let landing = record_of(landed.invocation.to_string());
        let held = record_of(refused.invocation.to_string());
        assert!(!held.allowed);
        assert_eq!(held.reason, landing.reason);
        assert_ne!(held.reason, crate::governance::ReasonCode::RuntimeHold);
        assert_eq!(held.policy_rule, landing.policy_rule);
        assert!(held.denial_id.is_some());
        assert_eq!(held.denial_id, landing.denial_id);
        assert_eq!(held.observed.host.as_deref(), Some("elsewhere.test"));
        assert!(held.summary.contains("elsewhere.test"), "{}", held.summary);
        assert!(!held.summary.contains("paused"), "{}", held.summary);
    }

    // a flow step on a held tab is blocked by policy.
    {
        let policy = example_com_only();
        let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
        let (handle, _) = held_tab(&executor, &browser, &workspace);
        let before = browser.calls().len();
        let result = executor.execute(
            &workspace,
            "browser_flow",
            json!({"on_error":"continue","steps":[
                {"id":"first","tool":"browser_read","arguments":{"tab":handle}},
                {"id":"second","tool":"browser_read","arguments":{"tab":handle}}
            ]}),
            &CancellationToken::default(),
        );
        assert_eq!(result.facts["progress"]["issue"]["cause"], "policy_blocked");
        // A pause stops a flow; a policy refusal lets it continue, so the second step ran.
        assert_eq!(result.facts["progress"]["stopped"], false, "{result:?}");
        assert_eq!(result.facts["steps"][1]["status"], "blocked", "{result:?}");
        assert_eq!(
            browser.calls().len(),
            before,
            "nothing reached the held tab"
        );
    }

    // a held tab refused under a republished policy credits the policy that refused it.
    {
        let policy = TestPolicy::new();
        policy.publish(&keep_out_policy(["example", "keep-out"], false));
        let facade = policy.facade();
        let (executor, browser, _, workspace, audit) = fixture_with_governance(facade.clone());
        let (handle, landed) = held_tab(&executor, &browser, &workspace);
        let record_of = |invocation: String| {
            audit
                .0
                .lock()
                .unwrap()
                .iter()
                .find(|record| record.invocation == invocation)
                .cloned()
                .expect("every call is audited")
        };
        let read_held = || {
            let refused = executor.execute(
                &workspace,
                "browser_read",
                json!({"tab":handle}),
                &CancellationToken::default(),
            );
            assert_eq!(refused.status, Status::Blocked, "{}", refused.summary);
            assert_eq!(refused.facts["reason"], "tab_held");
            (record_of(refused.invocation.to_string()), refused)
        };
        let landing = record_of(landed.invocation.to_string());
        assert_eq!(landing.policy_tier.as_deref(), Some("user"));
        assert_eq!(landing.grant_id.as_deref(), Some("keep-out"));

        // Reordered grants put `example` where `keep-out` was. The refusal still credits `keep-out`,
        // with the denial id the current authority gives.
        policy.publish(&keep_out_policy(["keep-out", "example"], false));
        let (held, refused) = read_held();
        let current = facade.snapshot();
        let denial = current.authorize_landing(
            crate::governance::Capability::Read,
            "https://elsewhere.test/",
        );
        assert_ne!(held.authority, landing.authority);
        assert_eq!(held.authority, current.id());
        assert_eq!(held.reason, landing.reason);
        assert_eq!(held.policy_tier.as_deref(), Some("user"));
        assert_eq!(held.grant_id.as_deref(), Some("keep-out"));
        assert_eq!(held.denial_id, denial.denial_id());
        assert_ne!(held.denial_id, landing.denial_id);
        assert_eq!(
            refused.facts["denial_id"].as_str(),
            denial.denial_id().as_deref()
        );

        // A policy that now admits the page leaves the tab held until it leaves it, and the record
        // keeps the hold's reason with no policy attribution.
        policy.publish(&keep_out_policy(["keep-out", "example"], true));
        let (held, refused) = read_held();
        assert!(!held.allowed);
        assert_eq!(held.reason, landing.reason);
        assert_eq!(held.policy_tier, None);
        assert_eq!(held.grant_id, None);
        assert_eq!(held.denial_id, None);
        assert_eq!(held.policy_rule, None);
        assert!(
            refused.facts.get("denial_id").is_none(),
            "{:?}",
            refused.facts
        );
    }

    // a read denied typing postcondition preserves the applied landing without holding it.
    {
        for location in ["target", "focused", "selector"] {
            for composed in [false, true] {
                let policy = temporary_policy("typing-postcondition-landing");
                fs::write(&policy, serde_json::to_vec(&json!({
                    "schema":4,"name":"Observe source, act at destination","version":"1","grants":[
                        {"id":"source","hosts":{"allow":["example.com"]},"allowed":["read","action"]},
                        {"id":"landing","hosts":{"allow":["landing.example"]},"allowed":["action"]}
                    ]
                })).unwrap()).unwrap();
                let (executor, browser, _, workspace, audit) =
                    fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));
                browser.push(Ok(BrowserOutcome::TabOpened {
                    tab: tab(7, "https://example.com/"),
                    committed_urls: vec!["https://example.com/".into()],
                }));
                let opened = executor.execute(
                    &workspace,
                    "browser_navigate",
                    json!({"url":"https://example.com/"}),
                    &CancellationToken::default(),
                );
                assert_eq!(opened.status, Status::Succeeded);
                let field = ObservedTarget {
                    locator: "draft".into(),
                    role: "textbox".into(),
                    name: "Draft".into(),
                    state: vec![],
                    credential_class: false,
                    ..Default::default()
                };
                let mut arguments = json!({"tab":opened.facts["tab"],"text":"PRIVATE_DRAFT","expect":{"condition":"load_ready"}});
                if location == "target" {
                    browser.push(Ok(BrowserOutcome::targets(7, vec![field.clone()])));
                    let inspected = executor.execute(
                        &workspace,
                        "browser_inspect",
                        json!({}),
                        &CancellationToken::default(),
                    );
                    assert_eq!(inspected.status, Status::Succeeded);
                    arguments["target"] = inspected.facts["items"][0]["target"].clone();
                } else if location == "focused" {
                    arguments["focused"] = json!(true);
                } else {
                    arguments["selector"] = json!({"name":"Draft","role":"textbox","exact":true});
                    browser.push(Ok(BrowserOutcome::targets(7, vec![field.clone()])));
                }
                browser.push(Ok(BrowserOutcome::TargetsDescribed {
                    rebound: false,
                    tab_id: 7,
                    targets: vec![field],
                }));
                browser.push(Ok(BrowserOutcome::Typed {
                    tab: tab(7, "https://landing.example/"),
                    character_count: 13,
                    subject: None,
                    committed_urls: vec!["https://landing.example/".into()],
                }));
                let before = browser.calls().len();
                let invocation = if composed {
                    json!({"steps":[{"id":"draft","tool":"browser_type_text","arguments":arguments}]})
                } else {
                    arguments
                };

                let result = executor.execute(
                    &workspace,
                    if composed {
                        "browser_flow"
                    } else {
                        "browser_type_text"
                    },
                    invocation,
                    &CancellationToken::default(),
                );
                let label = format!("{location}, composed={composed}");
                assert_eq!(result.status, Status::Blocked, "{label}: {result:?}");
                assert_eq!(
                    result.effect,
                    if composed {
                        Effect::Partial
                    } else {
                        Effect::Applied
                    },
                    "{label}"
                );
                if composed {
                    assert_eq!(
                        result.facts["steps"][0]["result"]["effect"], "applied",
                        "{label}: composition retains the child's confirmed effect"
                    );
                }
                assert!(!result.repeat_safe, "{label}");
                let facts = if composed {
                    &result.facts["steps"][0]["result"]["facts"]
                } else {
                    &result.facts
                };
                assert!(
                    facts.get("held").is_none(),
                    "{label}: failed observation must not masquerade as a landing hold"
                );
                assert_eq!(
                    browser.calls().len(),
                    before + if location == "selector" { 3 } else { 2 },
                    "{label}: no observation of the read-denied landing"
                );
                let records = audit.0.lock().unwrap();
                let checks: Vec<_> = records
                    .iter()
                    .filter(|record| record.invocation == result.invocation)
                    .flat_map(|record| &record.permissions.checks)
                    .collect();
                assert!(
                    checks.iter().any(|check| check.requirements
                        == crate::governance::CapabilitySet::ACTION
                        && check.allowed),
                    "{label}: acknowledged action landing is admitted"
                );
                assert!(
                    checks.iter().any(|check| !check.allowed),
                    "{label}: separate observation denial remains visible"
                );
                assert!(!serde_json::to_string(&*records)
                    .unwrap()
                    .contains("PRIVATE_DRAFT"));
                drop(records);
                browser.push(Ok(BrowserOutcome::KeyPressed {
                    tab: tab(7, "https://landing.example/"),
                    key: "Tab".into(),
                    subject: None,
                    committed_urls: vec![],
                }));
                let follow_up = executor.execute(
                    &workspace,
                    "browser_press_key",
                    json!({"key":"Tab"}),
                    &CancellationToken::default(),
                );
                assert_eq!(
                    follow_up.status,
                    Status::Succeeded,
                    "{label}: the admitted landing stays usable: {follow_up:?}"
                );
            }
        }
    }
}

/// ACCEPTANCE Browser journey 2: a same-document route keeps a tab's handles, and a new origin or a
/// committed document in the same origin expires them.
#[test]
fn a_same_document_route_keeps_handles_and_a_commit_or_new_origin_expires_them() {
    // a same document route keeps handles and a new origin expires them.
    {
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
            &CancellationToken::default(),
        );
        assert_eq!(moved.status, Status::Succeeded, "{}", moved.summary);
        assert_ne!(moved.facts["landing"]["document_generation"], generation);
        let stale = executor.execute(
            &workspace,
            "browser_click",
            json!({"target":target}),
            &CancellationToken::default(),
        );
        assert_eq!(stale.status, Status::Failed, "{}", stale.summary);
        assert_eq!(stale.facts["reason"], "stale_target");
    }

    // a committed document in the same origin still expires handles.
    {
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
            &CancellationToken::default(),
        );
        assert_eq!(stale.facts["reason"], "stale_target");
    }
}

/// Open one fresh controlled tab titled `title` and return its handle.
fn open_titled(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
    tab_id: u64,
    url: &str,
    title: &str,
) -> String {
    let mut opened = tab(tab_id, url);
    opened.title = title.into();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: opened,
        committed_urls: vec![url.into()],
    }));
    let result = executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":url,"new_tab":true}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    result.facts["tab"].as_str().unwrap().to_owned()
}

/// Answer one script evaluation on `tab_id`, the browser work a no-tab call reaches.
fn push_script(browser: &FakeBrowser, tab_id: u64, url: &str) {
    browser.push(Ok(BrowserOutcome::ScriptEvaluated {
        tab: tab(tab_id, url),
        value: "1".into(),
        truncated: false,
        form: crate::browser::adapter::ScriptForm::AsWritten,
        committed_urls: vec![],
    }));
}

fn script_without_tab(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_execute",
        json!({"script":"1"}),
        &CancellationToken::default(),
    )
}

/// ACCEPTANCE Executor 6 (ea58ecb4): which tab a call means.
///
/// A call naming no tab uses the session's only tab; among several it is refused with the tabs and
/// does nothing; a flow that opens a tab acts in it without naming it. A target from another of
/// this session's tabs names that tab, a fill across two tabs says a fill works in one tab, and an
/// ownership refusal names the owner as the window does.
#[test]
fn a_call_means_the_tab_it_names_or_the_sessions_only_tab() {
    // a call without a tab uses the sessions only tab.
    {
        let (executor, browser, _, workspace, _) = fixture();
        let only = open_titled(
            &executor,
            &browser,
            &workspace,
            7,
            "https://example.com/",
            "Home",
        );
        push_script(&browser, 7, "https://example.com/");
        let before = browser.calls().len();

        let result = script_without_tab(&executor, &workspace);

        assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
        assert_eq!(result.facts["tab"], only.as_str());
        let calls = browser.calls()[before..].to_vec();
        assert!(
            matches!(
                calls.as_slice(),
                [BrowserCommand::EvaluateScript { tab_id: 7, .. }]
            ),
            "{calls:?}"
        );
    }

    // a call without a tab among several is refused with the tabs and does nothing.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        let first = open_titled(
            &executor,
            &browser,
            &workspace,
            7,
            "https://example.com/",
            "Home",
        );
        let second = open_titled(
            &executor,
            &browser,
            &workspace,
            9,
            "https://example.org/",
            "Docs",
        );
        let before = browser.calls().len();
        let audited = audit.0.lock().unwrap().len();

        for result in [
            script_without_tab(&executor, &workspace),
            navigate(&executor, &workspace, "https://example.net/"),
        ] {
            assert_eq!(result.status, Status::Failed, "{}", result.summary);
            assert_eq!(result.effect, Effect::None);
            assert!(result.repeat_safe);
            assert_eq!(
                result.summary,
                "More than one tab is open in this session, so the call must name one."
            );
            assert_eq!(result.facts["reason"], "ambiguous_tab");
            let mut listed = result.facts["tabs"].as_array().unwrap().clone();
            listed.sort_by_key(|tab| tab["tab"].as_str().unwrap().to_owned());
            let mut expected = vec![
                json!({"tab":first,"title":"Home","url":"https://example.com/"}),
                json!({"tab":second,"title":"Docs","url":"https://example.org/"}),
            ];
            expected.sort_by_key(|tab| tab["tab"].as_str().unwrap().to_owned());
            assert_eq!(listed, expected);
            assert_eq!(
                result.next_steps,
                vec!["Repeat the call with tab set to the handle of the tab you mean, listed in tabs."]
            );
        }
        assert_eq!(browser.calls().len(), before, "nothing reached the browser");
        let records = audit.0.lock().unwrap();
        assert_eq!(records.len(), audited + 2);
        assert!(
            records[audited..]
                .iter()
                .all(|record| !serde_json::to_string(record).unwrap().contains("Docs")),
            "the candidate tabs are recovery help, never audit"
        );
        drop(records);

        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![
                tab(7, "https://example.com/"),
                tab(9, "https://example.org/"),
            ],
        }));
        let listed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            &CancellationToken::default(),
        );
        assert_eq!(listed.status, Status::Succeeded, "a list needs no tab");
        assert_eq!(listed.facts["tabs"].as_array().unwrap().len(), 2);

        push_script(&browser, 9, "https://example.org/");
        let named = executor.execute(
            &workspace,
            "browser_execute",
            json!({"script":"1","tab":second}),
            &CancellationToken::default(),
        );
        assert_eq!(named.status, Status::Succeeded, "{}", named.summary);
        assert_eq!(named.facts["tab"], second.as_str());
    }

    // a flow that opens a tab acts in it without naming it.
    {
        let (executor, browser, _, workspace, _) = fixture();
        open_titled(
            &executor,
            &browser,
            &workspace,
            7,
            "https://example.com/",
            "Home",
        );
        open_titled(
            &executor,
            &browser,
            &workspace,
            9,
            "https://example.org/",
            "Docs",
        );
        let mut opened = tab(11, "https://example.net/");
        opened.title = "Mine".into();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: opened,
            committed_urls: vec!["https://example.net/".into()],
        }));
        push_script(&browser, 11, "https://example.net/");
        let before = browser.calls().len();

        let result = executor.execute(
            &workspace,
            "browser_flow",
            json!({"steps":[
                {"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.net/","new_tab":true}},
                {"id":"run","tool":"browser_execute","arguments":{"script":"1"}}
            ]}),
            &CancellationToken::default(),
        );

        assert_eq!(result.status, Status::Succeeded, "{result:?}");
        let steps = result.facts["steps"].as_array().unwrap();
        let mine = steps[0]["result"]["facts"]["tab"].clone();
        assert_eq!(steps[1]["result"]["facts"]["tab"], mine);
        let calls = browser.calls()[before..].to_vec();
        assert!(
            matches!(
                calls.last(),
                Some(BrowserCommand::EvaluateScript { tab_id: 11, .. })
            ),
            "{calls:?}"
        );
    }

    // a target from another of this sessions tabs names that tab.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        let first = open_at(&executor, &browser, &workspace, 7, "https://example.com/a");
        let second = open_at(&executor, &browser, &workspace, 8, "https://example.com/b");
        let target = find_save(&executor, &browser, &workspace, 8, &second);
        let dispatched = browser.calls().len();

        let refused = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":first,"target":target}),
            &CancellationToken::default(),
        );

        assert_eq!(refused.status, Status::Failed, "{}", refused.summary);
        assert_eq!(refused.effect, Effect::None);
        assert!(refused.repeat_safe);
        assert_eq!(refused.facts["reason"], "handle_on_other_tab");
        assert_eq!(refused.facts["handle_tab"], second.as_str());
        assert_eq!(
            refused.summary,
            format!("That handle belongs to {second}, another of this session's tabs.")
        );
        assert_eq!(
            refused.next_steps,
            vec![
                "Repeat the call with handles from one tab, and with tab left out or set to that tab."
            ]
        );
        assert_eq!(
            browser.calls().len(),
            dispatched,
            "nothing reaches the browser"
        );
        // The durable record keeps the cause word, and neither tab.
        let records = audit.0.lock().unwrap();
        let record = records.last().unwrap();
        assert_eq!(
            serde_json::to_value(&record.refusal_facts).unwrap(),
            json!({"reason":"workspace_unusable","cause":"handle_on_other_tab"})
        );
        let line = serde_json::to_string(record).unwrap();
        assert!(!line.contains(&second), "{line}");
        assert!(!line.contains(&first), "{line}");
    }

    // a fill across two tabs says a fill works in one tab.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        let first = open_at(&executor, &browser, &workspace, 7, "https://example.com/a");
        let second = open_at(&executor, &browser, &workspace, 8, "https://example.com/b");
        let on_first = find_save(&executor, &browser, &workspace, 7, &first);
        let on_second = find_save(&executor, &browser, &workspace, 8, &second);
        let dispatched = browser.calls().len();

        for fill in [
            json!({"fields":[{"target":on_first,"value":"a"},{"target":on_second,"value":"b"}]}),
            json!({"fields":[{"target":on_first,"value":"a"}],"submit_target":on_second}),
        ] {
            let refused = executor.execute(
                &workspace,
                "browser_fill_form",
                fill.clone(),
                &CancellationToken::default(),
            );
            assert_eq!(
                refused.status,
                Status::Failed,
                "{fill}: {}",
                refused.summary
            );
            assert_eq!(refused.effect, Effect::None);
            assert_eq!(refused.facts["reason"], "fields_on_several_tabs", "{fill}");
            assert_eq!(
                refused.summary,
                "A fill works in one tab, and these targets are on more than one."
            );
            assert_eq!(
                refused.next_steps,
                vec!["Make one browser_fill_form call for each tab."]
            );
            assert_eq!(
                serde_json::to_value(&audit.0.lock().unwrap().last().unwrap().refusal_facts)
                    .unwrap(),
                json!({"reason":"workspace_unusable","cause":"fields_on_several_tabs"})
            );
        }

        // A fill that names its tab and a field on another one is told which tab that field is in.
        let named = executor.execute(
            &workspace,
            "browser_fill_form",
            json!({"tab":first,"fields":[{"target":on_first,"value":"a"},{"target":on_second,"value":"b"}]}),
            &CancellationToken::default(),
        );
        assert_eq!(named.facts["reason"], "handle_on_other_tab");
        assert_eq!(named.facts["handle_tab"], second.as_str());
        assert_eq!(
            browser.calls().len(),
            dispatched,
            "nothing reaches the browser"
        );
    }

    // an ownership refusal names the owner as the window does.
    {
        let (executor, browser, workspaces, workspace, audit) = fixture();
        let owner = workspaces.admit("claude-code".into(), IntakeChannel::Mcp);
        let window_name =
            crate::provenance::ConnectionEvidence::claimed("claude-code", IntakeChannel::Mcp)
                .details()
                .agent
                .name;
        assert_eq!(window_name, "Claude Code");
        let owned = open_at(&executor, &browser, &owner, 9, "https://example.com/");

        let refused = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":owned}),
            &CancellationToken::default(),
        );

        assert_eq!(refused.facts["reason"], "ownership_mismatch");
        assert_eq!(refused.facts["owner_workspace"], window_name.as_str());
        assert_eq!(
            refused.summary,
            "That handle belongs to another Ghostlight session (Claude Code)."
        );
        let line = serde_json::to_string(audit.0.lock().unwrap().last().unwrap()).unwrap();
        assert!(!line.contains("Claude Code"), "{line}");
        assert!(!line.contains("claude-code"), "{line}");

        // An owner whose agent nothing names is the unknown agent, and the refusal names no one.
        let unnamed = workspaces.admit(String::new(), IntakeChannel::Mcp);
        let unnamed_tab = open_at(&executor, &browser, &unnamed, 10, "https://example.com/");
        let refused = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":unnamed_tab}),
            &CancellationToken::default(),
        );
        assert_eq!(refused.facts["reason"], "ownership_mismatch");
        assert!(refused.facts.get("owner_workspace").is_none());
        assert_eq!(
            refused.summary,
            "That handle belongs to a different Ghostlight session."
        );
        assert!(
            !refused
                .summary
                .contains(crate::language::provenance::UNKNOWN_AGENT),
            "{}",
            refused.summary
        );
    }
}

/// Open a tab on example.com and let a navigation of it land on a host the policy refuses, which
/// holds the tab. Returns the held tab's handle and the refused landing's result.
fn held_tab(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
) -> (String, crate::work::result::InvocationResult) {
    let handle = open_at(executor, browser, workspace, 7, "https://example.com/");
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://elsewhere.test/"),
        committed_urls: vec!["https://elsewhere.test/".into()],
    }));
    let landed = executor.execute(
        workspace,
        "browser_navigate",
        json!({"tab":handle,"url":"https://example.com/redirects"}),
        &CancellationToken::default(),
    );
    assert_eq!(landed.status, Status::Blocked, "{}", landed.summary);
    assert_eq!(landed.facts["held"], true);
    (handle, landed)
}

/// ACCEPTANCE Governance 9 and 14: a close is authorized on its own. Closing needs Action on the
/// tab's host; a held tab can be closed; under the close setting a denied landing, a retained one
/// and a held tab are closed even under a read-only policy.
#[test]
fn a_close_is_authorized_on_its_own() {
    // closing a tab needs action on its host.
    {
        let policy = TestPolicy::new();
        policy.publish(
            &json!({"schema":4,"name":"test authority","version":"1","grants":[
                {"id":"example","hosts":{"allow":["example.com"]},"allowed":["read","action"]},
                {"id":"elsewhere","hosts":{"allow":["elsewhere.test"]},"allowed":["read"]}
            ]}),
        );
        let (executor, browser, workspaces, workspace, _) =
            fixture_with_governance(policy.facade());
        let acting = open_at(&executor, &browser, &workspace, 7, "https://example.com/");
        let reading = open_at(
            &executor,
            &browser,
            &workspace,
            8,
            "https://elsewhere.test/",
        );

        let before = browser.calls().len();
        let refused = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":reading}),
            &CancellationToken::default(),
        );
        assert_eq!(refused.status, Status::Blocked, "{}", refused.summary);
        assert_eq!(refused.effect, Effect::None);
        assert_eq!(
            refused.facts["reason"], "capability_denied",
            "{}",
            refused.facts
        );
        assert_eq!(browser.calls().len(), before, "nothing reached the browser");
        assert_eq!(workspaces.tab_bindings(workspace.as_str()).len(), 2);

        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":acting}),
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Succeeded, "{}", closed.summary);
        assert!(matches!(
            browser.calls().last(),
            Some(BrowserCommand::CloseTab { tab_id: 7, .. })
        ));
    }

    // a held tab can be closed.
    {
        let policy = example_com_only();
        let (executor, browser, workspaces, workspace, _) =
            fixture_with_governance(policy.facade());
        let (handle, _) = held_tab(&executor, &browser, &workspace);

        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Succeeded, "{}", closed.summary);
        assert_eq!(closed.facts["closed"], true);
        assert!(matches!(
            browser.calls().last(),
            Some(BrowserCommand::CloseTab { tab_id: 7, .. })
        ));
        assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());
    }

    // a denied landing is closed under a read only policy when the setting allows.
    {
        let policy = TestPolicy::new();
        policy.publish(&example_com_with(&["read"], true));
        let (executor, browser, workspaces, workspace, _) =
            fixture_with_governance(policy.facade());
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://elsewhere.test/"),
            committed_urls: vec![
                "https://example.com/".into(),
                "https://elsewhere.test/".into(),
            ],
        }));
        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let landed = navigate(&executor, &workspace, "https://example.com/");
        assert_eq!(landed.status, Status::Blocked, "{}", landed.summary);
        assert_eq!(landed.effect, Effect::None);
        assert_eq!(landed.facts["compensated"], true);
        assert!(landed.repeat_safe);
        assert!(matches!(
            browser.calls().last(),
            Some(BrowserCommand::CloseTab { tab_id: 7, .. })
        ));
        assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());
    }

    // a retained denied landing is closed under the setting alone.
    {
        // No grant admits Action anywhere, and the refused landing's host is refused everything.
        let policy = TestPolicy::new();
        policy.publish(&example_com_with(&["read"], true));
        let (executor, browser, workspaces, workspace, _) =
            fixture_with_governance(policy.facade());
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://elsewhere.test/"),
            committed_urls: vec![
                "https://example.com/".into(),
                "https://elsewhere.test/".into(),
            ],
        }));
        browser.push(Err(crate::browser::BrowserError::LocalInterlock(
            "preserved".into(),
        )));
        let landed = navigate(&executor, &workspace, "https://example.com/");
        assert_eq!(landed.status, Status::Blocked, "{}", landed.summary);
        assert_eq!(landed.effect, Effect::Applied);
        assert_eq!(landed.facts["retained"], true);
        assert_eq!(landed.facts["held"], true);
        let handle = landed.facts["tab"].as_str().unwrap().to_owned();
        assert_eq!(workspaces.tab_bindings(workspace.as_str()).len(), 1);

        // The retained tab is held, so reading it is refused as a held tab, naming the refused host.
        let read = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(read.status, Status::Blocked, "{}", read.summary);
        assert_eq!(read.facts["reason"], "tab_held", "{}", read.facts);
        assert!(read.summary.contains("elsewhere.test"), "{}", read.summary);

        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Succeeded, "{}", closed.summary);
        assert_eq!(closed.facts["closed"], true);
        assert!(matches!(
            browser.calls().last(),
            Some(BrowserCommand::CloseTab { tab_id: 7, .. })
        ));
        assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());
    }

    // a held tab is closed under the setting alone.
    {
        for allow_close in [true, false] {
            // No grant admits Action anywhere, and the held page's host is refused everything.
            let policy = TestPolicy::new();
            policy.publish(&example_com_with(&["read"], allow_close));
            let (executor, browser, workspaces, workspace, _) =
                fixture_with_governance(policy.facade());
            let (handle, _) = held_tab(&executor, &browser, &workspace);

            let before = browser.calls().len();
            if allow_close {
                browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
            }
            let closed = executor.execute(
                &workspace,
                "browser_tabs",
                json!({"action":"close","tab":handle}),
                &CancellationToken::default(),
            );
            if allow_close {
                assert_eq!(closed.status, Status::Succeeded, "{}", closed.summary);
                assert_eq!(closed.facts["closed"], true);
                assert!(workspaces.tab_bindings(workspace.as_str()).is_empty());
            } else {
                assert_eq!(closed.status, Status::Blocked, "{}", closed.summary);
                assert_eq!(closed.facts["reason"], "tab_close_denied");
                assert_eq!(closed.effect, Effect::None);
                assert_eq!(browser.calls().len(), before, "nothing reached the browser");
                assert_eq!(workspaces.tab_bindings(workspace.as_str()).len(), 1);
            }
        }
    }
}

/// A policy document granting `allowed` on example.com and nothing else, with the tab-close
/// setting at `allow_close`.
fn example_com_with(allowed: &[&str], allow_close: bool) -> serde_json::Value {
    json!({"schema":4,"name":"test authority","version":"1",
        "grants":[{"id":"example","hosts":{"allow":["example.com"]},"allowed":allowed}],
        "config":[{"key":"browser.tabs.allow_close","value":allow_close,"level":"mandatory"}]})
}

/// A policy whose grant `keep-out` refuses elsewhere.test and whose grant `example` admits
/// example.com, listed in `order`. `admit` lifts the refusal.
fn keep_out_policy(order: [&str; 2], admit: bool) -> serde_json::Value {
    let deny: &[&str] = if admit { &[] } else { &["elsewhere.test"] };
    let grant = |id: &str| match id {
        "example" => json!({"id":"example","hosts":{"allow":["example.com"]},
            "allowed":["read","action","write","execute"]}),
        _ => json!({"id":"keep-out","hosts":{"allow":["*"],"deny":deny},"allowed":["read"]}),
    };
    json!({"schema":4,"name":"test authority","version":"1",
        "grants":[grant(order[0]), grant(order[1])]})
}

/// Find the one Save field on the tab `tab_id` that the handle `tab_handle` names, and return
/// its target handle.
fn find_save(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
    tab_id: u64,
    tab_handle: &str,
) -> String {
    browser.push(Ok(BrowserOutcome::targets(
        tab_id,
        vec![ObservedTarget {
            locator: format!("save-{tab_id}"),
            role: "textbox".into(),
            name: "Save".into(),
            state: vec![],
            credential_class: false,
            ..Default::default()
        }],
    )));
    let found = executor.execute(
        workspace,
        "browser_find",
        json!({"tab":tab_handle,"text":"Save"}),
        &CancellationToken::default(),
    );
    assert_eq!(found.status, Status::Succeeded, "{}", found.summary);
    found.facts["matches"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned()
}
