//! The workspace aggregate's tests.

use crate::browser::adapter::{
    BrowserReadiness, CaptureScope, ObservedTarget, PhysicalTab, ViewportGeometry,
};
use crate::intake::session::{IntakeChannel, SessionMarker};

use crate::language::outcome::TargetRole;
use crate::language::HandleKind;

use super::{
    short_handle, BindingState, ReleasedTabs, SnapshotScope, StaleReason, TabHold, TargetBook,
    TargetHandle, WorkspaceError, WorkspaceId, WorkspaceStore,
};
use crate::language::outcome::DialogKind;

const TEST_BROWSER: &str = "browser_test";

/// Admit one test workspace. Its first tab binds it to [`TEST_BROWSER`].
fn admit_test(store: &WorkspaceStore) -> WorkspaceId {
    store.admit("test".into(), IntakeChannel::Mcp)
}

fn physical(id: u64, url: &str) -> PhysicalTab {
    PhysicalTab {
        tab_id: id,
        title: "title".into(),
        url: url.into(),
        active: true,
        readiness: BrowserReadiness::Complete,
    }
}

fn marker(pid: u32, started_at: u64) -> SessionMarker {
    SessionMarker::Process {
        pid,
        started_at,
        name: "pwsh.exe".into(),
    }
}

/// ADR-0106 (c78d4bd3, 10fc554a): a session is its opener and owner and outlives its connections.
///
/// Connection details stay plural and a close detaches only its own connection; a later connection
/// renames neither the session nor its tab group; one caller resumes its own workspace and a
/// recycled pid does not; a workspace outlives its connection and dies with its owner, forgetting
/// its recording; another workspace is named only by its client label.
#[test]
fn a_session_is_its_opener_and_owner_and_outlives_its_connections() {
    // connection details stay plural and detach only the closing connection.
    {
        use crate::provenance::ConnectionEvidence;
        use std::sync::Arc;
        let store = WorkspaceStore::default();
        let first = Arc::new(ConnectionEvidence::claimed("first", IntakeChannel::Mcp));
        let second = Arc::new(ConnectionEvidence::claimed("second", IntakeChannel::Cli));
        let owner = SessionMarker::Declared {
            key: "shared".into(),
        };
        let workspace = store.connect(first.clone(), Some(owner.clone()));
        assert_eq!(store.connect(second.clone(), Some(owner)), workspace);
        assert_eq!(store.summaries()[0].connections.len(), 2);
        assert!(store.single_connection(&workspace).is_none());
        store.disconnect(
            &workspace,
            first.attribution().connection_id.as_deref().unwrap(),
        );
        assert_eq!(store.summaries()[0].connections, vec![second.details()]);
        assert_eq!(
            store.single_connection(&workspace).unwrap().attribution(),
            second.attribution()
        );
        assert_eq!(
            first.details().reported_application.as_deref(),
            Some("first")
        );
    }

    // a later connection neither renames the session nor its tab group.
    {
        use crate::provenance::ConnectionEvidence;
        use std::sync::Arc;
        let store = WorkspaceStore::default();
        let opener = Arc::new(ConnectionEvidence::claimed(
            "claude-code",
            IntakeChannel::Mcp,
        ));
        let later = Arc::new(ConnectionEvidence::claimed(
            "codex-mcp-client",
            IntakeChannel::Mcp,
        ));
        let owner = SessionMarker::Declared {
            key: "shared".into(),
        };
        let workspace = store.connect(opener.clone(), Some(owner.clone()));
        assert_eq!(store.connect(later.clone(), Some(owner)), workspace);
        let summary = &store.summaries()[0];
        assert_eq!(summary.opener, opener.details());
        assert_eq!(summary.opener.agent.name, "Claude Code");
        assert_eq!(summary.connections.len(), 2);
        let named = store.opener(&workspace).unwrap();
        assert_eq!(named.agent, opener.details().agent);
        assert_eq!(named.reuse_key.as_deref(), Some("claude-code"));

        // The opener names the workspace after it leaves, as long as the workspace lives.
        store.disconnect(
            &workspace,
            opener.attribution().connection_id.as_deref().unwrap(),
        );
        let summary = &store.summaries()[0];
        assert_eq!(summary.connections, vec![later.details()]);
        assert_eq!(summary.opener, opener.details());
        assert_eq!(store.opener(&workspace).unwrap(), named);
    }

    // one caller resumes its own workspace and a recycled pid does not.
    {
        let store = WorkspaceStore::default();
        let first = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100));
        let again = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100));
        assert_eq!(
            first, again,
            "the same caller must reach the same workspace"
        );

        // The negative control, and the reason identity is not pid plus name: a recycled pid
        // running the same program must not inherit the dead session's tabs.
        let recycled = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 200));
        assert_ne!(first, recycled);

        // A connection with no marker is bound to that connection, as the MCP edge expects.
        let bound = store.admit("codex".into(), IntakeChannel::Mcp);
        assert!(!store.is_owned(&bound));
        assert!(store.is_owned(&first));
    }

    // a workspace outlives its connection and dies with its owner.
    {
        let store = WorkspaceStore::default();
        let owned = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100));
        let lease = store.acquire(&owned).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(41, "https://example.com/"))
            .unwrap();
        lease
            .apply_landing(&tab.handle, &physical(41, "https://example.com/"))
            .unwrap();
        drop(lease);

        // While the owner lives, nothing is reaped, so the next command still finds the tab.
        assert!(store.reap(&|_| true).is_empty());
        assert!(store.is_owned(&owned));

        // When the owner is gone the workspace goes with it, and hands back the tabs it held so
        // the caller's browser does not keep them forever.
        let abandoned = store.reap(&|_| false);
        assert_eq!(
            abandoned,
            vec![ReleasedTabs {
                browser: Some(TEST_BROWSER.into()),
                physical_ids: vec![41],
            }]
        );
        assert!(!store.is_owned(&owned));
        assert_eq!(store.summaries().len(), 0);
    }

    // a released workspace forgets its recording.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        store.hold_recording(&workspace, "browser_test", "recording_1");
        assert_eq!(
            store.recording(&workspace).map(|held| held.id),
            Some("recording_1".to_owned())
        );
        store.release(&workspace);
        assert_eq!(store.recording(&workspace), None);
        let next = admit_test(&store);
        assert_eq!(
            store.recording(&next),
            None,
            "a new workspace holds no recording"
        );
    }

    // another workspace is named only by its client label.
    {
        let store = WorkspaceStore::default();
        let caller = store.admit("caller".into(), IntakeChannel::Mcp);
        let labeled = store.admit("Codex".into(), IntakeChannel::Mcp);
        let unlabeled = store.admit(String::new(), IntakeChannel::Mcp);
        let handles = |workspace: &WorkspaceId, physical_id: u64| {
            let lease = store.acquire(workspace).unwrap();
            let tab = lease
                .add_tab(TEST_BROWSER, &physical(physical_id, "https://example.com"))
                .unwrap();
            let targets = lease
                .register_targets(
                    &tab,
                    &[ObservedTarget {
                        locator: "l1".into(),
                        role: "button".into(),
                        name: "Go".into(),
                        state: vec![],
                        credential_class: false,
                        ..Default::default()
                    }],
                )
                .unwrap();
            (
                tab.handle.as_str().to_owned(),
                targets[0].0.as_str().to_owned(),
            )
        };
        let (labeled_tab, labeled_target) = handles(&labeled, 1);
        let (unlabeled_tab, unlabeled_target) = handles(&unlabeled, 2);

        assert_eq!(
            store.tab_owner(&caller, &labeled_tab).as_deref(),
            Some("Codex")
        );
        assert_eq!(
            store.target_owner(&caller, &labeled_target).as_deref(),
            Some("Codex")
        );
        // An unlabeled owner is never named, so its workspace id cannot stand in for a label.
        assert_eq!(store.tab_owner(&caller, &unlabeled_tab), None);
        assert_eq!(store.target_owner(&caller, &unlabeled_target), None);
        // A workspace's own handle has no other owner to name.
        assert_eq!(store.tab_owner(&labeled, &labeled_tab), None);
        assert_eq!(store.target_owner(&labeled, &labeled_target), None);
    }
}

/// A busy workspace waits within the deadline: a contended lease is refused as busy when the
/// deadline passes or the call is cancelled, and taken once the holder lets go; work in flight is
/// never reaped underneath itself.
#[test]
fn a_busy_workspace_waits_within_the_deadline() {
    // acquire until synchronizes contention via condvar.
    {
        use crate::work::CancellationToken;
        use std::thread;
        use std::time::{Duration, Instant};

        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let first_lease = store.acquire(&workspace).unwrap();

        // 1. Contention with deadline expiry:
        let cancellation = CancellationToken::default();
        let short_deadline = Instant::now() + Duration::from_millis(30);
        let expired = store.acquire_until(&workspace, short_deadline, &cancellation);
        assert_eq!(expired.err(), Some(WorkspaceError::Busy));

        // 2. Contention with cancellation:
        let cancelled = CancellationToken::default();
        cancelled.cancel(crate::language::outcome::CancelCause::Client);
        let long_deadline = Instant::now() + Duration::from_secs(5);
        let aborted = store.acquire_until(&workspace, long_deadline, &cancelled);
        assert_eq!(aborted.err(), Some(WorkspaceError::Busy));

        // 3. Contention resolved by dropping the active lease:
        let store_clone = store.clone();
        let workspace_clone = workspace.clone();
        let waiter = thread::spawn(move || {
            let cancellation = CancellationToken::default();
            let deadline = Instant::now() + Duration::from_secs(5);
            store_clone.acquire_until(&workspace_clone, deadline, &cancellation)
        });

        // Whether the waiter is already parked or arrives after the drop, it takes the lease.
        drop(first_lease);

        let acquired = waiter.join().unwrap();
        assert!(
            acquired.is_ok(),
            "waiter successfully acquired lease after notification"
        );
    }

    // work in flight is never reaped underneath itself.
    {
        let store = WorkspaceStore::default();
        let owned = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100));
        let lease = store.acquire(&owned).unwrap();
        assert!(
            store.reap(&|_| false).is_empty(),
            "a leased workspace is mid-invocation; releasing it would pull the tabs out from under it"
        );
        drop(lease);
        assert_eq!(store.reap(&|_| false).len(), 0);
        assert!(!store.is_owned(&owned));
    }
}

/// The name the client of the pool tests claims unless a test names another.
const TEST_CLIENT: &str = "test";

/// The reuse key of a session whose MCP client claims `client`.
fn key_of(client: &str) -> String {
    crate::provenance::ConnectionEvidence::claimed(client, IntakeChannel::Mcp)
        .reuse_key()
        .expect("the claim names an agent")
}

/// Admit a workspace of `TEST_CLIENT` in `browser` and land one tab per `(id, url)` in it.
fn workspace_with_tabs(store: &WorkspaceStore, browser: &str, tabs: &[(u64, &str)]) -> WorkspaceId {
    client_workspace_with_tabs(store, TEST_CLIENT, browser, tabs)
}

/// Admit a workspace of `client` in `browser` and land one tab per `(id, url)` in it.
fn client_workspace_with_tabs(
    store: &WorkspaceStore,
    client: &str,
    browser: &str,
    tabs: &[(u64, &str)],
) -> WorkspaceId {
    let workspace = store.admit(client.into(), IntakeChannel::Mcp);
    let lease = store.acquire(&workspace).unwrap();
    for (id, url) in tabs {
        let tab = lease.add_tab(browser, &physical(*id, url)).unwrap();
        lease
            .apply_landing(&tab.handle, &physical(*id, url))
            .unwrap();
    }
    workspace
}

/// The tab a `TEST_CLIENT` session opening `url` in `browser` claims, if any.
fn claimed(store: &WorkspaceStore, browser: &str, url: &str) -> Option<u64> {
    claimed_by(store, browser, &key_of(TEST_CLIENT), url)
}

/// The tab a session whose reuse key is `key` claims when it opens `url` in `browser`, if any.
fn claimed_by(store: &WorkspaceStore, browser: &str, key: &str, url: &str) -> Option<u64> {
    store
        .claim_released(browser, key, url)
        .map(|released| released.physical_id)
}

/// Answer every release close as the person's preserve-tabs setting would: kept open.
fn kept_open(store: &WorkspaceStore, released: &ReleasedTabs) {
    let browser = released.browser.as_deref().unwrap();
    for &physical_id in &released.physical_ids {
        store.settle_released(browser, physical_id, true);
    }
}

/// 7bd0bf20: a tab let go of is reused only as kept, and a tab forgotten by any close leaves nothing
/// its page minted.
///
/// A released tab is claimable only after its close kept it open, by host and exact address first
/// and then lowest id, only by a session of the client that released it and never from a claim
/// naming no agent, after a reconnect only where its browser still shows it, and it leaves the pool
/// when closed, committed or taken. A decisive close receipt tolerates an earlier close event; a
/// tab the person closes leaves no snapshot or image, and a recreated handle never serves the
/// closed page's capture or diff; snapshots and captured images supersede per tab, respect their
/// ceiling and expire on commit.
#[test]
fn a_tab_let_go_of_is_reused_only_as_kept_and_a_closed_one_leaves_nothing() {
    // a released tab is claimable only after its close kept it open.
    {
        let url = "https://example.com/";
        let store = WorkspaceStore::default();
        let workspace = workspace_with_tabs(
            &store,
            TEST_BROWSER,
            &[(1, url), (2, url), (3, url), (4, url)],
        );
        store.release(&workspace);
        assert_eq!(
            claimed(&store, TEST_BROWSER, url),
            None,
            "a tab whose release close has not answered is never claimed"
        );

        // Tab 1 closed. Tab 2 closed and tab 3 loaded a page while their cleanup ran, so a later
        // kept-open answer does not bring them back. An answer from another browser names
        // another tab 4.
        store.settle_released(TEST_BROWSER, 1, false);
        store.apply_browser_close(TEST_BROWSER, 2);
        store.settle_released(TEST_BROWSER, 2, true);
        store.note_browser_landing(TEST_BROWSER, 3, "https://example.com/mine");
        store.settle_released(TEST_BROWSER, 3, true);
        store.settle_released("browser_other", 4, true);
        assert_eq!(claimed(&store, TEST_BROWSER, url), None);

        store.settle_released(TEST_BROWSER, 4, true);
        assert_eq!(claimed(&store, TEST_BROWSER, url), Some(4));
        assert_eq!(claimed(&store, TEST_BROWSER, url), None);
    }

    // released tabs are claimed by host exact url first then lowest id.
    {
        let store = WorkspaceStore::default();
        let workspace = workspace_with_tabs(
            &store,
            TEST_BROWSER,
            &[
                (9, "https://example.com/a"),
                (4, "https://example.com/b"),
                (6, "https://example.com:8443/c"),
                (2, "https://other.test/"),
            ],
        );
        kept_open(&store, &store.release(&workspace));

        assert_eq!(
            claimed(&store, "browser_other", "https://example.com/a"),
            None
        );
        assert_eq!(claimed(&store, TEST_BROWSER, "file:///C:/a"), None);
        assert_eq!(
            store.claim_released(TEST_BROWSER, &key_of(TEST_CLIENT), "https://example.com/a"),
            Some(super::ReleasedTab {
                physical_id: 9,
                url: super::UrlDigest::of("https://example.com/a"),
            }),
            "an exact URL wins over a lower id"
        );
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://example.com/a"),
            Some(4)
        );
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://example.com/a"),
            None,
            "a different port is a different site"
        );
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://example.com:8443/"),
            Some(6)
        );
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://OTHER.test/x"),
            Some(2)
        );
    }

    // a workspace whose claim names no agent leaves no tab for reuse.
    {
        let store = WorkspaceStore::default();
        let unnamed =
            client_workspace_with_tabs(&store, "", TEST_BROWSER, &[(3, "https://example.com/")]);
        assert_eq!(store.opener(&unnamed).unwrap().reuse_key, None);
        let released = store.release(&unnamed);
        assert_eq!(
            released.physical_ids,
            vec![3],
            "its tabs are still released"
        );
        kept_open(&store, &released);
        assert!(
            store.lock().released.is_empty(),
            "a tab no one may reuse is never pooled"
        );
    }

    // a released tab is claimed only by a session of the client that released it.
    {
        const SCRIPTS: &str = "ghostlight call";
        const AGENT: &str = "Claude Code";
        let store = WorkspaceStore::default();
        let scripts = client_workspace_with_tabs(
            &store,
            SCRIPTS,
            TEST_BROWSER,
            &[(3, "https://example.com/")],
        );
        kept_open(&store, &store.release(&scripts));
        let agent = client_workspace_with_tabs(
            &store,
            AGENT,
            TEST_BROWSER,
            &[(7, "https://example.com/other")],
        );
        kept_open(&store, &store.release(&agent));

        // An exact URL and a lower id do not make tab 3 the agent's: the agent gets the tab its
        // own sessions kept.
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, &key_of(AGENT), "https://example.com/"),
            Some(7)
        );
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, &key_of(AGENT), "https://example.com/"),
            None,
            "another client's tab is never the agent's to reuse"
        );
        // Asking left the scripts' tab in the pool for their next session.
        assert_eq!(
            claimed_by(
                &store,
                TEST_BROWSER,
                &key_of(SCRIPTS),
                "https://example.com/"
            ),
            Some(3)
        );
    }

    // released tabs leave the pool when closed committed or taken.
    {
        const SHELL: &str = "shell";
        let store = WorkspaceStore::default();
        let owned = store.resume_or_admit(SHELL.into(), IntakeChannel::Cli, marker(4312, 100));
        let lease = store.acquire(&owned).unwrap();
        for id in [1, 2, 3, 4, 5] {
            let tab = lease
                .add_tab(TEST_BROWSER, &physical(id, "https://example.com/"))
                .unwrap();
            lease
                .apply_landing(&tab.handle, &physical(id, "https://example.com/"))
                .unwrap();
        }
        // A held tab sits on a refused destination and is never offered.
        let held = lease
            .tabs()
            .unwrap()
            .into_iter()
            .find(|tab| tab.physical_id == 5)
            .unwrap();
        lease
            .hold_tab(
                &held.handle,
                TabHold {
                    denial: crate::governance::Decision::refused(
                        crate::governance::ReasonCode::HostDenied,
                    ),
                    authority: "authority_test".into(),
                    requirements: crate::governance::CapabilitySet::READ,
                    url: "https://elsewhere.test/".into(),
                },
            )
            .unwrap();
        drop(lease);
        // The reaper releases the workspace of a caller that is gone, and the browser keeps every
        // tab open. The held tab still never joins the pool.
        let reaped = store.reap(&|_| false);
        assert_eq!(reaped.len(), 1);
        kept_open(&store, &reaped[0]);

        store.apply_browser_close(TEST_BROWSER, 1);
        store.note_browser_landing(TEST_BROWSER, 2, "https://example.com/next");
        let taker = workspace_with_tabs(&store, TEST_BROWSER, &[]);
        store
            .acquire(&taker)
            .unwrap()
            .add_tab(TEST_BROWSER, &physical(3, "https://example.com/"))
            .unwrap();
        let command_line = crate::agent::Agent::command_line().id;
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, &command_line, "https://example.com/"),
            Some(4)
        );
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, &command_line, "https://example.com/"),
            None
        );
    }

    // a released tab is reused after a reconnect only where its browser still shows it.
    {
        let url = "https://example.com/kept";
        let store = WorkspaceStore::default();
        let released = workspace_with_tabs(&store, TEST_BROWSER, &[(8, url), (9, url)]);
        kept_open(&store, &store.release(&released));
        store.apply_browser_disconnect(TEST_BROWSER, None);

        let restarted = [
            physical(8, "https://bank.example/the-persons-own"),
            physical(9, url),
        ];
        let first = store
            .claim_released(TEST_BROWSER, &key_of(TEST_CLIENT), url)
            .unwrap();
        let second = store
            .claim_released(TEST_BROWSER, &key_of(TEST_CLIENT), url)
            .unwrap();
        let (moved, kept) = if first.physical_id == 8 {
            (first, second)
        } else {
            (second, first)
        };
        assert_eq!(moved.confirmed_in(&restarted), None);
        assert_eq!(kept.confirmed_in(&restarted), Some(&restarted[1]));
    }

    // decisive close receipt tolerates an earlier async close event.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        store.apply_browser_close(TEST_BROWSER, 7);
        assert!(lease.forget_closed_tab(&tab.handle).is_ok());
        assert_eq!(lease.select_tab(None).unwrap_err(), WorkspaceError::NoTab);
    }

    // a tab the person closes leaves no snapshot or image.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let _ = tab_with_everything(&lease, 7);

        store.apply_browser_close(TEST_BROWSER, 7);

        let state = store.lock();
        let kept = &state.workspaces[&workspace];
        assert!(kept.tabs.is_empty());
        assert!(kept.snapshots.is_empty(), "no snapshot outlives its tab");
        assert!(kept.images.is_empty(), "no image outlives its tab");
        assert!(kept.views.is_empty());
        assert!(kept.targets.records.is_empty());
    }

    // a recreated handle never serves the closed pages capture or diff.
    {
        for closed_first in [false, true] {
            let store = WorkspaceStore::default();
            let workspace = admit_test(&store);
            let lease = store.acquire(&workspace).unwrap();
            let (old, image, view, target) = tab_with_everything(&lease, 7);
            if closed_first {
                store.apply_browser_close(TEST_BROWSER, 7);
            }

            let restored = lease
                .restore_tab(
                    old.handle.as_str(),
                    TEST_BROWSER,
                    &physical(9, "about:blank"),
                )
                .unwrap();
            assert_eq!(restored.generation, 0);
            let landed = lease
                .apply_landing(&restored.handle, &physical(9, "https://example.com/new"))
                .unwrap();
            assert_eq!(landed.generation, old.generation, "{closed_first}");

            assert_eq!(
                lease.previous_snapshot(&landed, &whole_document()),
                None,
                "the closed page's tree is no diff base ({closed_first})"
            );
            assert_eq!(lease.take_image(image.as_str(), &landed), None);
            assert_eq!(
                lease
                    .resolve_view_point(view.as_str(), None, 1.0, 1.0)
                    .unwrap_err(),
                WorkspaceError::StaleView
            );
            assert_eq!(
                lease.resolve_target(target.as_str(), None).unwrap_err(),
                WorkspaceError::StaleTarget {
                    reason: StaleReason::TabClosed
                }
            );
        }
    }

    // snapshots supersede per tab go stale on commit and die with the tab.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        let whole = super::SnapshotScope {
            root: None,
            max_depth: 6,
        };
        let _first = lease
            .register_snapshot(&tab, whole.clone(), serde_json::json!({"n":1}))
            .unwrap();
        assert_eq!(
            lease.previous_snapshot(&tab, &whole),
            Some(serde_json::json!({"n":1})),
            "the just-registered tree is the current one"
        );
        let _second = lease
            .register_snapshot(&tab, whole.clone(), serde_json::json!({"n":2}))
            .unwrap();
        assert_eq!(
            lease.previous_snapshot(&tab, &whole),
            Some(serde_json::json!({"n":2})),
            "a new snapshot supersedes the earlier one per tab"
        );
        let shallower = super::SnapshotScope {
            root: None,
            max_depth: 4,
        };
        let rooted = super::SnapshotScope {
            root: Some("target_one".into()),
            max_depth: 6,
        };
        assert_eq!(
            lease.previous_snapshot(&tab, &shallower),
            None,
            "a tree of another depth is not comparable"
        );
        assert_eq!(
            lease.previous_snapshot(&tab, &rooted),
            None,
            "a tree of another root is not comparable"
        );
        let landed = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.com"))
            .unwrap();
        assert_eq!(
            lease.previous_snapshot(&landed, &whole),
            None,
            "commit stales snapshots"
        );
        assert!(lease
            .register_snapshot(&landed, whole.clone(), serde_json::json!({}))
            .is_ok());
        lease.forget_closed_tab(&landed.handle).unwrap();
        assert_eq!(
            lease.previous_snapshot(&landed, &whole),
            None,
            "closure removes snapshots"
        );
        let other = admit_test(&store);
        let other_lease = store.acquire(&other).unwrap();
        let stranger = other_lease
            .add_tab(TEST_BROWSER, &physical(9, "about:blank"))
            .unwrap();
        assert_eq!(
            other_lease.previous_snapshot(&stranger, &whole),
            None,
            "a foreign workspace never sees another workspace's snapshots"
        );
    }

    // captured images supersede respect the ceiling and expire.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        let first = lease
            .register_image(&tab, "image/jpeg", "AAAA", 3)
            .unwrap()
            .expect("under the ceiling");
        assert_eq!(
            lease.take_image(first.as_str(), &tab),
            Some(("image/jpeg".into(), "AAAA".into()))
        );
        let _second = lease
            .register_image(&tab, "image/jpeg", "BBBB", 3)
            .unwrap()
            .expect("superseding capture");
        assert_eq!(
            lease.take_image(first.as_str(), &tab),
            None,
            "a superseded asset is erased, not served"
        );
        let oversized = lease
            .register_image(&tab, "image/jpeg", "QAAA", 6_000_000)
            .unwrap();
        assert!(
            oversized.is_none(),
            "captures above the upload ceiling are refused"
        );
        let landed = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.com"))
            .unwrap();
        assert_eq!(
            lease.take_image(first.as_str(), &landed),
            None,
            "commit stales assets"
        );
    }
}

/// The test URL a remembered binding is kept at, and the person's page a restarted browser
/// shows under the same id.
const AGENT_URL: &str = "https://example.com/agent";
const PERSONS_URL: &str = "https://bank.example/the-persons-own";

/// A workspace in `TEST_BROWSER` with tab 7 landed at [`AGENT_URL`], after its browser's
/// stream ended. Returns the workspace and the tab's handle.
fn pending_workspace(store: &WorkspaceStore) -> (WorkspaceId, String) {
    let workspace = workspace_with_tabs(store, TEST_BROWSER, &[(7, AGENT_URL)]);
    let handle = store.tab_bindings(workspace.as_str())[0].handle.clone();
    store.apply_browser_disconnect(TEST_BROWSER, None);
    (workspace, handle)
}

/// ACCEPTANCE Contract 19 and ADR-0203 Decision 1: at the store, a binding across a gap is pending
/// and inert until its own browser confirms it.
///
/// A disconnect makes every binding pending and inert; a call selects the pending binding it names
/// or the only one, and omission selects only a sole tab; a crossing reads how its workspace holds
/// the tab. A confirmed binding keeps its handle and a moved one is forgotten; in its own life a
/// binding is confirmed by id, a routed page moves on and an unchanged one keeps its generation and
/// targets, while another life or none confirms only by address; a resumed binding remembers its
/// token's life. A new tab with a recycled id forgets the pending binding, a commit never confirms
/// one, a release after a restart closes none, a confirmation never takes another workspace's
/// tab, and binding a browser forgets another browser's pending tabs and says so. An address digest
/// reads the address bounded as the adapter bounds it, and one tab id in two browsers is two tabs.
#[test]
fn a_binding_across_a_gap_is_pending_and_inert_until_its_browser_confirms_it() {
    // a disconnect makes every binding pending and inert.
    {
        let store = WorkspaceStore::default();
        let (workspace, handle) = pending_workspace(&store);
        let bindings = store.tab_bindings(workspace.as_str());
        assert_eq!(bindings.len(), 1);
        assert!(bindings[0].pending);
        assert_eq!(bindings[0].browser, TEST_BROWSER);
        assert_eq!(bindings[0].url, super::UrlDigest::of(AGENT_URL));

        // Nothing selects it, lists it or counts it.
        let lease = store.acquire(&workspace).unwrap();
        assert_eq!(lease.pending_browsers(), vec![TEST_BROWSER.to_owned()]);
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest {
                tab: Some(&handle),
                ..Default::default()
            }),
            Some(TEST_BROWSER.into())
        );
        assert_eq!(
            lease.select_tab(Some(&handle)).unwrap_err(),
            WorkspaceError::StaleTab
        );
        assert_eq!(lease.select_tab(None).unwrap_err(), WorkspaceError::NoTab);
        assert!(lease.tabs().unwrap().is_empty());
        drop(lease);
        assert_eq!(store.summaries()[0].tab_count, 0);
        assert!(store.physical_tabs(workspace.as_str()).is_empty());

        // It takes no readiness or dialog event, and it is nobody's tab.
        store.apply_browser_dialog(TEST_BROWSER, 7, Some(DialogKind::Other));
        assert_eq!(store.dialog_open(&workspace, 7), None);
        assert_eq!(store.owner_of_physical(TEST_BROWSER, 7), None);
        let other = workspace_with_tabs(&store, TEST_BROWSER, &[]);
        assert!(store
            .acquire(&other)
            .unwrap()
            .add_tab(TEST_BROWSER, &physical(9, AGENT_URL))
            .is_ok());

        // The person's close forgets it.
        store.apply_browser_close(TEST_BROWSER, 7);
        assert!(store.tab_bindings(workspace.as_str()).is_empty());
    }

    // a call selects the pending binding it names or the only one.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(7, AGENT_URL))
            .unwrap();
        let target = lease
            .register_targets(&tab, &[control("#email", "Email")])
            .unwrap()[0]
            .0
            .clone();
        drop(lease);
        store.apply_browser_disconnect(TEST_BROWSER, None);
        let lease = store.acquire(&workspace).unwrap();
        let away = Some(TEST_BROWSER.to_owned());

        // A call that names nothing selects the only binding, and a target names its tab.
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest::default()),
            away
        );
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest {
                targets: vec!["target_unknown", target.as_str()],
                ..Default::default()
            }),
            away
        );

        // With a confirmed tab beside it, or pending tabs in two browsers, a call that names
        // nothing selects no pending one.
        store.remember_tab(
            &workspace,
            "tab_away",
            "browser_away",
            9,
            super::UrlDigest::of(AGENT_URL),
            None,
        );
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest::default()),
            None
        );
        store
            .confirm(&workspace, TEST_BROWSER, None, &[physical(7, AGENT_URL)])
            .unwrap();
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest::default()),
            None
        );
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest {
                tab: Some("tab_away"),
                ..Default::default()
            }),
            Some("browser_away".into()),
            "a named tab selects its binding wherever it is"
        );
    }

    // omission selects only a sole tab.
    {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        assert_eq!(lease.select_tab(None).unwrap_err(), WorkspaceError::NoTab);
        let first = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        assert_eq!(lease.select_tab(None).unwrap().handle, first.handle);
        let second = lease
            .add_tab(TEST_BROWSER, &physical(2, "about:blank"))
            .unwrap();
        assert_eq!(
            lease.select_tab(None).unwrap_err(),
            WorkspaceError::AmbiguousTab,
            "the newest of several tabs is not a default"
        );
        assert_eq!(
            lease
                .select_tab(Some(second.handle.as_str()))
                .unwrap()
                .handle,
            second.handle
        );
        lease.forget_closed_tab(&second.handle).unwrap();
        assert_eq!(lease.select_tab(None).unwrap().handle, first.handle);
    }

    // a crossing reads how its workspace holds the tab it names.
    {
        let store = WorkspaceStore::default();
        let (workspace, _) = pending_workspace(&store);
        let state = |id| store.binding_state(workspace.as_str(), TEST_BROWSER, id);
        assert_eq!(state(7), BindingState::Pending);
        assert_eq!(state(8), BindingState::Unbound);
        store
            .confirm(&workspace, TEST_BROWSER, None, &[physical(7, AGENT_URL)])
            .unwrap();
        assert_eq!(state(7), BindingState::Confirmed);
        assert_eq!(
            store.binding_state(workspace.as_str(), "browser_elsewhere", 7),
            BindingState::Unbound
        );
    }

    // a confirmed binding keeps its handle and a moved one is forgotten.
    {
        let store = WorkspaceStore::default();
        let workspace = workspace_with_tabs(
            &store,
            TEST_BROWSER,
            &[(7, AGENT_URL), (8, "https://example.com/second")],
        );
        let before = store.tab_bindings(workspace.as_str());
        store.apply_browser_disconnect(TEST_BROWSER, None);
        let lease = store.acquire(&workspace).unwrap();
        let mut live = physical(7, AGENT_URL);
        live.title = "Live title".into();
        store
            .confirm(
                &workspace,
                TEST_BROWSER,
                None,
                &[live, physical(8, PERSONS_URL)],
            )
            .unwrap();

        let tabs = lease.tabs().unwrap();
        assert_eq!(tabs.len(), 1);
        let kept = before.iter().find(|tab| tab.physical_id == 7).unwrap();
        assert_eq!(tabs[0].handle.as_str(), kept.handle);
        assert_eq!(tabs[0].title, "Live title");
        assert_eq!(tabs[0].generation, 1, "the landed generation is kept");
        let moved = before.iter().find(|tab| tab.physical_id == 8).unwrap();
        assert_eq!(
            lease.select_tab(Some(&moved.handle)).unwrap_err(),
            WorkspaceError::StaleTab
        );
        assert!(lease.pending_browsers().is_empty());
    }

    // in its own life a pending binding is confirmed by id and a routed page moves on.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(7, AGENT_URL))
            .unwrap();
        let target = lease
            .register_targets(&tab, &[control("#email", "Email")])
            .unwrap()[0]
            .0
            .clone();
        drop(lease);
        store.apply_browser_disconnect(TEST_BROWSER, Some(LIFE));
        let bindings = store.tab_bindings(workspace.as_str());
        assert_eq!(bindings[0].life.as_deref(), Some(LIFE));

        let lease = store.acquire(&workspace).unwrap();
        let mut live = physical(7, ROUTED_URL);
        live.title = "Routed".into();
        store
            .confirm(&workspace, TEST_BROWSER, Some(LIFE), &[live])
            .unwrap();
        let tabs = lease.tabs().unwrap();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].handle, tab.handle);
        assert_eq!(tabs[0].url, ROUTED_URL, "the binding takes the live URL");
        assert_eq!(tabs[0].title, "Routed");
        assert_eq!(tabs[0].generation, tab.generation + 1);
        assert_eq!(
            lease.resolve_target(target.as_str(), None).unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::Navigated
            },
            "no target of the earlier page answers for the routed one"
        );
        let bindings = store.tab_bindings(workspace.as_str());
        assert!(!bindings[0].pending);
        assert_eq!(bindings[0].life, None);
        assert_eq!(bindings[0].url, super::UrlDigest::of(ROUTED_URL));
    }

    // in its own life an unchanged page keeps its generation and targets.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(7, AGENT_URL))
            .unwrap();
        let target = lease
            .register_targets(&tab, &[control("#email", "Email")])
            .unwrap()[0]
            .0
            .clone();
        drop(lease);
        store.apply_browser_disconnect(TEST_BROWSER, Some(LIFE));
        let lease = store.acquire(&workspace).unwrap();
        store
            .confirm(
                &workspace,
                TEST_BROWSER,
                Some(LIFE),
                &[physical(7, AGENT_URL)],
            )
            .unwrap();
        assert_eq!(lease.tabs().unwrap()[0].generation, tab.generation);
        assert!(lease.resolve_target(target.as_str(), None).is_ok());
    }

    // another life or no life confirms only by url.
    {
        // (the life the binding remembers, the life the browser reports now)
        for (remembered, now) in [
            (Some(LIFE), Some(NEXT_LIFE)),
            (Some(LIFE), None),
            (None, Some(LIFE)),
            (None, None),
        ] {
            let store = WorkspaceStore::default();
            let workspace = workspace_with_tabs(&store, TEST_BROWSER, &[(7, AGENT_URL)]);
            store.apply_browser_disconnect(TEST_BROWSER, remembered);
            store
                .confirm(&workspace, TEST_BROWSER, now, &[physical(7, ROUTED_URL)])
                .unwrap();
            assert!(
                store.tab_bindings(workspace.as_str()).is_empty(),
                "{remembered:?} then {now:?}: a routed page is not the tab the binding knew"
            );

            let workspace = workspace_with_tabs(&store, TEST_BROWSER, &[(8, AGENT_URL)]);
            store.apply_browser_disconnect(TEST_BROWSER, remembered);
            store
                .confirm(&workspace, TEST_BROWSER, now, &[physical(8, AGENT_URL)])
                .unwrap();
            let bindings = store.tab_bindings(workspace.as_str());
            assert_eq!(bindings.len(), 1, "{remembered:?} then {now:?}");
            assert!(!bindings[0].pending, "{remembered:?} then {now:?}");
        }
    }

    // a resumed binding remembers the life its token names.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        store.remember_tab(
            &workspace,
            "tab_kept",
            TEST_BROWSER,
            7,
            super::UrlDigest::of(AGENT_URL),
            Some(LIFE),
        );
        assert_eq!(
            store.tab_bindings(workspace.as_str())[0].life.as_deref(),
            Some(LIFE)
        );
        store
            .confirm(
                &workspace,
                TEST_BROWSER,
                Some(LIFE),
                &[physical(7, ROUTED_URL)],
            )
            .unwrap();
        let bindings = store.tab_bindings(workspace.as_str());
        assert_eq!(bindings.len(), 1);
        assert!(!bindings[0].pending);
        assert_eq!(bindings[0].url, super::UrlDigest::of(ROUTED_URL));
        assert_eq!(
            store.browser_of(workspace.as_str()),
            Some(TEST_BROWSER.into())
        );
    }

    // a new tab with a recycled id forgets the pending binding.
    {
        let store = WorkspaceStore::default();
        let (workspace, _) = pending_workspace(&store);
        // The restarted browser opened a fresh tab under the old id: the id is proved stale even
        // where the new tab shows the same address.
        let opener = workspace_with_tabs(&store, TEST_BROWSER, &[]);
        store
            .acquire(&opener)
            .unwrap()
            .add_tab(TEST_BROWSER, &physical(7, AGENT_URL))
            .unwrap();
        assert!(store.tab_bindings(workspace.as_str()).is_empty());
        let lease = store.acquire(&workspace).unwrap();
        store
            .confirm(&workspace, TEST_BROWSER, None, &[physical(7, AGENT_URL)])
            .unwrap();
        assert!(lease.tabs().unwrap().is_empty());
    }

    // a commit never confirms a pending binding.
    {
        let store = WorkspaceStore::default();
        let (workspace, _) = pending_workspace(&store);
        // The person's own page commits in the tab that took the old id.
        assert_eq!(
            store.note_browser_landing(TEST_BROWSER, 7, PERSONS_URL),
            None
        );
        let bindings = store.tab_bindings(workspace.as_str());
        assert!(bindings[0].pending);
        assert_eq!(bindings[0].url, super::UrlDigest::of(AGENT_URL));
        let lease = store.acquire(&workspace).unwrap();
        store
            .confirm(&workspace, TEST_BROWSER, None, &[physical(7, PERSONS_URL)])
            .unwrap();
        assert!(lease.tabs().unwrap().is_empty());
        assert!(store.tab_bindings(workspace.as_str()).is_empty());
    }

    // a release after a browser restart closes no pending tab.
    {
        let store = WorkspaceStore::default();
        let (workspace, _) = pending_workspace(&store);
        let released = store.release(&workspace);
        assert_eq!(released.browser.as_deref(), Some(TEST_BROWSER));
        assert!(released.physical_ids.is_empty());
        assert_eq!(claimed(&store, TEST_BROWSER, AGENT_URL), None);
    }

    // a confirmation never takes a tab another workspace controls.
    {
        let store = WorkspaceStore::default();
        // Another session opened the tab while this one's binding waited in a resume token.
        let other = workspace_with_tabs(&store, TEST_BROWSER, &[(7, AGENT_URL)]);
        let workspace = admit_test(&store);
        store.remember_tab(
            &workspace,
            "tab_kept",
            TEST_BROWSER,
            7,
            super::UrlDigest::of(AGENT_URL),
            None,
        );
        let lease = store.acquire(&workspace).unwrap();
        store
            .confirm(&workspace, TEST_BROWSER, None, &[physical(7, AGENT_URL)])
            .unwrap();
        assert!(lease.tabs().unwrap().is_empty());
        assert!(store.tab_bindings(workspace.as_str()).is_empty());
        assert_eq!(store.tab_bindings(other.as_str()).len(), 1);
    }

    // binding a browser forgets another browsers pending tabs and says so.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        store.remember_tab(
            &workspace,
            "tab_away",
            "browser_away",
            7,
            super::UrlDigest::of(AGENT_URL),
            None,
        );
        let lease = store.acquire(&workspace).unwrap();
        lease
            .add_tab(TEST_BROWSER, &physical(3, "https://example.com/"))
            .unwrap();
        assert_eq!(
            lease
                .take_forgotten()
                .iter()
                .map(super::TabHandle::as_str)
                .collect::<Vec<_>>(),
            vec!["tab_away"]
        );
        assert!(lease.take_forgotten().is_empty(), "reported once");
        assert!(lease.pending_browsers().is_empty());
        assert_eq!(
            store.browser_of(workspace.as_str()).as_deref(),
            Some(TEST_BROWSER)
        );
    }

    // a remembered tab is confirmed by its own browser and binds it.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        store.remember_tab(
            &workspace,
            "tab_kept",
            TEST_BROWSER,
            7,
            super::UrlDigest::of(AGENT_URL),
            None,
        );
        assert_eq!(store.browser_of(workspace.as_str()), None);
        let lease = store.acquire(&workspace).unwrap();
        assert_eq!(lease.forget_pending("browser_elsewhere"), Ok(()));
        store
            .confirm(&workspace, TEST_BROWSER, None, &[physical(7, AGENT_URL)])
            .unwrap();
        let tab = lease.select_tab(Some("tab_kept")).unwrap();
        assert_eq!(tab.physical_id, 7);
        assert_eq!(tab.url, AGENT_URL);
        assert_eq!(
            store.browser_of(workspace.as_str()).as_deref(),
            Some(TEST_BROWSER)
        );
    }

    // a url digest reads the url bounded as the adapter bounds it.
    {
        let long = format!("https://example.com/{}", "a".repeat(super::TAB_URL_CHARS));
        let bounded: String = long.chars().take(super::TAB_URL_CHARS).collect();
        assert_eq!(super::UrlDigest::of(&long), super::UrlDigest::of(&bounded));
        assert_ne!(
            super::UrlDigest::of(AGENT_URL),
            super::UrlDigest::of(PERSONS_URL)
        );
        let digest = super::UrlDigest::of(AGENT_URL);
        assert_eq!(super::UrlDigest::parse(digest.as_str()), Some(digest));
        assert_eq!(super::UrlDigest::parse(AGENT_URL), None);
        assert_eq!(super::UrlDigest::parse(""), None);

        let vocabulary: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/golden/protocol4/vocabulary.json"
        ))
        .unwrap();
        assert_eq!(
            vocabulary["limits"]["url_chars"],
            serde_json::json!(super::TAB_URL_CHARS),
            "the digest bounds a URL as the adapter does"
        );
        assert_eq!(
            vocabulary["limits"]["title_chars"],
            serde_json::json!(super::TAB_TITLE_CHARS)
        );
    }

    // the same tab id in two browsers is two tabs.
    {
        let store = WorkspaceStore::default();
        let _chrome = workspace_with_tabs(&store, "browser_chrome", &[(5, "https://a.test/")]);
        let edge = workspace_with_tabs(&store, "browser_edge", &[]);
        let edge_lease = store.acquire(&edge).unwrap();
        assert!(edge_lease
            .add_tab("browser_edge", &physical(5, "https://b.test/"))
            .is_ok());
        drop(edge_lease);

        let second_chrome = workspace_with_tabs(&store, "browser_chrome", &[]);
        let lease = store.acquire(&second_chrome).unwrap();
        assert_eq!(
            lease
                .add_tab("browser_chrome", &physical(5, "https://a.test/"))
                .unwrap_err(),
            WorkspaceError::PhysicalTabOwned
        );
        assert_eq!(
            lease
                .restore_tab(
                    "tab_restored_too",
                    "browser_chrome",
                    &physical(5, "https://a.test/")
                )
                .unwrap_err(),
            WorkspaceError::PhysicalTabOwned
        );
    }
}

/// The extension life a test browser's tabs were confirmed in, and the life after it.
const LIFE: &str = "life_first";
const NEXT_LIFE: &str = "life_next";

/// The address a single-page app routed the agent's tab to without a navigation the service
/// saw.
const ROUTED_URL: &str = "https://example.com/agent/routed";

/// d80b64c3: a control keeps one handle and one record per document, the book stays bounded, and
/// every handle has the decoded shape.
///
/// A same-document change keeps targets and expires views; a new generation mints new handles and
/// forgets the old; the record limit evicts earlier generations first; a retired handle refuses
/// with the word that says why; recovered controls get handles on the stale handle's tab, a removed
/// control read again revives its handle, and a re-render the page proves keeps one handle and one
/// record; tombstones are bounded; handles are short base32, unique and owned, and expire on
/// commit; page-authored roles are narrowed before they are stored; screenshot views map
/// coordinates and expire on commit; a dialog is remembered for its own tab until it closes.
#[test]
fn a_control_keeps_one_handle_and_one_record_per_document() {
    // a same document change keeps targets and expires views.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.com/"))
            .unwrap();
        let targets = lease
            .register_targets(
                &tab,
                &[ObservedTarget {
                    locator: "l1".into(),
                    role: "button".into(),
                    name: "Go".into(),
                    state: vec![],
                    credential_class: false,
                    ..Default::default()
                }],
            )
            .unwrap();
        let geometry = ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 0.0,
            page_y: 0.0,
            css_width: 800.0,
            css_height: 600.0,
            visual_page_x: 0.0,
            visual_page_y: 0.0,
            visual_css_width: 800.0,
            visual_css_height: 600.0,
            device_scale: 1.0,
            zoom: 1.0,
            output_scale: 1.0,
        };
        let view = lease.register_view(&tab, geometry, 800, 600).unwrap();

        let routed = lease
            .apply_same_document(&tab.handle, &physical(7, "https://example.com/#next"))
            .unwrap();
        assert_eq!(routed.generation, tab.generation);
        assert_eq!(routed.url, "https://example.com/#next");
        assert!(lease.resolve_target(targets[0].0.as_str(), None).is_ok());
        assert_eq!(
            lease
                .resolve_view_point(view.as_str(), None, 1.0, 1.0)
                .unwrap_err(),
            WorkspaceError::StaleView
        );
        assert_eq!(
            lease
                .apply_same_document(&tab.handle, &physical(8, "https://example.com/"))
                .unwrap_err(),
            WorkspaceError::StaleTab
        );
    }

    // a control keeps its handle within a document.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let first = lease
            .register_targets(&tab, &[control("#email", "Email")])
            .unwrap();
        let second = lease
            .register_targets(
                &tab,
                &[
                    control("#email", "Work email"),
                    control("#email", "Work email"),
                ],
            )
            .unwrap();
        assert_eq!(first[0].0, second[0].0);
        assert_eq!(second[0].0, second[1].0);
        let context = store
            .stale_target_context(&workspace, first[0].0.as_str())
            .expect("the record is kept");
        assert_eq!(context.name, "Work email");
        assert_eq!(store.lock().workspaces[&workspace].targets.records.len(), 1);
        assert!(lease.resolve_target(first[0].0.as_str(), None).is_ok());
    }

    // a new generation mints new handles and old ones are forgotten.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let mut current = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/0"))
            .unwrap();
        let mut handles = Vec::new();
        for generation in 0..4 {
            let registered = lease
                .register_targets(&current, &[control("#email", "Email")])
                .unwrap();
            handles.push(registered[0].0.clone());
            if generation < 3 {
                current = lease
                    .apply_landing(
                        &tab.handle,
                        &physical(1, &format!("https://example.com/{}", generation + 1)),
                    )
                    .unwrap();
            }
        }
        let distinct: std::collections::HashSet<_> = handles.iter().collect();
        assert_eq!(distinct.len(), 4);
        // Three advances later the first generation's record is gone; the two before the
        // current one are still known.
        assert_eq!(store.lock().workspaces[&workspace].targets.records.len(), 3);
        assert!(store
            .stale_target_context(&workspace, handles[0].as_str())
            .is_none());
        assert!(store
            .stale_target_context(&workspace, handles[1].as_str())
            .is_some());
        // The forgotten handle is told apart from the two kept: both left the document.
        for old in &handles[..3] {
            assert_eq!(
                lease.resolve_target(old.as_str(), None).unwrap_err(),
                WorkspaceError::StaleTarget {
                    reason: StaleReason::Navigated
                }
            );
        }
        assert!(lease.resolve_target(handles[3].as_str(), None).is_ok());
    }

    // the record limit evicts earlier generations before the current one.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let first = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/1"))
            .unwrap();
        let earlier = lease.register_targets(&first, &controls(1500)).unwrap();
        let second = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/2"))
            .unwrap();
        let current = lease.register_targets(&second, &controls(1500)).unwrap();
        let records =
            |store: &WorkspaceStore| store.lock().workspaces[&workspace].targets.records.len();
        assert_eq!(records(&store), super::targets::TARGET_RECORD_LIMIT);
        assert!(current
            .iter()
            .all(|(handle, _)| lease.resolve_target(handle.as_str(), None).is_ok()));
        let survivors = earlier
            .iter()
            .filter(|(handle, _)| {
                store
                    .stale_target_context(&workspace, handle.as_str())
                    .is_some()
            })
            .count();
        assert_eq!(survivors, super::targets::TARGET_RECORD_LIMIT - 1500);
        let index_len = store.lock().workspaces[&workspace].targets.index.len();
        assert_eq!(index_len, super::targets::TARGET_RECORD_LIMIT);

        let third = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/3"))
            .unwrap();
        let crowded = lease.register_targets(&third, &controls(2500)).unwrap();
        assert_eq!(records(&store), super::targets::TARGET_RECORD_LIMIT);
        let dropped = 2500 - super::targets::TARGET_RECORD_LIMIT;
        for (position, (handle, _)) in crowded.iter().enumerate() {
            let resolved = lease.resolve_target(handle.as_str(), None);
            if position < dropped {
                assert_eq!(
                    resolved.unwrap_err(),
                    WorkspaceError::StaleTarget {
                        reason: StaleReason::Unknown
                    }
                );
            } else {
                assert!(resolved.is_ok());
            }
        }
    }

    // a retired handle refuses with the word that says why.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let mut current = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/0"))
            .unwrap();
        let removed = lease
            .register_targets(&current, &[control("#gone", "Gone")])
            .unwrap()[0]
            .0
            .clone();
        store.mark_target_removed(&workspace, removed.as_str());
        let refused = |handle: &TargetHandle| lease.resolve_target(handle.as_str(), None);
        assert_eq!(
            refused(&removed).unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::Removed
            }
        );
        let context = store
            .stale_target_context(&workspace, removed.as_str())
            .expect("a removed control keeps its record for the recovery search");
        assert_eq!(context.name, "Gone");

        let navigated = lease
            .register_targets(&current, &[control("#old", "Old")])
            .unwrap()[0]
            .0
            .clone();
        for generation in 1..=3 {
            current = lease
                .apply_landing(
                    &tab.handle,
                    &physical(1, &format!("https://example.com/{generation}")),
                )
                .unwrap();
        }
        assert_eq!(
            refused(&navigated).unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::Navigated
            }
        );

        let closed = lease
            .register_targets(&current, &[control("#open", "Open")])
            .unwrap()[0]
            .0
            .clone();
        lease.forget_closed_tab(&tab.handle).unwrap();
        assert_eq!(
            refused(&closed).unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::TabClosed
            }
        );
        assert_eq!(
            lease
                .resolve_target("target_neverissued", None)
                .unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::Unknown
            }
        );
    }

    // recovered controls get handles on the stale handles tab.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let stale = lease
            .register_targets(&tab, &[control("#old", "Old")])
            .unwrap()[0]
            .0
            .clone();
        let found = [control("#new", "Old"), control("#other", "Old")];

        let same = store.register_recovered(&workspace, stale.as_str(), true, &found);

        assert_eq!(same.len(), 2);
        let again = store.register_recovered(&workspace, stale.as_str(), true, &found[..1]);
        assert_eq!(
            again[0].0, same[0].0,
            "a control keeps its handle in one document"
        );
        for (handle, _) in &same {
            assert!(lease.resolve_target(handle.as_str(), Some(&tab)).is_ok());
        }
        assert!(store
            .register_recovered(&workspace, "target_neverissued", true, &found)
            .is_empty());

        let moved = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/next"))
            .unwrap();
        assert!(
            store
                .register_recovered(&workspace, stale.as_str(), true, &found)
                .is_empty(),
            "controls of the old document are not registered on the new one"
        );
        let current = store.register_recovered(&workspace, stale.as_str(), false, &found);
        assert_eq!(current.len(), 2);
        assert!(lease
            .resolve_target(current[0].0.as_str(), Some(&moved))
            .is_ok());
    }

    // reading a removed control again revives its handle.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let handle = lease
            .register_targets(&tab, &[control("#save", "Save")])
            .unwrap()[0]
            .0
            .clone();
        store.mark_target_removed(&workspace, "target_neverissued");
        assert!(lease.resolve_target(handle.as_str(), None).is_ok());
        store.mark_target_removed(&workspace, handle.as_str());
        assert!(lease.resolve_target(handle.as_str(), None).is_err());
        let again = lease
            .register_targets(&tab, &[control("#save", "Save")])
            .unwrap()[0]
            .0
            .clone();
        assert_eq!(again, handle);
        assert!(lease.resolve_target(handle.as_str(), None).is_ok());
        let book = &store.lock().workspaces[&workspace].targets;
        assert!(book.retired.is_empty() && book.retired_order.is_empty());
    }

    // a re render the page proves keeps one handle and one record.
    {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let handle = lease
            .register_targets(&tab, &[control("#more", "More options")])
            .unwrap()[0]
            .0
            .clone();
        store.mark_target_removed(&workspace, handle.as_str());
        assert!(lease.resolve_target(handle.as_str(), None).is_err());

        for _ in 0..5 {
            let again = lease
                .register_targets(&tab, &[control("#more", "More options")])
                .unwrap()[0]
                .0
                .clone();
            assert_eq!(again, handle, "the same locator is the same handle");
        }

        assert!(
            lease.resolve_target(handle.as_str(), Some(&tab)).is_ok(),
            "the revived handle resolves"
        );
        let book = &store.lock().workspaces[&workspace].targets;
        assert_eq!(book.records.len(), 1);
        assert_eq!(book.index.len(), 1);
        assert!(book.retired.is_empty());
    }

    // tombstones are bounded and the oldest are dropped.
    {
        let mut book = TargetBook::default();
        let handle = |position: usize| TargetHandle(format!("target_{position}"));
        for position in 0..super::targets::TARGET_RECORD_LIMIT + 10 {
            book.retire(handle(position), StaleReason::Navigated);
        }
        assert_eq!(book.retired.len(), super::targets::TARGET_RECORD_LIMIT);
        assert_eq!(
            book.retired_order.len(),
            super::targets::TARGET_RECORD_LIMIT
        );
        assert_eq!(book.retired_reason(&handle(0)), None);
        assert_eq!(book.retired_reason(&handle(9)), None);
        assert_eq!(
            book.retired_reason(&handle(10)),
            Some(StaleReason::Navigated)
        );
        // The first reason a handle is given stays.
        book.retire(handle(10), StaleReason::Unknown);
        assert_eq!(
            book.retired_reason(&handle(10)),
            Some(StaleReason::Navigated)
        );
    }

    // handles are short base32 and unique within their workspace.
    {
        let shaped = |handle: &str, kind: HandleKind| kind.minted(handle);
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        assert!(
            shaped(tab.handle.as_str(), HandleKind::Tab),
            "{}",
            tab.handle.as_str()
        );
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let observed: Vec<ObservedTarget> = (0..200)
            .map(|index| ObservedTarget {
                locator: format!("locator_{index}"),
                role: "button".into(),
                name: format!("Button {index}"),
                state: vec![],
                credential_class: false,
                ..Default::default()
            })
            .collect();
        let targets = lease.register_targets(&tab, &observed).unwrap();
        let mut seen = std::collections::HashSet::new();
        for (handle, _) in &targets {
            assert!(
                shaped(handle.as_str(), HandleKind::Target),
                "{}",
                handle.as_str()
            );
            assert!(seen.insert(handle.as_str().to_owned()), "a handle repeated");
        }
        let view = lease
            .register_view(
                &tab,
                ViewportGeometry {
                    scope: CaptureScope::Viewport,
                    page_x: 0.0,
                    page_y: 0.0,
                    css_width: 800.0,
                    css_height: 600.0,
                    visual_page_x: 0.0,
                    visual_page_y: 0.0,
                    visual_css_width: 800.0,
                    visual_css_height: 600.0,
                    device_scale: 1.0,
                    zoom: 1.0,
                    output_scale: 1.0,
                },
                800,
                600,
            )
            .unwrap();
        assert!(shaped(view.as_str(), HandleKind::View), "{}", view.as_str());
        let snapshot = lease
            .register_snapshot(
                &tab,
                SnapshotScope {
                    root: None,
                    max_depth: 3,
                },
                serde_json::json!({}),
            )
            .unwrap();
        assert!(
            shaped(snapshot.as_str(), HandleKind::Snapshot),
            "{}",
            snapshot.as_str()
        );
        let image = lease
            .register_image(&tab, "image/png", "AAAA", 3)
            .unwrap()
            .expect("a small capture is kept");
        assert!(
            shaped(image.as_str(), HandleKind::Image),
            "{}",
            image.as_str()
        );
        assert_eq!(
            short_handle(HandleKind::Tab, |_| false).len(),
            HandleKind::Tab.prefix().len() + crate::language::handle::WORKSPACE_SUFFIX_CHARS
        );
    }

    // handles are owned and targets expire on commit.
    {
        let store = WorkspaceStore::default();
        let first = store.admit("first".into(), IntakeChannel::Mcp);
        let second = store.admit("second".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&first).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let targets = lease
            .register_targets(
                &tab,
                &[ObservedTarget {
                    locator: "l1".into(),
                    role: "button".into(),
                    name: "Go".into(),
                    state: vec![],
                    credential_class: false,
                    ..Default::default()
                }],
            )
            .unwrap();
        let handle = targets[0].0.as_str().to_owned();
        let resolved = lease.resolve_target(&handle, Some(&tab)).unwrap();
        assert_eq!(resolved.role, TargetRole::Button);
        let _new = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.org"))
            .unwrap();
        assert_eq!(
            lease.resolve_target(&handle, None).unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::Navigated
            }
        );
        drop(lease);
        let other = store.acquire(&second).unwrap();
        assert_eq!(
            other.select_tab(Some(tab.handle.as_str())).unwrap_err(),
            WorkspaceError::NotOwnedTab
        );
    }

    // page authored roles are narrowed before target state is stored.
    {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "https://example.com"))
            .unwrap();
        let targets = lease
            .register_targets(
                &tab,
                &[ObservedTarget {
                    locator: "l1".into(),
                    role: "Save my document".into(),
                    name: "untrusted name".into(),
                    state: vec![],
                    credential_class: false,
                    ..Default::default()
                }],
            )
            .unwrap();

        let resolved = lease
            .resolve_target(targets[0].0.as_str(), Some(&tab))
            .unwrap();
        assert_eq!(resolved.role, TargetRole::Control);
    }

    // screenshot views map coordinates and expire on commit.
    {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.com"))
            .unwrap();
        let geometry = ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 10.0,
            page_y: 20.0,
            css_width: 800.0,
            css_height: 600.0,
            visual_page_x: 10.0,
            visual_page_y: 20.0,
            visual_css_width: 800.0,
            visual_css_height: 600.0,
            device_scale: 2.0,
            zoom: 1.0,
            output_scale: 0.5,
        };
        let view = lease.register_view(&tab, geometry, 400, 300).unwrap();
        let (_, point) = lease
            .resolve_view_point(view.as_str(), Some(&tab), 100.0, 50.0)
            .unwrap();
        assert_eq!(point.x, 210.0);
        assert_eq!(point.y, 120.0);
        assert_eq!(
            lease
                .resolve_view_point(view.as_str(), Some(&tab), 401.0, 1.0)
                .unwrap_err(),
            WorkspaceError::OutsideImage {
                width: 400,
                height: 300
            }
        );
        let (_, region) = lease
            .resolve_view_region(view.as_str(), Some(&tab), 100.0, 50.0, 200.0, 100.0)
            .unwrap();
        assert_eq!(region.x, 210.0);
        assert_eq!(region.y, 120.0);
        assert_eq!(region.width, 400.0);
        assert_eq!(region.height, 200.0);
        assert_eq!(
            lease
                .resolve_view_region(view.as_str(), Some(&tab), 300.0, 0.0, 101.0, 10.0)
                .unwrap_err(),
            WorkspaceError::OutsideImage {
                width: 400,
                height: 300
            }
        );
        let magnified = lease
            .register_view(
                &tab,
                ViewportGeometry {
                    scope: CaptureScope::Region,
                    page_x: region.x,
                    page_y: region.y,
                    css_width: region.width,
                    css_height: region.height,
                    output_scale: 4.0,
                    ..geometry
                },
                1600,
                800,
            )
            .unwrap();
        assert_eq!(
            lease
                .resolve_view_region(view.as_str(), None, 0.0, 0.0, 1.0, 1.0)
                .unwrap_err(),
            WorkspaceError::StaleView
        );
        let (_, chained) = lease
            .resolve_view_region(magnified.as_str(), Some(&tab), 400.0, 200.0, 400.0, 200.0)
            .unwrap();
        assert_eq!(chained.x, 310.0);
        assert_eq!(chained.y, 170.0);
        assert_eq!(chained.width, 100.0);
        assert_eq!(chained.height, 50.0);
        let _ = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.org"))
            .unwrap();
        assert_eq!(
            lease
                .resolve_view_point(magnified.as_str(), None, 1.0, 1.0)
                .unwrap_err(),
            WorkspaceError::StaleView
        );
    }

    // a dialog is remembered for its own tab until it closes.
    {
        let store = WorkspaceStore::default();
        let workspace = workspace_with_tabs(
            &store,
            TEST_BROWSER,
            &[(1, "https://a.test/"), (2, "https://b.test/")],
        );
        assert_eq!(store.dialog_open(&workspace, 1), None);
        store.apply_browser_dialog(TEST_BROWSER, 1, Some(DialogKind::Prompt));
        assert_eq!(store.dialog_open(&workspace, 1), Some(DialogKind::Prompt));
        assert_eq!(store.dialog_open(&workspace, 2), None);
        store.apply_browser_dialog("another_browser", 2, Some(DialogKind::Alert));
        assert_eq!(store.dialog_open(&workspace, 2), None);
        store.apply_browser_dialog(TEST_BROWSER, 1, None);
        assert_eq!(store.dialog_open(&workspace, 1), None);
    }
}

fn control(locator: &str, name: &str) -> ObservedTarget {
    ObservedTarget {
        locator: locator.into(),
        role: "button".into(),
        name: name.into(),
        state: vec![],
        credential_class: false,
        ..Default::default()
    }
}

fn controls(count: usize) -> Vec<ObservedTarget> {
    (0..count)
        .map(|index| control(&format!("locator_{index}"), "Button"))
        .collect()
}

/// One landed tab carrying a snapshot, an image, a view and a target, with their handles.
fn tab_with_everything(
    lease: &super::WorkspaceLease,
    physical_id: u64,
) -> (
    super::SelectedTab,
    super::ImageHandle,
    super::ViewHandle,
    TargetHandle,
) {
    let tab = lease
        .add_tab(TEST_BROWSER, &physical(physical_id, "about:blank"))
        .unwrap();
    let tab = lease
        .apply_landing(
            &tab.handle,
            &physical(physical_id, "https://example.com/old"),
        )
        .unwrap();
    lease
        .register_snapshot(&tab, whole_document(), serde_json::json!({"old":true}))
        .unwrap();
    let image = lease
        .register_image(&tab, "image/jpeg", "AAAA", 3)
        .unwrap()
        .expect("under the ceiling");
    let view = lease.register_view(&tab, viewport(), 800, 600).unwrap();
    let target = lease
        .register_targets(&tab, &[control("#old", "Old")])
        .unwrap()[0]
        .0
        .clone();
    (tab, image, view, target)
}

fn whole_document() -> SnapshotScope {
    SnapshotScope {
        root: None,
        max_depth: 6,
    }
}

fn viewport() -> ViewportGeometry {
    ViewportGeometry {
        scope: CaptureScope::Viewport,
        page_x: 0.0,
        page_y: 0.0,
        css_width: 800.0,
        css_height: 600.0,
        visual_page_x: 0.0,
        visual_page_y: 0.0,
        visual_css_width: 800.0,
        visual_css_height: 600.0,
        device_scale: 1.0,
        zoom: 1.0,
        output_scale: 1.0,
    }
}
