//! What Ghostlight tells a browser extension it cannot serve.
//!
//! The extension shows these words as they arrive and never writes its own about the service, so
//! the sentence stays with the rest of Ghostlight's language.

/// How a person updates the Ghostlight extension right now, instead of waiting for the browser's
/// own scheduled update.
///
/// Chrome, Edge and Brave all reach their extensions page from the `chrome://extensions`
/// address; the Update button shows once Developer mode is on (checked against Chrome's
/// extension documentation, 2026-10-16).
pub const UPDATE_NOW: &str = "open chrome://extensions, turn on Developer mode and click Update";

/// The message of the `adapter_incompatible` error frame: the extension speaks an adapter
/// protocol this service does not serve.
pub const ADAPTER_INCOMPATIBLE: &str = "Ghostlight in Browser is out of date. It updates itself within a day; to update now, open chrome://extensions, turn on Developer mode and click Update.";
