//! The one id a request carries from intake to audit (ADR-0202 Decision 6).
//!
//! The MCP intake mints an [`InvocationId`] for each tool call. The session queue, the domain
//! events, the result the client receives, the audit line and the diagnostics all name that same
//! id, so a client's result joins its audit line without any client-written value becoming
//! durable.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What every invocation id starts with.
const INVOCATION_PREFIX: &str = "invocation_";

/// One request's id: `invocation_` and 32 lowercase hex digits, minted once per request.
///
/// It serializes as its text, so the result envelope and the audit line read it unchanged.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvocationId(String);

impl InvocationId {
    /// A fresh id, unique to the request it is minted for.
    #[must_use]
    pub fn mint() -> Self {
        Self(format!("{INVOCATION_PREFIX}{}", Uuid::new_v4().simple()))
    }

    /// The id's text, as events, results and records carry it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InvocationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&InvocationId> for String {
    fn from(id: &InvocationId) -> Self {
        id.0.clone()
    }
}

/// A record or event that carries the id as text names this id when the text is equal.
impl PartialEq<InvocationId> for String {
    fn eq(&self, other: &InvocationId) -> bool {
        *self == other.0
    }
}

#[cfg(test)]
mod tests {
    use super::{InvocationId, INVOCATION_PREFIX};

    #[test]
    fn an_id_is_the_prefix_and_32_hex_digits_and_each_is_new() {
        let id = InvocationId::mint();
        let digits = id.as_str().strip_prefix(INVOCATION_PREFIX).unwrap();
        assert_eq!(digits.len(), 32);
        assert!(digits.chars().all(|digit| digit.is_ascii_hexdigit()));
        assert_ne!(id, InvocationId::mint());
        assert_eq!(serde_json::to_value(&id).unwrap(), id.as_str());
    }
}
