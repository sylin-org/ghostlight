//! Failure templates: the sentences an MCP edge may say when it cannot reach the service.
//!
//! The edge authors no words. The service hands it these in every accepted [`Reply`]: for each
//! kind of failure, a JSON-RPC response object per request method, plus a `default`. When a
//! request is cut off, expires or is too large to forward, the edge fills the matching template
//! with the request's id and sends it. With no reply at all, which only happens before the first
//! connection, or no template for the failure, as from a service older than revision 5 for
//! `too_large`, the edge answers a bare internal error with an empty message.
//!
//! [`Reply`]: super::Reply

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Key of the template used when none is named for a request's method.
pub const DEFAULT_TEMPLATE: &str = "default";

/// JSON-RPC internal error code, used when there is no template.
const INTERNAL_ERROR_CODE: i64 = -32603;

/// Which failure the edge is reporting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// The service went away while the request was outstanding.
    CutOff,
    /// The request was held for a returning service until the hold limit passed.
    Expired,
    /// The request came in a line, or would go in a frame, over [`MAX_FRAME_BYTES`], so it was
    /// never sent (revision 5).
    ///
    /// [`MAX_FRAME_BYTES`]: crate::MAX_FRAME_BYTES
    TooLarge,
}

/// The failure sentences carried by an accepted reply.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Templates {
    /// Responses for requests cut off by the service going away, by request method.
    #[serde(default)]
    pub cut_off: BTreeMap<String, Value>,
    /// Responses for held requests that expired, by request method.
    #[serde(default)]
    pub expired: BTreeMap<String, Value>,
    /// Responses for requests too large to forward, by request method (revision 5). Left out of
    /// the reply when empty, so a reply without it is revision 4's byte for byte.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub too_large: BTreeMap<String, Value>,
}

impl Templates {
    /// The template for `method` under `failure`, falling back to the `default` entry. `None`
    /// when the reply carries neither, as an empty map does.
    #[must_use]
    pub fn template(&self, failure: Failure, method: &str) -> Option<&Value> {
        let templates = match failure {
            Failure::CutOff => &self.cut_off,
            Failure::Expired => &self.expired,
            Failure::TooLarge => &self.too_large,
        };
        templates
            .get(method)
            .or_else(|| templates.get(DEFAULT_TEMPLATE))
    }
}

/// Fill a template with a request id. The id keeps its JSON type: a number stays a number.
///
/// A template that is not a JSON object cannot carry an id, so it yields the bare error instead.
fn fill(template: &Value, id: &Value) -> Value {
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
fn bare_error(id: &Value) -> Value {
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
    templates: Option<&Templates>,
    failure: Failure,
    method: &str,
    id: &Value,
) -> Value {
    match templates.and_then(|templates| templates.template(failure, method)) {
        Some(template) => fill(template, id),
        None => bare_error(id),
    }
}
