//! What Ghostlight says on its MCP intake.
//!
//! Four kinds of words live here: the refusals of a relay opening, the messages of the JSON-RPC
//! errors the MCP session engine answers itself, the sentences of the failure templates a
//! connector sends when it cannot reach the service, and the sentence the service answers to work
//! that arrives during a quit. The connector authors no words, so all of them come from this
//! module.

/// Longest message a JSON-RPC error carries, in characters.
const MAX_ERROR_MESSAGE_CHARS: usize = 500;

/// The service's stable reason code for an invocation that arrives after a quit has begun.
pub const SHUTTING_DOWN_CODE: &str = "shutting_down";

/// What the service says to an invocation that arrives after a quit has begun.
pub const SHUTTING_DOWN_MESSAGE: &str =
    "Ghostlight is quitting and takes no new work. Nothing was sent. Try the call again in a moment.";

/// Why the service refused a relay opening, so the peer can tell the person.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpeningRefusal {
    /// The token did not match this service.
    Authentication,
    /// The relay revision, stream kind or control operation is not one this service speaks.
    Unsupported,
    /// The opening could not be read.
    Malformed,
    /// The service is at its limit of open sessions.
    Capacity,
    /// The service is quitting.
    ShuttingDown,
}

impl OpeningRefusal {
    /// The sentence the refusal carries.
    #[must_use]
    pub fn message(self) -> String {
        match self {
            Self::Authentication => "The token does not match this service.".into(),
            Self::Unsupported => {
                "This Ghostlight service does not speak what the connection asked for. Update Ghostlight on this computer."
                    .into()
            }
            Self::Malformed => "The connection did not open the way Ghostlight expects.".into(),
            Self::Capacity => {
                "Ghostlight has too many open sessions right now. Try again in a moment.".into()
            }
            Self::ShuttingDown => {
                "Ghostlight is quitting and takes no new connections. Try again in a moment."
                    .into()
            }
        }
    }
}

/// A request the MCP session engine answers with a JSON-RPC error instead of forwarding it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolRefusal {
    /// The line is not JSON. Carries the parser's own detail.
    Parse(String),
    /// The message is not a JSON-RPC 2.0 request or notification object.
    InvalidRequest,
    /// The line is a batch array with no elements.
    EmptyBatch,
    /// A second `initialize` arrived on an initialized session.
    AlreadyInitialized,
    /// A request reused the id of a call that is still running.
    DuplicateId,
    /// A request other than discovery, `initialize` or `ping` arrived before `initialize`.
    NotInitialized,
    /// The method is not one this server implements.
    MethodNotFound,
    /// `initialize` named no protocol revision.
    MissingProtocolVersion,
    /// `server/discover` carried a null id.
    DiscoveryNullId,
    /// `server/discover` named another protocol revision.
    DiscoveryVersion(&'static str),
    /// `server/discover` carried no client capabilities object.
    DiscoveryCapabilities,
    /// `tools/call` named no tool.
    MissingToolName,
    /// `tools/call` carried arguments that are not an object.
    ArgumentsNotObject,
    /// The service answered a forwarded request with an error code and message.
    Service {
        /// The service's stable reason code.
        code: String,
        /// The service's own message.
        message: String,
    },
}

impl ProtocolRefusal {
    /// The bounded, ASCII-safe message of the JSON-RPC error.
    #[must_use]
    pub fn message(&self) -> String {
        let message = match self {
            Self::Parse(detail) => format!("Parse error: {detail}"),
            Self::InvalidRequest => "Invalid request: expected a JSON-RPC 2.0 object.".into(),
            Self::EmptyBatch => "Invalid request: the batch is empty.".into(),
            Self::AlreadyInitialized => "This session is already initialized.".into(),
            Self::DuplicateId => "A request with this id is still running.".into(),
            Self::NotInitialized => "Server not initialized".into(),
            Self::MethodNotFound => "Method not found".into(),
            Self::MissingProtocolVersion => "initialize requires params.protocolVersion".into(),
            Self::DiscoveryNullId => "server/discover requires a non-null id".into(),
            Self::DiscoveryVersion(version) => {
                format!("server/discover requires protocol version {version}")
            }
            Self::DiscoveryCapabilities => "server/discover requires client capabilities".into(),
            Self::MissingToolName => "tools/call requires params.name".into(),
            Self::ArgumentsNotObject => "params.arguments must be an object".into(),
            Self::Service { code, message } => format!("{code}: {message}"),
        };
        message.chars().take(MAX_ERROR_MESSAGE_CHARS).collect()
    }
}

/// Why a connector answers a request without the service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectorFailure {
    /// The service went away while the request was outstanding.
    CutOff,
    /// The request was held for a service that did not come back in time.
    Expired,
}

impl ConnectorFailure {
    /// The summary of a tool call answered by this failure.
    #[must_use]
    pub const fn call_summary(self) -> &'static str {
        match self {
            Self::CutOff => {
                "Ghostlight restarted before this call finished. Whether it took effect is unknown, and it was not repeated."
            }
            Self::Expired => "Ghostlight was not running and did not start in time. Nothing was sent.",
        }
    }

    /// The safe next steps of a tool call answered by this failure.
    #[must_use]
    pub const fn call_next_steps(self) -> &'static [&'static str] {
        match self {
            Self::CutOff => {
                &["Read the page to see whether the call took effect before repeating it."]
            }
            Self::Expired => &["Try the call again in a moment."],
        }
    }

    /// The model-facing text of a tool call answered by this failure.
    #[must_use]
    pub fn call_text(self) -> String {
        super::outcome::model_text(self.call_summary(), self.call_next_steps())
    }

    /// The message of a JSON-RPC error for any other request answered by this failure.
    #[must_use]
    pub const fn request_message(self) -> &'static str {
        match self {
            Self::CutOff => "Ghostlight restarted before this request finished.",
            Self::Expired => {
                "Ghostlight was not running and did not start in time. Nothing was sent."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectorFailure, ProtocolRefusal, SHUTTING_DOWN_MESSAGE};

    #[test]
    fn every_failure_sentence_is_plain_ascii() {
        for failure in [ConnectorFailure::CutOff, ConnectorFailure::Expired] {
            assert!(failure.call_summary().is_ascii());
            assert!(failure.request_message().is_ascii());
            assert!(failure.call_next_steps().iter().all(|step| step.is_ascii()));
            assert!(failure.call_summary().ends_with('.'));
        }
    }

    #[test]
    fn the_quit_refusal_is_plain_ascii_and_says_nothing_was_sent() {
        assert!(SHUTTING_DOWN_MESSAGE.is_ascii());
        assert!(SHUTTING_DOWN_MESSAGE.contains("Nothing was sent"));
        assert!(SHUTTING_DOWN_MESSAGE.ends_with('.'));
    }

    #[test]
    fn every_opening_refusal_is_a_plain_ascii_sentence() {
        use super::OpeningRefusal;

        for refusal in [
            OpeningRefusal::Authentication,
            OpeningRefusal::Unsupported,
            OpeningRefusal::Malformed,
            OpeningRefusal::Capacity,
            OpeningRefusal::ShuttingDown,
        ] {
            let message = refusal.message();
            assert!(message.is_ascii());
            assert!(message.ends_with('.'));
        }
    }

    #[test]
    fn a_service_refusal_names_its_code_and_stays_bounded() {
        let refusal = ProtocolRefusal::Service {
            code: "duplicate_request".into(),
            message: "x".repeat(2_000),
        };
        let message = refusal.message();
        assert!(message.starts_with("duplicate_request: x"));
        assert_eq!(message.chars().count(), 500);
    }
}
