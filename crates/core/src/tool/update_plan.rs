// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The informational `update_plan` compatibility handler.
//!
//! Planning does not grant authority. Keeping this response in the service makes that invariant
//! explicit and avoids sending a no-op compatibility call to the browser shore.

use crate::tool::outcome::{CallOutcome, LocalCtx, LocalFuture};
use serde_json::Value;

/// Echo the caller's browser plan without changing permissions or contacting the extension.
pub(crate) fn update_plan_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move {
        CallOutcome::Success {
            result: crate::tool::result::text_content(render(ctx.args)),
        }
    })
}

fn render(args: &Value) -> String {
    let domains = args
        .get("domains")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let approach = args
        .get("approach")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|step| format!("- {step}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!("Plan (informational; permissions unchanged):\nDomains: {domains}\n{approach}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plan_is_informational_and_does_not_claim_approval() {
        let text = render(&json!({
            "domains": ["example.com", "docs.example.com"],
            "approach": ["read the page", "report the heading"]
        }));

        assert_eq!(
            text,
            "Plan (informational; permissions unchanged):\n\
             Domains: example.com, docs.example.com\n\
             - read the page\n\
             - report the heading"
        );
        assert!(!text.contains("approved"));
    }
}
