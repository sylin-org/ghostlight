//! Typed error type for the engine.
//!
//! Per the project style: **typed errors in library code** (this crate), **`anyhow` in the
//! binary and integration tests**.

use thiserror::Error;

/// Errors surfaced by the Browser MCP engine.
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

    /// Another Browser MCP session already owns the browser (single-session policy, v1.0).
    #[error("another Browser MCP session already owns the browser")]
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
}

/// Convenience alias for fallible engine operations.
pub type Result<T> = std::result::Result<T, Error>;
