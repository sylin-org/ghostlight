// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Browser-connection diagnostics (ADR-0059): the named vocabulary of lifecycle events
//! [`crate::hub::outbound::browser::Browser`] raises during a native-host connection's life --
//! hello outcomes, attach/detach, focus, and the extension's own forwarded debug notes. A
//! dedicated domain rather than inline `format!` calls scattered across `browser.rs`, so the
//! full event vocabulary is visible in one place and every wording change happens exactly once.
//!
//! Each variant renders through [`Diagnostic::describe`] into the single human-readable line
//! `ghostlight_transport::observability::DebugSink::ipc_note` already knows how to persist into
//! the SAME structured per-pid event ring `ghostlight doctor` and a raw `debug-state-<pid>.json`
//! read already surface every other lifecycle note from -- this module only decides the WORDING,
//! never a second storage or delivery mechanism.

use serde_json::Value;

/// One browser-connection lifecycle event, in the order a connection normally produces them.
pub enum Diagnostic<'a> {
    /// A connection arrived and sent no hello at all before closing -- the ordinary `doctor`
    /// probe shape (connect, read nothing, disconnect). Expected, routine traffic.
    BareProbe,
    /// A connection sent bytes that did not parse as JSON.
    MalformedHello { parse_error: &'a str },
    /// A connection's hello parsed but named a role other than `ROLE_BROWSER`.
    WrongRole { role: Option<&'a str> },
    /// A well-formed `ROLE_BROWSER` hello was admitted.
    Attached {
        browser_pid: u32,
        replaced_existing: bool,
    },
    /// A session's stream closed and its own entry was removed.
    Detached { browser_pid: u32 },
    /// A session's stream closed, but a NEWER hello for the same pid had already replaced it
    /// (a reconnect race); the newer entry was left untouched.
    DetachedStale { browser_pid: u32 },
    /// `browser_pid` reported (via `chrome.windows.onFocusChanged`) gaining window focus.
    FocusReported { browser_pid: u32 },
    /// A debug note the extension itself forwarded (ADR-0059's `debug_event` wire message),
    /// only ever sent when the extension's own local debug flag is on.
    FromExtension {
        browser_pid: u32,
        event: &'a str,
        detail: &'a Value,
    },
}

impl Diagnostic<'_> {
    /// Render this event as the one-line summary `DebugSink::ipc_note` persists.
    pub fn describe(&self) -> String {
        match self {
            Diagnostic::BareProbe => "native-host: bare probe (no hello); not admitted".to_string(),
            Diagnostic::MalformedHello { parse_error } => {
                format!("native-host: malformed hello JSON ({parse_error}); rejected")
            }
            Diagnostic::WrongRole { role } => {
                format!("native-host: hello carried an unexpected role ({role:?}); rejected")
            }
            Diagnostic::Attached {
                browser_pid,
                replaced_existing,
            } => format!(
                "native-host: browser attached, pid={browser_pid}{}",
                if *replaced_existing {
                    " (replaced an existing session for this pid)"
                } else {
                    " (new session)"
                }
            ),
            Diagnostic::Detached { browser_pid } => {
                format!("native-host: browser detached, pid={browser_pid}")
            }
            Diagnostic::DetachedStale { browser_pid } => format!(
                "native-host: pid={browser_pid}'s stream closed, but a NEWER session for the \
                 same pid already replaced it; leaving that one alone"
            ),
            Diagnostic::FocusReported { browser_pid } => {
                format!("native-host: pid={browser_pid} reported gaining focus")
            }
            Diagnostic::FromExtension {
                browser_pid,
                event,
                detail,
            } => {
                format!("extension (pid={browser_pid}) debug: {event} {detail}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attached_distinguishes_new_from_replaced() {
        let new = Diagnostic::Attached {
            browser_pid: 42,
            replaced_existing: false,
        }
        .describe();
        let replaced = Diagnostic::Attached {
            browser_pid: 42,
            replaced_existing: true,
        }
        .describe();
        assert!(new.contains("pid=42") && new.contains("new session"));
        assert!(replaced.contains("pid=42") && replaced.contains("replaced an existing session"));
        assert_ne!(new, replaced);
    }

    #[test]
    fn bare_probe_and_malformed_hello_are_distinguishable() {
        let probe = Diagnostic::BareProbe.describe();
        let malformed = Diagnostic::MalformedHello {
            parse_error: "EOF while parsing",
        }
        .describe();
        assert!(probe.contains("bare probe"));
        assert!(malformed.contains("malformed hello") && malformed.contains("EOF while parsing"));
    }

    #[test]
    fn from_extension_carries_pid_event_and_detail() {
        let detail = serde_json::json!({"lastError": "boom"});
        let line = Diagnostic::FromExtension {
            browser_pid: 7,
            event: "connect_disconnect",
            detail: &detail,
        }
        .describe();
        assert!(line.contains("pid=7"));
        assert!(line.contains("connect_disconnect"));
        assert!(line.contains("boom"));
    }
}
