//! The relay protocol 4 vocabulary: the opening, the reply, the frames of each role, the control
//! operations and the goodbye (ADR-0199 Decision 5).
//!
//! A connection to the endpoint starts with one [`Opening`] from the peer and one [`Reply`] from
//! the service. [`open`] runs that exchange. What follows depends on the opening's [`Role`]:
//! an `mcp` stream carries [`McpFrame`]s, a `browser` stream carries the adapter protocol
//! untouched, a `control` stream carries one [`ControlResult`] and closes. The `desktop` and
//! `command` roles exist from the first revision so a later shell or command stream needs no new
//! relay revision.
//!
//! Every name is a closed word, and every reader is tolerant: a word this revision does not know
//! decodes to the `Unknown` variant of its enum, and a field this revision does not know is
//! ignored. The service decides what to do with an `Unknown`. New behavior gets new names; a name
//! is never removed or repurposed. The words themselves are authored by the service, never here:
//! this module carries only the shapes.
//!
//! The executable form of this module is `tests/golden/relay4`, and the prose form is
//! `docs/design/relay-protocol-4.md`.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::endpoint::{self, ConnectError, Connection};
use crate::frame::{self, FrameError};
use crate::RELAY;

mod templates;

#[cfg(test)]
mod tests;

pub use templates::{
    bare_error, failure_response, fill, Failure, Templates, DEFAULT_TEMPLATE, INTERNAL_ERROR_CODE,
};

/// The fixed start of the goodbye frame a service writes on a browser stream.
const GOODBYE_PREFIX: &str = r#"{"goodbye":""#;

/// What a connection carries, and so what the service does with it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// One MCP client session, as [`McpFrame`]s.
    Mcp,
    /// One browser adapter port, carried as raw native-message frames.
    Browser,
    /// One control operation and its single result.
    Control,
    /// The desktop shell, which shares the service's build.
    Desktop,
    /// One typed command and its streams.
    Command,
    /// A role this revision does not know. The service refuses it as unsupported.
    #[serde(other)]
    Unknown,
}

/// Why a service was started. The service applies the quit latch by reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartReason {
    /// An MCP relay needed it.
    Agent,
    /// A browser relay needed it.
    Browser,
    /// A typed command needed it.
    Command,
    /// A person opened Ghostlight.
    Person,
    /// The shell restarts it after losing it without a goodbye.
    Recover,
    /// The installer starts it after a swap.
    Update,
    /// A reason this revision does not know.
    #[serde(other)]
    Unknown,
}

impl StartReason {
    /// The reason's name as it appears on the wire and on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Browser => "browser",
            Self::Command => "command",
            Self::Person => "person",
            Self::Recover => "recover",
            Self::Update => "update",
            Self::Unknown => "unknown",
        }
    }

    /// The reason named `text`, or `None` for a word that is not one of the six.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Agent,
            Self::Browser,
            Self::Command,
            Self::Person,
            Self::Recover,
            Self::Update,
        ]
        .into_iter()
        .find(|reason| reason.as_str() == text)
    }
}

/// Why a service is about to close its streams. Sent before the close.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoodbyeReason {
    /// The person quit Ghostlight.
    Quit,
    /// A newer service is about to take over.
    Replace,
    /// Ghostlight is being removed.
    Uninstall,
    /// The operating-system session is ending.
    SessionEnd,
    /// A reason this revision does not know. A peer treats it as the end of the stream.
    #[serde(other)]
    Unknown,
}

impl GoodbyeReason {
    /// The reason's name as it appears on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quit => "quit",
            Self::Replace => "replace",
            Self::Uninstall => "uninstall",
            Self::SessionEnd => "session_end",
            Self::Unknown => "unknown",
        }
    }

    /// The reason named `text`; a word that is not one of the four is [`GoodbyeReason::Unknown`].
    #[must_use]
    pub fn parse(text: &str) -> Self {
        [Self::Quit, Self::Replace, Self::Uninstall, Self::SessionEnd]
            .into_iter()
            .find(|reason| reason.as_str() == text)
            .unwrap_or(Self::Unknown)
    }
}

/// Why a `quit` control operation was asked for. The service turns it into the goodbye reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuitReason {
    /// The person asked Ghostlight to quit.
    Quit,
    /// A newer service is about to take over.
    Replace,
    /// Ghostlight is being removed.
    Uninstall,
    /// A reason this revision does not know.
    #[serde(other)]
    Unknown,
}

/// Which program is connecting.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Program {
    /// The frozen edge, in any of its roles.
    Edge,
    /// A process of the service image, such as a one-shot command.
    Service,
    /// The desktop shell.
    Desktop,
    /// A program this revision does not know.
    #[serde(other)]
    Unknown,
}

/// The connecting program, so the service can tell a restart from a blip and an old edge from a
/// new one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Which program is connecting.
    pub program: Program,
    /// The program's own revision, which moves only when its protocol does.
    pub revision: u16,
    /// The program's build, as its version string.
    pub build: String,
}

impl Edge {
    /// A program at this module's revision with the given build.
    #[must_use]
    pub fn new(program: Program, build: String) -> Self {
        Self {
            program,
            revision: RELAY,
            build,
        }
    }
}

/// One control operation, named by `op`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Control {
    /// Report whether the service is ready, without changing anything.
    Readiness,
    /// Report the service's state.
    Status,
    /// Bring the window forward.
    Show,
    /// Finish in-flight work and exit.
    Quit {
        /// Why, so the service can send the matching goodbye.
        reason: QuitReason,
    },
    /// An operation this revision does not know. The service refuses it as unsupported.
    #[serde(other)]
    Unknown,
}

/// The first frame a peer sends on a relay connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    /// Relay revision the peer speaks.
    pub relay: u16,
    /// What the peer wants this connection to carry.
    pub role: Role,
    /// The connecting program.
    pub edge: Edge,
    /// Why the peer is starting the service, when it had to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<StartReason>,
    /// The resume token the service last handed this session, when reconnecting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
    /// For a `browser` stream, the origin of the extension that opened the port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// For an `mcp` stream, how the client reached the edge (for example `cli`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// For an `mcp` stream, an opaque marker the peer uses to identify its session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<Value>,
    /// For a `control` stream, the operation requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<Control>,
}

impl Opening {
    /// An opening for `role` from `edge` at this module's revision, with every optional field
    /// unset.
    #[must_use]
    pub fn new(role: Role, edge: Edge) -> Self {
        Self {
            relay: RELAY,
            role,
            edge,
            reason: None,
            resume: None,
            origin: None,
            channel: None,
            session: None,
            control: None,
        }
    }
}

/// Why a service refused an opening.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// The relay revision, role or control operation is not one this service speaks.
    Unsupported,
    /// The service is at its limit of open connections.
    Capacity,
    /// The service is quitting and takes no new work.
    ShuttingDown,
    /// The opening could not be read.
    Malformed,
    /// A refusal code this revision does not know. The peer treats it as a refusal.
    #[serde(other)]
    Unknown,
}

/// The service's answer to an [`Opening`].
///
/// One shape covers both outcomes. An accepted reply carries `service`, `instance` and
/// `templates`; a refused one carries `code` and `say`, which the peer prints as is.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    /// Whether the opening was accepted and the stream is live.
    pub accepted: bool,
    /// Product version of the service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// An opaque value that changes on every service start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// The failure sentences an MCP edge may say when it cannot reach the service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<Templates>,
    /// Why the opening was refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<RefusalCode>,
    /// The service's own words for the person, when it refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub say: Option<String>,
}

impl Reply {
    /// An accepted reply. Only an `mcp` stream's reply carries failure templates.
    #[must_use]
    pub fn accepted(service: String, instance: String, templates: Option<Templates>) -> Self {
        Self {
            accepted: true,
            service: Some(service),
            instance: Some(instance),
            templates,
            code: None,
            say: None,
        }
    }

    /// A refused reply with the service's own words.
    #[must_use]
    pub fn refused(code: RefusalCode, say: String) -> Self {
        Self {
            accepted: false,
            service: None,
            instance: None,
            templates: None,
            code: Some(code),
            say: Some(say),
        }
    }
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
    /// The service hands the edge the token that resumes this session after a restart.
    Resume {
        /// The opaque resume token.
        token: String,
    },
    /// The service is about to close the stream.
    Goodbye {
        /// Why.
        reason: GoodbyeReason,
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

/// The payload of the goodbye frame on a browser stream: `{"goodbye":"<reason>"}`.
///
/// A browser stream carries the adapter protocol untouched, so the goodbye is a fixed byte
/// string that the relay can recognize without parsing the adapter's frames.
#[must_use]
pub fn goodbye_frame(reason: GoodbyeReason) -> Vec<u8> {
    format!("{GOODBYE_PREFIX}{}\"}}", reason.as_str()).into_bytes()
}

/// The goodbye reason in a frame read from a browser stream, or `None` for any other frame.
///
/// Only a frame that starts with the fixed goodbye prefix is looked at, and no other frame is
/// ever parsed: adapter frames start with `{"kind":`. A reason this revision does not know is
/// [`GoodbyeReason::Unknown`].
#[must_use]
pub fn goodbye_in(frame: &[u8]) -> Option<GoodbyeReason> {
    let rest = frame.strip_prefix(GOODBYE_PREFIX.as_bytes())?;
    let end = rest.iter().position(|&byte| byte == b'"')?;
    if !matches!(rest.get(end + 1), Some(b'}' | b',')) {
        return None;
    }
    let name = std::str::from_utf8(&rest[..end]).ok()?;
    Some(GoodbyeReason::parse(name))
}

/// Connect to the service of `home` and open it with `opening`, all before `deadline`.
///
/// [`endpoint::connect`] checks that the server runs as this user before the opening is sent. A
/// refused reply is returned like any other; only the caller knows what to do about it.
pub fn connect(
    home: &Path,
    opening: &Opening,
    deadline: Instant,
) -> Result<(Connection, Reply), ConnectError> {
    let connection = endpoint::connect(home, deadline)?;
    let reply = open(&connection, opening, deadline)?;
    Ok((connection, reply))
}

/// Write `opening` and read the service's reply, all before `deadline`.
///
/// The connection's timeouts are cleared again once the reply is read, so the caller owns them
/// from there. A refused reply is returned like any other; only the caller knows what to do
/// about it.
pub fn open(
    connection: &Connection,
    opening: &Opening,
    deadline: Instant,
) -> Result<Reply, ConnectError> {
    connection.set_write_timeout(Some(remaining(deadline)?))?;
    let mut writer = connection;
    frame::write_frame(&mut writer, opening).map_err(frame_failure)?;
    connection.set_write_timeout(None)?;
    connection.set_read_timeout(Some(remaining(deadline)?))?;
    let mut reader = connection;
    let reply = match frame::read_frame::<Reply>(&mut reader) {
        Ok(Some(reply)) => reply,
        Ok(None) => return Err(ConnectError::Closed),
        Err(error) => return Err(frame_failure(error)),
    };
    connection.set_read_timeout(None)?;
    Ok(reply)
}

/// Time left before `deadline`, or [`ConnectError::Deadline`] when none is.
fn remaining(deadline: Instant) -> Result<Duration, ConnectError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or(ConnectError::Deadline)
}

/// Map a framing failure: a timed-out read or write is the deadline, any other I/O failure is
/// I/O, and a frame that cannot be read is invalid data.
fn frame_failure(error: FrameError) -> ConnectError {
    match error {
        FrameError::Io(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) =>
        {
            ConnectError::Deadline
        }
        FrameError::Io(error) => ConnectError::Io(error),
        other => ConnectError::Io(io::Error::new(io::ErrorKind::InvalidData, other)),
    }
}
