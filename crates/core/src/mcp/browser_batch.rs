// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `browser_batch` tool (ADR-0050 Decision 3): the TRAINED batch front door. It takes
//! `actions: [{name, input}]`, runs them through the SAME shared sequential engine `script` uses
//! ([`crate::mcp::script::run_batch`]), and returns the result in browser_batch's own trained shape
//! -- each action's content blocks flattened in order, with images preserved.
//!
//! `browser_batch` is byte-faithful to the official v1.0.80 schema: it has no `$prev`/`onError`/
//! `dry_run`/`budget_ms` (those stay `script`'s richer surface) and no `_batch_id` side channel (it
//! never propagates a batch id to its parent audit record; that machinery is script's). Its own
//! per-step audit records are attributed to the `"browser_batch"` orchestrator (not `"script"`), so
//! the audit trail names the front door the model actually called.

use crate::mcp::outcome::{CallOutcome, LocalCtx, LocalFuture};
use crate::mcp::script::{run_batch, BatchRun, PipelineRunner};
use serde_json::{json, Value};

/// Translate browser_batch's `{actions: [{name, input}]}` (plus an optional top-level `tabId`) into
/// the script-shaped `{steps: [{tool, args}], tabId?}` the shared engine consumes. Each action's
/// `name` becomes the step `tool` and its `input` the step `args`; an absent `input` becomes `{}`.
fn translate_actions(args: &Value) -> Value {
    let steps: Vec<Value> = args
        .get("actions")
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .map(|a| {
                    json!({
                        "tool": a.get("name").cloned().unwrap_or(Value::Null),
                        "args": a.get("input").cloned().unwrap_or_else(|| json!({})),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let mut translated = json!({ "steps": steps });
    if let Some(tid) = args.get("tabId") {
        translated["tabId"] = tid.clone();
    }
    translated
}

/// Build browser_batch's ONE MCP result from a [`BatchRun`]: the `content` array is, IN ORDER, each
/// executed step's own content (for `ok` steps, its blocks verbatim so text AND images survive; for a
/// non-ok step, a single note naming the step/tool/status), followed by the run summary. `not_run`
/// steps contribute nothing (the summary reports them).
fn build_batch_result(run: BatchRun) -> Value {
    let mut content: Vec<Value> = Vec::new();
    for outcome in &run.steps {
        match outcome.status {
            "not_run" => {}
            "ok" => {
                if let Some(blocks) = outcome.result.get("content").and_then(Value::as_array) {
                    content.extend(blocks.iter().cloned());
                }
            }
            status => {
                let first_text = outcome
                    .result
                    .get("content")
                    .and_then(Value::as_array)
                    .and_then(|b| b.first())
                    .and_then(|b| b.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                content.push(json!({
                    "type": "text",
                    "text": format!("step {} ({}) {status}: {first_text}", outcome.step, outcome.tool),
                }));
            }
        }
    }
    content.push(json!({ "type": "text", "text": run.summary }));
    json!({ "content": content })
}

/// The `browser_batch` `Handler::Local` entry point (ADR-0050 D3): translate `actions` into the
/// shared engine's step shape, run them through [`run_batch`] (attributed to the `"browser_batch"`
/// orchestrator), and flatten the outcomes into browser_batch's trained result. An absent/empty
/// `actions` array returns a single-text guidance result rather than dispatching or panicking.
pub(crate) fn browser_batch_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move {
        let has_actions = ctx
            .args
            .get("actions")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if !has_actions {
            return CallOutcome::Success {
                result: crate::mcp::types::text_content(
                    "browser_batch requires a non-empty `actions` array".to_string(),
                ),
            };
        }

        let translated = translate_actions(ctx.args);
        let mut runner = PipelineRunner {
            browser: ctx.browser,
            store: ctx.store,
            governance: ctx.governance,
            guid: ctx.guid,
            overlay: ctx.overlay,
        };
        // browser_batch is byte-faithful to its trained schema: no dry_run, no budget_ms override.
        let run = run_batch(
            &translated,
            &mut runner,
            ctx.config.script_budget_ms(),
            false,
            "browser_batch",
        );
        CallOutcome::Success {
            result: build_batch_result(run),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::outcome::CallOutcome;
    use crate::mcp::script::{run_batch, StepRunner};

    /// A minimal stub runner (mirrors script.rs's own test seam): records each dispatched (tool,
    /// args) and replays a fixed outcome sequence, defaulting to a plain ok past the end.
    struct StubRunner {
        outcomes: Vec<CallOutcome>,
        calls: Vec<(String, Value)>,
    }
    impl StubRunner {
        fn new(outcomes: Vec<CallOutcome>) -> Self {
            StubRunner {
                outcomes,
                calls: Vec::new(),
            }
        }
    }
    impl StepRunner for StubRunner {
        fn run(
            &mut self,
            name: &str,
            args: &Value,
            _orchestration: Option<(&'static str, &str, u32)>,
            _dry_run: bool,
        ) -> CallOutcome {
            let i = self.calls.len();
            self.calls.push((name.to_string(), args.clone()));
            if i < self.outcomes.len() {
                clone_outcome(&self.outcomes[i])
            } else {
                CallOutcome::Success {
                    result: json!({"content": [{"type":"text","text":"ok"}]}),
                }
            }
        }
    }
    fn clone_outcome(o: &CallOutcome) -> CallOutcome {
        match o {
            CallOutcome::Success { result } => CallOutcome::Success {
                result: result.clone(),
            },
            CallOutcome::Failure { error } => CallOutcome::Failure {
                error: crate::ToolError::invalid_request(error.to_string()),
            },
            CallOutcome::Denied { message, source } => CallOutcome::Denied {
                message: message.clone(),
                source: match source {
                    crate::mcp::outcome::DenialSource::Policy => {
                        crate::mcp::outcome::DenialSource::Policy
                    }
                    crate::mcp::outcome::DenialSource::Sacred => {
                        crate::mcp::outcome::DenialSource::Sacred
                    }
                },
            },
            CallOutcome::Held { message } => CallOutcome::Held {
                message: message.clone(),
            },
        }
    }
    fn ok(text: &str) -> CallOutcome {
        CallOutcome::Success {
            result: json!({"content": [{"type":"text","text": text}]}),
        }
    }
    fn denied() -> CallOutcome {
        CallOutcome::Denied {
            message: "denied".to_string(),
            source: crate::mcp::outcome::DenialSource::Policy,
        }
    }

    #[test]
    fn browser_batch_translates_actions_to_steps() {
        let args = json!({"actions": [
            {"name": "find", "input": {"query": "x"}},
            {"name": "navigate", "input": {"url": "u"}},
        ]});
        let translated = translate_actions(&args);
        let mut runner = StubRunner::new(vec![ok("a"), ok("b")]);
        let _ = run_batch(&translated, &mut runner, 120000, false, "browser_batch");
        assert_eq!(runner.calls[0].0, "find");
        assert_eq!(runner.calls[0].1, json!({"query": "x"}));
        assert_eq!(runner.calls[1].0, "navigate");
        assert_eq!(runner.calls[1].1, json!({"url": "u"}));
    }

    #[test]
    fn browser_batch_result_flattens_content_in_order() {
        let translated = json!({"steps": [
            {"tool": "a", "args": {}},
            {"tool": "b", "args": {}},
        ]});
        let mut runner = StubRunner::new(vec![ok("a"), ok("b")]);
        let run = run_batch(&translated, &mut runner, 120000, false, "browser_batch");
        let result = build_batch_result(run);
        assert_eq!(
            result["content"],
            json!([
                {"type": "text", "text": "a"},
                {"type": "text", "text": "b"},
                {"type": "text", "text": "2/2 steps completed"},
            ])
        );
    }

    #[test]
    fn browser_batch_preserves_image_blocks() {
        let image_block = json!({"type": "image", "source": {"type": "base64", "data": "AAAA"}});
        let step_result = json!({"content": [image_block.clone()]});
        let translated = json!({"steps": [{"tool": "computer", "args": {}}]});
        let mut runner = StubRunner::new(vec![CallOutcome::Success {
            result: step_result,
        }]);
        let run = run_batch(&translated, &mut runner, 120000, false, "browser_batch");
        let result = build_batch_result(run);
        let content = result["content"].as_array().expect("content array");
        assert_eq!(
            content[0], image_block,
            "the image block is preserved verbatim in the flattened content"
        );
    }

    #[test]
    fn browser_batch_stops_on_first_error_and_notes_it() {
        let translated = json!({"steps": [
            {"tool": "a", "args": {}},
            {"tool": "b", "args": {}},
            {"tool": "c", "args": {}},
        ]});
        let mut runner = StubRunner::new(vec![ok("a"), denied()]);
        let run = run_batch(&translated, &mut runner, 120000, false, "browser_batch");
        assert_eq!(
            run.steps[2].status, "not_run",
            "the third step never ran after the second was denied"
        );
        assert_eq!(run.summary, "1/3 steps completed; step 2 denied");
    }

    #[test]
    fn a_batch_tool_step_is_rejected() {
        for batch_tool in ["browser_batch", "script"] {
            let translated = json!({"steps": [{"tool": batch_tool, "args": {}}]});
            let mut runner = StubRunner::new(vec![]);
            let run = run_batch(&translated, &mut runner, 120000, false, "browser_batch");
            assert_eq!(
                run.steps[0].status, "error",
                "a {batch_tool} step is rejected as an error"
            );
            assert!(
                runner.calls.is_empty(),
                "a {batch_tool} step is never dispatched to the runner"
            );
        }
    }
}
