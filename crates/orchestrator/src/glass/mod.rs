//! The portable Glass UI layer.
//!
//! This module embeds the Ghostlight Web Components into the Rust binary,
//! allowing the orchestrator to inject the UI directly into the browser
//! via WebDriver BiDi `addPreloadScript`, entirely bypassing the extension.

/// The raw JavaScript for the Ghostlight overlay web component.
pub const OVERLAY_SCRIPT: &str = include_str!("overlay.js");

/// The raw JavaScript for the Ghostlight DOM sensor logic.
pub const SENSOR_SCRIPT: &str = include_str!("sensor.js");

/// The raw JavaScript for the core Ghostlight content script.
pub const CONTENT_SCRIPT: &str = include_str!("content.js");

pub fn inject_glass(writer: &ghostlight_bridge::transport::SocketWriter) {
    let script = format!("{}\n{}\n{}", SENSOR_SCRIPT, CONTENT_SCRIPT, OVERLAY_SCRIPT);
    let command = ghostlight_bridge::browser::BrowserCommand::BiDi {
        tab_id: 0,
        payload: ghostlight_bridge::bidi::Command {
            id: 99999,
            method: "script.addPreloadScript".into(),
            params: serde_json::json!({
                "functionDeclaration": format!("() => {{ {} }}", script)
            }),
        },
    };

    let frame = ghostlight_bridge::browser::BrowserFrame::Request {
        request: ghostlight_bridge::browser::BrowserRequest {
            correlation: "glass-injection".into(),
            workspace: "system".into(),
            command,
        },
    };

    let _ = writer.native(&frame);
}
