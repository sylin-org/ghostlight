//! The last-browser record: which installed browser last connected to Ghostlight.
//!
//! When a browser's extension connection is accepted and its reported name maps to exactly one
//! supported browser, the service writes that browser's native-host id (`chrome`, `edge`,
//! `brave`, `chromium`) to `state/last-browser.json`. Browser recovery reads it when several
//! browsers are installed and none is connected, and starts the remembered one. The record holds
//! the id and nothing else: no profile, no page, no time. A missing or unparsable record means no
//! memory.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::files::atomic_replace;

/// The content of `state/last-browser.json`. A reader ignores fields it does not know.
#[derive(Debug, Deserialize, Serialize)]
struct LastBrowserRecord {
    /// The native-host id of the browser that last connected.
    browser: String,
}

/// The browser id the record at `path` names, or `None` when it is missing or does not parse.
#[must_use]
pub fn read(path: &Path) -> Option<String> {
    let record: LastBrowserRecord = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    Some(record.browser)
}

/// Make the record at `path` name `browser`, as a private file. A record that already names it
/// is left alone, so a browser that reconnects writes nothing.
pub fn record(path: &Path, browser: &str) -> io::Result<()> {
    if read(path).as_deref() == Some(browser) {
        return Ok(());
    }
    let bytes = serde_json::to_vec(&LastBrowserRecord {
        browser: browser.into(),
    })
    .map_err(io::Error::other)?;
    atomic_replace(path, &bytes)
}
