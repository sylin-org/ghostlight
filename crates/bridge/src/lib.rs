//! Local lifecycle, diagnostics and framing the service still shares with its desktop.

#[cfg(target_os = "linux")]
pub mod desktop_activation;
pub mod diagnostics;
pub mod framing;
pub mod installation;
pub mod lifecycle;
mod parent_process;

/// The largest accepted line or native-message frame.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
