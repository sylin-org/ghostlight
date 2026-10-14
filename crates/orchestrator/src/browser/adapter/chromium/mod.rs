//! The Chromium browser adapter.
//!
//! It owns the wire levels the Chromium extension can speak and every asset that runs in a
//! Chromium page. Level 3 is the published extension's protocol and the only level.

pub mod assets;
pub mod level3;
pub mod level4;

use super::Level;

/// The newest wire level a Chromium extension can speak.
pub(super) const NEWEST_LEVEL: Level = Level::Three;

/// The Chromium wire level that speaks a hello's protocol major, if any.
pub(super) fn level_for(major: u16) -> Option<Level> {
    (major == Level::Three.major()).then_some(Level::Three)
}
