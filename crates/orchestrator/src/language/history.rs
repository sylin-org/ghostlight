//! Closed composition receipt identities and readable retained-history explanations.

use crate::governance::evidence::PermissionCheck;
use crate::governance::ReasonCode;
use serde::{Deserialize, Serialize};

/// Maximum child count supported by a composition, including restored history.
pub const COMPOSITION_STEP_LIMIT: usize = 20;

/// Current flow and historical sequence identities; caller labels never supply an identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionKind {
    Flow,
    /// Retained for receipts written before sequence was consolidated into flow.
    Sequence,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn retired_request_and_sequence_receipts_keep_their_original_meaning() {
        let check: PermissionCheck = serde_json::from_value(json!({
            "requirements":["execute"],"host":null,"allowed":false,"observed":false,
            "reason":"host_denied","layers":[],"request_restricted":true,"request_evaluated":true
        }))
        .unwrap();
        assert_eq!(
            permission(&check),
            "Refused: request restrictions refused this work."
        );
        let receipt: StepReceipt = serde_json::from_value(json!({
            "parent":"sequence","position":2,"total":3,"preparation_failed":false
        }))
        .unwrap();
        assert_eq!(receipt.parent.tool(), "browser_sequence");
    }
}

impl CompositionKind {
    /// Return the canonical catalog name.
    #[must_use]
    pub const fn tool(self) -> &'static str {
        match self {
            Self::Flow => "browser_flow",
            Self::Sequence => "browser_sequence",
        }
    }
}

/// Parent invocation plus position identifies one child receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepReceipt {
    pub parent: CompositionKind,
    pub position: usize,
    pub total: usize,
    pub preparation_failed: bool,
}

/// Explain actual permission evidence, without reading today's policy or guessing grants.
#[must_use]
pub fn permission(check: &PermissionCheck) -> String {
    let verdict = if check.observed {
        "Allowed in observe mode; policy would refuse this work"
    } else if check.allowed {
        "Allowed"
    } else {
        "Refused"
    };
    if check.reason != ReasonCode::Permitted && check.layers.is_empty() && !check.request_evaluated
    {
        return format!("{verdict}: {}.", check.reason.as_str().replace('_', " "));
    }
    if check.requirements.is_empty() {
        return format!("{verdict}; no capability grant was required.");
    }
    if check.layers.is_empty() && !check.request_restricted {
        return format!("{verdict} without configured policy.");
    }
    let mut sources: Vec<String> = check
        .layers
        .iter()
        .map(|layer| {
            let source = if layer.tier == "managed" {
                "organization policy"
            } else {
                "local policy"
            };
            let rules = if layer.grants.is_empty() {
                "no matching grant".into()
            } else {
                layer.grants.join(", ")
            };
            let verdict = if layer.allowed {
                "permits this work"
            } else {
                "refuses this work"
            };
            format!("{source} {verdict} ({rules})")
        })
        .collect();
    if check.request_restricted {
        sources.push(
            if !check.request_evaluated {
                "request restrictions were not evaluated"
            } else if check.allowed {
                "request restrictions permit this work"
            } else {
                "request restrictions refused this work"
            }
            .into(),
        );
    }
    format!("{verdict}: {}.", sources.join("; "))
}
