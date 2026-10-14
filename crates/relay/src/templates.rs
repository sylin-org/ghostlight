//! Failure templates: the sentences the MCP connector may say when it cannot reach the service.
//!
//! The connector authors no words. While the service is up it writes `relay-templates.json`: for
//! each kind of failure, a JSON-RPC response object per request method, plus a `default`. When a
//! request is cut off or expires, the connector fills the matching template with the request's
//! id and sends it. With no file at all, which only happens before the first install, the
//! connector answers a bare internal error with an empty message.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{file, RELAY};

/// Largest template file a reader accepts, in bytes.
const MAX_TEMPLATES_FILE_BYTES: u64 = 1024 * 1024;

/// Key of the template used when none is named for a request's method.
pub const DEFAULT_TEMPLATE: &str = "default";

/// JSON-RPC internal error code, used when there is no template.
pub const INTERNAL_ERROR_CODE: i64 = -32603;

/// Which failure the connector is reporting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// The service went away while the request was outstanding.
    CutOff,
    /// The request was held for a returning service until the hold limit passed.
    Expired,
}

/// The contents of `relay-templates.json`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FailureTemplates {
    /// Relay revision the file was written for.
    pub relay: u16,
    /// Responses for requests cut off by the service going away, by request method.
    #[serde(default)]
    pub cut_off: BTreeMap<String, Value>,
    /// Responses for held requests that expired, by request method.
    #[serde(default)]
    pub expired: BTreeMap<String, Value>,
}

impl FailureTemplates {
    /// Read the template file at `path`.
    ///
    /// Returns `None` when the file is missing, malformed, too large, or written for another relay
    /// revision. Unknown fields are ignored. Only an unexpected I/O failure is an error.
    pub fn read(path: &Path) -> io::Result<Option<Self>> {
        let Some(document) = file::read_json(path, MAX_TEMPLATES_FILE_BYTES)? else {
            return Ok(None);
        };
        if document.get("relay").and_then(Value::as_u64) != Some(u64::from(RELAY)) {
            return Ok(None);
        }
        Ok(serde_json::from_value(document).ok())
    }

    /// The template for `method` under `failure`, falling back to the `default` entry.
    #[must_use]
    pub fn template(&self, failure: Failure, method: &str) -> Option<&Value> {
        let templates = match failure {
            Failure::CutOff => &self.cut_off,
            Failure::Expired => &self.expired,
        };
        templates
            .get(method)
            .or_else(|| templates.get(DEFAULT_TEMPLATE))
    }
}

/// Fill a template with a request id. The id keeps its JSON type: a number stays a number.
///
/// A template that is not a JSON object cannot carry an id, so it yields the bare error instead.
#[must_use]
pub fn fill(template: &Value, id: &Value) -> Value {
    match template {
        Value::Object(fields) => {
            let mut response = fields.clone();
            response.insert("id".to_owned(), id.clone());
            Value::Object(response)
        }
        _ => bare_error(id),
    }
}

/// A JSON-RPC internal error with an empty message: the answer when no template exists.
#[must_use]
pub fn bare_error(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": INTERNAL_ERROR_CODE, "message": ""},
    })
}

/// The response for a request that failed: the filled template when there is one, else the bare
/// error.
#[must_use]
pub fn failure_response(
    templates: Option<&FailureTemplates>,
    failure: Failure,
    method: &str,
    id: &Value,
) -> Value {
    match templates.and_then(|templates| templates.template(failure, method)) {
        Some(template) => fill(template, id),
        None => bare_error(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> FailureTemplates {
        serde_json::from_value(json!({
            "relay": 3,
            "cut_off": {
                "tools/call": {"jsonrpc": "2.0", "id": null, "result": {"isError": true}},
                "default": {"jsonrpc": "2.0", "id": null, "error": {"code": -32603, "message": "cut"}},
            },
            "expired": {
                "default": {"jsonrpc": "2.0", "id": null, "error": {"code": -32603, "message": "late"}},
            },
        }))
        .unwrap()
    }

    #[test]
    fn a_template_file_reads_back_and_other_files_read_as_none() {
        let directory =
            std::env::temp_dir().join(format!("ghostlight-relay-templates-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("relay-templates.json");
        assert_eq!(FailureTemplates::read(&path).unwrap(), None);
        std::fs::write(&path, serde_json::to_vec(&sample()).unwrap()).unwrap();
        assert_eq!(FailureTemplates::read(&path).unwrap(), Some(sample()));
        std::fs::write(&path, br#"{"relay":4,"cut_off":{},"expired":{}}"#).unwrap();
        assert_eq!(FailureTemplates::read(&path).unwrap(), None);
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(FailureTemplates::read(&path).unwrap(), None);
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn fill_keeps_the_json_type_of_the_id() {
        let template = json!({"jsonrpc": "2.0", "id": null, "result": {}});
        assert_eq!(fill(&template, &json!(7))["id"], json!(7));
        assert_eq!(fill(&template, &json!("7"))["id"], json!("7"));
        assert_eq!(fill(&template, &json!(null))["id"], json!(null));
    }

    #[test]
    fn fill_leaves_the_rest_of_the_template_alone() {
        let template = json!({"jsonrpc": "2.0", "id": null, "result": {"isError": true}});
        let filled = fill(&template, &json!(1));
        assert_eq!(filled["result"], json!({"isError": true}));
        assert_eq!(filled["jsonrpc"], json!("2.0"));
    }

    #[test]
    fn a_non_object_template_fills_as_the_bare_error() {
        assert_eq!(fill(&json!("text"), &json!(1)), bare_error(&json!(1)));
    }

    #[test]
    fn the_bare_error_is_an_internal_error_with_an_empty_message() {
        assert_eq!(
            bare_error(&json!(3)),
            json!({"jsonrpc": "2.0", "id": 3, "error": {"code": -32603, "message": ""}})
        );
    }

    #[test]
    fn a_method_template_wins_over_the_default() {
        let templates = sample();
        let response = failure_response(Some(&templates), Failure::CutOff, "tools/call", &json!(5));
        assert_eq!(response["result"], json!({"isError": true}));
        assert_eq!(response["id"], json!(5));
    }

    #[test]
    fn an_unnamed_method_uses_the_default_template() {
        let templates = sample();
        let response = failure_response(Some(&templates), Failure::Expired, "ping", &json!(5));
        assert_eq!(response["error"]["message"], json!("late"));
    }

    #[test]
    fn no_templates_means_the_bare_error() {
        let response = failure_response(None, Failure::CutOff, "tools/call", &json!(5));
        assert_eq!(response, bare_error(&json!(5)));
        let empty = FailureTemplates {
            relay: RELAY,
            cut_off: BTreeMap::new(),
            expired: BTreeMap::new(),
        };
        let response = failure_response(Some(&empty), Failure::CutOff, "tools/call", &json!(5));
        assert_eq!(response, bare_error(&json!(5)));
    }
}
