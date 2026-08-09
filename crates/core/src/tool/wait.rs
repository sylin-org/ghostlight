// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight wait semantics above the policy-free browser mechanism.
//!
//! A conclusive condition timeout is a normal `not_met` observation. Transport failures and
//! uncertain delivery remain failures; the model never needs a hidden semantic flag.

use crate::browser::mechanism::{MechanismId, MechanismRequest};
use crate::tool::outcome::{
    delivery_failure_outcome, ExecutionOutcome as CallOutcome, LocalCtx, LocalFuture,
};
use crate::ToolError;
use serde_json::{json, Map, Value};

const RESULT_MARKER_POINTER: &str = "/structuredContent/wait/met";

/// Canonical registry entry point for one condition or settlement wait.
pub(crate) fn wait_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(run(ctx))
}

async fn run(ctx: LocalCtx<'_>) -> CallOutcome {
    dispatch_wait(ctx.browser, ctx.guid, ctx.input, ctx.execution).await
}

async fn dispatch_wait(
    browser: &crate::hub::outbound::browser::Browser,
    guid: &str,
    input: &Value,
    execution: &crate::hub::scheduling::ExecutionContext,
) -> CallOutcome {
    let Some(input) = input.as_object().cloned() else {
        return CallOutcome::Failure {
            error: ToolError::invalid_request("browser.wait arguments must be an object"),
        };
    };
    let request = match MechanismRequest::for_operation(
        ghostlight_transport::operation::OperationKind::BrowserWaitFor,
        MechanismId::WaitUntil,
        Value::Object(input.clone()),
    ) {
        Ok(request) => request,
        Err(error) => return CallOutcome::Failure { error },
    };
    match browser
        .execute_mechanism_with_delivery_outcome(guid, &request, execution)
        .await
    {
        Ok(mut result) => {
            clear_adapter_not_met_marker(&mut result);
            CallOutcome::Success {
                result: result.into(),
            }
        }
        Err(failure) => {
            if !failure.outcome_unknown {
                if let Some(message) = conclusive_wait_timeout(&failure.error, &input) {
                    return CallOutcome::Success {
                        result: json!({
                            "content": [{"type":"text", "text":message}],
                            "structuredContent": {"wait":{"met":false}}
                        })
                        .into(),
                    };
                }
            }
            delivery_failure_outcome(failure)
        }
    }
}

fn clear_adapter_not_met_marker(result: &mut Value) {
    let Some(structured) = result
        .get_mut("structuredContent")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let remove_wait = structured
        .get_mut("wait")
        .and_then(Value::as_object_mut)
        .is_some_and(|wait| {
            wait.remove("met");
            wait.is_empty()
        });
    if remove_wait {
        structured.remove("wait");
    }
}

fn conclusive_wait_timeout<'a>(
    error: &'a ToolError,
    input: &Map<String, Value>,
) -> Option<&'a str> {
    let ToolError::Page { message, .. } = error else {
        return None;
    };
    let timeout_ms = input.get("timeout_ms")?.as_u64()?;
    let selector = input.get("selector").and_then(Value::as_str);
    let text = input.get("text").and_then(Value::as_str);
    match (selector, text) {
        (Some(predicate), None) | (None, Some(predicate)) => {
            let prefix =
                format!("\"{predicate}\" not visible within {timeout_ms}ms. Page title: \"");
            (message.starts_with(&prefix) && message.ends_with("\".")).then_some(message.as_str())
        }
        (None, None) => {
            let prefix = format!("did not settle within {timeout_ms}ms (still changing at ~");
            (message.starts_with(&prefix) && message.ends_with(" mutations/500ms)"))
                .then_some(message.as_str())
        }
        (Some(_), Some(_)) => None,
    }
}

/// Whether a successful local wait result proves the requested predicate was not met.
pub(crate) fn result_is_not_met(result: &Value) -> bool {
    result
        .pointer(RESULT_MARKER_POINTER)
        .and_then(Value::as_bool)
        == Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_recognition_is_page_attributed_and_argument_exact() {
        let settle = Map::from_iter([
            ("tab".into(), json!(7)),
            ("state".into(), json!("settled")),
            ("timeout_ms".into(), json!(750)),
        ]);
        assert_eq!(
            conclusive_wait_timeout(
                &ToolError::page(
                    "did not settle within 750ms (still changing at ~12 mutations/500ms)"
                ),
                &settle,
            ),
            Some("did not settle within 750ms (still changing at ~12 mutations/500ms)")
        );
        assert!(conclusive_wait_timeout(
            &ToolError::extension(
                "did not settle within 750ms (still changing at ~12 mutations/500ms)"
            ),
            &settle,
        )
        .is_none());
        assert!(conclusive_wait_timeout(
            &ToolError::page("did not settle within 751ms (still changing at ~12 mutations/500ms)"),
            &settle,
        )
        .is_none());

        let condition = Map::from_iter([
            ("tab".into(), json!(7)),
            ("selector".into(), json!("#ready")),
            ("timeout_ms".into(), json!(1_000)),
        ]);
        assert!(conclusive_wait_timeout(
            &ToolError::page("\"#ready\" not visible within 1000ms. Page title: \"Example\"."),
            &condition,
        )
        .is_some());
        assert!(conclusive_wait_timeout(
            &ToolError::page("Element #ready was not visible"),
            &condition,
        )
        .is_none());
    }

    #[test]
    fn only_the_service_owned_result_marker_is_not_met() {
        assert!(result_is_not_met(&json!({
            "structuredContent":{"wait":{"met":false}}
        })));
        for result in [
            json!({}),
            json!({"structuredContent":{"wait":{"met":true}}}),
            json!({"structuredContent":{"wait":{"met":"false"}}}),
            json!({"wait":{"met":false}}),
        ] {
            assert!(!result_is_not_met(&result));
        }

        let mut adapter_result = json!({
            "structuredContent":{"wait":{"met":false,"elapsed_ms":10}}
        });
        clear_adapter_not_met_marker(&mut adapter_result);
        assert!(!result_is_not_met(&adapter_result));
        assert_eq!(
            adapter_result,
            json!({"structuredContent":{"wait":{"elapsed_ms":10}}})
        );
    }
}
