//! Browser availability at the one browser seam: a call waits a bounded grace for a browser that
//! is on its way back, starts a session's own browser again when Ghostlight can, and otherwise
//! says what is true (ADR-0196 Decision 12 and its 2026-10-17 amendment).
//!
//! The relay cases run the executor over a real `RelayBrowserPort` and scripted protocol-4
//! extensions, because what they prove lives in the port: a connection that left moments ago,
//! and a silent connection that answers again.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

use crate::browser::adapter::chromium::protocol4::assets::page_assets;
use crate::browser::adapter::chromium::protocol4::fake::{FakeExtension, Reply};

use super::protocol4::ordinary;
use super::*;
use crate::browser::{
    BrowserPort, BrowserSummary, HeartbeatSettings, RelayBrowserPort, RELAY_RETURN_WINDOW,
    RETURN_GRACE,
};
use crate::language::outcome::{BrowserUnavailableReason, Refusal};

/// The browser identity every scripted extension here announces.
const RELAY_BROWSER: &str = "browser_relay";

/// Attach one scripted extension that answers as an ordinary browser does, announcing itself
/// attended when asked.
fn attach(port: &RelayBrowserPort, attended: bool) -> FakeExtension {
    let (service, extension) = ghostlight_relay::endpoint::pair().unwrap();
    let mut hello = FakeExtension::hello(RELAY_BROWSER);
    hello["attended"] = json!(attended);
    let fake = FakeExtension::connect(
        extension,
        &hello,
        Box::new(|correlation, primitive| Reply::Now(ordinary(correlation, primitive))),
    );
    port.attach(service).unwrap();
    fake
}

/// A port that forwards to `inner` and runs its armed arrival the moment a call starts waiting
/// for a browser, so a test proves the call waited instead of timing a reconnect.
struct ArrivesWhenAwaited {
    inner: Arc<dyn BrowserPort>,
    arrival: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl ArrivesWhenAwaited {
    fn over(inner: Arc<dyn BrowserPort>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            arrival: Mutex::new(None),
        })
    }

    fn arm(&self, arrival: impl FnOnce() + Send + 'static) {
        *self.arrival.lock().unwrap() = Some(Box::new(arrival));
    }

    /// Whether the armed arrival ran, because a call waited.
    fn arrived(&self) -> bool {
        self.arrival.lock().unwrap().is_none()
    }

    fn arrive(&self) {
        let arrival = self.arrival.lock().unwrap().take();
        if let Some(arrival) = arrival {
            arrival();
        }
    }
}

impl BrowserPort for ArrivesWhenAwaited {
    fn call(
        &self,
        browser: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.inner.call(browser, command, deadline, cancelled)
    }
    fn call_guarded(
        &self,
        browser: &str,
        command: BrowserCommand,
        dispatch: crate::browser::BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.inner.call_guarded(browser, command, dispatch)
    }
    fn browsers(&self) -> Vec<BrowserSummary> {
        self.inner.browsers()
    }
    fn last_connected_browser(&self) -> Option<String> {
        self.inner.last_connected_browser()
    }
    fn installed_browser_of(&self, browser: &str) -> Option<String> {
        self.inner.installed_browser_of(browser)
    }
    fn arrivals_of(&self, installed: &str) -> u64 {
        self.inner.arrivals_of(installed)
    }
    fn life_of(&self, browser: &str) -> Option<String> {
        self.inner.life_of(browser)
    }
    fn await_available(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> crate::browser::Availability {
        self.arrive();
        self.inner.await_available(browser, until, cancelled)
    }
    fn await_returning(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> crate::browser::Availability {
        self.arrive();
        self.inner.await_returning(browser, until, cancelled)
    }
    fn await_any(
        &self,
        until: Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Vec<BrowserSummary> {
        self.arrive();
        self.inner.await_any(until, cancelled)
    }
    fn browser_returning(&self) -> bool {
        self.inner.browser_returning()
    }
    fn returning_of(&self, browser: &str) -> bool {
        self.inner.returning_of(browser)
    }
    fn publish_control_state(
        &self,
        state: crate::browser::adapter::RuntimeControlState,
    ) -> Result<(), BrowserError> {
        self.inner.publish_control_state(state)
    }
}

/// An executor over a real relay port, with fixed installed-browser facts so recovery, if it
/// ran, would never inspect or launch anything on this machine.
fn relay_fixture(
    port: Arc<dyn BrowserPort>,
    governance: GovernanceFacade,
) -> (
    ApplicationExecutor,
    WorkspaceStore,
    crate::workspace::WorkspaceId,
) {
    let workspaces = WorkspaceStore::default();
    let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp);
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
            extension: true,
        }]);
    (executor, workspaces, workspace)
}

/// Governance that leaves browser startup to the person, so a call that reached recovery would
/// answer `browser_startup_manual` instead of quietly succeeding.
fn manual_startup() -> (GovernanceFacade, Scratch) {
    let policy = temporary_policy("manual-browser-startup");
    fs::write(
        &policy,
        all_open_policy_with(r#"[{"key":"browser.startup","value":"manual","level":"mandatory"}]"#),
    )
    .unwrap();
    (GovernanceFacade::new(Some(policy.to_path_buf())), policy)
}

fn list_tabs(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_tabs",
        json!({"action":"list"}),
        &CancellationToken::default(),
    )
}

/// A navigation with the default deadline, from a session that works in no browser yet.
fn navigate(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        &CancellationToken::default(),
    )
}

/// A navigation bounded by `timeout_ms`, from a session that works in no browser yet.
fn navigate_for(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
    timeout_ms: u64,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"timeout_ms":timeout_ms}),
        &CancellationToken::default(),
    )
}

/// Bind one tab of `browser` to the session, as its first open would, so the session works in
/// that browser and its calls cross to it.
fn bind_tab(workspaces: &WorkspaceStore, workspace: &crate::workspace::WorkspaceId, browser: &str) {
    workspaces
        .acquire(workspace)
        .unwrap()
        .add_tab(
            browser,
            &PhysicalTab {
                tab_id: 12,
                url: "https://example.com/".into(),
                title: "Example".into(),
                active: true,
                readiness: BrowserReadiness::Complete,
            },
        )
        .unwrap();
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

/// Governance that starts a browser on demand, the Windows default, written out so the tests
/// read the same on every platform.
fn on_demand_startup() -> (GovernanceFacade, Scratch) {
    let policy = temporary_policy("on-demand-browser-startup");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"browser.startup","value":"on_demand","level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    (GovernanceFacade::new(Some(policy.to_path_buf())), policy)
}

/// The installed browser the relay fixture's scripted extensions connect as: their hello
/// reports the name Chrome.
const RELAY_INSTALLED: &str = "chrome";

/// Installed-browser facts naming Chrome, the browser the scripted extensions connect as.
fn chrome_installed() -> Vec<RecoveryCandidate> {
    vec![RecoveryCandidate {
        id: RELAY_INSTALLED.into(),
        name: "Google Chrome".into(),
        package: BrowserPackage::Native,
        package_detail: "Google Chrome native package".into(),
        registration: NativeHostState::Current,
        ordinary_executable: Some(PathBuf::from("chrome")),
        extension: true,
    }]
}

/// A fake-port executor whose session works in `browser_chrome`, a browser that connected as
/// the fixture's one installed browser and is gone now. Each launch counts, then runs
/// `arrive` with the port.
fn closed_session_browser(
    governance: GovernanceFacade,
    arrive: impl Fn(&FakeBrowser) + Send + Sync + 'static,
) -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    crate::workspace::WorkspaceId,
    Arc<AtomicUsize>,
) {
    let (mut executor, browser, workspaces, workspace, _) = fixture_with_governance(governance);
    browser.connect(vec![]);
    browser.remember_installed("browser_chrome", "chromium");
    bind_tab(&workspaces, &workspace, "browser_chrome");
    let launches = Arc::new(AtomicUsize::new(0));
    {
        let (browser, launches) = (Arc::clone(&browser), Arc::clone(&launches));
        executor.recovery.set_test_launch(move || {
            launches.fetch_add(1, Ordering::SeqCst);
            arrive(&browser);
        });
    }
    (executor, browser, workspace, launches)
}

fn navigate_within_a_second(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
) -> crate::work::result::InvocationResult {
    navigate_for(executor, workspace, 1000)
}

/// A relay port and its waiting wrapper.
fn relay_pair() -> (Arc<RelayBrowserPort>, Arc<ArrivesWhenAwaited>) {
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
    let waiting = ArrivesWhenAwaited::over(port.clone());
    (port, waiting)
}

/// Count launches into `launches`, attaching a scripted browser to `port` on each when `port`
/// is given.
fn counting_launches(
    executor: &mut ApplicationExecutor,
    port: Option<Arc<RelayBrowserPort>>,
) -> (Arc<AtomicUsize>, Arc<Mutex<Vec<FakeExtension>>>) {
    let launches = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Mutex::new(Vec::new()));
    let (counted, kept) = (Arc::clone(&launches), Arc::clone(&started));
    executor.recovery.set_test_launch(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        if let Some(port) = &port {
            kept.lock().unwrap().push(attach(port, false));
        }
    });
    (launches, started)
}

/// ADR-0196 Decision 12 and its 2026-10-17 amendment (ACCEPTANCE Contract 17, 20): a session's
/// browser that left is waited for while it is on its way back, and started again only when
/// Ghostlight knows which installed browser it is and may start it.
///
/// - Back within the grace, by reconnecting or answering a heartbeat again: the call waits and
///   goes through there, and nothing is started even under on-demand startup.
/// - Gone longer than the grace under on-demand startup: it is started at once, without spending
///   the grace, and the call goes through in it, also beside another connected browser.
/// - Started but connecting as another profile: the call waits out its deadline for its own
///   profile and is the wrong profile, sending nothing; when the other profile only comes
///   first, the call goes to its own. Started and never connecting: the handshake timeout.
/// - Never started under manual startup, or when this service never saw which installed browser
///   it is: not connected, or the wrong profile when another browser is connected, each with its
///   own sentence and steps, nothing sent, the session not moved, and no browser in the record.
#[test]
fn a_session_browser_that_left_is_waited_for_or_started_again_only_when_known() {
    // Back within the grace: by reconnecting, or by answering a heartbeat again.
    let (governance, _policy) = on_demand_startup();
    let (port, waiting) = relay_pair();
    let (mut executor, workspaces, workspace) = relay_fixture(waiting.clone(), governance);
    executor.recovery.set_test_candidates(chrome_installed());
    let (launches, _) = counting_launches(&mut executor, None);
    let first = attach(&port, false);
    bind_tab(&workspaces, &workspace, RELAY_BROWSER);
    first.close();
    await_unregistered(&port);
    let second = Arc::new(Mutex::new(None));
    {
        let (port, second) = (port.clone(), second.clone());
        waiting.arm(move || *second.lock().unwrap() = Some(attach(&port, false)));
    }
    let listed = list_tabs(&executor, &workspace);
    assert!(waiting.arrived(), "the call waited for its browser");
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert_eq!(listed.facts["browsers"][0]["browser"], json!(RELAY_BROWSER));
    assert_eq!(launches.load(Ordering::SeqCst), 0);

    let port = Arc::new(RelayBrowserPort::with_heartbeat_settings(
        "service_test".into(),
        page_assets(),
        HeartbeatSettings {
            interval: Duration::from_millis(20),
            timeout: Duration::from_millis(150),
        },
    ));
    let waiting = ArrivesWhenAwaited::over(port.clone());
    let (executor, workspaces, workspace) =
        relay_fixture(waiting.clone(), GovernanceFacade::new(None));
    let fake = Arc::new(attach(&port, false));
    bind_tab(&workspaces, &workspace, RELAY_BROWSER);
    fake.acknowledge_heartbeats(false);
    let until = Instant::now() + Duration::from_secs(5);
    while !port.browsers().is_empty() {
        assert!(Instant::now() < until, "silence never hid the browser");
        thread::sleep(Duration::from_millis(10));
    }
    {
        let fake = Arc::clone(&fake);
        waiting.arm(move || fake.acknowledge_heartbeats(true));
    }
    let listed = list_tabs(&executor, &workspace);
    assert!(waiting.arrived());
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);

    // Gone longer than the grace: started at once.
    let (governance, _policy) = on_demand_startup();
    let (port, waiting) = relay_pair();
    let (mut executor, workspaces, workspace) = relay_fixture(waiting, governance);
    executor.recovery.set_test_candidates(chrome_installed());
    let (launches, _started) = counting_launches(&mut executor, Some(port.clone()));
    let first = attach(&port, false);
    bind_tab(&workspaces, &workspace, RELAY_BROWSER);
    assert_eq!(
        port.installed_browser_of(RELAY_BROWSER).as_deref(),
        Some(RELAY_INSTALLED)
    );
    first.close();
    await_unregistered(&port);
    port.backdate(RETURN_GRACE);
    assert!(!port.browser_returning());
    let began = Instant::now();
    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert_eq!(listed.facts["browsers"][0]["browser"], json!(RELAY_BROWSER));
    assert_eq!(launches.load(Ordering::SeqCst), 1);
    assert!(
        began.elapsed() < RETURN_GRACE / 2,
        "took {:?}",
        began.elapsed()
    );
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some(RELAY_BROWSER)
    );

    let (governance, _policy) = on_demand_startup();
    let (executor, browser, workspace, launches) = closed_session_browser(governance, |browser| {
        browser.connect(vec![
            summary("browser_edge", false),
            summary("browser_chrome", true),
        ]);
    });
    browser.connect(vec![summary("browser_edge", true)]);
    browser.remember_installed("browser_edge", "edge");
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = navigate_within_a_second(&executor, &workspace);
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(launches.load(Ordering::SeqCst), 1);
    assert_eq!(browser.routed(), vec!["browser_chrome".to_owned()]);

    // Started as another profile: the wrong profile after the deadline.
    let (governance, _policy) = on_demand_startup();
    let (executor, browser, workspace, launches) = closed_session_browser(governance, |browser| {
        browser.remember_installed("browser_other_profile", "chromium");
        browser.connect(vec![summary("browser_other_profile", true)]);
    });
    let began = Instant::now();
    let refused = navigate_within_a_second(&executor, &workspace);
    assert!(
        began.elapsed() >= Duration::from_millis(900),
        "took {:?}",
        began.elapsed()
    );
    assert_eq!(refused.effect, Effect::None);
    assert_eq!(refused.facts["reason"], json!("browser_wrong_profile"));
    assert_eq!(launches.load(Ordering::SeqCst), 1);
    assert!(browser.routed().is_empty());

    // Another profile first, then its own: the call goes to its own.
    let (governance, _policy) = on_demand_startup();
    let browser = Arc::new(FakeBrowser::default());
    let waiting = ArrivesWhenAwaited::over(browser.clone());
    let workspaces = WorkspaceStore::default();
    let (mut executor, workspace, _) = executor_on(governance, &workspaces, waiting.clone());
    browser.connect(vec![]);
    browser.remember_installed("browser_chrome", "chromium");
    browser.remember_installed("browser_other_profile", "chromium");
    bind_tab(&workspaces, &workspace, "browser_chrome");
    {
        let (fake, waiting) = (Arc::clone(&browser), Arc::clone(&waiting));
        executor.recovery.set_test_launch(move || {
            fake.connect(vec![summary("browser_other_profile", true)]);
            let fake = Arc::clone(&fake);
            waiting.arm(move || {
                fake.connect(vec![
                    summary("browser_other_profile", true),
                    summary("browser_chrome", false),
                ]);
            });
        });
    }
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = navigate_within_a_second(&executor, &workspace);
    assert!(waiting.arrived(), "the call waited past the other profile");
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(browser.routed(), vec!["browser_chrome".to_owned()]);

    // Started and never connecting: the handshake timeout.
    let (governance, _policy) = on_demand_startup();
    let (executor, browser, workspace, launches) = closed_session_browser(governance, |_| {});
    let refused = navigate_within_a_second(&executor, &workspace);
    assert_eq!(refused.effect, Effect::None);
    assert_eq!(refused.facts["reason"], json!("browser_handshake_timeout"));
    assert_eq!(launches.load(Ordering::SeqCst), 1);
    assert!(browser.routed().is_empty());

    // Never started: under manual startup, or with no known installed browser.
    for known in [true, false] {
        let (governance, _policy) = if known {
            manual_startup()
        } else {
            on_demand_startup()
        };
        let (mut executor, browser, workspaces, workspace, audit) =
            fixture_with_governance(governance);
        browser.connect(vec![]);
        browser.set_returning(true);
        if known {
            browser.remember_installed("browser_chrome", "chromium");
        }
        bind_tab(&workspaces, &workspace, "browser_chrome");
        let (launches, _) = counting_launches(&mut executor, None);
        for (reason, connected) in [
            (BrowserUnavailableReason::NotConnected, vec![]),
            (
                BrowserUnavailableReason::WrongProfile,
                vec![summary("browser_edge", true)],
            ),
        ] {
            browser.connect(connected);
            let refused = navigate_within_a_second(&executor, &workspace);
            let expected = Refusal::BrowserUnavailable { reason };
            let label = format!("known={known} {reason:?}");
            assert_eq!(refused.effect, Effect::None, "{label}");
            assert!(refused.repeat_safe, "{label}");
            assert_eq!(refused.summary, expected.summary(), "{label}");
            assert_eq!(refused.next_steps, expected.next_steps(), "{label}");
            assert!(browser.routed().is_empty(), "{label}");
            assert_eq!(
                workspaces.browser_of(workspace.as_str()).as_deref(),
                Some("browser_chrome"),
                "{label}"
            );
            assert_eq!(
                audit.0.lock().unwrap().last().unwrap().browser,
                None,
                "{label}"
            );
        }
        assert_eq!(launches.load(Ordering::SeqCst), 0, "known={known}");
    }
}

/// ADR-0196 Decision 12 (ACCEPTANCE Contract 17): a session with no browser waits only while
/// one is evidently returning.
///
/// Right after a service start a reconnecting relay is waited for and recovery never runs; the
/// empty session's list before that answers at once and binds nothing, with or without a
/// browser on its way. A browser closed longer ago than the grace is not waited for, even if it
/// once reported attention; a freshly published port stops waiting at the end of its reconnect
/// window; and evidence that never turns into a browser falls back to recovery.
#[test]
fn a_session_with_no_browser_waits_only_while_one_is_returning() {
    let (governance, _policy) = manual_startup();
    let (port, waiting) = relay_pair();
    let (executor, workspaces, workspace) = relay_fixture(waiting.clone(), governance);
    port.endpoint_published();
    let began = Instant::now();
    let listed = list_tabs(&executor, &workspace);
    assert!(
        began.elapsed() < RETURN_GRACE / 2,
        "the list waited {:?}",
        began.elapsed()
    );
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert_eq!(listed.facts["tabs"], json!([]));
    assert_eq!(
        listed.next_steps,
        crate::language::outcome::Outcome::TabsListed { count: 0 }.next_steps()
    );
    assert_eq!(workspaces.browser_of(workspace.as_str()), None);
    let returned = Arc::new(Mutex::new(None));
    {
        let (port, returned) = (port.clone(), returned.clone());
        waiting.arm(move || *returned.lock().unwrap() = Some(attach(&port, false)));
    }
    let opened = navigate(&executor, &workspace);
    assert!(waiting.arrived(), "the first open waited for the relay");
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some(RELAY_BROWSER)
    );

    // Closed longer ago than the grace.
    let (governance, _policy) = manual_startup();
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
    let (executor, _, workspace) = relay_fixture(port.clone(), governance);
    port.backdate(RETURN_GRACE);
    let fake = attach(&port, true);
    assert!(
        port.browsers()[0].attended,
        "the browser reported attention"
    );
    fake.close();
    await_unregistered(&port);
    assert!(
        port.browser_returning(),
        "a connection that ended moments ago may come back"
    );
    port.backdate(RETURN_GRACE);
    assert!(!port.browser_returning());
    let started = Instant::now();
    let refused = navigate(&executor, &workspace);
    assert_eq!(refused.facts["reason"], json!("browser_startup_manual"));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "took {:?}",
        started.elapsed()
    );

    // A freshly published port with no relay.
    let (governance, _policy) = manual_startup();
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
    let (executor, _, workspace) = relay_fixture(port.clone(), governance);
    port.backdate(RETURN_GRACE);
    assert!(!port.browser_returning(), "an old port with no relay");
    let published = Instant::now();
    port.endpoint_published();
    assert!(port.browser_returning(), "relays may be reconnecting");
    let refused = navigate(&executor, &workspace);
    assert_eq!(refused.facts["reason"], json!("browser_startup_manual"));
    assert!(
        published.elapsed() >= RELAY_RETURN_WINDOW,
        "the call waited out the window"
    );
    assert!(
        published.elapsed() < RETURN_GRACE / 2,
        "the wait ended with the window, took {:?}",
        published.elapsed()
    );

    // Evidence that never arrives.
    let (governance, _policy) = manual_startup();
    let (executor, browser, _, workspace, _) = fixture_with_governance(governance);
    browser.connect(vec![]);
    browser.set_returning(true);
    let refused = navigate_within_a_second(&executor, &workspace);
    assert_eq!(refused.facts["reason"], json!("browser_startup_manual"));
}

/// The refusal a session with no browser hears under manual startup, with the setup record
/// present or not and the profile scan finding the extension or not.
fn manual_refusal(
    setup_complete: bool,
    extension_in_a_profile: bool,
) -> crate::work::result::InvocationResult {
    let scratch = crate::scratch::Scratch::new();
    let record = scratch.path().join("setup.json");
    if setup_complete {
        crate::install::setup_state::record_if_absent(
            &record,
            &crate::install::setup_state::SetupRecord {
                first_request_ms: 1,
            },
        )
        .unwrap();
    }
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
    let (governance, policy) = manual_startup();
    let (mut executor, _, workspace) = relay_fixture(port.clone(), governance);
    executor
        .workbench
        .attach_setup(crate::workbench::setup::SetupProgress::load(&record));
    executor.extension_probe = Arc::new(move || extension_in_a_profile);
    port.backdate(RETURN_GRACE);
    let refused = navigate(&executor, &workspace);
    drop(policy);
    assert_eq!(refused.facts["reason"], json!("browser_startup_manual"));
    refused
}

/// Governance that starts a browser on demand, the Windows default, and an executor whose launch
/// of Chrome succeeds but never brings an adapter, with the extension in a profile of it or not.
fn launch_without_adapter(
    setup_complete: bool,
    extension_in_a_profile: bool,
) -> crate::work::result::InvocationResult {
    let scratch = crate::scratch::Scratch::new();
    let record = scratch.path().join("setup.json");
    if setup_complete {
        crate::install::setup_state::record_if_absent(
            &record,
            &crate::install::setup_state::SetupRecord {
                first_request_ms: 1,
            },
        )
        .unwrap();
    }
    let policy = temporary_policy("on-demand-browser-startup");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"browser.startup","value":"on_demand","level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
    let (mut executor, _, workspace) = relay_fixture(
        port.clone(),
        GovernanceFacade::new(Some(policy.to_path_buf())),
    );
    executor
        .workbench
        .attach_setup(crate::workbench::setup::SetupProgress::load(&record));
    executor.extension_probe = Arc::new(move || extension_in_a_profile);
    executor.recovery.set_test_candidates(
        chrome_installed()
            .into_iter()
            .map(|chrome| RecoveryCandidate {
                extension: extension_in_a_profile,
                ..chrome
            })
            .collect(),
    );
    executor.recovery.set_test_launch_without_adapter();
    port.backdate(RETURN_GRACE);
    let refused = navigate_for(&executor, &workspace, 300);
    drop(policy);
    refused
}

/// ADR-0196 Decision 12 and S-D7: a no-browser answer names where the extension comes from only
/// while it can help.
///
/// Under manual startup, the answer names the store and the Ghostlight window while setup is
/// incomplete and no profile has the extension, with its usual summary, and is unchanged with an
/// extension in a profile or a finished setup. A browser Ghostlight started whose profiles hold
/// no extension is the missing extension and names the store whether or not setup finished; with
/// the extension in a profile, a missing adapter is the handshake timeout with its own step.
#[test]
fn the_no_browser_answers_name_the_store_only_while_it_can_help() {
    let refused = manual_refusal(false, false);
    assert!(refused.summary.starts_with("No browser is connected."));
    assert_eq!(refused.next_steps.len(), 1, "{:?}", refused.next_steps);
    assert!(refused.next_steps[0].contains(crate::language::setup::STORE_LISTING_URL));
    assert!(refused.next_steps[0].contains("Ghostlight window"));
    for (complete, extension) in [(false, true), (true, false), (true, true)] {
        assert!(
            manual_refusal(complete, extension).next_steps.is_empty(),
            "complete={complete} extension={extension}"
        );
    }

    for complete in [false, true] {
        let refused = launch_without_adapter(complete, false);
        assert_eq!(
            refused.facts["reason"],
            json!("browser_extension_absent"),
            "{:?}",
            refused.facts
        );
        assert_eq!(refused.next_steps.len(), 1, "{:?}", refused.next_steps);
        assert!(refused.next_steps[0].contains(crate::language::setup::STORE_LISTING_URL));

        let refused = launch_without_adapter(complete, true);
        assert_eq!(refused.facts["reason"], json!("browser_handshake_timeout"));
        assert_eq!(
            refused.next_steps,
            vec![crate::language::outcome::ASK_FOR_THAT_BROWSER]
        );
    }
}

/// A browser tab the scripted fake opens, at `url`.
fn opened_tab(id: u64, url: &str) -> Result<BrowserOutcome, BrowserError> {
    Ok(BrowserOutcome::TabOpened {
        tab: tab(id, url),
        committed_urls: vec![url.into()],
    })
}

/// ADR-0196 Decision 12 and W-D5 (ACCEPTANCE Contract 19): a session binds its browser with its
/// first tab and leaves it only when it is gone and the agent names another.
///
/// An empty session's list and recording status ask no browser and bind none, so it opens its
/// first tab in the browser it names. Once bound, it stays: after that tab closes a call naming
/// another browser is refused as pinned, and neither a list nor a later open moves it. When its
/// browser is gone and another is connected, naming that one moves the session there, and the
/// gone browser's tab reads as no longer open.
#[test]
fn a_session_binds_its_browser_with_its_first_tab_and_leaves_it_only_when_it_is_gone() {
    let (executor, browser, workspaces, workspace, _) = fixture();
    browser.connect(vec![
        summary("browser_chrome", true),
        summary("browser_edge", false),
    ]);
    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.facts["tabs"], json!([]));
    assert_eq!(
        listed.facts["browsers"][1]["browser"],
        json!("browser_edge")
    );
    let status = executor.execute(
        &workspace,
        "browser_record",
        json!({"action":"status"}),
        &CancellationToken::default(),
    );
    assert_eq!(status.facts["reason"], "not_found", "{}", status.summary);
    assert!(browser.routed().is_empty(), "nothing asked a browser");
    assert_eq!(workspaces.browser_of(workspace.as_str()), None);
    browser.push(opened_tab(7, "https://example.com/"));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"browser":"browser_edge"}),
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    assert_eq!(browser.routed(), vec!["browser_edge"]);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some("browser_edge")
    );

    // Pinned: never rebound while its browser is there.
    workspaces.apply_browser_close("browser_edge", 7);
    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    let refused = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"browser":"browser_chrome"}),
        &CancellationToken::default(),
    );
    assert_eq!(refused.effect, Effect::None);
    assert_eq!(refused.facts["reason"], json!("browser_pinned"));
    assert_eq!(browser.routed(), vec!["browser_edge"]);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some("browser_edge")
    );

    // Gone, and another named: the session moves.
    let (executor, browser, workspaces, workspace, _) = fixture();
    bind_tab(&workspaces, &workspace, "browser_chrome");
    browser.connect(vec![summary("browser_edge", true)]);
    browser.push(opened_tab(7, "https://example.com/"));
    let moved = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"browser":"browser_edge"}),
        &CancellationToken::default(),
    );
    assert_eq!(moved.status, Status::Succeeded, "{}", moved.summary);
    assert_eq!(browser.routed(), vec!["browser_edge".to_owned()]);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some("browser_edge")
    );
    assert_eq!(workspaces.physical_tabs(workspace.as_str()), vec![7]);
}

/// Defect C3b (ADR-0203): a wait pinned to one browser reads only its own installed browser's
/// evidence. Another installed browser's departure does not hold it, so a pinned start keeps its
/// budget, while another profile of the same installed browser still does.
#[test]
fn a_pinned_wait_reads_only_its_own_browsers_departures() {
    let port = RelayBrowserPort::new("service_test".into(), page_assets());
    let leave = |browser_id: &str, name: &str| {
        let (service, extension) = ghostlight_relay::endpoint::pair().unwrap();
        let mut hello = FakeExtension::hello(browser_id);
        hello["browser_name"] = json!(name);
        let fake = FakeExtension::connect(
            extension,
            &hello,
            Box::new(|correlation, primitive| Reply::Now(ordinary(correlation, primitive))),
        );
        port.attach(service).unwrap();
        fake.close();
        let until = Instant::now() + Duration::from_secs(5);
        while port.is_registered(browser_id) {
            assert!(Instant::now() < until, "{browser_id} stayed registered");
            thread::sleep(Duration::from_millis(10));
        }
    };
    leave("browser_pinned", "Google Chrome");
    port.backdate(RETURN_GRACE);
    assert!(!port.returning_of("browser_pinned"), "nothing is returning");

    leave("browser_edge", "Microsoft Edge");
    assert!(port.browser_returning(), "a browser left moments ago");
    assert!(
        !port.returning_of("browser_pinned"),
        "another browser's departure does not hold the pinned wait"
    );

    leave("browser_other_profile", "Google Chrome");
    assert!(
        port.returning_of("browser_pinned"),
        "another profile of the same browser still holds it"
    );
}
