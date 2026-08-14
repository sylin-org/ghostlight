//! Generic MCP-edge to orchestrator messages.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Service-edge bridge major understood by this build.
pub const SERVICE_BRIDGE_MAJOR: u16 = 2;

/// Product metadata supplied by the orchestrator and rendered by protocol edges.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerProfile {
    /// Stable product name.
    pub name: String,
    /// Product build version.
    pub version: String,
    /// Model-facing server instructions.
    pub instructions: String,
}

/// Standard MCP behavior hints supplied by the orchestrator for one tool.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolAnnotations {
    /// Human-readable display title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Whether the tool is expected to leave its environment unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    /// Whether the tool may perform destructive updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    /// Whether repeated calls with the same input have no additional effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    /// Whether the tool may interact with entities outside its local environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

/// A model-facing tool definition owned and supplied by the orchestrator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDefinition {
    /// Exact tool name.
    pub name: String,
    /// Concise model-facing description.
    pub description: String,
    /// JSON Schema for the input object.
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    /// Optional JSON Schema for the structured result object.
    #[serde(
        rename = "outputSchema",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub output_schema: Option<Value>,
    /// Optional standard MCP behavior hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

/// Generic model content rendered by the protocol edge alongside structured product facts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
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
/// recorded for attribution and presentation and is never an input to an authority decision
/// (ADR-0105 Decision 2).
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum IntakeChannel {
    /// A model-facing MCP client, through the stdio connector.
    #[default]
    Mcp,
    /// A local script or program, through the Ghostlight command line.
    Cli,
}

impl IntakeChannel {
    /// Render the stable ASCII channel name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mcp => "mcp",
            Self::Cli => "cli",
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

/// A request sent from the generic MCP edge to the orchestrator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServiceRequest {
    /// Ask an already-running desktop orchestrator to reveal its workbench.
    ActivateWorkbench {
        /// Bridge major supported by the requesting executable.
        major: u16,
        /// Runtime authentication token.
        token: String,
    },
    /// Establish a compatible authenticated session.
    Hello {
        /// Bridge major supported by the edge.
        major: u16,
        /// Runtime authentication token.
        token: String,
        /// Human-readable client label used only for audit and presentation.
        client_label: String,
        /// Which intake this session arrived on. Attribution only, never authority (ADR-0105).
        #[serde(default)]
        channel: IntakeChannel,
        /// What owns this session, when the edge can say. Absent keeps the workspace bound to the
        /// connection, which is what the MCP edge wants.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<SessionMarker>,
    },
    /// Retrieve the orchestrator-owned catalog.
    Catalog,
    /// Invoke a catalog operation with an opaque JSON input.
    Invoke {
        /// Edge-local opaque correlation id.
        id: String,
        /// Catalog tool name.
        tool: String,
        /// Opaque input validated by the orchestrator.
        input: Value,
        /// Optional caller deadline relative to receipt.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_ms: Option<u64>,
    },
    /// Forward cancellation for an active invocation.
    Cancel {
        /// Correlation id supplied to `Invoke`.
        id: String,
    },
}

/// A response sent from the orchestrator to the generic MCP edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServiceResponse {
    /// Result of asking the current desktop presentation adapter to reveal itself.
    WorkbenchActivated {
        /// Whether a native workbench was attached and accepted the request.
        available: bool,
    },
    /// A compatible session was established.
    HelloAccepted {
        /// Orchestrator bridge major.
        major: u16,
        /// Opaque workspace session handle.
        session: String,
        /// Orchestrator-owned product metadata.
        server: ServerProfile,
    },
    /// The complete model-facing catalog.
    Catalog {
        /// Tool definitions owned by the orchestrator.
        tools: Vec<ToolDefinition>,
    },
    /// The policy-projected catalog changed after this session's initial projection.
    CatalogChanged {
        /// Monotonic generation within this service connection.
        generation: u64,
        /// Fresh complete projection, still owned by the orchestrator.
        tools: Vec<ToolDefinition>,
    },
    /// One terminal opaque product result.
    Result {
        /// Correlation id supplied to `Invoke`.
        id: String,
        /// Concise model-facing outcome authored by the orchestrator.
        text: String,
        /// Structured product result rendered generically by the edge.
        result: Value,
        /// Whether the product result reports an invocation failure.
        is_error: bool,
        /// Optional protocol-neutral content rendered generically by the edge.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content: Vec<ServiceContent>,
    },
    /// A bridge or pre-invocation failure.
    Error {
        /// Correlation id when one is available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Stable bridge-level reason code.
        code: String,
        /// Bounded human-readable detail.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        IntakeChannel, ServerProfile, ServiceContent, ServiceRequest, ServiceResponse,
        SessionMarker, ToolAnnotations, ToolDefinition, SERVICE_BRIDGE_MAJOR,
    };
    use serde_json::json;

    #[test]
    fn service_messages_round_trip() {
        let requests = [
            ServiceRequest::ActivateWorkbench {
                major: SERVICE_BRIDGE_MAJOR,
                token: "token".into(),
            },
            ServiceRequest::Hello {
                major: SERVICE_BRIDGE_MAJOR,
                token: "token".into(),
                client_label: "test".into(),
                channel: IntakeChannel::Cli,
                session: Some(SessionMarker::Process {
                    pid: 4312,
                    started_at: 1_700_000_000,
                    name: "pwsh.exe".into(),
                }),
            },
            ServiceRequest::Catalog,
            ServiceRequest::Invoke {
                id: "request-1".into(),
                tool: "browser_tabs".into(),
                input: json!({}),
                deadline_ms: Some(500),
            },
            ServiceRequest::Cancel {
                id: "request-1".into(),
            },
        ];
        for request in requests {
            let encoded = serde_json::to_vec(&request).expect("request serializes");
            let decoded: ServiceRequest =
                serde_json::from_slice(&encoded).expect("request deserializes");
            assert_eq!(decoded, request);
        }

        let responses = [
            ServiceResponse::WorkbenchActivated { available: true },
            ServiceResponse::HelloAccepted {
                major: SERVICE_BRIDGE_MAJOR,
                session: "workspace_test".into(),
                server: ServerProfile {
                    name: "ghostlight".into(),
                    version: "1.0.0".into(),
                    instructions: "Use the advertised browser operations.".into(),
                },
            },
            ServiceResponse::Catalog {
                tools: vec![ToolDefinition {
                    name: "browser_tabs".into(),
                    description: "List controlled tabs.".into(),
                    input_schema: json!({"type": "object"}),
                    output_schema: Some(json!({
                        "type": "object",
                        "required": ["tabs"]
                    })),
                    annotations: Some(ToolAnnotations {
                        title: Some("List tabs".into()),
                        read_only_hint: Some(true),
                        destructive_hint: Some(false),
                        idempotent_hint: Some(true),
                        open_world_hint: Some(false),
                    }),
                }],
            },
            ServiceResponse::CatalogChanged {
                generation: 2,
                tools: Vec::new(),
            },
            ServiceResponse::Result {
                id: "request-1".into(),
                text: "Found one tab.".into(),
                result: json!({"status":"succeeded"}),
                is_error: false,
                content: vec![ServiceContent::Image {
                    mime_type: "image/jpeg".into(),
                    data: "base64-image".into(),
                }],
            },
        ];
        for response in responses {
            let encoded = serde_json::to_vec(&response).expect("response serializes");
            let decoded: ServiceResponse =
                serde_json::from_slice(&encoded).expect("response deserializes");
            assert_eq!(decoded, response);
        }
    }

    #[test]
    fn tool_metadata_uses_standard_mcp_wire_names_and_omits_absent_fields() {
        let complete = ToolDefinition {
            name: "browser_tabs".into(),
            description: "List controlled tabs.".into(),
            input_schema: json!({"type": "object"}),
            output_schema: Some(json!({"type": "object"})),
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
        assert_eq!(encoded["outputSchema"], json!({"type": "object"}));
        assert_eq!(encoded["annotations"]["title"], "List tabs");
        assert_eq!(encoded["annotations"]["readOnlyHint"], true);
        assert_eq!(encoded["annotations"]["destructiveHint"], false);
        assert_eq!(encoded["annotations"]["idempotentHint"], true);
        assert_eq!(encoded["annotations"]["openWorldHint"], false);
        assert!(encoded.get("input_schema").is_none());
        assert!(encoded.get("output_schema").is_none());
        assert!(encoded["annotations"].get("read_only_hint").is_none());

        let minimal = ToolDefinition {
            name: "browser_tabs".into(),
            description: "List controlled tabs.".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: None,
        };
        let encoded = serde_json::to_value(minimal).expect("tool serializes");
        assert!(encoded.get("outputSchema").is_none());
        assert!(encoded.get("annotations").is_none());
    }

    #[test]
    fn protocol_two_requires_authored_text_and_error_status() {
        let legacy = json!({
            "kind": "result",
            "id": "request-1",
            "result": {"status": "succeeded"}
        });
        assert!(serde_json::from_value::<ServiceResponse>(legacy).is_err());
    }
}
