//! Port contract tests against a scripted protocol-4 extension: routing between browsers,
//! connection replacement, availability, and the admission check at the writer.

mod controls;
mod relay;

use std::net::TcpListener;

use serde_json::{json, Value};

use super::RelayBrowserPort;
use crate::browser::adapter::chromium::protocol4::assets::page_assets;
use crate::browser::adapter::chromium::protocol4::fake::{FakeExtension, Reply, Script};
use crate::browser::adapter::chromium::protocol4::wire;

/// The browser a single-browser contract test attaches.
const TEST_BROWSER: &str = "browser_test";

/// A port with the service's own page assets.
fn port() -> RelayBrowserPort {
    RelayBrowserPort::new("service_test".into(), page_assets())
}

/// Attach one scripted extension for `browser` to `port`.
fn attach(port: &RelayBrowserPort, browser: &str, script: Script) -> FakeExtension {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let fake = FakeExtension::connect(
        listener.local_addr().unwrap(),
        &FakeExtension::hello(browser),
        script,
    );
    port.attach(listener.accept().unwrap().0).unwrap();
    fake
}

/// A script that answers every request with one tab titled after `browser`.
fn tabs_titled(browser: &'static str) -> Script {
    Box::new(move |correlation, _| {
        Reply::Now(FakeExtension::ok(
            correlation,
            json!({"tabs": [tab(browser)]}),
        ))
    })
}

/// One complete tab 5 with `title`.
fn tab(title: &str) -> Value {
    json!({
        "tab_id": 5,
        "title": title,
        "url": "about:blank",
        "active": true,
        "readiness": "complete"
    })
}

/// A receipt for `correlation` that navigated tab 5.
fn navigated(correlation: &str) -> wire::Receipt {
    wire::Receipt {
        dispatched: true,
        ..FakeExtension::ok(correlation, json!({"tab": tab("Example")}))
    }
}
