//! The session vocabulary between an intake and the service's session queue.
//!
//! An intake turns what its client sends into [`ServiceRequest`]s and renders the queue's
//! [`ServiceResponse`]s back to the client. These types never cross a process boundary; the wire
//! between processes is relay protocol 4. Only what a client or a durable record sees is
//! serializable: tool definitions, the intake channel and the session marker.

use ghostlight_relay::protocol::{CLI_CHANNEL, MCP_CHANNEL};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::work::InvocationId;

/// Product metadata the service hands every client.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerProfile {
    /// Stable product name.
    pub name: String,
    /// Product build version.
    pub version: String,
    /// Model-facing server instructions.
    pub instructions: String,
}

/// Standard MCP behavior hints for one tool.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Human-readable display title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Whether the tool is expected to leave its environment unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    /// Whether the tool may perform destructive updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    /// Whether repeated calls with the same input have no additional effect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    /// Whether the tool may interact with entities outside its local environment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

/// One model-facing tool definition, as `tools/list` advertises it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolDefinition {
    /// Exact tool name.
    pub name: String,
    /// Concise model-facing description.
    pub description: String,
    /// JSON Schema for the input object.
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    /// Standard MCP behavior hints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

/// Model content that travels beside a result's structured facts.
#[derive(Clone, Debug, PartialEq)]
pub enum ServiceContent {
    /// One bounded base64-encoded image.
    Image {
        /// Image media type.
        mime_type: String,
        /// Base64-encoded image bytes.
        data: String,
    },
}

/// Which intake a session arrived on.
///
/// Closed on purpose: an intake is a fact about the shape of the caller, not an open label. It is
/// recorded for attribution and presentation, and it decides one thing: a policy's channel switch
/// admits or refuses a request by the channel its connection declared (ADR-0202 Decision 22a).
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum IntakeChannel {
    /// A model-facing MCP client, through the MCP edge.
    #[default]
    Mcp,
    /// A local script or program, through the Ghostlight command line.
    Cli,
}

impl IntakeChannel {
    /// The intake an `mcp` opening's channel word names. Only the command line's word is the
    /// command line; an unknown word or none at all is an MCP client.
    #[must_use]
    pub fn of_opening(channel: Option<&str>) -> Self {
        if channel == Some(CLI_CHANNEL) {
            Self::Cli
        } else {
            Self::Mcp
        }
    }

    /// Render the stable ASCII channel name, the word an opening on this channel sends.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mcp => MCP_CHANNEL,
            Self::Cli => CLI_CHANNEL,
        }
    }
}

/// What owns a session, so a workspace can outlive the connection that opened it (ADR-0106).
///
/// Handles belong to a session. Keying that session on the caller rather than on the socket is what
/// lets a person type one command after another, and an application shell out repeatedly, and have
/// every call land in the same workspace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionMarker {
    /// The calling process. The session lives exactly as long as that process does.
    ///
    /// Identity is the pair the operating system actually keeps unique. A pid alone is recycled,
    /// and a pid with a name matches a recycled pid whenever the replacement is the same kind of
    /// program, which is the common case rather than the rare one.
    Process {
        /// Operating-system process id of the caller.
        pid: u32,
        /// Process start time, which disambiguates a recycled pid.
        started_at: u64,
        /// Executable file name, for attribution only. Never identity.
        name: String,
    },
    /// An explicit key, for a caller whose own children are ephemeral.
    ///
    /// Environment is inherited through intermediaries, so a program that shells out through a
    /// throwaway shell can still gather its calls into one session.
    Declared {
        /// Opaque caller-supplied key.
        key: String,
    },
}

impl SessionMarker {
    /// The stable string a workspace is filed under.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Process {
                pid, started_at, ..
            } => format!("process:{pid}:{started_at}"),
            Self::Declared { key } => format!("declared:{key}"),
        }
    }

    /// The caller's file name, when one was observed.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Process { name, .. } => Some(name),
            Self::Declared { .. } => None,
        }
    }
}

/// What an intake asks of the session queue.
#[derive(Clone, Debug, PartialEq)]
pub enum ServiceRequest {
    /// Retrieve the catalog this session may use.
    Catalog,
    /// Invoke a catalog tool with an opaque JSON input.
    Invoke {
        /// The request's id, minted by the intake for this call.
        id: InvocationId,
        /// Catalog tool name.
        tool: String,
        /// Opaque input the executor validates.
        input: Value,
    },
    /// Cancel an active invocation.
    Cancel {
        /// The id given to `Invoke`.
        id: InvocationId,
    },
}

/// What the session queue answers an intake.
#[derive(Clone, Debug, PartialEq)]
pub enum ServiceResponse {
    /// The catalog this session may use.
    Catalog {
        /// Tool definitions the service owns.
        tools: Vec<ToolDefinition>,
    },
    /// Policy changed the catalog after this session was given one.
    CatalogChanged {
        /// Monotonic generation within this session.
        generation: u64,
        /// The fresh complete catalog.
        tools: Vec<ToolDefinition>,
    },
    /// One finished invocation.
    Result {
        /// The id given to `Invoke`.
        id: InvocationId,
        /// Concise model-facing outcome the service wrote.
        text: String,
        /// The structured result envelope.
        result: Value,
        /// Whether the result reports a failed invocation.
        is_error: bool,
        /// Content that travels beside the envelope.
        content: Vec<ServiceContent>,
    },
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{IntakeChannel, SessionMarker, ToolAnnotations, ToolDefinition};

    #[test]
    fn each_intake_is_named_by_the_word_its_opening_sends_and_records() {
        for channel in [IntakeChannel::Mcp, IntakeChannel::Cli] {
            assert_eq!(IntakeChannel::of_opening(Some(channel.as_str())), channel);
            assert_eq!(serde_json::to_value(channel).unwrap(), channel.as_str());
        }
    }

    #[test]
    fn tool_metadata_uses_standard_mcp_wire_names_and_omits_absent_fields() {
        let complete = ToolDefinition {
            name: "browser_tabs".into(),
            description: "List controlled tabs.".into(),
            input_schema: json!({"type": "object"}),
            annotations: Some(ToolAnnotations {
                title: Some("List tabs".into()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        };
        let encoded = serde_json::to_value(complete).expect("tool serializes");
        assert_eq!(encoded["inputSchema"], json!({"type": "object"}));
        assert_eq!(encoded["annotations"]["title"], "List tabs");
        assert_eq!(encoded["annotations"]["readOnlyHint"], true);
        assert_eq!(encoded["annotations"]["destructiveHint"], false);
        assert_eq!(encoded["annotations"]["idempotentHint"], true);
        assert_eq!(encoded["annotations"]["openWorldHint"], false);
        assert!(encoded.get("input_schema").is_none());
        assert!(encoded["annotations"].get("read_only_hint").is_none());

        let minimal = ToolDefinition {
            name: "browser_tabs".into(),
            description: "List controlled tabs.".into(),
            input_schema: json!({"type": "object"}),
            annotations: None,
        };
        let encoded = serde_json::to_value(minimal).expect("tool serializes");
        assert!(encoded.get("annotations").is_none());
    }

    #[test]
    fn the_cli_session_marker_reads_as_the_relay_opening_fixture_writes_it() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/relay4/opening.cli.json");
        let opening: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(fixture).unwrap()).unwrap();
        let marker: SessionMarker = serde_json::from_value(opening["session"].clone()).unwrap();
        assert_eq!(marker.key(), "process:18244:1790000123");
        assert_eq!(marker.name(), Some("ghostlight"));
        assert_eq!(serde_json::to_value(&marker).unwrap(), opening["session"]);
    }
}
