//! Generic MCP-edge to orchestrator messages.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Service-edge bridge major understood by this build.
pub const SERVICE_BRIDGE_MAJOR: u16 = 1;

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
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
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
    /// One terminal opaque product result.
    Result {
        /// Correlation id supplied to `Invoke`.
        id: String,
        /// Product result rendered generically by the edge.
        result: Value,
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
        ToolDefinition, SERVICE_BRIDGE_MAJOR,
    };
    use serde_json::json;

    #[test]
    fn service_messages_round_trip() {
        let requests = [
            ServiceRequest::ActivateWorkbench {
                major: 1,
                token: "token".into(),
            },
            ServiceRequest::Hello {
                major: 1,
                token: "token".into(),
                client_label: "test".into(),
                channel: IntakeChannel::Cli,
            },
            ServiceRequest::Catalog,
            ServiceRequest::Invoke {
                id: "request-1".into(),
                tool: "browser_list_tabs".into(),
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
                    name: "browser_list_tabs".into(),
                    description: "List controlled tabs.".into(),
                    input_schema: json!({"type": "object"}),
                }],
            },
            ServiceResponse::Result {
                id: "request-1".into(),
                result: json!({"status":"succeeded"}),
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
}
