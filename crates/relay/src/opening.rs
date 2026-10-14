//! The opening frame and the frames that follow it.
//!
//! Every relay connection starts with one [`Opening`] from the peer. The service answers with one
//! [`Reply`]. After an accepted `mcp` opening both sides exchange [`McpFrame`]s; after an accepted
//! `browser` opening the stream carries the adapter protocol untouched; after an accepted
//! `control` opening the service sends one [`ControlResult`] and closes.
//!
//! Every enum here is tagged by `kind` (or named by a string) and every reader ignores unknown
//! fields, so later revisions add names without breaking older peers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::RELAY;

/// The first frame a peer sends on a relay connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    /// Relay revision the peer speaks.
    pub relay: u16,
    /// The service's secret, read from the runtime file.
    pub token: String,
    /// What the peer wants this connection to carry.
    pub stream: Stream,
}

impl Opening {
    /// An opening for `stream` at this crate's relay revision.
    #[must_use]
    pub fn new(token: String, stream: Stream) -> Self {
        Self {
            relay: RELAY,
            token,
            stream,
        }
    }
}

/// What a connection carries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Stream {
    /// One MCP client session, as [`McpFrame`]s.
    Mcp {
        /// How the client reached the connector (for example `cli` for the command line).
        channel: String,
        /// An opaque marker the peer uses to identify its session, such as the process.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<Value>,
        /// The resume token the service last handed this session, when reconnecting.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume: Option<String>,
    },
    /// One browser adapter port, carried as raw native-message frames.
    Browser,
    /// One control operation and its single result.
    Control {
        /// The operation requested.
        op: ControlOp,
    },
    /// A stream kind this revision does not know. The service refuses it as unsupported.
    #[serde(other)]
    Unknown,
}

/// The operations a control stream can request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlOp {
    /// Bring the workbench forward.
    ActivateWorkbench,
    /// Report whether the service is ready, without changing anything.
    Readiness,
    /// Ask the service to finish in-flight work and exit.
    Quit,
    /// An operation this revision does not know. The service refuses it as unsupported.
    #[serde(other)]
    Unknown,
}

/// The service's answer to an [`Opening`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reply {
    /// The opening was accepted; the stream is live.
    Accepted {
        /// Product version of the service.
        service_version: String,
    },
    /// The opening was refused and the service closes the connection.
    Refused {
        /// Why, as a closed code.
        code: RefusalCode,
        /// The service's own words for the person, when it has any.
        message: String,
    },
    /// A reply kind this revision does not know. The connector treats it as a refusal.
    #[serde(other)]
    Unknown,
}

/// Why a service refused an opening.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// The token did not match.
    AuthenticationFailed,
    /// The relay revision, stream kind or control operation is not one this service speaks.
    Unsupported,
    /// The service is at its limit of open connections.
    Capacity,
    /// The service is quitting and takes no new work.
    ShuttingDown,
    /// The opening could not be read.
    Malformed,
    /// A refusal code this revision does not know. The connector treats it as a refusal.
    #[serde(other)]
    Unknown,
}

/// A frame on an accepted `mcp` stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpFrame {
    /// One MCP line, exactly as the client or the service wrote it, in either direction.
    Message {
        /// The line, without its newline. A JSON-RPC batch array is one line.
        line: String,
    },
    /// The service hands the connector the token that resumes this session after a restart.
    Resume {
        /// The opaque resume token.
        token: String,
    },
    /// A frame kind this revision does not know. Readers skip it.
    #[serde(other)]
    Unknown,
}

/// The single frame an accepted `control` stream returns before it closes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlResult {
    /// The operation finished; its outcome is the service's to define.
    Result {
        /// The operation's result.
        value: Value,
    },
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_new_opening_carries_this_relay_revision() {
        assert_eq!(Opening::new("t".to_owned(), Stream::Browser).relay, RELAY);
    }

    #[test]
    fn absent_session_and_resume_are_not_written() {
        let opening = Opening::new(
            "t".to_owned(),
            Stream::Mcp {
                channel: "npm".to_owned(),
                session: None,
                resume: None,
            },
        );
        assert_eq!(
            serde_json::to_value(opening).unwrap(),
            json!({"relay": 3, "token": "t", "stream": {"kind": "mcp", "channel": "npm"}})
        );
    }

    #[test]
    fn unknown_names_decode_to_the_unknown_variants() {
        let stream: Stream = serde_json::from_value(json!({"kind": "telemetry"})).unwrap();
        assert_eq!(stream, Stream::Unknown);
        let op: ControlOp = serde_json::from_value(json!("reboot")).unwrap();
        assert_eq!(op, ControlOp::Unknown);
        let frame: McpFrame = serde_json::from_value(json!({"kind": "ping"})).unwrap();
        assert_eq!(frame, McpFrame::Unknown);
        let reply: Reply = serde_json::from_value(json!({"kind": "queued"})).unwrap();
        assert_eq!(reply, Reply::Unknown);
        let code: RefusalCode = serde_json::from_value(json!("installing")).unwrap();
        assert_eq!(code, RefusalCode::Unknown);
    }
}
