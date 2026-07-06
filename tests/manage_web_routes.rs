// SPDX-License-Identifier: Apache-2.0 OR MIT
//! K2 (`docs/tasks/console/K2-console-static-routes.md`; PINS.md CS1, CS1.1, CS1.2, CS1.3, CS10,
//! CS11): the Console's own static GET routes, served from the SAME TCP listener H8's web API
//! runs, gated by the SAME `inbound.web.from` decision the WS-upgrade path already uses.

mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static SEQ: AtomicU32 = AtomicU32::new(0);

/// PINS.md CS11: a test-unique port so concurrently-spawned real services never collide under
/// `cargo test`'s default parallel execution.
fn test_webapi_port(seq: u32) -> u16 {
    20000 + ((std::process::id()).wrapping_add(seq) % 10000) as u16
}

/// One raw HTTP/1.1 request/response round trip over a plain TCP connection (no WS upgrade).
/// Returns the full response text (status line, headers, body).
fn http_get(port: u16, path: &str) -> String {
    http_request(port, "GET", path)
}

fn http_request(port: u16, method: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the web API");
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
    response.split("\r\n\r\n").nth(1).unwrap_or_default()
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

#[test]
fn console_index_page_is_served_over_a_real_http_get() {
    let endpoint = format!(
        "ghostlight-console-index-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let port = test_webapi_port(0);
    let mut service = support::spawn_service_with_webapi_port(&endpoint, port);

    let response = http_get(port, "/");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
    assert_eq!(
        header_value(&response, "Content-Type"),
        Some("text/html; charset=utf-8")
    );
    let page = body(&response);
    assert!(
        page.contains("/console.css"),
        "index page must link console.css: {page}"
    );
    assert!(
        page.contains("/console.js"),
        "index page must link console.js: {page}"
    );

    let _ = service.kill();
    let _ = service.wait();
}

#[test]
fn console_css_and_js_are_served_with_correct_content_type() {
    let endpoint = format!(
        "ghostlight-console-assets-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let port = test_webapi_port(1);
    let mut service = support::spawn_service_with_webapi_port(&endpoint, port);

    let css = http_get(port, "/console.css");
    assert_eq!(status_line(&css), "HTTP/1.1 200 OK");
    assert_eq!(
        header_value(&css, "Content-Type"),
        Some("text/css; charset=utf-8")
    );

    let js = http_get(port, "/console.js");
    assert_eq!(status_line(&js), "HTTP/1.1 200 OK");
    assert_eq!(
        header_value(&js, "Content-Type"),
        Some("application/javascript; charset=utf-8")
    );

    let _ = service.kill();
    let _ = service.wait();
}

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
    let port = test_webapi_port(2);
    let mut service = support::spawn_service_with_webapi_port(&endpoint, port);

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

#[test]
fn wrong_method_on_a_known_path_is_405() {
    let endpoint = format!(
        "ghostlight-console-405-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let port = test_webapi_port(3);
    let mut service = support::spawn_service_with_webapi_port(&endpoint, port);

    let response = http_request(port, "POST", "/");
    assert_eq!(status_line(&response), "HTTP/1.1 405 Method Not Allowed");
    assert_eq!(body(&response), "method not allowed");

    let _ = service.kill();
    let _ = service.wait();
}

#[test]
fn a_real_ws_upgrade_request_is_unaffected() {
    let endpoint = format!(
        "ghostlight-console-ws-unaffected-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let port = test_webapi_port(4);
    let mut service = support::spawn_service_with_webapi_port(&endpoint, port);

    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
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
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(
        response.starts_with("HTTP/1.1 101 Switching Protocols"),
        "a real WS upgrade must still succeed unaffected by the Console router: {response}"
    );

    let _ = service.kill();
    let _ = service.wait();
}
