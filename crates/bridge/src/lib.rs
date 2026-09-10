//! Stable, versioned wire contracts shared across Ghostlight process boundaries.

pub mod browser;
pub mod client;
#[cfg(target_os = "linux")]
pub mod desktop_activation;
pub mod diagnostics;
pub mod framing;
pub mod lifecycle;
pub mod relay;
pub mod runtime;
pub mod service;
pub mod session;
pub mod transport;

/// The largest accepted line or native-message frame.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
