//! Stable, versioned wire contracts shared across Ghostlight process boundaries.

pub mod browser;
pub mod framing;
pub mod relay;
pub mod runtime;
pub mod service;

/// The largest accepted line or native-message frame.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
