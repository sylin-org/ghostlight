//! Routing between browsers and connection replacement on the relay port.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::super::{choose_browser, testing, BrowserError, BrowserPort, RelayBrowserPort};
use super::{attach, port, tabs_titled};
use crate::browser::adapter::{BrowserCommand, BrowserOutcome};

/// The title of the first tab `browser` lists, as the connection that serves it answers.
fn first_title(port: &RelayBrowserPort, browser: &str) -> String {
    let Ok(BrowserOutcome::Tabs { tabs }) = port.call(
        browser,
        BrowserCommand::ListTabs,
        Instant::now() + Duration::from_secs(5),
        &AtomicBool::new(false),
    ) else {
        panic!("{browser} answers its own request");
    };
    tabs[0].title.clone()
}

/// ACCEPTANCE C18 and the defect ADR-0114 fixed (a1be5295: one adapter slot made a duplicate
/// connection unrecoverable): the port keys its connections by browser identity. Two browsers
/// stay connected at once and each answers its own requests, its tab 5 its own. A second
/// connection carrying an identity already registered is the same browser arriving twice: it
/// replaces the first, which is closed rather than abandoned, and serves that browser's work.
#[test]
fn connections_are_keyed_by_browser_identity() {
    let port = port();
    let chrome = attach(&port, "browser_chrome", tabs_titled("browser_chrome"));
    let _edge = attach(&port, "browser_edge", tabs_titled("browser_edge"));
    let connected: Vec<_> = port
        .connected_browsers()
        .into_iter()
        .map(|browser| browser.id)
        .collect();
    assert_eq!(connected, vec!["browser_chrome", "browser_edge"]);
    for browser in ["browser_chrome", "browser_edge"] {
        assert_eq!(first_title(&port, browser), browser);
    }

    let replacement = attach(&port, "browser_chrome", tabs_titled("replacement"));
    assert_eq!(
        port.connected_browsers().len(),
        2,
        "one browser is still one browser"
    );
    assert!(
        chrome.wait(|seen| seen.closed).closed,
        "the replaced connection is closed"
    );
    assert!(!replacement.wait(|_| true).closed);
    assert_eq!(first_title(&port, "browser_chrome"), "replacement");
    assert_eq!(first_title(&port, "browser_edge"), "browser_edge");
}

/// ACCEPTANCE C20 and C19: a crossing goes to the browser an explicit selection names, else the
/// one its workspace is bound to, else the one the person last attended, else the only one
/// connected. It never guesses or fails over: two browsers and no evidence are refused naming
/// both, a bound browser that left is never replaced by another, a workspace cannot be sent where
/// its tabs are not, and a selection nobody serves is refused rather than substituted.
#[test]
fn routing_uses_selection_binding_attention_or_the_sole_browser_and_otherwise_refuses() {
    let chrome = testing::summary("browser_chrome", false);
    let edge = testing::summary("browser_edge", true);
    let connected = vec![chrome.clone(), edge.clone()];
    assert_eq!(
        choose_browser(Some("browser_chrome"), None, &connected).unwrap(),
        "browser_chrome"
    );
    assert_eq!(
        choose_browser(None, Some("browser_chrome"), &connected).unwrap(),
        "browser_chrome"
    );
    assert_eq!(
        choose_browser(None, None, &connected).unwrap(),
        "browser_edge"
    );
    assert_eq!(
        choose_browser(None, None, &[testing::summary("browser_only", false)]).unwrap(),
        "browser_only"
    );

    let unattended = vec![
        testing::summary("browser_chrome", false),
        testing::summary("browser_edge", false),
    ];
    assert_eq!(
        choose_browser(None, None, &unattended),
        Err(BrowserError::AmbiguousBrowser(vec![
            "browser_chrome".into(),
            "browser_edge".into()
        ]))
    );
    assert_eq!(
        choose_browser(None, Some("browser_gone"), &unattended),
        Err(BrowserError::DisconnectedBeforeDispatch)
    );
    assert_eq!(
        choose_browser(Some("browser_edge"), Some("browser_chrome"), &unattended),
        Err(BrowserError::BrowserPinned)
    );
    assert_eq!(
        choose_browser(Some("browser_absent"), None, &unattended),
        Err(BrowserError::UnknownBrowser("browser_absent".into()))
    );
    assert_eq!(
        choose_browser(None, None, &[]),
        Err(BrowserError::DisconnectedBeforeDispatch)
    );
}
