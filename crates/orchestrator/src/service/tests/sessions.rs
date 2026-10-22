//! MCP sessions: the opening, one set of tools per agent, resume tokens and resumed tabs, a
//! client that goes away, and the catalog watch.

use super::*;

/// The name the client of a session that returns with a resume token claims.
const RETURNING_CLIENT: &str = "returning client";

/// Serve the relay listener over a fake browser, reading each session's source with
/// `sources`.
fn serve_reading(home: &TestHome, sources: SourceReader) -> Served {
    serve(home, false, sources, None)
}

/// Serve the relay listener over a fake browser, reading each session's source with
/// `sources` and governing with the user policy at `policy`.
fn serve_governed(home: &TestHome, sources: SourceReader, policy: PathBuf) -> Served {
    serve(home, false, sources, Some(policy))
}

/// Write a user policy named `name` at `path` that allows the classes `allowed` on every host.
fn write_policy(path: &Path, name: &str, allowed: Value) {
    std::fs::write(
        path,
        serde_json::to_vec(&json!({
            "schema": 4,
            "name": name,
            "version": "1",
            "grants": [{"id": "all", "hosts": {"allow": ["*"]}, "allowed": allowed}]
        }))
        .unwrap(),
    )
    .unwrap();
}

/// The executable a held source read names.
const HELD_SOURCE: &str = "held.exe";

/// A source read that waits until the test releases it, and counts the reads that began.
#[derive(Default)]
struct HeldRead {
    released: std::sync::Mutex<bool>,
    changed: std::sync::Condvar,
    began: AtomicUsize,
}

impl HeldRead {
    /// A reader that waits on this hold, then names [`HELD_SOURCE`].
    fn reader(self: &Arc<Self>) -> SourceReader {
        let held = Arc::clone(self);
        SourceReader::with(move |_| {
            held.began.fetch_add(1, Ordering::SeqCst);
            let mut released = held.released.lock().unwrap();
            while !*released {
                released = held.changed.wait(released).unwrap();
            }
            Some(Observation {
                source: Source {
                    agent: None,
                    image: HELD_SOURCE.into(),
                },
                process: SourceProcess { id: 1, started: 1 },
            })
        })
    }

    /// How many reads have begun.
    fn began(&self) -> usize {
        self.began.load(Ordering::SeqCst)
    }

    /// Let every read finish.
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.changed.notify_all();
    }
}

/// Releases a hold when a test ends, however it ends, so no session thread stays blocked.
struct Release(Arc<HeldRead>);

impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// Now, in milliseconds since the Unix epoch, as attribution is dated.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap()
}

/// A resume token for a session that owned no tabs.
fn tabless_token() -> String {
    ResumeState {
        revision: REVISION.into(),
        claim: Some(RETURNING_CLIENT.into()),
        owned_tabs: Vec::new(),
        lives: BTreeMap::new(),
        standby: false,
    }
    .encode()
}

/// Call a tool on `socket` and return the accept time its audit line in `home` records.
fn recorded_accept_time(home: &TestHome, socket: &mut Connection) -> u64 {
    send_line(
        socket,
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {"name": "browser_tabs", "arguments": {"action": "list"}}
        }),
    );
    response(socket, 7);
    let audit = home.path().join("audit.jsonl");
    let accepted = || {
        let lines = std::fs::read_to_string(&audit).ok()?;
        lines.lines().find_map(|line| {
            serde_json::from_str::<Value>(line).ok()?["provenance"]["observed_at_ms"].as_u64()
        })
    };
    eventually(|| accepted().is_some());
    accepted().unwrap()
}

/// Defect 967d2147 (ADR-0202 Decision 3 as ADR-0203 amends it): the accept, not the source
/// read, is when a session begins, so the opening is answered before the read, which happens
/// once, and a fresh or resumed session is dated at its accept.
#[test]
fn an_opening_is_answered_before_the_source_is_read_and_the_session_is_dated_at_its_accept() {
    for resumed in [false, true] {
        let home = TestHome::new("accept");
        let held = Arc::new(HeldRead::default());
        let _release = Release(Arc::clone(&held));
        let served = serve_reading(&home, held.reader());
        let before = now_ms();

        // The opening is answered while the read is still held.
        let mut socket = open_mcp(resumed.then(tabless_token));
        eventually(|| held.began() == 1);
        assert!(
            served.context.workspaces.summaries().is_empty(),
            "the session opens only once its source is read"
        );

        // The accept came before the read began; the wall clock passes that moment before the
        // read ends, so a session dated by its read would read later than `released`.
        let read_began = now_ms();
        eventually(|| now_ms() > read_began);
        let released = now_ms();
        held.release();
        if !resumed {
            initialize(&mut socket);
        }
        let accepted = recorded_accept_time(&home, &mut socket);
        assert!(
            (before..released).contains(&accepted),
            "resumed {resumed}: accepted at {accepted}, dialed at {before}, read released at {released}"
        );
        assert_eq!(held.began(), 1, "the source is read once per connection");
    }
}

/// The catalog id and client name of the agent the toolset tests connect as.
const CLAUDE_CODE: &str = "claude-code";

/// A reader that answers each connection's read with the next of `observations`, in the order
/// the connections were accepted.
fn scripted(observations: Vec<Option<Observation>>) -> SourceReader {
    let script = Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(
        observations,
    )));
    SourceReader::with(move |_| script.lock().unwrap().pop_front().flatten())
}

/// A connection initialized as Claude Code: its instructions and the tools its first catalog
/// offers.
fn connect_as_claude_code() -> (Connection, Value, Vec<Value>) {
    let mut socket = open_mcp(None);
    let (initialized, _) = initialize_as(&mut socket, CLAUDE_CODE);
    let tools = list_tools(&mut socket, 2);
    (socket, initialized["result"]["instructions"].clone(), tools)
}

/// List the tools on `socket` with request `id`.
fn list_tools(socket: &mut Connection, id: u64) -> Vec<Value> {
    send_line(
        socket,
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/list"}),
    );
    let (listed, _) = response(socket, id);
    listed["result"]["tools"].as_array().unwrap().clone()
}

/// Defect L2-04 (4f52e834) and ADR-0203's amendment of 2026-10-19, items 1 to 3: connections
/// from one running app of the agent their clients claim share one set of tools, which passes
/// to the next when the holder ends; connections that are not one agent's each get it.
#[test]
fn one_agents_own_app_gets_one_set_of_tools_and_the_next_connection_takes_it_over() {
    {
        let home = TestHome::new("toolset-one-agent");
        let _served = serve_reading(
            &home,
            scripted(vec![running(CLAUDE_CODE, 40), running(CLAUDE_CODE, 40)]),
        );
        let (entry, instructions, offered) = connect_as_claude_code();
        assert!(!offered.is_empty(), "the first connection offers the tools");
        assert_eq!(instructions, crate::language::SERVER_INSTRUCTIONS);

        let (mut plugin, instructions, none) = connect_as_claude_code();
        assert!(
            none.is_empty(),
            "the same agent is offered no second set: {none:?}"
        );
        assert_eq!(instructions, crate::language::STANDBY_INSTRUCTIONS);

        // When the holder ends, the one that stood by offers the tools and says so.
        drop(entry);
        let tokens = until_list_changed(&mut plugin);
        let last = tokens.last().expect("the promotion writes a token");
        assert!(
            !ResumeState::decode(last).unwrap().standby,
            "a restart after the promotion resumes it offering the tools"
        );
        assert_eq!(
            list_tools(&mut plugin, 3),
            offered,
            "the agent keeps one working set of tools"
        );
    }

    let editor = Some(Observation {
        source: Source {
            agent: Some("vscode".into()),
            image: "code".into(),
        },
        process: SourceProcess { id: 40, started: 7 },
    });
    for (case, first, second) in [
        (
            "two agent processes",
            running(CLAUDE_CODE, 40),
            running(CLAUDE_CODE, 41),
        ),
        (
            "an editor whose terminal runs the agent",
            editor.clone(),
            editor,
        ),
        ("a source that could not be read", None, None),
    ] {
        let home = TestHome::new("toolset-two-agents");
        let _served = serve_reading(&home, scripted(vec![first, second]));
        let (_one, _, one) = connect_as_claude_code();
        let (_other, instructions, other) = connect_as_claude_code();
        assert!(!one.is_empty() && !other.is_empty(), "{case}");
        assert_eq!(other, one, "{case}");
        assert_eq!(instructions, crate::language::SERVER_INSTRUCTIONS, "{case}");
    }
}

/// Read frames on `socket` until it is told its tool list changed, and return the resume
/// tokens handed on the way.
fn until_list_changed(socket: &mut Connection) -> Vec<String> {
    let mut tokens = Vec::new();
    loop {
        match frame::read_frame::<McpFrame>(socket).unwrap().unwrap() {
            McpFrame::Message { line } => {
                let message: Value = serde_json::from_str(&line).unwrap();
                if message["method"] == "notifications/tools/list_changed" {
                    return tokens;
                }
            }
            McpFrame::Resume { token } => tokens.push(token),
            McpFrame::Goodbye { .. } | McpFrame::Unknown => {}
        }
    }
}

/// A resume token for a Claude Code session that owned no tabs and stood by when `standby`.
fn claude_code_token(standby: bool) -> String {
    ResumeState {
        revision: REVISION.into(),
        claim: Some(CLAUDE_CODE.into()),
        owned_tabs: Vec::new(),
        lives: BTreeMap::new(),
        standby,
    }
    .encode()
}

/// Defects d28bb1dc and L2-12 (0285ce2d), ADR-0203's amendment of 2026-10-19 item 5: after a
/// restart a resumed session keeps the toolset role its client saw, so the one that stood by
/// is never offered a second set and the one that offered the tools is told to list again.
#[test]
fn a_resumed_session_keeps_the_toolset_role_its_client_saw_and_a_holder_lists_again() {
    // A resumed holder's client listed its tools with the earlier service and does not list
    // them again on its own, so the new service tells it to.
    {
        let home = TestHome::new("resumed-relist");
        let _served = serve_fake(&home);
        let mut resumed = open_mcp(Some(claude_code_token(false)));
        until_list_changed(&mut resumed);
        assert!(!list_tools(&mut resumed, 2).is_empty());
    }

    // After a replace, the edge that stood by reconnects first and still stands by.
    let home = TestHome::new("toolset-resumed");
    let policy = home.path().join("policy.json");
    write_policy(
        &policy,
        "rules",
        json!(["read", "action", "write", "execute"]),
    );
    let served = serve_governed(
        &home,
        scripted(vec![running(CLAUDE_CODE, 40), running(CLAUDE_CODE, 40)]),
        policy.clone(),
    );
    let mut plugin = open_mcp(Some(claude_code_token(true)));
    assert!(next_resume(&mut plugin).standby);
    let mut entry = open_mcp(Some(claude_code_token(false)));
    assert!(!next_resume(&mut entry).standby);
    let offered = list_tools(&mut entry, 2);
    assert!(!offered.is_empty(), "the former holder offers the tools");
    assert!(list_tools(&mut plugin, 2).is_empty());

    // A policy change reaches the holder, and a re-list still offers no second set.
    write_policy(&policy, "narrower", json!(["read"]));
    served.context.governance.refresh_tick();
    until_list_changed(&mut entry);
    let narrowed = list_tools(&mut entry, 3);
    assert!(!narrowed.is_empty() && narrowed.len() < offered.len());
    assert!(
        list_tools(&mut plugin, 3).is_empty(),
        "the client that stood by is never offered a second set"
    );
}

/// The digest a resume token carries for a tab last known at `url`.
fn digest_of(url: &str) -> Option<String> {
    Some(crate::workspace::UrlDigest::of(url).as_str().to_owned())
}

/// The resume token that restores exactly `owned`: the first of `earlier`, the tokens read on
/// the way to a call's answer, that does, or else the next one on the stream that does. The
/// worker publishes after the answer and the session watch can publish during the call, so the
/// token can come on either side of the answer.
fn token_restoring(
    socket: &mut Connection,
    earlier: &[String],
    owned: &[crate::intake::mcp::OwnedTab],
) -> ResumeState {
    earlier
        .iter()
        .map(|token| ResumeState::decode(token).unwrap())
        .find(|state| state.owned_tabs == owned)
        .unwrap_or_else(|| loop {
            let state = next_resume(socket);
            if state.owned_tabs == owned {
                break state;
            }
        })
}

/// Send one `tools/call` and read its structured result.
fn call_tool(socket: &mut Connection, id: u64, name: &str, arguments: Value) -> Value {
    send_line(
        socket,
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": name, "arguments": arguments}}),
    );
    let (called, _) = response(socket, id);
    called["result"]["structuredContent"].clone()
}

/// A resume token for a session that owned `tab`.
fn token_with(tab: crate::intake::mcp::OwnedTab) -> String {
    token_in(tab, None)
}

/// A resume token for a session that owned `tab`, whose browser was in `life` when the token
/// was written, or named no life for `None`.
fn token_in(tab: crate::intake::mcp::OwnedTab, life: Option<&str>) -> String {
    ResumeState {
        revision: REVISION.into(),
        claim: Some(RETURNING_CLIENT.into()),
        lives: life
            .map(|life| BTreeMap::from([(tab.browser_id.clone(), life.to_owned())]))
            .unwrap_or_default(),
        owned_tabs: vec![tab],
        standby: false,
    }
    .encode()
}

/// A resume token for a session that owned the returning browser's tab.
fn returning_token() -> String {
    token_with(returning_tab())
}

fn returning_tab() -> crate::intake::mcp::OwnedTab {
    crate::intake::mcp::OwnedTab {
        browser_id: RETURNING_BROWSER.into(),
        tab_id: RETURNING_TAB,
        handle: "tab_kept".into(),
        url_digest: digest_of(RETURNING_URL),
    }
}

/// The pending tabs the one session holds in the workspace store, with their browsers.
fn pending_tabs(served: &Served) -> Vec<(String, String)> {
    let summaries = served.context.workspaces.summaries();
    served
        .context
        .workspaces
        .tab_bindings(&summaries[0].id)
        .into_iter()
        .filter(|binding| binding.pending)
        .map(|binding| (binding.handle, binding.browser))
        .collect()
}

/// The extension life the returning browser was in before the service was replaced.
const RETURNING_LIFE: &str = "life_returning";

/// The address the returning browser's single-page app routed its tab to meanwhile.
const ROUTED_URL: &str = "https://example.com/inbox";

/// ACCEPTANCE B26 and C10, ADR-0203 Decision 1 (slice C4, 1d10bf74; and a10882c5): a resumed
/// tab waits, kept in the token, until its own browser confirms it by id and its life or its
/// page; one its browser cannot confirm reads as no longer open, and the token follows.
#[test]
fn a_resumed_tab_is_confirmed_by_its_own_browser_before_its_first_use() {
    use crate::intake::mcp::OwnedTab;

    let succeeded = json!(Status::Succeeded);

    // The session resumes at once with its tab pending, asking no browser; a call that needs
    // no tab completes while the browser is away and forgets nothing; and the first call
    // after the browser returns confirms the tab and reads it, with no wait for the session to
    // notice the browser.
    {
        let home = TestHome::new("resume-late");
        let served = serve_relayed(&home);
        let mut socket = open_mcp(Some(returning_token()));
        let resumed = next_resume(&mut socket);
        assert_eq!(resumed.revision, REVISION);
        assert_eq!(resumed.claim.as_deref(), Some(RETURNING_CLIENT));
        assert_eq!(resumed.owned_tabs, vec![returning_tab()]);
        assert!(bound_tabs(&served).is_empty());
        assert_eq!(
            served.context.workspaces.summaries()[0]
                .opener
                .reported_application
                .as_deref(),
            Some(RETURNING_CLIENT)
        );

        let listed = call_tool(&mut socket, 2, "browser_tabs", json!({"action": "list"}));
        assert_eq!(listed["status"], succeeded, "{listed}");
        assert_eq!(
            pending_tabs(&served),
            vec![("tab_kept".to_owned(), RETURNING_BROWSER.to_owned())]
        );

        let _browser = attach_returning_browser(&served);
        let read = call_tool(&mut socket, 3, "browser_read", json!({"tab": "tab_kept"}));
        assert_eq!(read["status"], succeeded, "{read}");
        assert_eq!(
            bound_tabs(&served),
            vec![("tab_kept".to_owned(), RETURNING_TAB)]
        );
    }

    // A tab the browser no longer lists is forgotten and leaves the token, and confirming
    // reads only the browser's tab list: nothing reaches a tab.
    {
        let home = TestHome::new("resume-missing");
        let served = serve_relayed(&home);
        let gone = OwnedTab {
            browser_id: RETURNING_BROWSER.into(),
            tab_id: RETURNING_TAB + 1,
            handle: "tab_gone".into(),
            url_digest: digest_of("https://example.com/gone"),
        };
        let token = ResumeState {
            revision: REVISION.into(),
            claim: Some(RETURNING_CLIENT.into()),
            owned_tabs: vec![returning_tab(), gone],
            lives: BTreeMap::new(),
            standby: false,
        }
        .encode();
        let mut socket = open_mcp(Some(token));
        assert_eq!(next_resume(&mut socket).owned_tabs.len(), 2);
        let browser = attach_returning_browser(&served);
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {"name": "browser_tabs", "arguments": {"action": "list"}}}),
        );
        let (called, tokens) = response(&mut socket, 2);
        let listed = &called["result"]["structuredContent"];
        assert_eq!(listed["status"], succeeded, "{listed}");
        let tabs = listed["facts"]["tabs"].as_array().unwrap();
        assert_eq!(tabs.len(), 1, "{listed}");
        assert_eq!(tabs[0]["tab"], "tab_kept");
        token_restoring(&mut socket, &tokens, &[returning_tab()]);
        let seen = browser.wait(|seen| !requests_at(seen, "list_tabs").is_empty());
        let named_a_tab: Vec<&Value> = seen
            .frames
            .iter()
            .filter(|frame| {
                frame["kind"] == "request" && frame["primitive"]["args"].get("tab_id").is_some()
            })
            .collect();
        assert!(
            named_a_tab.is_empty(),
            "nothing reaches a tab: {named_a_tab:?}"
        );
    }

    // In the same browser life the tab is kept though its page routed itself, and the token
    // takes its live address and keeps the life.
    {
        let home = TestHome::new("resume-same-life");
        let served = serve_relayed(&home);
        let mut socket = open_mcp(Some(token_in(returning_tab(), Some(RETURNING_LIFE))));
        let resumed = next_resume(&mut socket);
        assert_eq!(
            resumed.lives.get(RETURNING_BROWSER).map(String::as_str),
            Some(RETURNING_LIFE),
            "a pending tab keeps the life its token named"
        );

        let _browser = attach_returning_browser_in(&served, Some(RETURNING_LIFE), ROUTED_URL);
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {"name": "browser_read", "arguments": {"tab": "tab_kept"}}}),
        );
        let (called, tokens) = response(&mut socket, 2);
        let read = &called["result"]["structuredContent"];
        assert_eq!(read["status"], succeeded, "{read}");
        assert_eq!(
            bound_tabs(&served),
            vec![("tab_kept".to_owned(), RETURNING_TAB)]
        );
        // The token the confirmation changed, published during the call or right after it.
        let token = tokens.last().map_or_else(
            || next_resume(&mut socket),
            |token| ResumeState::decode(token).unwrap(),
        );
        assert_eq!(token.owned_tabs.len(), 1);
        assert_eq!(
            token.owned_tabs[0].url_digest,
            digest_of(ROUTED_URL),
            "the confirmed tab is remembered at its live address"
        );
        assert_eq!(
            token.lives.get(RETURNING_BROWSER).map(String::as_str),
            Some(RETURNING_LIFE),
            "a confirmed tab is in the life its browser reports"
        );
    }

    // In a new browser life a tab at another address is forgotten.
    {
        let home = TestHome::new("resume-new-life");
        let served = serve_relayed(&home);
        let mut socket = open_mcp(Some(token_in(returning_tab(), Some(RETURNING_LIFE))));
        next_resume(&mut socket);
        let _browser = attach_returning_browser_in(&served, Some("life_restarted"), ROUTED_URL);
        let read = call_tool(&mut socket, 2, "browser_read", json!({"tab": "tab_kept"}));
        assert_ne!(read["status"], succeeded, "{read}");
        assert!(bound_tabs(&served).is_empty());
    }

    // A token written before digests resumes with its tabs stale, even where the browser
    // still shows a tab with that id at that address.
    {
        let home = TestHome::new("resume-no-digest");
        let served = serve_relayed(&home);
        let earlier = OwnedTab {
            url_digest: None,
            ..returning_tab()
        };
        let mut socket = open_mcp(Some(token_with(earlier)));
        assert!(next_resume(&mut socket).owned_tabs.is_empty());
        let _browser = attach_returning_browser(&served);
        let read = call_tool(&mut socket, 2, "browser_read", json!({"tab": "tab_kept"}));
        assert_ne!(read["status"], succeeded, "{read}");
        assert!(bound_tabs(&served).is_empty());
    }
}

/// Defect C11 (8c216920, finding E-06) and ADR-0203 Decision 1: the edge always holds a token
/// its session resumes by, since a token that cannot be read still resumes an initialized
/// session and every answered initialize comes with a token, even after a service end.
#[test]
fn every_session_can_be_resumed_by_the_token_its_edge_holds() {
    // A token that cannot be read resumes at the oldest revision with no tab, and the client,
    // which initialized with an earlier service, is served its next call.
    {
        let home = TestHome::new("unreadable-token");
        let served = serve_fake(&home);
        let mut socket = open_mcp(Some("rs1.***".into()));
        let resumed = next_resume(&mut socket);
        assert_eq!(resumed.revision, crate::intake::mcp::OLDEST_REVISION);
        assert_eq!(resumed.claim, None);
        assert!(resumed.owned_tabs.is_empty());
        let listed = call_tool(&mut socket, 2, "browser_tabs", json!({"action": "list"}));
        assert_eq!(listed["status"], json!(Status::Succeeded), "{listed}");
        assert_eq!(served.context.workspaces.summaries().len(), 1);
    }

    // A fresh session's token comes before its initialize answer, ahead of its workspace.
    {
        let home = TestHome::new("initialize-token-fresh");
        let _served = serve_fake(&home);
        let mut socket = open_mcp(None);
        let (initialized, tokens) = initialize(&mut socket);
        assert!(initialized["result"].is_object(), "{initialized}");
        let state =
            ResumeState::decode(tokens.first().expect("a token before the answer")).unwrap();
        assert_eq!(state.revision, REVISION);
        assert_eq!(state.claim.as_deref(), Some(CLIENT_NAME));
        assert!(state.owned_tabs.is_empty());
    }

    // So does one that initializes after a quit handed every stream its last token.
    {
        let home = TestHome::new("initialize-token-quit");
        let served = serve_fake(&home);
        let mut socket = open_mcp(None);
        // A ping is answered once the stream is enrolled for the quit's last tokens.
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 9, "method": "ping"}),
        );
        response(&mut socket, 9);
        served
            .context
            .quit
            .begin(QuitReason::Replace, Duration::ZERO);
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("an idle service ends at once");
        let (initialized, tokens) = initialize(&mut socket);
        assert!(initialized["result"].is_object(), "{initialized}");
        let state =
            ResumeState::decode(tokens.first().expect("a token before the answer")).unwrap();
        assert!(state.owned_tabs.is_empty());
    }
}

/// Open one MCP session, end its connection the way `goodbye` does, and wait until the
/// session is gone.
fn session_is_released_after(name: &str, goodbye: impl FnOnce(&mut Connection)) {
    let home = TestHome::new(name);
    let host = ServiceHost::start_for_test(home.path()).unwrap();
    host.publish_ready().unwrap();
    let mut socket = open_mcp(None);
    initialize(&mut socket);
    // The session opens once its source is read, which follows the `initialize` answer.
    eventually(|| host.workbench.look().connections().sessions.len() == 1);

    goodbye(&mut socket);
    drop(socket);
    // The teardown runs on the connection's own thread.
    eventually(|| host.workbench.look().connections().sessions.is_empty());
}

/// Defect 79cf4cc6: a client that goes away through the error path, with one frame that is not
/// JSON, still releases its session and every tab it held, as a tidy goodbye always did.
#[test]
fn a_client_that_goes_away_badly_still_releases_its_session() {
    // The negative control: a tidy goodbye has always worked, so if this were the only case
    // the wait below would prove nothing about the error path.
    session_is_released_after("goodbye-clean", |_| {});
    // An unowned workspace has no owning process for the reaper, so only the connection's own
    // teardown can release it.
    session_is_released_after("goodbye-malformed", |socket| {
        frame::write_frame_raw(socket, b"{ not json at all").unwrap();
    });
}

/// ACCEPTANCE Gov21: the session watch tells a client its tool list changed once, and only when
/// a policy change changes the catalog it was given, including one an earlier service gave.
#[test]
fn the_session_watch_tells_the_client_only_when_a_policy_change_changes_the_catalog() {
    use crate::governance::GovernanceFacade;

    use crate::service::mcp_stream::AdvertisedCatalog;

    let directory = temp_home("catalog");
    let path = directory.join("policy.json");
    let write = |name: &str, allowed: Value| write_policy(&path, name, allowed);
    write("rules", json!(["read", "action", "write", "execute"]));
    let governance = GovernanceFacade::new(Some(path.clone()));
    let advertised = AdvertisedCatalog::default();
    assert!(
        advertised.changed_by(&governance.snapshot()).is_none(),
        "nothing is compared before the client has a catalog"
    );
    let given = advertised.advertise(&governance.snapshot());
    assert!(advertised.changed_by(&governance.snapshot()).is_none());

    write(
        "renamed rules",
        json!(["read", "action", "write", "execute"]),
    );
    governance.refresh_tick();
    assert!(
        advertised.changed_by(&governance.snapshot()).is_none(),
        "a new policy that admits the same tools changes nothing the client sees"
    );

    write("narrower", json!(["read"]));
    governance.refresh_tick();
    let narrowed = advertised
        .changed_by(&governance.snapshot())
        .expect("a narrower policy changes the catalog");
    assert!(narrowed.len() < given.len());
    assert!(
        advertised.changed_by(&governance.snapshot()).is_none(),
        "one change is told once"
    );

    let resumed = AdvertisedCatalog::default();
    resumed.given_earlier();
    assert!(
        resumed.changed_by(&governance.snapshot()).is_some(),
        "a catalog an earlier service gave is told to list again"
    );
    assert!(resumed.changed_by(&governance.snapshot()).is_none());
}
