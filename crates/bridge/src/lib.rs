//! Diagnostics and framing the service still shares with its desktop.

pub mod diagnostics;
pub mod framing;

/// The largest accepted line or native-message frame.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
