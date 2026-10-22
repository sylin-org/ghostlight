//! How the service ends: the quit latch, the goodbyes, the drain of work in flight, the calls a
//! quit cancels, and the one decision over every session's tabs.

use super::*;

/// The audit lines of the test home, oldest first.
fn audit_lines(home: &TestHome) -> Vec<Value> {
    std::fs::read_to_string(home.path().join("audit.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

/// What the returning browser's page answers to one world message.
fn returning_world_value(message: &Value) -> Value {
    world_value_at(message, RETURNING_URL)
}

/// Read frames from `socket` until a goodbye arrives and return its reason.
fn goodbye_on(socket: &mut Connection) -> GoodbyeReason {
    loop {
        match frame::read_frame::<McpFrame>(socket).unwrap() {
            Some(McpFrame::Goodbye { reason }) => return reason,
            Some(_) => {}
            None => panic!("the stream closed with no goodbye"),
        }
    }
}

/// ACCEPTANCE C13 and ADR-0199 Decision 10: a person's Quit, from the tray, the page, control
/// or the service itself, writes the quit latch as it begins, and a replace, an uninstall or
/// the end of the session leaves it unset.
#[test]
fn only_a_persons_quit_sets_the_quit_latch() {
    /// One way the service is asked to end.
    #[derive(Clone, Copy, Debug)]
    enum End {
        /// The tray's Quit, over the desktop stream.
        Tray,
        /// The page's Quit, through the workbench.
        Page,
        /// A control quit with reason quit.
        Control,
        /// The service's own quit for a reason.
        Service(QuitReason),
        /// The end of the person's sign-in session.
        SignOut,
    }

    for (end, latched) in [
        (End::Tray, true),
        (End::Page, true),
        (End::Control, true),
        (End::Service(QuitReason::Quit), true),
        (End::Service(QuitReason::Replace), false),
        (End::Service(QuitReason::Uninstall), false),
        (End::SignOut, false),
    ] {
        let (home, host, quit_requested) = quittable_host("quit-latch");
        let latch = Layout::new(home.path()).quit_latch();
        assert!(!crate::service::latch::is_set(&latch), "{end:?}");
        let _shell = match end {
            End::Tray => {
                let mut shell = attached_shell(&host);
                frame::write_frame(
                    &mut shell,
                    &DesktopFrame::Tray {
                        item: TrayItemId::Quit,
                    },
                )
                .unwrap();
                Some(shell)
            }
            End::Page => {
                host.workbench.quit().unwrap();
                None
            }
            End::Control => {
                cli_client::control(Control::Quit {
                    reason: QuitReason::Quit,
                })
                .unwrap();
                None
            }
            End::Service(reason) => {
                host.begin_quit(reason, PATIENCE);
                None
            }
            End::SignOut => {
                host.quit_handle().session_end();
                None
            }
        };
        // The latch is written as the quit begins, before it answers and before the service
        // has left, so a browser that restarts meanwhile is held back too.
        if !matches!(end, End::Tray) {
            assert_eq!(
                crate::service::latch::is_set(&latch),
                latched,
                "{end:?} at once"
            );
        }
        quit_requested
            .recv_timeout(PATIENCE)
            .expect("the end asks the process to exit");
        assert_eq!(crate::service::latch::is_set(&latch), latched, "{end:?}");
        drop(host);
    }
}

/// Relay protocol 4 contract (ADR-0199 Decisions 5 and 13, ACCEPTANCE C13): before it closes
/// them, the service says goodbye with its end's reason on each MCP stream and the desktop
/// stream, and only then asks the process to exit.
#[test]
fn every_service_end_says_goodbye_with_its_reason_on_the_mcp_and_desktop_streams() {
    for (quit, goodbye) in [
        (Some(QuitReason::Quit), GoodbyeReason::Quit),
        (Some(QuitReason::Replace), GoodbyeReason::Replace),
        (Some(QuitReason::Uninstall), GoodbyeReason::Uninstall),
        (None, GoodbyeReason::SessionEnd),
    ] {
        let (_home, host, quit_requested) = quittable_host("goodbye");
        let mut mcp = open_mcp(None);
        initialize(&mut mcp);
        let mut shell = attached_shell(&host);

        match quit {
            Some(reason) => {
                let answer = cli_client::control(Control::Quit { reason }).unwrap();
                assert_eq!(
                    answer,
                    serde_json::to_value(control::Quitting { quitting: true }).unwrap(),
                    "{reason:?}"
                );
            }
            None => host.quit_handle().session_end(),
        }

        assert_eq!(goodbye_on(&mut mcp), goodbye, "on the MCP stream");
        assert_eq!(
            desktop_goodbye(&mut shell),
            goodbye,
            "on the desktop stream"
        );
        quit_requested
            .recv_timeout(PATIENCE)
            .expect("the process is asked to exit after the goodbyes");
        drop(host);
    }
}

/// ARCHITECTURE's quit and ADR-0199 Decision 13: an end admits no new work, and a quit lets
/// work in flight finish, for at most its grace, before its goodbye and exit, keeping the
/// reason it began with, while the end of the session does not wait for work at all.
#[test]
fn a_quit_lets_work_in_flight_finish_within_its_grace_and_the_first_quit_wins() {
    // Work that finishes: a quit over control answers at once, and the quit ends with the
    // work, well before its grace would have ended it.
    {
        let (_home, host, quit_requested) = quittable_host("quit-drains");
        let mut mcp = open_mcp(None);
        initialize(&mut mcp);
        let running = host.quit.drain.admit().unwrap();
        let began = Instant::now();
        let answer = cli_client::control(Control::Quit {
            reason: QuitReason::Quit,
        })
        .unwrap();
        assert_eq!(
            answer,
            serde_json::to_value(control::Quitting { quitting: true }).unwrap()
        );
        assert!(
            host.quit.drain.admit().is_err(),
            "no new work is admitted once the quit began"
        );
        drop(running);
        assert_eq!(goodbye_on(&mut mcp), GoodbyeReason::Quit);
        quit_requested
            .recv_timeout(PATIENCE)
            .expect("the quit ends once its work has finished");
        assert!(
            began.elapsed() < QUIT_GRACE,
            "the quit ended with its work, not with its grace"
        );
        drop(host);
    }

    // Work that never finishes: the goodbye and the exit wait out the grace, and a second
    // quit while one is under way changes neither the reason nor the grace.
    {
        const GRACE: Duration = Duration::from_millis(300);

        let (_home, host, quit_requested) = quittable_host("quit-bound");
        let mut mcp = open_mcp(None);
        initialize(&mut mcp);
        let _stuck = host.quit.drain.admit().unwrap();
        let began = Instant::now();
        host.begin_quit(QuitReason::Quit, GRACE);
        host.begin_quit(QuitReason::Replace, Duration::ZERO);
        assert_eq!(goodbye_on(&mut mcp), GoodbyeReason::Quit);
        assert!(
            began.elapsed() >= GRACE,
            "no goodbye while work was in flight inside the grace"
        );
        quit_requested
            .recv_timeout(PATIENCE)
            .expect("the quit ends when its grace does");
        drop(host);
    }

    // The end of the session says goodbye and leaves while work is still in flight.
    {
        let (_home, host, quit_requested) = quittable_host("session-end-drain");
        let mut mcp = open_mcp(None);
        initialize(&mut mcp);
        let running = host.quit.drain.admit().unwrap();
        host.quit_handle().session_end();
        assert_eq!(goodbye_on(&mut mcp), GoodbyeReason::SessionEnd);
        quit_requested
            .recv_timeout(PATIENCE)
            .expect("the end of the session does not wait for work in flight");
        assert!(
            host.quit.drain.admit().is_err(),
            "no new work is admitted once the session is ending"
        );
        drop(running);
        drop(host);
    }
}

/// Call `browser_tabs` list under `id` and return its tool result and the audit lines of its
/// invocation. The audit line is written before the result is delivered.
fn listed_with_audit(home: &TestHome, socket: &mut Connection, id: u64) -> (Value, Vec<Value>) {
    send_line(
        socket,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {"name": "browser_tabs", "arguments": {"action": "list"}}
        }),
    );
    let (called, _) = response(socket, id);
    let result = called["result"]["structuredContent"].clone();
    assert!(
        result.is_object(),
        "a tool result, not a protocol error: {called}"
    );
    let records = audit_lines(home)
        .into_iter()
        .filter(|record| record["invocation"] == result["invocation"])
        .collect();
    (result, records)
}

/// One call the service ended before it started: the tool result its client received, when a
/// client was there to receive it, and the audit lines of its invocation.
type EndedCall = (Option<Value>, Vec<Value>);

/// The tool results and audit lines of two calls made during a quit for `reason`: one while
/// the quit drains work in flight, and one after its end stopped the session's queue.
///
/// Each step waits on what it needs: the session is open once its first call is answered,
/// the drain refuses work once `begin` returns, and the end has run once the quit signal is
/// made. The answers are written by the thread that reads the stream, so nothing races them.
fn calls_during_a_quit(reason: QuitReason) -> Vec<EndedCall> {
    let home = TestHome::new("quit-calls");
    let served = serve_fake(&home);
    let mut socket = open_mcp(None);
    initialize(&mut socket);
    let (opened, _) = listed_with_audit(&home, &mut socket, 2);
    assert_eq!(opened["status"], json!(Status::Succeeded), "{opened}");

    let running = served.context.quit.drain.admit().unwrap();
    served.context.quit.begin(reason, PATIENCE);
    let draining = listed_with_audit(&home, &mut socket, 3);
    drop(running);
    served
        .quit_requested
        .recv_timeout(PATIENCE)
        .expect("the quit ends once its work in flight finishes");
    let ended = listed_with_audit(&home, &mut socket, 4);
    [draining, ended]
        .into_iter()
        .map(|(result, records)| (Some(result), records))
        .collect()
}

/// The audit lines of two calls that wait behind a workspace the test holds when their client
/// goes away, so their session's end cancels them before they start, with no client left to
/// hear it.
///
/// The service reads both calls, and queues them, before the end of the stream it then reads,
/// and the session's end has run once both calls have an audit line.
fn calls_queued_when_their_session_ends() -> Vec<EndedCall> {
    const CALLS: usize = 2;

    let home = TestHome::new("session-end-calls");
    let served = serve_fake(&home);
    // The calls join a declared session whose workspace the test holds, so they wait for it.
    let marker = SessionMarker::Declared {
        key: "session-end-calls".into(),
    };
    let workspace = served.context.workspaces.connect(
        Arc::new(ConnectionEvidence::claimed("holder", IntakeChannel::Cli)),
        Some(marker.clone()),
    );
    let _held = served.context.workspaces.acquire(&workspace).unwrap();
    let mut opening = mcp_opening(None);
    opening.session = Some(serde_json::to_value(marker).unwrap());
    let (mut socket, reply) = dial(&opening);
    assert!(reply.accepted, "{reply:?}");
    initialize(&mut socket);
    for id in 2..2 + CALLS {
        send_line(
            &mut socket,
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": "browser_tabs",
                    "arguments": {
                        "action": "list",
                        "timeout_ms": crate::language::MAX_TIMEOUT_MS
                    }
                }
            }),
        );
    }
    drop(socket);
    eventually(|| audit_lines(&home).len() >= CALLS);
    let mut calls: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for record in audit_lines(&home) {
        let invocation = record["invocation"].as_str().unwrap_or_default().to_owned();
        calls.entry(invocation).or_default().push(record);
    }
    assert_eq!(calls.len(), CALLS, "{calls:?}");
    calls.into_values().map(|records| (None, records)).collect()
}

/// Defect C11 (8c216920, finding E-03), ACCEPTANCE B27 and ADR-0203 Decision 4.2: a call the
/// service ends before it starts, because Ghostlight quits or because the call's session ends,
/// is cancelled with one audit line worded for that cause, and a client still there to hear it
/// gets a cancelled tool result that says to try again only when a call repeated in a moment
/// is served.
#[test]
fn a_call_ended_before_it_starts_is_cancelled_with_one_audit_line_worded_for_its_cause() {
    for (cause, calls) in [
        (
            CancelCause::Quit(QuitCause::Quit),
            calls_during_a_quit(QuitReason::Quit),
        ),
        (
            CancelCause::Quit(QuitCause::Replace),
            calls_during_a_quit(QuitReason::Replace),
        ),
        (
            CancelCause::Quit(QuitCause::Unknown),
            calls_during_a_quit(QuitReason::Unknown),
        ),
        (
            CancelCause::Quit(QuitCause::Uninstall),
            calls_during_a_quit(QuitReason::Uninstall),
        ),
        (
            CancelCause::SessionEnded,
            calls_queued_when_their_session_ends(),
        ),
    ] {
        let refusal = Refusal::CancelledBeforeStart { by: cause };
        let steps = refusal.next_steps();
        for (result, records) in calls {
            assert_eq!(records.len(), 1, "{cause:?}: {records:?}");
            assert_eq!(
                records[0]["status"],
                Status::Cancelled.stored_name(),
                "{records:?}"
            );
            assert_eq!(
                records[0]["effect"],
                Effect::None.stored_name(),
                "{records:?}"
            );
            assert_eq!(records[0]["summary"], refusal.summary(), "{cause:?}");
            let Some(result) = result else {
                continue;
            };
            assert_eq!(
                result["status"],
                json!(Status::Cancelled),
                "{cause:?}: {result}"
            );
            assert_eq!(result["effect"], json!(Effect::None), "{result}");
            assert_eq!(result["repeat_safe"], true, "{result}");
            assert_eq!(result["summary"], refusal.summary(), "{cause:?}");
            if steps.is_empty() {
                assert!(result.get("next_steps").is_none(), "{cause:?}: {result}");
            } else {
                assert_eq!(result["next_steps"], json!(steps), "{cause:?}");
            }
        }
    }
}

/// How an extension at a service end answers the page work a call sends it.
#[derive(Clone, Copy)]
enum PageWork {
    /// Answer at once, as the returning browser does.
    Answered,
    /// Hold the answer until the service cancels the request.
    HeldUntilCancelled,
    /// Never answer anything, a release command included.
    Silent,
}

/// Attach a protocol-4 extension for the returning browser that answers release commands and
/// page work as `work` says, and records every frame the service sends it.
fn attach_ending_browser(
    served: &Served,
    work: PageWork,
) -> crate::browser::adapter::chromium::protocol4::fake::FakeExtension {
    use crate::browser::adapter::chromium::protocol4::fake::{FakeExtension, Reply as FakeReply};
    use crate::browser::adapter::chromium::protocol4::wire::{Failure, Primitive};

    /// What the returning browser answers to one primitive it is asked to run.
    fn answer(primitive: &Primitive) -> Value {
        match primitive {
            Primitive::ListTabs(_) => json!({"tabs": []}),
            Primitive::CloseTab(close) => json!({"tab_id": close.tab_id}),
            Primitive::Frames(_) => json!({"frames": [{"frame_id": "T",
                "loader_id": "t1", "parent_frame_id": null, "url": RETURNING_URL}],
                "complete": true, "unseen": []}),
            Primitive::Call(call) => json!({"answers": call.calls.iter().map(|world| {
                json!({"status": "returned", "envelope": {"ok": true,
                    "value": returning_world_value(&world.message)}})
            }).collect::<Vec<_>>()}),
            _ => Value::Null,
        }
    }

    let (service, extension) = pair().unwrap();
    let fake = FakeExtension::connect(
        extension,
        &FakeExtension::hello(RETURNING_BROWSER),
        Box::new(move |correlation, primitive| {
            let page_work = matches!(primitive, Primitive::Frames(_) | Primitive::Call(_));
            match work {
                PageWork::Silent => FakeReply::Never,
                PageWork::HeldUntilCancelled if page_work => {
                    FakeReply::OnCancel(FakeExtension::failed(correlation, Failure::Cancelled))
                }
                _ => FakeReply::Now(FakeExtension::ok(correlation, answer(primitive))),
            }
        }),
    );
    served.relay.attach(service).unwrap();
    eventually(|| served.relay.is_connected());
    fake
}

/// Open an MCP session and wait until it is open, with its workspace, and return the stream
/// and that workspace.
fn open_session(served: &Served) -> (Connection, crate::workspace::WorkspaceId) {
    let before = served.context.workspaces.ids();
    let mut socket = open_mcp(None);
    initialize(&mut socket);
    // A forwarded request is served only once the session has opened.
    send_line(
        &mut socket,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    );
    response(&mut socket, 2);
    let workspace = served
        .context
        .workspaces
        .ids()
        .into_iter()
        .find(|workspace| !before.contains(workspace))
        .expect("the session opened a workspace");
    (socket, workspace)
}

/// Bind the returning browser's tab `tab_id` at its address to `workspace` as a confirmed
/// tab, and return its handle.
fn bind_tab(
    workspaces: &crate::workspace::WorkspaceStore,
    workspace: &crate::workspace::WorkspaceId,
    tab_id: u64,
) -> String {
    use crate::browser::adapter::{BrowserReadiness, PhysicalTab};

    let lease = workspaces.acquire(workspace).unwrap();
    let tab = PhysicalTab {
        tab_id,
        url: RETURNING_URL.into(),
        title: "Example".into(),
        active: true,
        readiness: BrowserReadiness::Complete,
    };
    let added = lease.add_tab(RETURNING_BROWSER, &tab).unwrap();
    lease.apply_landing(&added.handle, &tab).unwrap();
    added.handle.as_str().to_owned()
}

/// Read `socket` up to its goodbye, and return the resume tokens handed out on the way and
/// the goodbye's reason.
fn until_goodbye(socket: &mut Connection) -> (Vec<ResumeState>, GoodbyeReason) {
    let mut tokens = Vec::new();
    loop {
        match frame::read_frame::<McpFrame>(socket).unwrap() {
            Some(McpFrame::Resume { token }) => {
                tokens.push(ResumeState::decode(&token).unwrap());
            }
            Some(McpFrame::Goodbye { reason }) => return (tokens, reason),
            Some(_) => {}
            None => panic!("the stream closed with no goodbye"),
        }
    }
}

/// The handles a resume token restores.
fn handles_in(token: &ResumeState) -> Vec<String> {
    token
        .owned_tabs
        .iter()
        .map(|tab| tab.handle.clone())
        .collect()
}

/// Where in what an extension saw the service's goodbye came, if it came.
fn goodbye_at(seen: &crate::browser::adapter::chromium::protocol4::fake::Seen) -> Option<usize> {
    seen.frames
        .iter()
        .position(|frame| frame.get("goodbye").is_some())
}

/// The tabs a release closed, each before the browser goodbye, which has come.
fn closed_before_goodbye(
    browser: &crate::browser::adapter::chromium::protocol4::fake::FakeExtension,
) -> Vec<u64> {
    let seen = browser.wait(|seen| goodbye_at(seen).is_some());
    let goodbye = goodbye_at(&seen).unwrap();
    let closes = requests_at(&seen, "close_tab");
    assert!(
        closes.iter().all(|(at, _)| *at < goodbye),
        "every release close comes before the browser goodbye: {:?}",
        seen.frames
    );
    let mut tabs: Vec<u64> = closes.into_iter().map(|(_, tab)| tab).collect();
    tabs.sort_unstable();
    tabs
}

/// ACCEPTANCE B27 and ADR-0203 Decision 3 (slice C5, 9beb7976): a service end decides every
/// session's tabs once, by its reason and the session's kind, and the end of the session and
/// the idle exit forget every tab without a browser command.
#[test]
fn a_service_end_decides_each_sessions_tabs_once_by_its_reason() {
    // A replace keeps a live session's tab bound, closes and ungroups nothing, and hands the
    // tab to the session's last token for its edge's resume.
    {
        let home = TestHome::new("end-replace");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Answered);
        let (mut socket, workspace) = open_session(&served);
        let handle = bind_tab(&served.context.workspaces, &workspace, RETURNING_TAB);

        served
            .context
            .quit
            .begin(QuitReason::Replace, Duration::ZERO);

        let (tokens, goodbye) = until_goodbye(&mut socket);
        assert_eq!(goodbye, GoodbyeReason::Replace);
        let last = tokens
            .last()
            .expect("a last token comes before the goodbye");
        assert_eq!(handles_in(last), [handle.as_str()]);
        assert!(last.owned_tabs[0].url_digest.is_some(), "{last:?}");
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the replace finishes");
        assert_eq!(
            closed_before_goodbye(&browser),
            Vec::<u64>::new(),
            "nothing closes or ungroups a live session's tab"
        );
        assert!(requests_at(&browser.wait(|_| true), "diagnostics").is_empty());
        assert_eq!(bound_tabs(&served), vec![(handle, RETURNING_TAB)]);
    }

    // A quit, an uninstall or a reason this build does not know releases every session, and
    // each stream's last token carries no tab.
    for (reason, said) in [
        (QuitReason::Quit, GoodbyeReason::Quit),
        (QuitReason::Uninstall, GoodbyeReason::Uninstall),
        (QuitReason::Unknown, GoodbyeReason::Unknown),
    ] {
        let home = TestHome::new("end-quit");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Answered);
        let (mut first, first_workspace) = open_session(&served);
        let (mut second, second_workspace) = open_session(&served);
        let first_tab = bind_tab(&served.context.workspaces, &first_workspace, RETURNING_TAB);
        let second_tab = bind_tab(
            &served.context.workspaces,
            &second_workspace,
            RETURNING_TAB + 1,
        );
        // The session watch hands each stream a token with its tab within a second, so the
        // last token each stream gets says what the end decided.
        for (socket, handle) in [(&mut first, &first_tab), (&mut second, &second_tab)] {
            while handles_in(&next_resume(socket)) != [handle.as_str()] {}
        }

        served.context.quit.begin(reason, Duration::ZERO);

        for socket in [&mut first, &mut second] {
            let (tokens, goodbye) = until_goodbye(socket);
            assert_eq!(goodbye, said);
            let last = tokens
                .last()
                .expect("a last token comes before the goodbye");
            assert!(last.owned_tabs.is_empty(), "{reason:?}: {last:?}");
        }
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the quit finishes");
        assert_eq!(
            closed_before_goodbye(&browser),
            [RETURNING_TAB, RETURNING_TAB + 1],
            "{reason:?}"
        );
        assert!(served.context.workspaces.ids().is_empty(), "{reason:?}");
    }

    // A command-line session is released at every end, because nothing can resume it.
    for reason in [
        QuitReason::Quit,
        QuitReason::Replace,
        QuitReason::Uninstall,
        QuitReason::Unknown,
    ] {
        let home = TestHome::new("end-command-line");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Answered);
        let workspace = served.context.workspaces.resume_or_admit(
            "command line".into(),
            IntakeChannel::Cli,
            SessionMarker::Declared {
                key: "end-command-line".into(),
            },
        );
        bind_tab(&served.context.workspaces, &workspace, RETURNING_TAB);

        served.context.quit.begin(reason, Duration::ZERO);

        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the end finishes");
        assert_eq!(
            closed_before_goodbye(&browser),
            [RETURNING_TAB],
            "{reason:?}"
        );
        assert!(served.context.workspaces.ids().is_empty(), "{reason:?}");
    }

    // The end of the session and the idle exit send the browser nothing and forget the tab.
    for ends in [
        (|quit: &crate::service::Quit| quit.session_end()) as fn(&crate::service::Quit),
        |quit: &crate::service::Quit| quit.idle_exit(Duration::ZERO),
    ] {
        let home = TestHome::new("end-command-line-quietly");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Answered);
        let workspace = served.context.workspaces.resume_or_admit(
            "command line".into(),
            IntakeChannel::Cli,
            SessionMarker::Declared {
                key: "end-command-line".into(),
            },
        );
        bind_tab(&served.context.workspaces, &workspace, RETURNING_TAB);

        ends(&served.context.quit);

        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the end finishes");
        assert!(served.context.workspaces.ids().is_empty());
        assert!(requests_at(&browser.wait(|_| true), "close_tab").is_empty());
    }

    // On a replace, a session whose client left during the drain is released, and the
    // session that stays keeps its tab.
    {
        let home = TestHome::new("end-replace-left");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Answered);
        let (left, left_workspace) = open_session(&served);
        let (mut stays, stays_workspace) = open_session(&served);
        bind_tab(&served.context.workspaces, &left_workspace, RETURNING_TAB);
        let kept = bind_tab(
            &served.context.workspaces,
            &stays_workspace,
            RETURNING_TAB + 1,
        );
        let running = served.context.quit.drain.admit().unwrap();

        served.context.quit.begin(QuitReason::Replace, PATIENCE);
        drop(left);
        eventually(|| served.context.workspaces.ids() == [stays_workspace.clone()]);
        drop(running);

        let (tokens, goodbye) = until_goodbye(&mut stays);
        assert_eq!(goodbye, GoodbyeReason::Replace);
        assert_eq!(handles_in(tokens.last().unwrap()), [kept.as_str()]);
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the replace finishes");
        assert_eq!(closed_before_goodbye(&browser), [RETURNING_TAB]);
        assert_eq!(bound_tabs(&served), vec![(kept, RETURNING_TAB + 1)]);
    }

    // A workspace a call still holds is cancelled before its tabs close.
    {
        let home = TestHome::new("end-leased");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::HeldUntilCancelled);
        let (mut socket, workspace) = open_session(&served);
        let handle = bind_tab(&served.context.workspaces, &workspace, RETURNING_TAB);
        send_line(
            &mut socket,
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "browser_read", "arguments": {"tab": handle}}}),
        );
        drop(browser.wait(|seen| {
            !requests_at(seen, "frames").is_empty() || !requests_at(seen, "call").is_empty()
        }));

        // The drain gives up on the call, so the end meets the workspace still leased.
        served
            .context
            .quit
            .begin(QuitReason::Quit, Duration::from_millis(100));

        let (_, goodbye) = until_goodbye(&mut socket);
        assert_eq!(goodbye, GoodbyeReason::Quit);
        served
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the quit finishes");
        assert_eq!(closed_before_goodbye(&browser), [RETURNING_TAB]);
        let seen = browser.wait(|_| true);
        let cancelled = seen
            .frames
            .iter()
            .position(|frame| frame["kind"] == "cancel")
            .expect("the running call is cancelled");
        let closed = requests_at(&seen, "close_tab")[0].0;
        assert!(cancelled < closed, "{:?}", seen.frames);
        drop(seen);
        assert!(served.context.workspaces.ids().is_empty());
    }

    // However many tabs a session holds, the end of the session says both goodbyes inside its
    // fallback exit and sends the browser nothing.
    {
        const TABS: u64 = 300;

        let home = TestHome::new("end-session-many");
        let served = serve_relayed(&home);
        let browser = attach_ending_browser(&served, PageWork::Silent);
        let (mut socket, workspace) = open_session(&served);
        for tab in 0..TABS {
            bind_tab(&served.context.workspaces, &workspace, RETURNING_TAB + tab);
        }

        let began = Instant::now();
        served.context.quit.session_end();

        let (_, goodbye) = until_goodbye(&mut socket);
        assert_eq!(goodbye, GoodbyeReason::SessionEnd);
        let seen = browser.wait(|seen| goodbye_at(seen).is_some());
        assert!(
            began.elapsed() < ghostlight_relay::os::session::FALLBACK_EXIT,
            "both goodbyes come before the fallback exit: {:?}",
            began.elapsed()
        );
        assert!(requests_at(&seen, "close_tab").is_empty());
        assert!(requests_at(&seen, "diagnostics").is_empty());
        drop(seen);
        assert!(served.context.workspaces.ids().is_empty());
    }
}
