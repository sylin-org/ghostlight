//! Browser availability at the one browser seam: a call waits a bounded grace for a browser that
//! is on its way back, then says what is true (ADR-0196 Decision 12).
//!
//! The relay cases run the executor over a real `RelayBrowserPort` and scripted protocol-4
//! extensions, because what they prove lives in the port: a connection that left moments ago,
//! and a silent connection that answers again.

use std::thread;
use std::time::Instant;

use crate::browser::adapter::chromium::protocol4::assets::page_assets;
use crate::browser::adapter::chromium::protocol4::fake::{FakeExtension, Reply};

use super::*;
use crate::browser::{
    BrowserPort, HeartbeatSettings, RelayBrowserPort, RELAY_RETURN_WINDOW, RETURN_GRACE,
};

/// The browser identity every scripted extension here announces.
const RELAY_BROWSER: &str = "browser_relay";

/// Attach one scripted extension that lists no tabs, announcing itself attended when asked.
fn attach(port: &RelayBrowserPort, attended: bool) -> FakeExtension {
    let (service, extension) = ghostlight_relay::endpoint::pair().unwrap();
    let mut hello = FakeExtension::hello(RELAY_BROWSER);
    hello["attended"] = json!(attended);
    let fake = FakeExtension::connect(
        extension,
        &hello,
        Box::new(|correlation, _| Reply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))),
    );
    port.attach(service).unwrap();
    fake
}

/// Attach one scripted extension from another thread after `delay`, as a relay reconnects.
fn attach_later(
    port: &Arc<RelayBrowserPort>,
    delay: Duration,
) -> thread::JoinHandle<FakeExtension> {
    let port = Arc::clone(port);
    thread::spawn(move || {
        thread::sleep(delay);
        attach(&port, false)
    })
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

/// The same browser reconnecting, as when its extension worker restarts the native port, is
/// waited for. A session pinned to it continues there; it is never told about another profile.
#[test]
fn a_pinned_browser_that_reconnects_takes_the_call() {
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
    let (executor, workspaces, workspace) =
        relay_fixture(Arc::clone(&port), GovernanceFacade::new(None, None));

    let first = attach(&port, false);
    workspaces
        .pin_browser(workspace.as_str(), RELAY_BROWSER)
        .unwrap();
    first.close();
    await_unregistered(&port);
    let second = attach_later(&port, Duration::from_millis(300));

    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert_eq!(listed.facts["browsers"][0]["browser"], json!(RELAY_BROWSER));
    drop(second.join().unwrap());
}

/// A pinned browser whose liveness lapsed is waited for, and the call goes through as soon as it
/// answers a heartbeat again.
#[test]
fn a_silent_pinned_browser_that_answers_again_takes_the_call() {
    let port = Arc::new(RelayBrowserPort::with_heartbeat_settings(
        "service_test".into(),
        page_assets(),
        HeartbeatSettings {
            interval: Duration::from_millis(20),
            timeout: Duration::from_millis(150),
        },
    ));
    let (executor, workspaces, workspace) =
        relay_fixture(Arc::clone(&port), GovernanceFacade::new(None, None));
    let fake = Arc::new(attach(&port, false));
    workspaces
        .pin_browser(workspace.as_str(), RELAY_BROWSER)
        .unwrap();

    fake.acknowledge_heartbeats(false);
    let until = Instant::now() + Duration::from_secs(5);
    while !port.browsers().is_empty() {
        assert!(Instant::now() < until, "silence never hid the browser");
        thread::sleep(Duration::from_millis(10));
    }
    let answering = {
        let fake = Arc::clone(&fake);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            fake.acknowledge_heartbeats(true);
        })
    };

    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    answering.join().unwrap();
}

/// Right after a service start, the browser's relay is still reconnecting. A session with no
/// browser yet waits for it inside the grace, and recovery never runs.
#[test]
fn a_fresh_service_waits_for_the_returning_browser_instead_of_recovering() {
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
    let (governance, policy) = manual_startup();
    let (executor, workspaces, workspace) = relay_fixture(Arc::clone(&port), governance);

    let reconnecting = attach_later(&port, Duration::from_millis(300));

    let listed = list_tabs(&executor, &workspace);
    assert_eq!(listed.status, Status::Succeeded, "{}", listed.summary);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some(RELAY_BROWSER)
    );
    drop(reconnecting.join().unwrap());
    let _ = fs::remove_file(policy);
}

/// A browser the person used and then closed is not on its way back. Once its departure is
/// older than the grace, a session with no browser goes straight to browser startup, which keeps
/// its whole launch budget instead of the second or so the grace would have left it. Attention
/// the browser reported while it was open is not evidence.
#[test]
fn a_browser_closed_longer_ago_than_the_grace_is_not_waited_for() {
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
    let (governance, policy) = manual_startup();
    let (executor, _, workspace) = relay_fixture(Arc::clone(&port), governance);
    // Long past the port's own reconnect window, so the departure is the only evidence.
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
    let port = Arc::new(RelayBrowserPort::new("service_test".into(), page_assets()));
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

/// The refusal a session with no browser hears under manual startup, with the setup record
/// present or not and the profile scan finding the extension or not.
fn manual_refusal(
    setup_complete: bool,
    extension_in_a_profile: bool,
) -> crate::work::result::InvocationResult {
    let scratch = crate::install::scratch::Scratch::new();
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
    let (mut executor, _, workspace) = relay_fixture(Arc::clone(&port), governance);
    executor
        .workbench
        .attach_setup(crate::workbench::setup::SetupProgress::load(&record));
    executor.extension_probe = Arc::new(move || extension_in_a_profile);
    port.backdate(RETURN_GRACE);
    let refused = navigate(&executor, &workspace);
    let _ = fs::remove_file(policy);
    assert_eq!(refused.facts["reason"], json!("browser_startup_manual"));
    refused
}

/// While setup is incomplete and no profile has the extension, the no-browser answer says where
/// the extension comes from. The summary is the one it always was.
#[test]
fn the_no_browser_answer_names_the_store_while_setup_is_incomplete_and_no_profile_has_it() {
    let refused = manual_refusal(false, false);
    assert!(refused.summary.starts_with("No browser is connected."));
    assert_eq!(refused.next_steps.len(), 1, "{:?}", refused.next_steps);
    assert!(
        refused.next_steps[0].contains(crate::language::setup::STORE_LISTING_URL),
        "{}",
        refused.next_steps[0]
    );
    assert!(refused.next_steps[0].contains("Ghostlight window"));
}

/// An extension in some profile, or a finished setup, leaves the answer as it was.
#[test]
fn the_no_browser_answer_is_unchanged_with_an_extension_or_a_finished_setup() {
    assert!(manual_refusal(false, true).next_steps.is_empty());
    assert!(manual_refusal(true, false).next_steps.is_empty());
    assert!(manual_refusal(true, true).next_steps.is_empty());
}

/// Governance that starts a browser on demand, the Windows default, and an executor whose launch
/// succeeds but never brings an adapter: Edge on a machine without the extension.
fn launch_without_extension(
    setup_complete: bool,
    extension_in_a_profile: bool,
) -> crate::work::result::InvocationResult {
    let scratch = crate::install::scratch::Scratch::new();
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
        Arc::clone(&port),
        GovernanceFacade::new(Some(policy.clone()), None),
    );
    executor
        .workbench
        .attach_setup(crate::workbench::setup::SetupProgress::load(&record));
    executor.extension_probe = Arc::new(move || extension_in_a_profile);
    executor.recovery.set_test_launch_without_adapter();
    port.backdate(RETURN_GRACE);
    let refused = navigate_for(&executor, &workspace, 300);
    let _ = fs::remove_file(policy);
    assert_eq!(
        refused.facts["reason"],
        json!("browser_handshake_timeout"),
        "{:?}",
        refused.facts
    );
    refused
}

/// A browser Ghostlight started whose adapter never arrived, while setup is incomplete and no
/// profile has the extension, sends the person to the store: the extension is what is missing.
#[test]
fn a_launched_browser_without_the_extension_names_the_store_while_setup_is_incomplete() {
    let refused = launch_without_extension(false, false);
    assert!(refused
        .summary
        .starts_with("The browser started, but its Ghostlight adapter did not connect"));
    assert_eq!(refused.next_steps.len(), 1, "{:?}", refused.next_steps);
    assert!(
        refused.next_steps[0].contains(crate::language::setup::STORE_LISTING_URL),
        "{}",
        refused.next_steps[0]
    );
}

/// With the extension in some profile, or setup finished, the timeout keeps its own step.
#[test]
fn a_launched_browser_that_timed_out_keeps_its_own_step_with_an_extension_or_a_finished_setup() {
    for (complete, present) in [(false, true), (true, false), (true, true)] {
        let refused = launch_without_extension(complete, present);
        assert_eq!(
            refused.next_steps,
            vec!["Start the selected browser yourself, then repeat the call."]
        );
    }
}
