// SPDX-License-Identifier: Apache-2.0 OR MIT
//! K2 (`docs/tasks/console/K2-console-static-routes.md`; PINS.md CS1, CS1.1, CS1.2, CS1.3, CS10,
//! CS11): the Console's own static GET routes, served from the SAME TCP listener H8's web API
//! runs, gated by the SAME `inbound.web.from` decision the WS-upgrade path already uses.

mod support;

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static SEQ: AtomicU32 = AtomicU32::new(0);

/// One raw HTTP/1.1 request/response round trip over a plain TCP connection (no WS upgrade).
/// Returns the full response text (status line, headers, body).
fn http_get(port: u16, path: &str) -> String {
    http_request(port, "GET", path)
}

fn http_request(port: u16, method: &str, path: &str) -> String {
    let mut stream = support::connect_webapi(port);
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let request =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn status_line(response: &str) -> &str {
    response.lines().next().unwrap_or_default()
}

fn body(response: &str) -> &str {
    // split_once: everything after the FIRST header/body delimiter, even when the body itself
    // contains a blank line (a "\r\n\r\n" run). A plain split(..).nth(1) would return only the
    // segment up to the body's first blank line and silently truncate it.
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default()
}

fn header_value<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    response
        .split("\r\n")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .find_map(|line| {
            line.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case(name))
        })
        .map(|(_, v)| v.trim())
}

#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn console_index_page_is_served_over_a_real_http_get() {
    let endpoint = format!(
        "ghostlight-console-index-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let (mut service, port) = support::spawn_service_with_webapi_port(&endpoint);

    let response = http_get(port, "/");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
    assert_eq!(
        header_value(&response, "Content-Type"),
        Some("text/html; charset=utf-8")
    );
    let page = body(&response);
    assert!(
        page.contains("/manage.css"),
        "index page must link manage.css: {page}"
    );
    assert!(
        page.contains("/manage.js"),
        "index page must link manage.js: {page}"
    );

    let _ = service.kill();
    let _ = service.wait();
}

#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn console_css_and_js_are_served_with_correct_content_type() {
    let endpoint = format!(
        "ghostlight-console-assets-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let (mut service, port) = support::spawn_service_with_webapi_port(&endpoint);

    let css = http_get(port, "/manage.css");
    assert_eq!(status_line(&css), "HTTP/1.1 200 OK");
    assert_eq!(
        header_value(&css, "Content-Type"),
        Some("text/css; charset=utf-8")
    );

    let js = http_get(port, "/manage.js");
    assert_eq!(status_line(&js), "HTTP/1.1 200 OK");
    assert_eq!(
        header_value(&js, "Content-Type"),
        Some("application/javascript; charset=utf-8")
    );

    let _ = service.kill();
    let _ = service.wait();
}

#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn unknown_path_under_api_v1_is_404() {
    // PINS.md CS1's fallback scope is EXACTLY "/" or under "/api/v1/**" (CS1.1's own example is
    // "/api/v1/unknown") -- a bare top-level path like "/nope" is outside the Console's route
    // scope entirely and correctly falls through unchanged to the pre-existing 400 Bad Request
    // (no Sec-WebSocket-Key); it is not this router's concern and must not become a 404.
    let endpoint = format!(
        "ghostlight-console-404-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let (mut service, port) = support::spawn_service_with_webapi_port(&endpoint);

    let response = http_get(port, "/api/v1/nope");
    assert_eq!(status_line(&response), "HTTP/1.1 404 Not Found");
    assert_eq!(body(&response), "not found");

    let outside_scope = http_get(port, "/nope");
    assert_eq!(
        status_line(&outside_scope),
        "HTTP/1.1 400 Bad Request",
        "a bare top-level path outside / and /api/v1/** is outside the Console's scope"
    );

    let _ = service.kill();
    let _ = service.wait();
}

#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn wrong_method_on_a_known_path_is_405() {
    let endpoint = format!(
        "ghostlight-console-405-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let (mut service, port) = support::spawn_service_with_webapi_port(&endpoint);

    let response = http_request(port, "POST", "/");
    assert_eq!(status_line(&response), "HTTP/1.1 405 Method Not Allowed");
    assert_eq!(body(&response), "method not allowed");

    let _ = service.kill();
    let _ = service.wait();
}

/// Send a WS-upgrade handshake and return the raw first response chunk.
fn ws_upgrade_response(port: u16) -> String {
    let mut stream = support::connect_webapi(port);
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let request = format!(
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut buf = [0u8; 512];
    let n = stream.read(&mut buf).unwrap();
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// SEC hardening pass (2026-07): web (WS) ingestion is OFF by default now, so a real WS upgrade
/// is REFUSED with 403 out of the box -- the loopback Console (GET routes above) still works,
/// but driving the browser over TCP is opt-in. The shared listener is up (the Console served the
/// GET routes), yet the ingestion gate refuses the session.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn a_ws_upgrade_is_refused_by_default_because_web_ingestion_is_opt_in() {
    let endpoint = format!(
        "ghostlight-console-ws-default-off-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let (mut service, port) = support::spawn_service_with_webapi_port(&endpoint);

    let response = ws_upgrade_response(port);
    assert!(
        response.starts_with("HTTP/1.1 403 Forbidden"),
        "web ingestion is off by default; a WS upgrade must be refused: {response}"
    );

    let _ = service.kill();
    let _ = service.wait();
}

/// With `inbound.web.enabled = true` opted in at the user-config layer, a real WS upgrade
/// succeeds and is unaffected by the Console router sharing the listener.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn a_real_ws_upgrade_succeeds_once_web_ingestion_is_enabled() {
    let pid = std::process::id();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let user_config_dir =
        std::env::temp_dir().join(format!("ghostlight-console-ws-optin-{pid}-{seq}"));
    std::fs::create_dir_all(user_config_dir.join("ghostlight")).unwrap();
    std::fs::write(
        user_config_dir.join("ghostlight").join("config.json"),
        r#"{"config":{"inbound.web.enabled":true}}"#,
    )
    .unwrap();

    let endpoint = format!("ghostlight-console-ws-optin-{pid}-{seq}");
    let (mut service, port) =
        support::spawn_service_with_user_config_dir_and_webapi_port(&endpoint, &user_config_dir);

    let response = ws_upgrade_response(port);
    assert!(
        response.starts_with("HTTP/1.1 101 Switching Protocols"),
        "with web ingestion enabled, a real WS upgrade must succeed unaffected by the Console router: {response}"
    );

    let _ = service.kill();
    let _ = service.wait();
    std::fs::remove_dir_all(&user_config_dir).ok();
}
