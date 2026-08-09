// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Typed parsing and canonical readiness facts for one navigation transaction.
//!
//! The extension owns only policy-free committed-document observation. The operation pipeline
//! consumes these bounded facts, authorizes every committed landing, and keeps the original
//! dispatch-to-readiness deadline intact across follow-up requests.

use ghostlight_transport::operation::{
    Readiness, ReadinessSettlement, ReadinessStatus, SettlementStatus,
};
use serde_json::Value;

const MAX_HANDLE_BYTES: usize = 160;

/// One exact policy-free state returned by the browser adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavigationState {
    Committed,
    Ready,
    TimedOut,
    Unavailable,
    LandingUnknown,
    NotRequested,
    Same,
}

/// Bounded evidence for one navigation transaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NavigationEvidence {
    pub state: NavigationState,
    pub navigation_token: String,
    pub document_handle: Option<String>,
    pub url: Option<String>,
    pub deadline_at_ms: u64,
    pub elapsed_ms: u64,
}

/// Canonical readiness policy after operation defaults are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NavigationReadinessPolicy {
    pub settle: bool,
    pub timeout_ms: u64,
    pub min_ms: u64,
}

impl NavigationReadinessPolicy {
    pub(crate) fn from_arguments(arguments: &Value) -> Self {
        let readiness = arguments.get("readiness");
        Self {
            settle: readiness
                .and_then(|value| value.get("settle"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            timeout_ms: readiness
                .and_then(|value| value.get("timeout_ms"))
                .and_then(Value::as_u64)
                .unwrap_or(10_000),
            min_ms: readiness
                .and_then(|value| value.get("min_ms"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
        }
    }
}

/// Remove and parse the adapter-owned navigation evidence from an internal success value.
///
/// `Ok(None)` is the covered adapter-without-readiness path. The remaining browser evidence keeps
/// its ordinary content and structured fields for the operation reducer.
pub(crate) fn take_navigation_evidence(
    result: &mut Value,
) -> Result<Option<NavigationEvidence>, &'static str> {
    let Some(structured) = result
        .get_mut("structuredContent")
        .and_then(Value::as_object_mut)
    else {
        return Ok(None);
    };
    let Some(value) = structured.remove("navigation") else {
        return Ok(None);
    };
    parse_navigation_evidence(&value).map(Some)
}

fn parse_navigation_evidence(value: &Value) -> Result<NavigationEvidence, &'static str> {
    let object = value
        .as_object()
        .ok_or("navigation evidence must be an object")?;
    if object.keys().any(|field| {
        !matches!(
            field.as_str(),
            "state"
                | "navigation_token"
                | "document_handle"
                | "url"
                | "deadline_at_ms"
                | "elapsed_ms"
        )
    }) {
        return Err("navigation evidence contains an unsupported field");
    }
    let state = match object.get("state").and_then(Value::as_str) {
        Some("committed") => NavigationState::Committed,
        Some("ready") => NavigationState::Ready,
        Some("timed_out") => NavigationState::TimedOut,
        Some("unavailable") => NavigationState::Unavailable,
        Some("landing_unknown") => NavigationState::LandingUnknown,
        Some("not_requested") => NavigationState::NotRequested,
        Some("same") => NavigationState::Same,
        _ => return Err("navigation evidence has an unknown state"),
    };
    let navigation_token = bounded_handle(object.get("navigation_token"), "n_")
        .ok_or("navigation evidence has an invalid navigation_token")?;
    let document_handle = match object.get("document_handle") {
        None => None,
        value => Some(
            bounded_handle(value, "d_")
                .ok_or("navigation evidence has an invalid document_handle")?,
        ),
    };
    let url = match object.get("url") {
        None => None,
        Some(Value::String(url))
            if !url.is_empty() && url.len() <= 4096 && !url.chars().any(char::is_control) =>
        {
            Some(url.clone())
        }
        Some(_) => return Err("navigation evidence has an invalid url"),
    };
    let deadline_at_ms = object
        .get("deadline_at_ms")
        .and_then(Value::as_u64)
        .ok_or("navigation evidence requires deadline_at_ms")?;
    let elapsed_ms = object
        .get("elapsed_ms")
        .and_then(Value::as_u64)
        .ok_or("navigation evidence requires elapsed_ms")?;
    if matches!(
        state,
        NavigationState::Committed
            | NavigationState::Ready
            | NavigationState::NotRequested
            | NavigationState::Same
    ) && (document_handle.is_none() || url.is_none())
    {
        return Err("navigation state requires document_handle and url");
    }
    Ok(NavigationEvidence {
        state,
        navigation_token,
        document_handle,
        url,
        deadline_at_ms,
        elapsed_ms,
    })
}

fn bounded_handle(value: Option<&Value>, prefix: &str) -> Option<String> {
    let value = value?.as_str()?;
    (value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= MAX_HANDLE_BYTES
        && value.is_ascii()
        && !value.chars().any(char::is_control))
    .then(|| value.to_owned())
}

pub(crate) fn canonical_readiness(state: NavigationState, elapsed_ms: u64) -> Option<Readiness> {
    let (status, settlement) = match state {
        NavigationState::Ready | NavigationState::Same => (
            ReadinessStatus::Ready,
            Some(ReadinessSettlement {
                requested: true,
                status: SettlementStatus::Settled,
            }),
        ),
        NavigationState::TimedOut => (
            ReadinessStatus::TimedOut,
            Some(ReadinessSettlement {
                requested: true,
                status: SettlementStatus::NotSettled,
            }),
        ),
        NavigationState::Unavailable => (
            ReadinessStatus::Unavailable,
            Some(ReadinessSettlement {
                requested: true,
                status: SettlementStatus::Unavailable,
            }),
        ),
        NavigationState::NotRequested => (ReadinessStatus::NotRequested, None),
        NavigationState::Committed | NavigationState::LandingUnknown => return None,
    };
    Some(Readiness {
        status,
        condition: None,
        settlement,
        elapsed_ms: Some(elapsed_ms),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evidence_is_removed_without_changing_legacy_fields() {
        let mut result = json!({
            "content": [{"type":"text","text":"Navigated."}],
            "structuredContent": {
                "tabId": 7,
                "url": "https://example.com",
                "navigation": {
                    "state": "committed",
                    "navigation_token": "n_test",
                    "document_handle": "d_test",
                    "url": "https://example.com",
                    "deadline_at_ms": 10000,
                    "elapsed_ms": 12
                }
            }
        });
        let evidence = take_navigation_evidence(&mut result).unwrap().unwrap();
        assert_eq!(evidence.state, NavigationState::Committed);
        assert_eq!(
            result,
            json!({
                "content": [{"type":"text","text":"Navigated."}],
                "structuredContent": {"tabId":7,"url":"https://example.com"}
            })
        );
    }

    #[test]
    fn readiness_axes_are_truthful() {
        let ready = canonical_readiness(NavigationState::Ready, 25).unwrap();
        assert_eq!(ready.status, ReadinessStatus::Ready);
        assert_eq!(ready.settlement.unwrap().status, SettlementStatus::Settled);
        let not_requested = canonical_readiness(NavigationState::NotRequested, 0).unwrap();
        assert_eq!(not_requested.status, ReadinessStatus::NotRequested);
        assert!(not_requested.settlement.is_none());
    }

    #[test]
    fn malformed_or_unbounded_evidence_fails_closed() {
        for navigation in [
            json!({"state":"ready"}),
            json!({
                "state":"ready","navigation_token":"bad","document_handle":"d_1",
                "url":"https://example.com","deadline_at_ms":1,"elapsed_ms":1
            }),
            json!({
                "state":"bogus","navigation_token":"n_1","document_handle":"d_1",
                "url":"https://example.com","deadline_at_ms":1,"elapsed_ms":1
            }),
        ] {
            let mut result = json!({"structuredContent":{"navigation":navigation}});
            assert!(take_navigation_evidence(&mut result).is_err());
        }
    }
}
