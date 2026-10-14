//! Browser availability at the one browser seam: a call waits a bounded grace for a browser that
//! is on its way back, then says what is true (ADR-0196 Decision 12).
//!
//! The relay cases run the executor over a real `RelayBrowserPort` and scripted adapter
//! connections, because what they prove lives in the port: a connection still installing its
//! page runtime, and a silent connection that answers one immediate probe.

use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use ghostlight_bridge::browser::{
    adapter_capability, AdapterCapability, BrowserFrame, BrowserPlatform, BrowserReceipt,
    ADAPTER_PROTOCOL_MAJOR,
};
use ghostlight_bridge::framing::{read_native, write_native};

use super::*;
use crate::browser::{
    BrowserPort, HeartbeatSettings, RelayBrowserPort, RELAY_RETURN_WINDOW, RETURN_GRACE,
};

/// The browser identity every scripted adapter here announces.
const RELAY_BROWSER: &str = "browser_relay";

/// What a scripted adapter does before it settles into answering everything.
#[derive(Clone, Copy)]
enum Script {
    /// Answer from the start.
    Answer,
    /// Hold the page-runtime installation receipt for this long. The signal fires when the
    /// installation request arrives, while the connection is registered but not ready.
    InstallSlowly(Duration),
    /// Ignore everything through the first cancellation, as a stalled extension worker would.
    /// The signal fires when the worker comes back.
    SilentUntilCancel,
}

/// One scripted adapter connection serving on its own thread.
struct Adapter {
    closer: TcpStream,
    thread: thread::JoinHandle<()>,
}

impl Adapter {
    /// Connect to the service listener and run `script`, then list no tabs and acknowledge
    /// every probe until the stream closes.
    fn start(
        address: SocketAddr,
        capabilities: &'static [&'static str],
        script: Script,
        signal: Option<mpsc::Sender<()>>,
    ) -> Self {
        let mut stream = TcpStream::connect(address).unwrap();
        let closer = stream.try_clone().unwrap();
        let thread = thread::spawn(move || {
            hello(&mut stream, capabilities);
            let mut silent = matches!(script, Script::SilentUntilCancel);
            while let Ok(Some(frame)) = read_native::<BrowserFrame>(&mut stream) {
                let reply = match frame {
                    BrowserFrame::Request { request } if silent => {
                        if matches!(request.command, BrowserCommand::Cancel { .. }) {
                            silent = false;
                            if let Some(signal) = &signal {
                                let _ = signal.send(());
                            }
                        }
                        None
                    }
                    BrowserFrame::Heartbeat { .. } if silent => None,
                    BrowserFrame::Request { request } => match request.command {
                        BrowserCommand::InstallPageRuntime {
                            revision, sha256, ..
                        } => {
                            if let Script::InstallSlowly(delay) = script {
                                if let Some(signal) = &signal {
                                    let _ = signal.send(());
                                }
                                thread::sleep(delay);
                            }
                            Some(receipt(
                                request.correlation,
                                BrowserOutcome::PageRuntimeInstalled { revision, sha256 },
                            ))
                        }
                        BrowserCommand::ListTabs => Some(receipt(
                            request.correlation,
                            BrowserOutcome::Tabs { tabs: vec![] },
                        )),
                        _ => None,
                    },
                    BrowserFrame::Heartbeat { sequence } => {
                        Some(BrowserFrame::HeartbeatAck { sequence })
                    }
                    _ => None,
                };
                if let Some(reply) = reply {
                    if write_native(&mut stream, &reply).is_err() {
                        return;
                    }
                }
            }
        });
        Self { closer, thread }
    }

    fn close(self) {
        let _ = self.closer.shutdown(Shutdown::Both);
        self.thread.join().unwrap();
    }
}

/// Announce the scripted adapter. One that reports attention announces itself attended, as the
/// extension does at hello for the window the person is using.
fn hello(stream: &mut TcpStream, capabilities: &[&str]) {
    write_native(
        stream,
        &BrowserFrame::Hello {
            major: ADAPTER_PROTOCOL_MAJOR,
            adapter_version: "1.0.0".into(),
            browser_id: RELAY_BROWSER.into(),
            adapter_epoch: "adapter_relay".into(),
            browser_name: None,
            platform: Some(BrowserPlatform::Chromium),
            attended: capabilities.contains(&adapter_capability::ADAPTER_ATTENTION),
            capabilities: capabilities
                .iter()
                .map(|name| AdapterCapability {
                    name: (*name).into(),
                    revision: 1,
                })
                .collect(),
        },
    )
    .unwrap();
    assert!(matches!(
        read_native::<BrowserFrame>(stream).unwrap(),
        Some(BrowserFrame::HelloAccepted { .. })
    ));
}

fn receipt(correlation: String, result: BrowserOutcome) -> BrowserFrame {
    BrowserFrame::Receipt {
        receipt: BrowserReceipt {
            correlation,
            result,
        },
    }
}

/// An executor over a real relay port, with fixed installed-browser facts so recovery, if it
/// ran, would never inspect or launch anything on this machine.
fn relay_fixture(
    port: Arc<RelayBrowserPort>,
    governance: GovernanceFacade,
) -> (
    ApplicationExecutor,
    WorkspaceStore,
    crate::workspace::WorkspaceId,
) {
    let workspaces = WorkspaceStore::default();
    let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp, None);
    let audit = Arc::new(MemoryAudit::default());
    let mut executor = ApplicationExecutor::new(
        governance,
        workspaces.clone(),
        port,
        PresentationReactor::new(Arc::new(NoPresentation)),
        WorkbenchProjection::default(),
        Arc::new(crate::audit::AuditRecorder::new(
            audit,
            WorkbenchProjection::default(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    executor
        .recovery
        .set_test_candidates(vec![RecoveryCandidate {
            id: "chromium".into(),
            name: "Chromium".into(),
            package: BrowserPackage::Native,
            package_detail: "Chromium native package".into(),
            registration: NativeHostState::Current,
            ordinary_executable: Some(PathBuf::from("chromium")),
        }]);
    (executor, workspaces, workspace)
}

/// Governance that leaves browser startup to the person, so a call that reached recovery would
/// answer `browser_startup_manual` instead of quietly succeeding.
fn manual_startup() -> (GovernanceFacade, PathBuf) {
    let policy = temporary_policy("manual-browser-startup");
    fs::write(
        &policy,
        all_open_policy_with(r#"[{"key":"browser.startup","value":"manual","level":"mandatory"}]"#),
    )
    .unwrap();
    (GovernanceFacade::new(Some(policy.clone()), None), policy)
}

fn list_tabs(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    )
}

const RUNTIME_AND_TABS: &[&str] = &[adapter_capability::PAGE_RUNTIME, adapter_capability::TABS];
const LIVE_TABS: &[&str] = &[
    adapter_capability::TABS,
    adapter_capability::ADAPTER_LIVENESS,
];
const TABS_ONLY: &[&str] = &[adapter_capability::TABS];
const ATTENDED_TABS: &[&str] = &[
    adapter_capability::TABS,
    adapter_capability::ADAPTER_ATTENTION,
];

/// A navigation with the default deadline, from a session that works in no browser yet.
fn navigate(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    )
}

/// Wait until the port has dropped the browser's registration after its stream closed.
fn await_unregistered(port: &RelayBrowserPort) {
    let until = Instant::now() + Duration::from_secs(5);
    while port.is_registered(RELAY_BROWSER) {
        assert!(
            Instant::now() < until,
            "the closed connection stayed registered"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

/// The same browser reconnecting is invisible while its new connection installs the page
/// runtime. A session pinned to it waits and continues; it is never told about another profile.
#[test]
fn a_pinned_browser_reconnecting_during_runtime_install_takes_the_call() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let port = Arc::new(RelayBrowserPort::new("service_test".into()));
    let (executor, workspaces, workspace) =
        relay_fixture(Arc::clone(&port), GovernanceFacade::new(None, None));

    let first = Adapter::start(address, TABS_ONLY, Script::Answer, None);
    port.attach(listener.accept().unwrap().0).unwrap();
    workspaces
        .pin_browser(workspace.as_str(), RELAY_BROWSER)
        .unwrap();

    let (installing, install_arrived) = mpsc::channel();
    let second = Adapter::start(
        address,
        RUNTIME_AND_TABS,
        Script::InstallSlowly(Duration::from_millis(400)),
        Some(installing),
    );
    let attaching = {
        let port = Arc::clone(&port);
        thread::spawn(move || port.attach(listener.accept().unwrap().0))
    };
    install_arrived.recv().unwrap();
    assert!(
        port.browsers().is_empty(),
        "the reconnecting browser is private until its runtime is installed"
    );

    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert_eq!(listed.facts["browsers"][0]["browser"], json!(RELAY_BROWSER));
    attaching.join().unwrap().unwrap();
    first.close();
    second.close();
}

/// A pinned browser whose liveness lapsed is probed once, at once, and the call goes through as
/// soon as it answers, long before the next timer heartbeat.
#[test]
fn a_silent_pinned_browser_that_answers_the_probe_takes_the_call() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let port = Arc::new(RelayBrowserPort::with_heartbeat_settings(
        "service_test".into(),
        HeartbeatSettings {
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(30),
            verdict: Duration::from_millis(50),
        },
    ));
    let (executor, workspaces, workspace) =
        relay_fixture(Arc::clone(&port), GovernanceFacade::new(None, None));
    let (back, came_back) = mpsc::channel();
    let adapter = Adapter::start(address, LIVE_TABS, Script::SilentUntilCancel, Some(back));
    port.attach(listener.accept().unwrap().0).unwrap();
    workspaces
        .pin_browser(workspace.as_str(), RELAY_BROWSER)
        .unwrap();

    // One unanswered probe past the verdict window makes the browser silent.
    assert_eq!(
        port.call(
            RELAY_BROWSER,
            workspace.as_str(),
            BrowserCommand::ListTabs,
            Instant::now() + Duration::from_millis(150),
            &std::sync::atomic::AtomicBool::new(false),
        ),
        Err(BrowserError::DeadlineAfterDispatch)
    );
    assert!(port.browsers().is_empty());
    came_back.recv().unwrap();

    let started = Instant::now();
    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the call probed at once instead of waiting for the 10 s heartbeat"
    );
    adapter.close();
}

/// Right after a service start, the browser's relay is still reconnecting. A session with no
/// browser yet waits for it inside the grace, and recovery never runs.
#[test]
fn a_fresh_service_waits_for_the_returning_browser_instead_of_recovering() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let port = Arc::new(RelayBrowserPort::new("service_test".into()));
    let (governance, policy) = manual_startup();
    let (executor, workspaces, workspace) = relay_fixture(Arc::clone(&port), governance);

    let reconnecting = {
        let port = Arc::clone(&port);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            let adapter = Adapter::start(address, TABS_ONLY, Script::Answer, None);
            port.attach(listener.accept().unwrap().0).unwrap();
            adapter
        })
    };

    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some(RELAY_BROWSER)
    );
    reconnecting.join().unwrap().close();
    let _ = fs::remove_file(policy);
}

/// A browser the person used and then closed is not on its way back. Once its departure is
/// older than the grace, a session with no browser goes straight to browser startup, which keeps
/// its whole launch budget instead of the second or so the grace would have left it. Attention
/// the browser reported while it was open is not evidence.
#[test]
fn a_browser_closed_longer_ago_than_the_grace_is_not_waited_for() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let port = Arc::new(RelayBrowserPort::new("service_test".into()));
    let (governance, policy) = manual_startup();
    let (executor, _, workspace) = relay_fixture(Arc::clone(&port), governance);
    // Long past the port's own reconnect window, so the departure is the only evidence.
    port.backdate(RETURN_GRACE);

    let adapter = Adapter::start(address, ATTENDED_TABS, Script::Answer, None);
    port.attach(listener.accept().unwrap().0).unwrap();
    assert!(
        port.browsers()[0].attended,
        "the browser reported attention"
    );
    adapter.close();
    await_unregistered(&port);
    assert!(
        port.browser_returning(),
        "a connection that ended moments ago may come back"
    );

    port.backdate(RETURN_GRACE);
    assert!(!port.browser_returning());
    let started = Instant::now();
    let refused = navigate(&executor, &workspace);
    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.facts["reason"], json!("browser_startup_manual"));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "browser startup ran at once instead of after a wait, took {:?}",
        started.elapsed()
    );
    let _ = fs::remove_file(policy);
}

/// Publishing the endpoint opens a short window for relays that served an earlier service. When
/// none arrives, the call stops waiting at the end of that window, not at the end of the grace,
/// so a cold start with no browser leaves browser startup most of the deadline.
#[test]
fn a_freshly_published_port_with_no_relay_stops_waiting_after_the_reconnect_window() {
    let port = Arc::new(RelayBrowserPort::new("service_test".into()));
    let (governance, policy) = manual_startup();
    let (executor, _, workspace) = relay_fixture(Arc::clone(&port), governance);
    port.backdate(RETURN_GRACE);
    assert!(!port.browser_returning(), "an old port with no relay");

    let published = Instant::now();
    port.endpoint_published();
    assert!(port.browser_returning(), "relays may be reconnecting");
    let refused = navigate(&executor, &workspace);
    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.facts["reason"], json!("browser_startup_manual"));
    assert!(
        published.elapsed() >= RELAY_RETURN_WINDOW,
        "the call waited out the reconnect window"
    );
    assert!(
        published.elapsed() < RETURN_GRACE / 2,
        "the wait ended with the reconnect window, not the grace, took {:?}",
        published.elapsed()
    );
    let _ = fs::remove_file(policy);
}

/// A relay that reconnects is registered before its page runtime is installed. That registration
/// alone carries the wait of a session with no browser yet, long after the port's own reconnect
/// window has closed, and the call takes the browser instead of reaching browser startup.
#[test]
fn a_registered_connection_still_installing_carries_the_wait_past_the_reconnect_window() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let port = Arc::new(RelayBrowserPort::new("service_test".into()));
    let (governance, policy) = manual_startup();
    let (executor, workspaces, workspace) = relay_fixture(Arc::clone(&port), governance);
    port.backdate(RETURN_GRACE);

    let (installing, install_arrived) = mpsc::channel();
    let adapter = Adapter::start(
        address,
        RUNTIME_AND_TABS,
        Script::InstallSlowly(Duration::from_millis(400)),
        Some(installing),
    );
    let attaching = {
        let port = Arc::clone(&port);
        thread::spawn(move || port.attach(listener.accept().unwrap().0))
    };
    install_arrived.recv().unwrap();
    assert!(port.browsers().is_empty() && port.browser_returning());

    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some(RELAY_BROWSER)
    );
    attaching.join().unwrap().unwrap();
    adapter.close();
    let _ = fs::remove_file(policy);
}

/// A pinned browser that stays gone while a different browser is connected is the one case
/// that names another profile. Nothing is sent, and the session is not moved.
#[test]
fn a_pinned_browser_absent_while_another_is_connected_is_the_wrong_profile() {
    let (executor, browser, workspaces, workspace, _) = fixture();
    browser.connect(vec![summary("browser_edge", true)]);
    workspaces
        .pin_browser(workspace.as_str(), "browser_chrome")
        .unwrap();

    let refused = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"timeout_ms":1000}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.effect, Effect::None);
    assert!(refused.repeat_safe);
    assert_eq!(refused.facts["reason"], json!("browser_wrong_profile"));
    assert_eq!(
        refused.summary,
        "This session works in a browser that is not connected; a different browser is."
    );
    assert_eq!(
        refused.next_steps,
        vec![
            "Ask the user to reopen the browser profile this session started in, then repeat the call."
        ]
    );
    assert!(browser.routed().is_empty());
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some("browser_chrome")
    );
}

/// A pinned browser that stays gone with nothing else connected is reported as not connected.
/// It never reaches recovery, which would choose or launch a browser for an already-bound
/// session.
#[test]
fn a_pinned_browser_absent_with_nothing_connected_is_not_connected_and_never_recovered() {
    let (executor, browser, workspaces, workspace, _) = fixture();
    browser.connect(vec![]);
    browser.set_returning(true);
    workspaces
        .pin_browser(workspace.as_str(), "browser_chrome")
        .unwrap();

    let refused = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"timeout_ms":1000}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.effect, Effect::None);
    assert!(refused.repeat_safe);
    assert_eq!(refused.facts["reason"], json!("browser_disconnected"));
    assert_eq!(
        refused.summary,
        "The browser this session works in is not connected. Nothing was sent."
    );
    assert_eq!(
        refused.next_steps,
        vec!["Ask the user to reopen that browser, then repeat the call."]
    );
    assert!(browser.routed().is_empty());
}

/// Evidence of a returning browser earns a wait, not a different answer: when nothing arrives
/// inside the grace, recovery runs as it always did.
#[test]
fn a_returning_browser_that_never_arrives_falls_back_to_recovery() {
    let (governance, policy) = manual_startup();
    let (executor, browser, _, workspace, _) = fixture_with_governance(governance);
    browser.connect(vec![]);
    browser.set_returning(true);

    let refused = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"timeout_ms":1000}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.facts["reason"], json!("browser_startup_manual"));
    let _ = fs::remove_file(policy);
}
