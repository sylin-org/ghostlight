//! The Chromium browser adapter.
//!
//! It owns the wire levels the Chromium extension can speak and every asset that runs in a
//! Chromium page. Level 3 is the published extension's protocol; level 4 is the frozen-primitive
//! protocol beside it.

pub mod assets;
pub mod level3;
pub mod level4;

use super::Level;

/// Every wire level a Chromium extension can speak, oldest first.
pub(super) const LEVELS: [Level; 2] = [Level::Three, Level::Four];

/// The newest wire level a Chromium extension can speak.
pub(super) const NEWEST_LEVEL: Level = Level::Four;

/// The Chromium wire level that speaks a hello's protocol major, if any.
pub(super) fn level_for(major: u16) -> Option<Level> {
    LEVELS.into_iter().find(|level| level.major() == major)
}
