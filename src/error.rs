//! Typed error type for the engine.
//!
//! Per the project style: **typed errors in library code** (this crate), **`anyhow` in the
//! binary and integration tests**.

use thiserror::Error;

/// Errors surfaced by the Ghostlight engine.
#[derive(Debug, Error)]
pub enum Error {
    /// The MCP JSON-RPC layer received or produced something malformed.
    #[error("MCP protocol error: {0}")]
    Protocol(String),

    /// A failure in the Chrome native-messaging framing (4-byte LE length prefix + JSON).
    #[error("native messaging error: {0}")]
    NativeMessaging(String),

    /// A failure on the inter-instance IPC (named pipe / Unix domain socket).
    #[error("ipc error: {0}")]
    Ipc(String),

    /// Another Ghostlight session already owns the browser (single-session policy, v1.0).
    #[error("another Ghostlight session already owns the browser")]
    SessionBusy,

    /// JSON (de)serialization failure.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Underlying I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The installer needs the unpacked extension ID (no build-time `key` yet); pass --extension-id.
    #[error("extension id required: pass --extension-id <id> (see docs/research/11-install-detection.md)")]
    MissingExtensionId,

    /// The provided extension id is not a valid 32-char a-p Chrome id.
    #[error("invalid extension id: {0}")]
    InvalidExtensionId(String),

    /// A native-messaging host registration (file drop or registry write) failed.
    #[error("native host registration failed: {0}")]
    HostRegistration(String),

    /// An MCP client config write/merge/CLI invocation failed.
    #[error("client registration failed: {0}")]
    ClientRegistration(String),

    /// A client config exists but is not a shape we can safely merge into.
    #[error("cannot merge config (unexpected shape): {0}")]
    MergeConflict(String),

    /// The running platform/browser/client combination is not supported in this build.
    #[error("unsupported target: {0}")]
    Unsupported(String),

    /// A configuration operation failed: a config file failed to load or validate (user config
    /// or org policy file), or a `config` CLI request failed (unknown key, invalid value,
    /// org-locked key, unusable user config file). Display is the full, self-contained
    /// user-facing message; callers do not add their own prefix.
    #[error("{0}")]
    Config(String),
}

/// Convenience alias for fallible engine operations.
pub type Result<T> = std::result::Result<T, Error>;

/// A tool-call failure attributed to the dispatch hop that broke. Rendered for the MCP client as:
/// "[hop: <hop>] <message>. Next step: <next step>."
///
/// The dispatch path is: MCP client -> binary (mcp-server role) -> IPC -> binary (native-host
/// role) -> Chrome native messaging -> extension -> CDP or the page's content script. Each variant
/// names one hop in that chain so the client always knows which layer broke and what to try next,
/// instead of an opaque "native messaging error: ...".
#[derive(Debug, Clone, Error)]
pub enum ToolError {
    /// The MCP client's request itself was invalid (bad tool name, malformed arguments).
    #[error("[hop: invalid-request] {message}. Next step: {next_step}.")]
    InvalidRequest {
        /// One-sentence, specific description of what was wrong with the request.
        message: String,
        /// One imperative clause describing what the caller should try next.
        next_step: String,
    },
    /// The binary itself failed (encoding, internal bookkeeping) before anything left the process.
    #[error("[hop: binary] {message}. Next step: {next_step}.")]
    Binary {
        /// One-sentence, specific description of the binary-side failure.
        message: String,
        /// One imperative clause describing what the caller should try next.
        next_step: String,
    },
    /// The inter-instance transport (named pipe / Unix domain socket) between the mcp-server and
    /// native-host processes failed.
    #[error("[hop: ipc] {message}. Next step: {next_step}.")]
    Ipc {
        /// One-sentence, specific description of the IPC failure.
        message: String,
        /// One imperative clause describing what the caller should try next.
        next_step: String,
    },
    /// The extension itself failed (not connected, disconnected mid-call, timed out, or an
    /// untagged internal error).
    #[error("[hop: extension] {message}. Next step: {next_step}.")]
    Extension {
        /// One-sentence, specific description of the extension-side failure.
        message: String,
        /// One imperative clause describing what the caller should try next.
        next_step: String,
    },
    /// A Chrome DevTools Protocol command the extension issued was rejected.
    #[error("[hop: cdp] {message}. Next step: {next_step}.")]
    Cdp {
        /// One-sentence, specific description of the CDP failure (often the CDP method name).
        message: String,
        /// One imperative clause describing what the caller should try next.
        next_step: String,
    },
    /// The page itself was the problem (stale reference, blocked script injection, unusable
    /// content).
    #[error("[hop: page] {message}. Next step: {next_step}.")]
    Page {
        /// One-sentence, specific description of what went wrong on the page.
        message: String,
        /// One imperative clause describing what the caller should try next.
        next_step: String,
    },
}

impl ToolError {
    /// Build an `InvalidRequest` error with the default next step.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
            next_step: "fix the tool arguments to match the advertised schema and retry".into(),
        }
    }

    /// Build a `Binary` error with the default next step.
    pub fn binary(message: impl Into<String>) -> Self {
        Self::Binary {
            message: message.into(),
            next_step:
                "retry the call; if it keeps failing, restart the MCP client and report a bug"
                    .into(),
        }
    }

    /// Build an `Ipc` error with the default next step.
    pub fn ipc(message: impl Into<String>) -> Self {
        Self::Ipc {
            message: message.into(),
            next_step: "restart the MCP client so both ghostlight processes restart and reconnect"
                .into(),
        }
    }

    /// Build an `Extension` error with the default next step.
    pub fn extension(message: impl Into<String>) -> Self {
        Self::Extension {
            message: message.into(),
            next_step: "check chrome://extensions and that Chrome is running".into(),
        }
    }

    /// Build a `Cdp` error with the default next step.
    pub fn cdp(message: impl Into<String>) -> Self {
        Self::Cdp {
            message: message.into(),
            next_step: "retry after taking a screenshot to re-ground coordinates".into(),
        }
    }

    /// Build a `Page` error with the default next step.
    pub fn page(message: impl Into<String>) -> Self {
        Self::Page {
            message: message.into(),
            next_step: "take a screenshot or call read_page to re-locate the element, then retry"
                .into(),
        }
    }

    /// Return a copy of this error with the next step replaced (immutable builder; does not
    /// mutate `self` in place).
    pub fn next_step(self, step: impl Into<String>) -> Self {
        let step = step.into();
        match self {
            Self::InvalidRequest { message, .. } => Self::InvalidRequest {
                message,
                next_step: step,
            },
            Self::Binary { message, .. } => Self::Binary {
                message,
                next_step: step,
            },
            Self::Ipc { message, .. } => Self::Ipc {
                message,
                next_step: step,
            },
            Self::Extension { message, .. } => Self::Extension {
                message,
                next_step: step,
            },
            Self::Cdp { message, .. } => Self::Cdp {
                message,
                next_step: step,
            },
            Self::Page { message, .. } => Self::Page {
                message,
                next_step: step,
            },
        }
    }

    /// Map a wire-level extension error to a hop-attributed variant. The extension only ever
    /// tags `hop` as `"cdp"` or `"page"`; anything else (including a missing `hop`, which is the
    /// common case for the extension's own untagged internal errors) is attributed to the
    /// `extension` hop itself.
    pub fn from_extension_wire(hop: Option<&str>, message: String) -> Self {
        match hop {
            Some("cdp") => Self::cdp(message),
            Some("page") => Self::page(message),
            _ => Self::extension(message),
        }
    }
}

#[cfg(test)]
mod tool_error_tests {
    use super::*;

    #[test]
    fn extension_not_connected_renders_the_canonical_message() {
        let err = ToolError::extension("Browser extension not connected");
        assert_eq!(
            err.to_string(),
            "[hop: extension] Browser extension not connected. Next step: check chrome://extensions and that Chrome is running."
        );
    }

    #[test]
    fn invalid_request_renders_prefix_and_default_next_step() {
        let err = ToolError::invalid_request("Unknown tool: bogus_tool");
        assert_eq!(
            err.to_string(),
            "[hop: invalid-request] Unknown tool: bogus_tool. Next step: fix the tool arguments to match the advertised schema and retry."
        );
    }

    #[test]
    fn binary_renders_prefix_and_default_next_step() {
        let err = ToolError::binary("failed to encode the tool request: boom");
        assert_eq!(
            err.to_string(),
            "[hop: binary] failed to encode the tool request: boom. Next step: retry the call; if it keeps failing, restart the MCP client and report a bug."
        );
    }

    #[test]
    fn ipc_renders_prefix_and_default_next_step() {
        let err = ToolError::ipc("IPC transport failed: broken pipe");
        assert_eq!(
            err.to_string(),
            "[hop: ipc] IPC transport failed: broken pipe. Next step: restart the MCP client so both ghostlight processes restart and reconnect."
        );
    }

    #[test]
    fn cdp_renders_prefix_and_default_next_step() {
        let err = ToolError::cdp("Input.dispatchMouseEvent failed: no target");
        assert_eq!(
            err.to_string(),
            "[hop: cdp] Input.dispatchMouseEvent failed: no target. Next step: retry after taking a screenshot to re-ground coordinates."
        );
    }

    #[test]
    fn page_renders_prefix_and_default_next_step() {
        let err = ToolError::page("Element ref_5 not found");
        assert_eq!(
            err.to_string(),
            "[hop: page] Element ref_5 not found. Next step: take a screenshot or call read_page to re-locate the element, then retry."
        );
    }

    #[test]
    fn from_extension_wire_maps_cdp() {
        let err = ToolError::from_extension_wire(Some("cdp"), "boom".into());
        assert!(err.to_string().starts_with("[hop: cdp]"));
    }

    #[test]
    fn from_extension_wire_maps_page() {
        let err = ToolError::from_extension_wire(Some("page"), "boom".into());
        assert!(err.to_string().starts_with("[hop: page]"));
    }

    #[test]
    fn from_extension_wire_defaults_untagged_to_extension() {
        let err = ToolError::from_extension_wire(None, "boom".into());
        assert!(err.to_string().starts_with("[hop: extension]"));
    }

    #[test]
    fn from_extension_wire_defaults_unknown_hop_to_extension() {
        let err = ToolError::from_extension_wire(Some("bogus"), "boom".into());
        assert!(err.to_string().starts_with("[hop: extension]"));
    }

    #[test]
    fn next_step_replaces_the_default() {
        let err = ToolError::extension("Browser extension disconnected before responding")
            .next_step("retry the call; the extension reconnects automatically");
        assert_eq!(
            err.to_string(),
            "[hop: extension] Browser extension disconnected before responding. Next step: retry the call; the extension reconnects automatically."
        );
    }
}
