//! Routing between browsers, connection replacement, and availability on the relay port.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::super::{
    choose_browser, testing, AdapterRegistry, Availability, BrowserError, BrowserPort,
    HeartbeatSettings, RelayBrowserPort,
};
use super::{attach, page_assets, port, tabs_titled, TEST_BROWSER};
use crate::browser::adapter::chromium::protocol4::fake::Reply;
use crate::browser::adapter::{BrowserCommand, BrowserOutcome};

#[test]
fn two_browsers_are_two_adapters_and_each_keeps_its_own_work() {
    let port = port();
    let _chrome = attach(&port, "browser_chrome", tabs_titled("browser_chrome"));
    let _edge = attach(&port, "browser_edge", tabs_titled("browser_edge"));

    // Both are connected at once. Neither replaced the other, because they are two browsers.
    let connected: Vec<_> = port
        .connected_browsers()
        .into_iter()
        .map(|browser| browser.id)
        .collect();
    assert_eq!(connected, vec!["browser_chrome", "browser_edge"]);

    // A request reaches the browser it named, and each browser's tab 5 is its own.
    for browser in ["browser_chrome", "browser_edge"] {
        let Ok(BrowserOutcome::Tabs { tabs }) = port.call(
            browser,
            BrowserCommand::ListTabs,
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(false),
        ) else {
            panic!("each browser answers its own request");
        };
        assert_eq!(tabs[0].title, browser);
    }
}

#[test]
fn a_second_connection_from_one_browser_collapses_onto_the_first() {
    let port = port();
    // One browser opening two native ports is the failure this design exists to make
    // impossible: the browser mints its identity once, so the second connection is the same
    // adapter arriving twice, not a second browser.
    let first = attach(&port, TEST_BROWSER, Box::new(|_, _| Reply::Never));
    let second = attach(&port, TEST_BROWSER, Box::new(|_, _| Reply::Never));

    // One browser is still one browser, served by its newest connection.
    assert_eq!(port.connected_browsers().len(), 1);

    // The replaced connection is closed rather than abandoned. Left open, it would keep its
    // relay process alive and the browser's stale native port with it, and every request
    // written into it would be dropped in silence instead of refused.
    assert!(first.wait(|seen| seen.closed).closed);
    assert!(!second.wait(|_| true).closed);
}

#[test]
fn waiting_for_a_browser_tells_a_silent_connection_from_an_absent_one() {
    let port = RelayBrowserPort::with_heartbeat_settings(
        "service_test".into(),
        page_assets(),
        HeartbeatSettings {
            interval: Duration::from_millis(10),
            timeout: Duration::from_millis(60),
        },
    );
    let fake = attach(&port, TEST_BROWSER, Box::new(|_, _| Reply::Never));
    let cancelled = AtomicBool::new(false);
    let soon = || Instant::now() + Duration::from_millis(50);
    assert_eq!(
        port.await_available(TEST_BROWSER, soon(), &cancelled),
        Availability::Available
    );

    fake.acknowledge_heartbeats(false);
    let until = Instant::now() + Duration::from_secs(5);
    while port.await_available(TEST_BROWSER, soon(), &cancelled) == Availability::Available {
        assert!(Instant::now() < until, "silence never hid the connection");
    }
    assert_eq!(
        port.await_available(TEST_BROWSER, soon(), &cancelled),
        Availability::NotAnswering
    );
    assert!(port.is_registered(TEST_BROWSER));
    assert!(
        port.browser_returning(),
        "a silent connection may answer again"
    );
    assert_eq!(
        port.await_available("browser_absent", soon(), &cancelled),
        Availability::Absent
    );

    fake.acknowledge_heartbeats(true);
    assert_eq!(
        port.await_available(
            TEST_BROWSER,
            Instant::now() + Duration::from_secs(5),
            &cancelled
        ),
        Availability::Available
    );
}

#[test]
fn attention_is_reported_move_to_front_and_never_routes_to_an_absent_browser() {
    let mut registry = AdapterRegistry::default();
    assert_eq!(registry.attended(), None);

    registry.attend("browser_chrome");
    registry.attend("browser_edge");
    registry.attend("browser_chrome");
    assert_eq!(registry.attention, ["browser_chrome", "browser_edge"]);

    // Attention outlives connections, so a browser that reconnects keeps the place it earned.
    // Until one of them is actually connected, it routes nothing.
    assert_eq!(registry.attended(), None);
}

#[test]
fn routing_prefers_selection_then_binding_then_attention() {
    let chrome = testing::summary("browser_chrome", false);
    let edge = testing::summary("browser_edge", true);
    let connected = vec![chrome.clone(), edge.clone()];

    // An explicit selection wins over the attended default.
    assert_eq!(
        choose_browser(Some("browser_chrome"), None, &connected).unwrap(),
        "browser_chrome"
    );
    // An established binding wins over the attended default, so work stays where it started.
    assert_eq!(
        choose_browser(None, Some("browser_chrome"), &connected).unwrap(),
        "browser_chrome"
    );
    // With nothing else to go on, the browser the person last attended gets the work.
    assert_eq!(
        choose_browser(None, None, &connected).unwrap(),
        "browser_edge"
    );
    // A sole browser needs no evidence at all.
    assert_eq!(
        choose_browser(None, None, &[testing::summary("browser_only", false)]).unwrap(),
        "browser_only"
    );
}

#[test]
fn routing_refuses_rather_than_guessing_or_failing_over() {
    let unattended = vec![
        testing::summary("browser_chrome", false),
        testing::summary("browser_edge", false),
    ];
    // Two browsers and no evidence: name the candidates, choose nothing.
    assert_eq!(
        choose_browser(None, None, &unattended),
        Err(BrowserError::AmbiguousBrowser(vec![
            "browser_chrome".into(),
            "browser_edge".into()
        ]))
    );
    // A bound browser that stopped never fails over to the other one.
    assert_eq!(
        choose_browser(None, Some("browser_gone"), &unattended),
        Err(BrowserError::DisconnectedBeforeDispatch)
    );
    // A workspace cannot be told to continue somewhere its tabs are not.
    assert_eq!(
        choose_browser(Some("browser_edge"), Some("browser_chrome"), &unattended),
        Err(BrowserError::BrowserPinned)
    );
    // A selection nobody is serving is a refusal, not a substitution.
    assert_eq!(
        choose_browser(Some("browser_absent"), None, &unattended),
        Err(BrowserError::UnknownBrowser("browser_absent".into()))
    );
    assert_eq!(
        choose_browser(None, None, &[]),
        Err(BrowserError::DisconnectedBeforeDispatch)
    );
}
