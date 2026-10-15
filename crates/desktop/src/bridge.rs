//! The one bridge between a page and the shell (ADR-0199 Decision 7).
//!
//! The webview's only channel is "post JSON, receive JSON". [`BRIDGE_SCRIPT`] is injected into the
//! main frame before any page script and defines `window.ghostlightShell`. This module holds the
//! pure rules around it: decoding what a page posted, building the script that delivers a message
//! to the page, and deciding which pages the shell listens to at all.

use serde_json::{Map, Value};

use crate::bundle::SHELL_PREFIX;
use crate::scheme::request_path;

/// The script injected into the main frame before page scripts.
pub const BRIDGE_SCRIPT: &str = include_str!("web/bridge.js");

/// The largest message a page may post, in bytes.
pub const PAGE_MESSAGE_LIMIT: usize = 1024 * 1024;

/// The page-side function a delivery calls.
const DELIVER_FUNCTION: &str = "window.__ghostlightShellDeliver";

/// Why a posted message was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BridgeError {
    /// The body is larger than [`PAGE_MESSAGE_LIMIT`].
    #[error("page message is larger than {PAGE_MESSAGE_LIMIT} bytes")]
    TooLarge,
    /// The body is not valid JSON.
    #[error("page message is not JSON")]
    NotJson,
    /// The body is JSON but not an object.
    #[error("page message is not a JSON object")]
    NotAnObject,
}

/// Which kind of page posted a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrigin {
    /// A page of the shell's own bundle, under the reserved shell prefix.
    ShellPage,
    /// A page the service sent.
    ServicePage,
    /// Anything else. Its messages are dropped.
    Foreign,
}

/// Decodes the body a page posted into a JSON object.
///
/// Refuses a body over [`PAGE_MESSAGE_LIMIT`], invalid JSON and any JSON value that is not an
/// object.
pub fn decode_page_message(body: &str) -> Result<Map<String, Value>, BridgeError> {
    if body.len() > PAGE_MESSAGE_LIMIT {
        return Err(BridgeError::TooLarge);
    }
    match serde_json::from_str::<Value>(body).map_err(|_| BridgeError::NotJson)? {
        Value::Object(map) => Ok(map),
        _ => Err(BridgeError::NotAnObject),
    }
}

/// The script that delivers `message` to the page's listeners.
///
/// The message's JSON text is encoded once more as a JSON string literal, so nothing in the
/// message can end the call early or break the script.
pub fn delivery_script(message: &Value) -> String {
    let text = message.to_string();
    let literal = Value::String(text).to_string();
    format!("{DELIVER_FUNCTION}({literal})")
}

/// Classifies the page a message came from by its URL.
pub fn page_origin(url: &str) -> PageOrigin {
    match request_path(url) {
        Some(path) => match path.strip_prefix(SHELL_PREFIX) {
            Some(rest) if rest.starts_with('/') => PageOrigin::ShellPage,
            _ => PageOrigin::ServicePage,
        },
        None => PageOrigin::Foreign,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decode_accepts_an_object() {
        let map = decode_page_message(r#"{"a":1}"#).expect("object");
        assert_eq!(map["a"], json!(1));
    }

    #[test]
    fn decode_refuses_everything_but_an_object() {
        assert_eq!(decode_page_message("[1]"), Err(BridgeError::NotAnObject));
        assert_eq!(decode_page_message("\"x\""), Err(BridgeError::NotAnObject));
        assert_eq!(decode_page_message("{nope"), Err(BridgeError::NotJson));
    }

    #[test]
    fn decode_refuses_a_body_over_the_limit() {
        let body = " ".repeat(PAGE_MESSAGE_LIMIT + 1);
        assert_eq!(decode_page_message(&body), Err(BridgeError::TooLarge));
    }

    #[test]
    fn delivery_round_trips_hostile_text() {
        let message = json!({
            "html": "</script><script>alert(1)</script>",
            "sep": "a\u{2028}b\u{2029}c",
            "quotes": "he said \"hi\" and 'bye'",
            "slashes": "back\\slash \\\" end",
            "close": "))}",
        });
        let script = delivery_script(&message);
        let literal = script
            .strip_prefix("window.__ghostlightShellDeliver(")
            .and_then(|rest| rest.strip_suffix(')'))
            .expect("fixed prefix and closing parenthesis");
        let text: String = serde_json::from_str(literal).expect("string literal");
        let back: Value = serde_json::from_str(&text).expect("json text");
        assert_eq!(back, message);
    }

    #[test]
    fn origin_of_both_scheme_forms() {
        for origin in ["https://ghostlight.localhost", "ghostlight://localhost"] {
            assert_eq!(
                page_origin(&format!("{origin}/_shell/home.html")),
                PageOrigin::ShellPage
            );
            assert_eq!(
                page_origin(&format!("{origin}/workbench/app.html?x=1")),
                PageOrigin::ServicePage
            );
            assert_eq!(page_origin(&format!("{origin}/")), PageOrigin::ServicePage);
        }
    }

    #[test]
    fn origin_of_other_hosts_and_schemes_is_foreign() {
        assert_eq!(
            page_origin("https://example.com/_shell/x.html"),
            PageOrigin::Foreign
        );
        assert_eq!(
            page_origin("http://ghostlight.localhost/a.html"),
            PageOrigin::Foreign
        );
        assert_eq!(page_origin("file:///etc/passwd"), PageOrigin::Foreign);
        assert_eq!(page_origin("about:blank"), PageOrigin::Foreign);
    }
}
