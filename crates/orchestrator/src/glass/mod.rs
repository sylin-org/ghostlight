//! The portable Glass UI layer.
//!
//! This module embeds the Ghostlight Web Components into the Rust binary,
//! allowing the orchestrator to inject the UI directly into the browser
//! via WebDriver BiDi `addPreloadScript`, entirely bypassing the extension.



/// The raw JavaScript for the Ghostlight DOM sensor logic.
pub const SENSOR_SCRIPT: &str = include_str!("sensor.js");
pub const CONTENT_SCRIPT: &str = include_str!("content.js");
pub const PRESENTATION_SCRIPT: &str = include_str!("../../../../extension/lib/presentation.js");

pub const SHARED_SCRIPT: &str = include_str!("../../../../extension/lib/shared.js");
pub const FORM_DIAGNOSTICS_SCRIPT: &str = include_str!("../../../../extension/lib/form-diagnostics.js");

pub fn inject_glass(writer: &ghostlight_bridge::transport::SocketWriter) {
    let script = format!("(() => {{\n{}\n{}\n{}\n{}\n{}\n}})();", SHARED_SCRIPT, FORM_DIAGNOSTICS_SCRIPT, SENSOR_SCRIPT, PRESENTATION_SCRIPT, CONTENT_SCRIPT);
    
    let frame = ghostlight_bridge::browser::BrowserFrame::Request {
        request: ghostlight_bridge::browser::BrowserRequest {
            correlation: "glass-injection".into(),
            workspace: "system".into(),
            command: ghostlight_bridge::browser::BrowserCommand::SetPreloadScript {
                script,
            },
        },
    };

    let _ = writer.native(&frame);
}
