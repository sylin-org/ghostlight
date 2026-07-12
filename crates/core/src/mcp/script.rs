// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `script` tool's interpreter (ADR-0035, PINS.md SS7): sequential multi-tool composition with
//! reference resolution, honest per-step status, correlated audit, and a wall-clock budget.
//!
//! Each step re-enters [`pipeline::run_tool_call`] through the SAME governance chokepoint every
//! individual tool call enters -- per-step authorization, audit, post-processing, and snapshot.
//! The interpreter owns iteration, reference resolution (SS6), tabId inheritance, no-nesting
//! rejection, budget accounting, status mapping, and the compact-result format. The parent
//! `script` call is itself a free-action `Handler::Local` dispatch; its audit record's `batch_id`
//! is stamped by the free-action arm from this handler's `_batch_id` result side-channel (SS7).
//!
//! Testability seam: the interpreter is generic over a [`StepRunner`] so unit tests can drive
//! fixed outcomes without a live `Browser`; the real handler wires [`pipeline::run_tool_call`] in.

use crate::governance::config::reload::ConfigStore;
use crate::governance::dispatch::Governance;
use crate::hub::outbound::browser::Browser;
use crate::mcp::outcome::{CallOutcome, LocalCtx, LocalFuture};
use crate::mcp::pipeline::run_tool_call;
use crate::mcp::refs::resolve_refs;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One step's outcome as the shared batch engine records it (ADR-0050 D3): the step's FULL MCP
/// result Value (content array + optional `structuredContent`), so a batch front door that wants
/// images (`browser_batch`) can preserve every content block, while `script`'s compact formatter
/// derives its truncated text + structured twin from the same source. `result` is `Value::Null` for
/// a `not_run` step. For a Failure/Denied/Held step it is a synthesized
/// `{"content":[{"type":"text","text": <message>}]}` (the honest per-step message), so the derived
/// text is byte-identical to the previous `step_text`-based record.
pub(crate) struct StepOutcome {
    pub step: u32,
    pub tool: String,
    pub status: &'static str, // "ok" | "error" | "denied" | "held" | "not_run"
    pub result: Value,
}

/// The raw result of running a batch of steps through the shared engine, BEFORE any front-door
/// formatting (ADR-0050 D3). `script` renders this via [`build_compact`]; `browser_batch` renders it
/// via its own flattening formatter that keeps image blocks.
pub(crate) struct BatchRun {
    pub steps: Vec<StepOutcome>,
    pub summary: String,
    pub duration_ms: u64,
    pub batch_id: String,
}

/// The first content-array text block of a step's result Value, or None (a `not_run`/Null result, or
/// a result with no text block). Mirrors the Success branch of [`step_text`] and, for a synthesized
/// error result, returns the message -- so [`build_compact`] derives the same text the old
/// `StepRecord.text` carried.
fn first_result_text(result: &Value) -> Option<String> {
    result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.first())
        .and_then(|b| b.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The full MCP result Value for a step outcome, as [`StepOutcome::result`] stores it: a Success
/// keeps its whole result (content + structuredContent, so images survive); a Failure/Denied/Held
/// becomes a synthesized single-text-block result carrying its honest message.
fn outcome_result(outcome: &CallOutcome) -> Value {
    match outcome {
        CallOutcome::Success { result } => result.clone(),
        _ => json!({
            "content": [ { "type": "text", "text": step_text(outcome).unwrap_or_default() } ]
        }),
    }
}

/// A synthesized single-text-block result for an engine-level step error (nesting rejection, a
/// reference-resolution failure) that never reached a runner outcome.
fn error_result(message: &str) -> Value {
    json!({ "content": [ { "type": "text", "text": message } ] })
}

/// The testability seam: run one resolved step through the governance chokepoint and return its
/// outcome. `dry_run` forwards to `run_tool_call` so a dry-run step gets the real verdict without
/// dispatch. The production impl re-enters `run_tool_call`; tests supply fixed outcomes.
pub(crate) trait StepRunner {
    fn run(
        &mut self,
        name: &str,
        args: &Value,
        orchestration: Option<(&'static str, &str, u32)>,
        dry_run: bool,
    ) -> CallOutcome;
}

/// The production step runner: re-enters the pipeline chokepoint for one step. `pub(crate)` (with
/// `pub(crate)` fields) so the `browser_batch` front door can wire one from its `LocalCtx` and drive
/// the SAME shared engine (ADR-0050 D3), exactly as `script_handler` does.
pub(crate) struct PipelineRunner<'a> {
    pub(crate) browser: &'a Browser,
    pub(crate) store: &'a Arc<ConfigStore>,
    pub(crate) governance: &'a Governance,
    pub(crate) guid: &'a str,
    /// ADR-0060: the session's tighten-only overlay, threaded from the `LocalCtx` so every
    /// orchestrated sub-step is bound by the SAME session tier its parent call was.
    pub(crate) overlay: Option<&'a crate::governance::overlay::SessionOverlay>,
}

impl<'a> StepRunner for PipelineRunner<'a> {
    fn run(
        &mut self,
        name: &str,
        args: &Value,
        orchestration: Option<(&'static str, &str, u32)>,
        dry_run: bool,
    ) -> CallOutcome {
        // Safety: the future returned by run_tool_call is awaited synchronously here via the
        // interpreter's tokio handle. The interpreter itself runs inside a LocalFuture (boxed),
        // so this re-entry is the async recursion ADR-0035 Decision 6 prices (pipeline -> local
        // handler -> pipeline).
        futures_await_block(
            name,
            args,
            orchestration,
            dry_run,
            self.browser,
            self.store,
            self.governance,
            self.guid,
            self.overlay,
        )
    }
}

// A step's text content: the first text block of its MCP result, or a synthesized note.
fn step_text(outcome: &CallOutcome) -> Option<String> {
    let result = match outcome {
        CallOutcome::Success { result } => result,
        CallOutcome::Failure { error } => {
            return Some(error.to_string());
        }
        CallOutcome::Denied { message, .. } => return Some(message.clone()),
        CallOutcome::Held { message } => return Some(message.clone()),
    };
    result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.first())
        .and_then(|b| b.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The structured result carried on a step's Success outcome (ADR-0038 `structuredContent`), if any.
fn step_structured(outcome: &CallOutcome) -> Option<Value> {
    if let CallOutcome::Success { result } = outcome {
        result.get("structuredContent").cloned()
    } else {
        None
    }
}

/// Map a step's outcome to its honest status string (the load-bearing fix: a denial or hold is a
/// successful MCP text result on the wire, so envelope sniffing would lie; the structured outcome
/// is the only honest source). Under `dry_run`, an allowed step reports `would_allow` and a denied
/// one `would_deny` (ADR-0035 Decision 8): the verdict, not the execution.
fn status_of(outcome: &CallOutcome, dry_run: bool) -> &'static str {
    match outcome {
        CallOutcome::Success { .. } => {
            if dry_run {
                "would_allow"
            } else {
                "ok"
            }
        }
        CallOutcome::Failure { .. } => "error",
        CallOutcome::Denied { .. } => {
            if dry_run {
                "would_deny"
            } else {
                "denied"
            }
        }
        CallOutcome::Held { .. } => "held",
    }
}

const STEP_TEXT_BUDGET: usize = 2000;
const COMPACT_BUDGET: usize = 25000;

/// Drive the shared batch engine over `args.steps` with `runner`, returning the raw [`BatchRun`]
/// (per-step full results + summary + duration + batch_id) BEFORE any front-door formatting
/// (ADR-0050 D3). `script` renders it via [`build_compact`]; `browser_batch` via its own flattening
/// formatter. Pure over the runner: the production handlers wire `run_tool_call`, tests wire a stub.
pub(crate) fn run_batch<R: StepRunner>(
    args: &Value,
    runner: &mut R,
    config_budget_ms: u64,
    dry_run: bool,
    orchestrator: &'static str,
) -> BatchRun {
    let started = Instant::now();
    let tab_id = args.get("tabId").cloned();
    let on_error = args
        .get("onError")
        .and_then(Value::as_str)
        .unwrap_or("stop");
    let on_continue = on_error == "continue";

    // Budget: the configured ceiling, lowered (never raised) by the call's own budget_ms.
    let mut budget_ms = config_budget_ms;
    if let Some(arg) = args.get("budget_ms").and_then(Value::as_u64) {
        if arg < budget_ms {
            budget_ms = arg;
        }
    }
    let deadline = started + Duration::from_millis(budget_ms);

    let Some(steps) = args.get("steps").and_then(Value::as_array) else {
        return error_batch("script requires a 'steps' array");
    };
    let total = steps.len() as u32;
    if total == 0 {
        return error_batch("script requires at least one step");
    }

    let mut records: Vec<StepOutcome> = Vec::with_capacity(steps.len());
    // The structured results so far, indexed by step (1-indexed via `structured[i-1]`). None when a
    // step failed, was skipped, or its tool declares no vocabulary -- exactly what resolve_refs reads.
    let mut structured: Vec<Option<Value>> = Vec::with_capacity(steps.len());

    let mut batch_id = uuid::Uuid::new_v4().to_string();
    let _ = &mut batch_id; // referenced by the parent record below

    let mut stopped_at: Option<u32> = None; // the step that halted the chain (1-indexed)
    let mut stop_reason: StopReason = StopReason::None;

    for (i, step) in steps.iter().enumerate() {
        let step_no = (i + 1) as u32;

        // Budget gate: once the deadline has passed, remaining steps do not run. The first step
        // always begins -- it is the call's primary work, and a 0-budget call means "stop after the
        // first step completes", not "do nothing".
        if step_no > 1 && Instant::now() > deadline {
            for _ in i..steps.len() {
                let tool = steps
                    .get(records.len())
                    .and_then(|s| s.get("tool"))
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string();
                records.push(StepOutcome {
                    step: (records.len() + 1) as u32,
                    tool,
                    status: "not_run",
                    result: Value::Null,
                });
                structured.push(None);
            }
            stopped_at = Some(step_no.saturating_sub(1).max(1));
            stop_reason = StopReason::Budget;
            break;
        }

        let tool = step
            .get("tool")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        // No nesting (ADR-0050 D3, symmetric): neither batch front door may appear as a step of
        // either batcher -- a `script` step or a `browser_batch` step is rejected before dispatch.
        if tool == "script" || tool == "browser_batch" {
            let message = if tool == "script" {
                "script steps may not include script itself"
            } else {
                "browser_batch steps may not include a batch tool"
            };
            records.push(StepOutcome {
                step: step_no,
                tool,
                status: "error",
                result: error_result(message),
            });
            structured.push(None);
            stopped_at = Some(step_no);
            stop_reason = StopReason::Failed;
            break;
        }

        let mut step_args = step.get("args").cloned().unwrap_or(json!({}));

        // Inherit the script-level tabId when the step omits its own.
        if let Some(tid) = &tab_id {
            if step_args.get("tabId").is_none() {
                step_args["tabId"] = tid.clone();
            }
        }

        // Resolve $prev / $N references against prior structured results (SS6). A resolution error
        // fails this step before any dispatch, with a corrective message; it respects onError.
        let resolved = resolve_refs(&step_args, &structured);
        let step_args = match resolved {
            Ok(a) => a,
            Err(msg) => {
                records.push(StepOutcome {
                    step: step_no,
                    tool,
                    status: "error",
                    result: error_result(&msg),
                });
                structured.push(None);
                if dry_run || on_continue {
                    continue;
                }
                stopped_at = Some(step_no);
                stop_reason = StopReason::Failed;
                break;
            }
        };

        let outcome = runner.run(
            &tool,
            &step_args,
            Some((orchestrator, &batch_id, step_no)),
            dry_run,
        );
        let status = status_of(&outcome, dry_run);

        // A held step stops the script UNCONDITIONALLY, regardless of onError: the user grabbed the
        // wheel; burning through more steps that each answer "held" would be technically correct and
        // humanly wrong.
        if status == "held" {
            records.push(StepOutcome {
                step: step_no,
                tool,
                status,
                result: outcome_result(&outcome),
            });
            structured.push(None);
            stopped_at = Some(step_no);
            stop_reason = StopReason::Held;
            break;
        }

        let structured_for_ref = step_structured(&outcome);
        records.push(StepOutcome {
            step: step_no,
            tool: tool.clone(),
            status,
            result: outcome_result(&outcome),
        });
        structured.push(structured_for_ref);

        // onError "stop": any non-ok step halts the chain. Under dry_run the chain always runs to
        // completion -- the point is to see EVERY step's verdict, not stop at the first denial.
        if !dry_run && status != "ok" && !on_continue {
            stopped_at = Some(step_no);
            stop_reason = StopReason::from_status(status);
            break;
        }
    }

    // Fill remaining not_run slots when the chain stopped before exhausting steps (failure/denied/hold
    // under stop, or a hold under continue). Budget already filled its own tail above.
    if let Some(stop) = stopped_at {
        if (stop as usize) < steps.len()
            && !matches!(stop_reason, StopReason::Budget | StopReason::None)
        {
            for j in (stop as usize)..steps.len() {
                let tool = steps
                    .get(j)
                    .and_then(|s| s.get("tool"))
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string();
                records.push(StepOutcome {
                    step: (j + 1) as u32,
                    tool,
                    status: "not_run",
                    result: Value::Null,
                });
                structured.push(None);
            }
        }
    }

    let completed = records.iter().filter(|r| r.status == "ok").count() as u32;
    let summary = summarize(stop_reason, stopped_at, completed, total);
    let duration_ms = started.elapsed().as_millis() as u64;

    BatchRun {
        steps: records,
        summary,
        duration_ms,
        batch_id,
    }
}

/// The `script` front door: run the shared engine and render its compact result (ADR-0035, SS7).
/// Behavior-preserving wrapper around [`run_batch`] + [`build_compact`].
fn interpret<R: StepRunner>(
    args: &Value,
    runner: &mut R,
    config_budget_ms: u64,
    dry_run: bool,
) -> Value {
    let run = run_batch(args, runner, config_budget_ms, dry_run, "script");
    build_compact(run)
}

#[derive(Clone, Copy)]
enum StopReason {
    None,
    Failed,
    Denied,
    Held,
    Budget,
}

impl StopReason {
    fn from_status(status: &str) -> Self {
        match status {
            "denied" => StopReason::Denied,
            "held" => StopReason::Held,
            _ => StopReason::Failed,
        }
    }
}

/// Render the exact summary string per PINS SS7.
fn summarize(reason: StopReason, stopped_at: Option<u32>, completed: u32, total: u32) -> String {
    match reason {
        StopReason::None => format!("{completed}/{total} steps completed"),
        StopReason::Budget => {
            // "budget exhausted after step {K}" where K is the last step that ran.
            let k = stopped_at.unwrap_or(completed);
            format!("{completed}/{total} steps completed; budget exhausted after step {k}")
        }
        StopReason::Failed => {
            let k = stopped_at.unwrap_or(completed + 1);
            format!("{completed}/{total} steps completed; step {k} failed")
        }
        StopReason::Denied => {
            let k = stopped_at.unwrap_or(completed + 1);
            format!("{completed}/{total} steps completed; step {k} denied")
        }
        StopReason::Held => {
            let k = stopped_at.unwrap_or(completed + 1);
            format!("{completed}/{total} steps completed; held at step {k}")
        }
    }
}

/// Assemble the compact result object: the `results` array (per-step records with truncated text
/// and optional structured twin), the `summary`, the `duration_ms`, and the `_batch_id` side
/// channel the free-action arm strips before rendering. The whole JSON is capped at 25000 chars.
fn build_compact(run: BatchRun) -> Value {
    let results: Vec<Value> = run
        .steps
        .iter()
        .map(|o| {
            let mut entry = json!({
                "step": o.step,
                "tool": o.tool,
                "status": o.status,
            });
            // Derive the compact text from the step's full result (the same first-text-block value
            // the old `StepRecord.text` carried), truncated to the step budget.
            if let Some(t) = first_result_text(&o.result) {
                entry["result"] = json!(truncate_step_text(&t));
            }
            // The structured twin is the step's `structuredContent`, if any (Success steps only).
            if let Some(s) = o.result.get("structuredContent") {
                entry["structured"] = s.clone();
            }
            entry
        })
        .collect();
    let mut compact = json!({
        "results": results,
        "summary": run.summary,
        "duration_ms": run.duration_ms,
        "_batch_id": run.batch_id,
    });
    cap_compact(&mut compact);
    compact
}

/// Truncate a step's text to the 2000-char budget with a `(truncated)` marker.
fn truncate_step_text(t: &str) -> String {
    if t.chars().count() <= STEP_TEXT_BUDGET {
        return t.to_string();
    }
    let head: String = t.chars().take(STEP_TEXT_BUDGET).collect();
    format!("{head}(truncated)")
}

/// Cap the whole compact result at 25000 chars by truncating step texts from the end (preserving the
/// summary and the leading steps). The marker names the cap so a model knows output was dropped.
fn cap_compact(compact: &mut Value) {
    loop {
        let serialized = serde_json::to_string(compact).unwrap_or_default();
        if serialized.len() <= COMPACT_BUDGET {
            return;
        }
        // Shorten the longest remaining step text; if none are left to shorten, drop trailing steps.
        let results = compact
            .get_mut("results")
            .and_then(Value::as_array_mut)
            .expect("compact has a results array");
        let mut longest: Option<(usize, usize)> = None;
        for (i, r) in results.iter().enumerate() {
            if r.get("result").and_then(Value::as_str).is_some() {
                let len = r["result"].as_str().unwrap_or("").len();
                if longest.is_none_or(|(l, _)| len > l) {
                    longest = Some((len, i));
                }
            }
        }
        if let Some((_, i)) = longest {
            let cur = results[i]["result"].as_str().unwrap_or("").to_string();
            if cur.len() > 200 {
                let head: String = cur.chars().take(200).collect();
                results[i]["result"] = json!(format!("{head}...(truncated)"));
                continue;
            }
        }
        // No step text left to shorten: drop the last step entry entirely.
        if results.len() > 1 {
            results.pop();
        } else {
            return;
        }
    }
}

/// A minimal empty [`BatchRun`] for a top-level engine error (malformed/empty steps array); its
/// [`build_compact`] renders the same `{results:[], summary, duration_ms:0, _batch_id}` the old
/// `error_compact` produced.
fn error_batch(msg: &str) -> BatchRun {
    BatchRun {
        steps: Vec::new(),
        summary: msg.to_string(),
        duration_ms: 0,
        batch_id: uuid::Uuid::new_v4().to_string(),
    }
}

/// The `script` tool's `Handler::Local` entry point: runs the interpreter over the pipeline
/// chokepoint per step and returns a `Success` whose result is the compact JSON rendered as text,
/// carrying the SAME object as `structuredContent`. The `_batch_id` side channel is embedded at the
/// result's top level for the free-action arm to strip and stamp onto the parent audit record.
///
/// `dry_run` (ADR-0035 Decision 8): when true, each step is dispatched through `run_tool_call` with
/// `dry_run=true`, which runs the REAL decision path (registry, schema, hold, sacred, authorize)
/// but returns the verdict without dispatching and writes no step audit record. The parent record
/// is marked dry-run via the `_dry_run` side channel.
pub(crate) fn script_handler(ctx: LocalCtx<'_>) -> LocalFuture<'_> {
    Box::pin(async move {
        let dry_run = ctx.args.get("dry_run").and_then(Value::as_bool) == Some(true);
        let mut runner = PipelineRunner {
            browser: ctx.browser,
            store: ctx.store,
            governance: ctx.governance,
            guid: ctx.guid,
            overlay: ctx.overlay,
        };
        let mut compact = interpret(
            ctx.args,
            &mut runner,
            ctx.config.script_budget_ms(),
            dry_run,
        );

        // The `_batch_id` side channel (PINS.md SS7): pulled out of the compact and placed at the
        // result's TOP LEVEL (not inside structuredContent) so the free-action arm's take_batch_id
        // finds and strips it before rendering, stamping the parent audit record. The client-facing
        // compact/structuredContent never carries this key.
        let batch_id = compact
            .get("_batch_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if let Some(obj) = compact.as_object_mut() {
            obj.remove("_batch_id");
        }

        // The compact object IS the structured result; render it as text too (two views of one
        // source object, per ADR-0038 Decision 1).
        let text = serde_json::to_string_pretty(&compact).unwrap_or_default();
        let mut result = crate::mcp::types::text_content(text);
        if let Some(obj) = result.as_object_mut() {
            obj.insert("structuredContent".to_string(), compact);
            // The side channel lives at the top level, where take_batch_id looks for it.
            obj.insert("_batch_id".to_string(), Value::String(batch_id));
            if dry_run {
                // Signals the free-action arm to call audit.mark_dry_run() on the parent record.
                obj.insert("_dry_run".to_string(), Value::Bool(true));
            }
        }
        CallOutcome::Success { result }
    })
}

// Re-enter run_tool_call from within the boxed interpreter future. Tokio's current-thread handle is
// available because the whole pipeline runs on a tokio runtime; this bridges the sync StepRunner
// trait to the async run_tool_call without forcing the interpreter itself to be async-generic.
// ADR-0047 D3 threads the session `guid` through to run_tool_call, pushing this bridge to 8
// params; the arity mirrors run_tool_call's pinned signature, so the lint is allowed here too.
#[allow(clippy::too_many_arguments)]
fn futures_await_block(
    name: &str,
    args: &Value,
    orchestration: Option<(&'static str, &str, u32)>,
    dry_run: bool,
    browser: &Browser,
    store: &Arc<ConfigStore>,
    governance: &Governance,
    guid: &str,
    overlay: Option<&crate::governance::overlay::SessionOverlay>,
) -> CallOutcome {
    tokio::task::block_in_place(|| {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(run_tool_call(
            browser,
            store,
            governance,
            guid,
            name,
            args,
            orchestration,
            dry_run,
            overlay,
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolError;
    use serde_json::json;

    /// One recorded dispatch into the stub runner, for asserting on what the interpreter actually
    /// sent (tool name, resolved args, orchestration stamp).
    struct RecordedCall {
        tool: String,
        args: Value,
        orchestrator: Option<&'static str>,
        batch_id: Option<String>,
        step: Option<u32>,
    }

    /// A stub runner that replays a fixed sequence of outcomes per step and records each call so
    /// tests can assert on what the interpreter actually dispatched.
    struct StubRunner {
        outcomes: Vec<CallOutcome>,
        calls: Vec<RecordedCall>,
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
            orchestration: Option<(&'static str, &str, u32)>,
            _dry_run: bool,
        ) -> CallOutcome {
            let i = self.calls.len();
            self.calls.push(RecordedCall {
                tool: name.to_string(),
                args: args.clone(),
                orchestrator: orchestration.map(|(o, _, _)| o),
                batch_id: orchestration.map(|(_, b, _)| b.to_string()),
                step: orchestration.map(|(_, _, s)| s),
            });
            // Consume one outcome; if exhausted, default to a success so the loop's own stop logic
            // is what's under test.
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
                error: ToolError::invalid_request(error.to_string()),
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
    fn held() -> CallOutcome {
        CallOutcome::Held {
            message: "held".to_string(),
        }
    }
    fn denied() -> CallOutcome {
        CallOutcome::Denied {
            message: "denied".to_string(),
            source: crate::mcp::outcome::DenialSource::Policy,
        }
    }

    fn statuses(compact: &Value) -> Vec<String> {
        compact["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["status"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn hold_stops_unconditionally_even_on_continue() {
        let args = json!({"steps":[
            {"tool":"a","args":{}},
            {"tool":"b","args":{}},
            {"tool":"c","args":{}}
        ], "onError":"continue"});
        let mut runner = StubRunner::new(vec![ok("a"), held(), ok("c")]);
        let compact = interpret(&args, &mut runner, 120000, false);
        assert_eq!(statuses(&compact), vec!["ok", "held", "not_run"]);
        assert_eq!(
            compact["summary"], "1/3 steps completed; held at step 2",
            "got: {}",
            compact["summary"]
        );
    }

    #[test]
    fn denied_step_reports_denied_not_ok() {
        let args = json!({"steps":[{"tool":"a","args":{}},{"tool":"b","args":{}}]});
        let mut runner = StubRunner::new(vec![ok("a"), denied()]);
        let compact = interpret(&args, &mut runner, 120000, false);
        assert_eq!(statuses(&compact), vec!["ok", "denied"]);
        assert_eq!(compact["summary"], "1/2 steps completed; step 2 denied");
    }

    #[test]
    fn budget_exhaustion_marks_not_run() {
        // budget_ms of 0 forces exhaustion after the first step completes.
        let args = json!({"steps":[
            {"tool":"a","args":{}},
            {"tool":"b","args":{}},
            {"tool":"c","args":{}}
        ], "budget_ms":0});
        let mut runner = StubRunner::new(vec![ok("a")]);
        let compact = interpret(&args, &mut runner, 120000, false);
        assert_eq!(statuses(&compact), vec!["ok", "not_run", "not_run"]);
        assert_eq!(
            compact["summary"], "1/3 steps completed; budget exhausted after step 1",
            "got: {}",
            compact["summary"]
        );
    }

    #[test]
    fn nested_script_step_errors() {
        let args = json!({"steps":[{"tool":"script","args":{}}]});
        let mut runner = StubRunner::new(vec![]);
        let compact = interpret(&args, &mut runner, 120000, false);
        assert_eq!(statuses(&compact), vec!["error"]);
        let text = compact["results"][0]["result"].as_str().unwrap_or("");
        assert!(
            text.contains("script steps may not include script itself"),
            "got: {text}"
        );
    }

    #[test]
    fn truncation_applies_at_2000() {
        let big = "x".repeat(3000);
        let args = json!({"steps":[{"tool":"a","args":{}}]});
        let mut runner = StubRunner::new(vec![ok(&big)]);
        let compact = interpret(&args, &mut runner, 120000, false);
        let text = compact["results"][0]["result"].as_str().unwrap_or("");
        assert!(
            text.ends_with("(truncated)"),
            "got tail: {:?}",
            &text[text.len().saturating_sub(20)..]
        );
        assert!(
            text.len() <= 2011,
            "step text length {} must be <= 2011",
            text.len()
        );
    }

    #[test]
    fn all_ok_summary_is_n_of_n() {
        let args = json!({"steps":[{"tool":"a","args":{}},{"tool":"b","args":{}}]});
        let mut runner = StubRunner::new(vec![ok("a"), ok("b")]);
        let compact = interpret(&args, &mut runner, 120000, false);
        assert_eq!(statuses(&compact), vec!["ok", "ok"]);
        assert_eq!(compact["summary"], "2/2 steps completed");
    }

    #[test]
    fn tabid_is_inherited_by_steps_that_omit_it() {
        // The script-level tabId flows into every step whose args omit their own; a step may override.
        let args = json!({"tabId":7,"steps":[
            {"tool":"a","args":{}},
            {"tool":"b","args":{"tabId":99}},
            {"tool":"c","args":{}}
        ]});
        let mut runner = StubRunner::new(vec![ok("a"), ok("b"), ok("c")]);
        let _ = interpret(&args, &mut runner, 120000, false);
        assert_eq!(runner.calls.len(), 3, "all three steps ran");
        assert_eq!(
            runner.calls[0].args["tabId"], 7,
            "step 1 inherits the script tabId"
        );
        assert_eq!(
            runner.calls[1].args["tabId"], 99,
            "step 2 keeps its own override"
        );
        assert_eq!(
            runner.calls[2].args["tabId"], 7,
            "step 3 inherits the script tabId"
        );
    }

    #[test]
    fn references_resolve_through_the_interpreter() {
        // Step 2 references step 1's structured result: $prev.results.0.ref. The interpreter must
        // resolve it against step 1's structuredContent BEFORE dispatching step 2.
        let args = json!({"steps":[
            {"tool":"find","args":{"query":"x"}},
            {"tool":"computer","args":{"action":"left_click","ref":"$prev.results.0.ref"}}
        ]});
        let find_structured = json!({"results":[{"ref":"ref_42","x":1,"y":2}],"more":false});
        let mut runner = StubRunner::new(vec![
            CallOutcome::Success {
                result: json!({"content":[{"type":"text","text":"found"}],"structuredContent": find_structured}),
            },
            ok("clicked"),
        ]);
        let compact = interpret(&args, &mut runner, 120000, false);
        assert_eq!(statuses(&compact), vec!["ok", "ok"]);
        // Step 2 was dispatched with the resolved ref, not the literal "$prev..." string.
        assert_eq!(
            runner.calls[1].args["ref"], "ref_42",
            "step 2's ref resolved from step 1's structured result"
        );
        // Each step carries its orchestration stamp (SS7): orchestrator "script", the shared
        // batch_id, and a 1-indexed step number -- so the per-step audit record is correlatable.
        assert_eq!(runner.calls[0].tool, "find");
        assert_eq!(runner.calls[0].orchestrator, Some("script"));
        assert_eq!(runner.calls[0].step, Some(1));
        assert!(runner.calls[0]
            .batch_id
            .as_ref()
            .is_some_and(|b| !b.is_empty()));
        assert_eq!(runner.calls[1].step, Some(2), "step 2 is numbered 2");
        assert_eq!(
            runner.calls[1].batch_id, runner.calls[0].batch_id,
            "all steps share the parent's batch_id"
        );
        // The structured twin rides along on step 1's record.
        assert_eq!(
            compact["results"][0]["structured"]["results"][0]["ref"], "ref_42",
            "step 1 carries its structured result"
        );
    }

    #[test]
    fn reference_resolution_error_fails_the_step_before_dispatch() {
        // $2.x references step 2, but only one step runs before it -- a forward reference. The step
        // fails with status "error" and the runner is NEVER called for it.
        let args = json!({"steps":[
            {"tool":"a","args":{}},
            {"tool":"b","args":{"ref":"$2.x"}}
        ]});
        let mut runner = StubRunner::new(vec![ok("a")]);
        let compact = interpret(&args, &mut runner, 120000, false);
        assert_eq!(statuses(&compact), vec!["ok", "error"]);
        assert_eq!(
            runner.calls.len(),
            1,
            "step 2 was never dispatched (ref error)"
        );
        let step2_text = compact["results"][1]["result"].as_str().unwrap_or("");
        assert!(
            step2_text.contains("references step 2"),
            "got: {step2_text}"
        );
    }

    #[test]
    fn on_error_continue_runs_remaining_steps_after_a_denial() {
        // Under onError "continue", a denied step does NOT halt: the chain runs to the end.
        let args = json!({"steps":[
            {"tool":"a","args":{}},
            {"tool":"b","args":{}},
            {"tool":"c","args":{}}
        ], "onError":"continue"});
        let mut runner = StubRunner::new(vec![ok("a"), denied(), ok("c")]);
        let compact = interpret(&args, &mut runner, 120000, false);
        assert_eq!(statuses(&compact), vec!["ok", "denied", "ok"]);
        assert_eq!(runner.calls.len(), 3, "continue ran all three steps");
        assert_eq!(
            compact["summary"], "2/3 steps completed",
            "two of three completed; no stop suffix under continue"
        );
    }

    #[test]
    fn budget_arg_may_only_lower_not_raise() {
        // A budget_ms ABOVE the configured ceiling is ignored; the config ceiling governs. With a
        // config budget of 0 and an arg of 999999, the arg cannot raise the effective budget, so the
        // chain still stops after step 1.
        let args = json!({"steps":[
            {"tool":"a","args":{}},
            {"tool":"b","args":{}}
        ], "budget_ms":999999});
        let mut runner = StubRunner::new(vec![ok("a")]);
        let compact = interpret(&args, &mut runner, 0, false);
        assert_eq!(statuses(&compact), vec!["ok", "not_run"]);
        assert_eq!(
            compact["summary"], "1/2 steps completed; budget exhausted after step 1",
            "arg budget above the config ceiling is ignored"
        );
    }

    #[test]
    fn whole_compact_result_is_capped_at_25000_chars() {
        // Two steps each returning a ~20k-char text: the compact result must be capped at 25000
        // chars total, with the longer texts truncated down.
        let big = "y".repeat(20000);
        let args = json!({"steps":[{"tool":"a","args":{}},{"tool":"b","args":{}}]});
        let mut runner = StubRunner::new(vec![ok(&big), ok(&big)]);
        let compact = interpret(&args, &mut runner, 120000, false);
        let serialized = serde_json::to_string(&compact).unwrap();
        assert!(
            serialized.len() <= 25000,
            "compact serialized to {} chars, must be <= 25000",
            serialized.len()
        );
    }

    #[test]
    fn batch_id_side_channel_is_embedded_and_nonempty() {
        // The compact result carries a _batch_id the handler lifts to the result top level (tested
        // via interpret's embedding); it must be a non-empty string so the parent audit record is
        // correlatable.
        let args = json!({"steps":[{"tool":"a","args":{}}]});
        let mut runner = StubRunner::new(vec![ok("a")]);
        let compact = interpret(&args, &mut runner, 120000, false);
        let batch_id = compact["_batch_id"].as_str().expect("_batch_id present");
        assert!(!batch_id.is_empty(), "batch_id is a non-empty string");
    }

    #[test]
    fn dry_run_maps_step_outcomes_to_would_allow_and_would_deny() {
        // Under dry_run, a step the pipeline would allow reports "would_allow"; a step it would
        // deny reports "would_deny". The runner is invoked with dry_run=true (it records it, though
        // the stub does not branch on it -- the status mapping is what's under test here).
        let args = json!({"steps":[
            {"tool":"find","args":{}},
            {"tool":"navigate","args":{}}
        ], "dry_run":true});
        let mut runner = StubRunner::new(vec![ok("would allow"), denied()]);
        let compact = interpret(&args, &mut runner, 120000, true);
        let status: Vec<&str> = compact["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["status"].as_str().unwrap())
            .collect();
        assert_eq!(status, vec!["would_allow", "would_deny"]);
    }
}
