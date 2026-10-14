//! The translation of each unit of browser work into protocol-4 primitives.
//!
//! Each submodule owns one family of work. A function there sends the primitives its work
//! needs through the [`Link`](super::link::Link) and turns their outcomes into the one
//! [`BrowserOutcome`](crate::browser::adapter::BrowserOutcome) the rest of the service reads.

pub mod present;
pub mod records;
pub mod tabs;

use super::link::{UNSUPPORTED_ADVERTISED, UNSUPPORTED_REQUIRED};
use crate::browser::adapter::BrowserCommand;
use crate::browser::BrowserError;

/// The tag that names a unit of work.
const WORK_TAG: &str = "command";

/// The refusal for work this service does not yet translate into primitives: the work needs a
/// first revision, and the connection advertises none.
#[must_use]
pub fn untranslated(command: &BrowserCommand) -> BrowserError {
    BrowserError::CapabilityVersion {
        capability: work_name(command),
        required: UNSUPPORTED_REQUIRED,
        advertised: UNSUPPORTED_ADVERTISED,
    }
}

/// The name of one unit of work, as its tag spells it.
fn work_name(command: &BrowserCommand) -> String {
    serde_json::to_value(command)
        .ok()
        .and_then(|value| value.get(WORK_TAG)?.as_str().map(str::to_owned))
        .unwrap_or_default()
}
