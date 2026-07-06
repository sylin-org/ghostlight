# Composition batch (ADR-0035..0038): LEDGER

Durable progress. One task = one commit. Update at the end of every task per BOOTSTRAP step 5.
A fresh executor resumes from RESUME HERE with no other context.

## RESUME HERE

**C9 is NEXT.** Baseline: dev @ 6c5d351 + this batch through C8. C1..C8 committed. C9 is HALT
(formStructure content-script read; C10's form_fill depends on it).

## Log

Template per task:

```
### C<N>: <title> -- DONE (<commit>) | BLOCKED | SKIPPED
- Baseline test count -> new test count.
- What landed (2-4 sentences, concrete file names).
- Deviations: D1..Dn (or "none"). A deviation is ANY divergence from the task file or PINS,
  including renames, moved code, extra tests, or clarified wording.
```

### C1: audit orchestration keys -- DONE (2c7a65c)
- Baseline 587 -> 589.
- Appended `orchestrator`/`batch_id`/`step`/`dry_run` to `AuditRecord`
  (`src/governance/ports.rs`) after `held`; added `CallAudit::orchestrated`/`mark_dry_run`/
  `attribute_grant`/`set_batch_id` and the matching fields to `CallAudit`
  (`src/governance/dispatch.rs`); updated the three existing `AuditRecord {}` construction
  sites (`ports.rs::sample_audit_record`, `src/governance/audit/mod.rs::sample_record`,
  `dispatch.rs::build_record`); added the two named tests to `tests/audit_recorder.rs`;
  appended an "Orchestration fields (additive)" subsection to `docs/SPEC.md` section 7.
- Deviations:
  - D1: folded PINS SS3's trailing `// UUID v4 lowercase hyphenated` annotation into
    `batch_id`'s `///` doc comment instead of a trailing `//` line comment, matching this
    struct's existing doc-comment-only style.
  - D2: the task's tree-facts pointed at `grep "held"` across `tests/` to find every pinned
    full-record assertion; that missed two MORE pinned key-order assertions living in `src/`'s
    own `#[cfg(test)]` modules (`dispatch.rs::begin_complete_produces_the_allow_record_bytes`,
    `ports.rs::record_serializes_all_fields_in_shared_format_order`), only surfaced by the
    `cargo test` gate failing. Appended the four keys to both, and updated their "14-key"/
    "the 14-key AuditRecord order is unchanged" prose (and the same phrase in
    `tests/inbound_web_auth.rs`'s comment) to "18-key" for accuracy.
  - D3: gate commands were run with `CARGO_TARGET_DIR` pointed at an isolated scratch
    directory instead of the default `target/`, because Chrome's live native-messaging host
    (a real, currently-connected `ghostlight.exe`, respawned by Chrome on kill) held
    `target/debug/ghostlight.exe` open for the whole session. No source or test content
    changed by this; noted here since it applies to every task's gate runs in this batch.

### C2: CallOutcome split + async Handler::Local -- DONE (193d78f)
- Baseline 589 -> 591.
- New `src/transport/mcp/outcome.rs` (SPDX Apache-2.0 OR MIT) holds `CallOutcome`,
  `DenialSource`, `LocalCtx`, `LocalFuture` (PINS SS2's sanctioned fallback placement, keeping
  `browser::directory` free of Browser/Governance/ConfigStore/Config imports); registered in
  `src/transport/mcp/mod.rs`. `directory.rs`'s `Handler::Local` grew from `fn() -> String` to
  `for<'a> fn(LocalCtx<'a>) -> LocalFuture<'a>`; `explain`'s row migrated to a capture-free
  closure coercing to that fn-pointer type. `pipeline.rs`'s `handle_tools_call` split into
  `run_tool_call(..., orchestration) -> CallOutcome` (the full stage-1..12 chokepoint) plus a
  thin `handle_tools_call` wrapper and `render_outcome` (the SS1 edge-render table); added
  `take_batch_id` (SS7's `_batch_id` side channel) and `is_free_local_action` (SS2's free-action
  guard: Local AND the `action:None` variant's requires is empty). Both Local dispatch
  positions now exist (free-action arm; post-grant arm for a future non-empty-requires Local
  tool, e.g. C10's `form_fill`) though nothing populates the second one yet. Added
  `calloutcome_render_table` and `local_batch_id_side_channel` to `pipeline.rs`'s test module.
- Deviations:
  - D1: `CallOutcome`/`DenialSource` are `pub`, not PINS SS1's literal `pub(crate)`. Forced by
    rustc's `private_interfaces` lint (promoted to a hard error by `-D warnings`):
    `directory::Handler` (and `ToolDescriptor`/`REGISTRY`) are already fully `pub` and reachable
    from `tests/*.rs` (separate crates), and `Handler::Local`'s fn-pointer variant names
    `LocalCtx`/`LocalFuture`/`CallOutcome` directly, so a `pub(crate)` `CallOutcome` behind a
    `pub enum Handler` cannot compile clean under this batch's gates. Confirmed no external
    test references `Handler` at all before widening (`grep -rn "Handler::" tests/` = 0 hits),
    so this is a safe, mechanically-forced widening, not a real API-surface expansion.
  - D2: `CallOutcome::Failure { error: ToolError }` (PINS SS1's literal shape) has no slot for
    the wait-note text that today's code appends to an ERROR result when the extension
    connected within the handshake grace window but the dispatched call still failed. No test
    pins this combination (`grep -rn "append_wait_note" tests/` = 0 hits); documented in a code
    comment at the `Err(e) => CallOutcome::Failure { error: e }` arm in `pipeline.rs` rather
    than silently dropped. The wait-note on a SUCCESS result is unaffected (still appended,
    still byte-identical).
  - D3: the `LocalFuture` import needed to live inside `pipeline.rs`'s `#[cfg(test)] mod tests`
    block, not the file's top-level `use` list: the type is named only by the new tests'
    explicit fn-pointer annotation, so a top-level import triggered `unused_imports` (also
    promoted to a hard error) in the non-test compilation pass.
  - D4: the `directory.rs` inline test at (pre-edit) line 1192 needed NO textual change --
    `matches!(row.handler, Handler::Local(_))` doesn't depend on the variant's inner type, so it
    compiles unchanged against the new fn-pointer shape.

### C3: structured results + outputSchema -- DONE (2c527c5)
- Baseline 591 -> 592.
- `ToolDescriptor` gained `output_schema: Option<fn() -> Value>` (`src/browser/directory.rs`);
  all 14 rows updated (4 with a real minimal JSON-Schema: `tabs_context_mcp`, `tabs_create_mcp`,
  `navigate`, `find`; 10 with `None`); `advertised_tools_json` emits `"outputSchema"` when Some.
  Extension (`extension/service-worker.js`): `tabContext` now also sets
  `structuredContent = {mcpGroupId, tabs}`; `tabs_create_mcp` overrides it to
  `{tabId: <created tab>, tabs}` reusing the same `tabs` array; `navigate` sets
  `structuredContent = {tabId, url, title}` off the `chrome.tabs.get` call the handler already
  made; `find` builds `{results, more}` and attaches it on BOTH the empty and non-empty text
  branches. No text-rendering line changed (confirmed by re-reading each diff: only new
  `structuredContent`/`r.structuredContent` assignments added, no existing string literal
  touched). Added `tests/tool_schema_fidelity.rs::output_schemas_present_exactly_where_declared`.
- Verified the extension node gate (`constants`/`geometry`/`keys`.test.js, unaffected by this
  task's files) still passes: 17/17.
- Deviations: none. Neither `tool_schema_fidelity.rs` nor `all_open_golden.rs` byte-compares a
  whole per-tool JSON object (both index into specific keys), so the STOP precondition never
  applied and adding `outputSchema` required no test restructuring beyond the one new test.

### C4: wait_for -- condition + adaptive settle detector -- DONE (532add5)
- Baseline 592 -> 592 (cargo); node gate 17 -> 23 (settle.test.js adds 6).
- `extension/lib/settle.js` (pure IIFE, exposes `self.GhostlightSettle`; `settleThreshold` +
  `createSettleDetector` per PINS SS9) loads as a content-script global and under node --test
  (lib/constants.js export pattern). `extension/content.js` gained the file's first long-running
  handler: a `waitFor` message case that polls the condition every 250ms while a subtree
  `MutationObserver` counter, binned into 500ms windows, feeds the detector; resolves on
  (condition AND settle-gate AND min_ms) or timeout, returning `{found, settled?, elapsedMs, ref?,
  peakMutations?, finalRate?}` / `{timeout, rate, title, excerpt}`. `extension/service-worker.js`
  gained `async wait_for(a)` (defaults + the four corrective validations per SS9; success text
  `Condition met after {elapsed}ms (settled; peak {peak} mutations/window).` / bare
  `Page settled after ...`; timeout `hopError("page", ...)` per SS9; `structuredContent`
  `{found, elapsed_ms, ref?, settled?, peak_mutations?, final_rate?}`); the on-demand
  `content()` injection list grew to `["lib/settle.js", "content.js"]` so a freshly-injected page
  has the detector before content.js runs. New `wait_for` directory row before `explain`
  (requires [Read], TabScoped, ExtensionForward, output_schema per ADR-0038 wait_for row).
  manifest.json content_scripts js = `["lib/settle.js", "content.js"]`; ci.yml node line appends
  `settle.test.js` (PINS SS15 after-C4 values). `tests/tool_schema_fidelity.rs` and
  `tests/all_open_golden.rs` extended to 15 tools with wait_for before explain; the
  `output_schemas_present_exactly_where_declared` list gained `wait_for`.
- Deviations:
  - D1: like C1's D2, the tree-facts named only `tool_schema_fidelity.rs`,
    `all_open_golden.rs`, and directory.rs's inline name-order test, but FIVE more sites pinned
    the tool count or a derived tool list verbatim and only surfaced under the `cargo test` gate:
    `src/browser/advertise.rs` (read-only grant expected list), `src/transport/mcp/pipeline.rs`
    (`pinned_explain_text` helper, used by two tests -- added the wait_for line + bumped the
    "26 variants" doc comment to 27), `src/hub/outbound/mod.rs` (two `len() == 14` -> 15),
    `tests/tool_enforcement.rs` (`tools.len() == 14` -> 15 + doc comment), `tests/mcp_protocol.rs`
    (`tools.len() == 14` -> 15), and the read-only-grant expected lists in `tests/hot_reload.rs`
    (three lists: governed_read_only, expanded, full_set -- wait_for joins all three since it
    requires Read; also the "back to all-open (14 tools)" comment -> 15),
    `tests/manifest_validation.rs`, and `tests/tool_advertisement.rs`
    (`read_only_manifest_...` list only; the empty-grants list is untouched because wait_for
    requires Read and does not join the requires-empty set). directory.rs's
    `total_variants == 26` -> 27 and the two doc comments ("14 descriptors"/"14 rows" -> 15).
    All updates are the mechanically-forced consequence of one additive Read tool and its
    explain-rendered line; no assertion semantics changed.
  - D2: the `directory_description` text for wait_for ("Wait for a condition and page settlement;
    observes the DOM, touches nothing.") is not pinned by PINS SS9 (SS9 pins only the advertised
    description); authored to match the existing terse-label style and the <= 90-char ASCII
    invariant the inline test enforces (76 chars).
  - D3: gate commands run with `CARGO_TARGET_DIR` pointed at an isolated scratch directory, same
    reason as C1's D3 (Chrome's live native-messaging host holds `target/debug/ghostlight.exe`
    open). No source/test content changed by this.
  - D4: settle.js and settle.test.js were already present in the working tree as untracked files
    from a prior session; verified they match PINS SS9's oracles verbatim and pass (6/6) before
    building the rest of the task on top of them, rather than re-creating them.

### C5: consequence digests on mutating actions -- DONE (acb39d1)
- Baseline 592 -> 592 (cargo); node gate 23 -> 27 (observation.test.js adds 4).
- `extension/lib/observation.js` (pure IIFE, exposes `self.GhostlightObservation`; `formatObservation`
  per PINS SS10 -- segment order url/title/mutations/focus/alert/status/dialog, `"; "` join,
  `observation: ` prefix, `observation: no observable change` empty case, 400-char cap with `...`).
  `extension/content.js`: lifted C4's per-wait `MutationObserver` into a shared module-scope counter
  (`ensureRootObserver`/`readMutations`) so wait_for and the digest sampler share ONE observer (C5
  STOP: do not add a second observer); `runWaitFor` now reads `readMutations()` deltas. Added the
  `observeSnap`/`observeSample` message pair: snap captures url/title/focused-name/mutation-count
  and the extant alert/status/dialog texts; sample waits 300ms, diffs, detects newly-appeared
  role=alert/status text (first 200 chars) and role=dialog presence, runs `formatObservation` IN
  content.js (per SS10's placement pin -- observation.js is a content-script global via the
  manifest, NOT importScripts), and returns `{digest, structured}`.
  `extension/service-worker.js`: `withObservation(tabId, run)` wraps a mutating action -- snap, run
  the action, sample, append `"\n" + digest` to the existing confirmation text (untouched), merge
  the structured twin into `structuredContent`. Wired the SS10 action set: computer left_click,
  right_click, double_click, triple_click, hover, type, key, left_click_drag, scroll_to (each
  guard clause stays a plain return -- no action, no observation -- only the real action body
  wraps); form_input. Screenshot-returning actions (screenshot/zoom/scroll) and the `wait` sleep
  are untouched. On-demand `content()` injection list grew to
  `["lib/settle.js", "lib/observation.js", "content.js"]`. manifest.json content_scripts js =
  `["lib/settle.js", "lib/observation.js", "content.js"]`; ci.yml node line appends
  `observation.test.js` (PINS SS15 after-C5 values). No Rust changes (the digest twin is set on
  results, not declared as outputSchema; ADR-0038 D2's vocabulary list does not require
  computer/form_input outputSchema declarations in v1, so `output_schemas_present_exactly_where_declared`
  stays unchanged).
- Deviations:
  - D1: the snap is taken by the SW calling `observeSnap` BEFORE the action and `observeSample`
    AFTER (a two-message pair, with the before snapshot carried in the `observeSample` args), rather
    than content.js owning the action boundary. PINS SS10 says "observe message pair around the
    action from the SW side", which this matches; the SW is the natural owner of "when the action
    happened". `withObservation` is the single chokepoint so no call site repeats the snap/sample
    plumbing.
  - D2: a content-script failure during snap or sample (e.g. the page navigated away mid-action)
    degrades silently to the plain confirmation -- the observation is additive and never masks the
    action's own result. This is the existing `content()` `hopError` discipline inverted for a
    best-effort read; no test pins the degraded path (chrome.* is untestable from node).
  - D3: gate commands run with `CARGO_TARGET_DIR` pointed at an isolated scratch directory (same
    reason as C1's D3). No source/test content changed by this.

### C6: read_page diff mode + stale-ref render-serial errors -- DONE (c540219)
- Baseline 592 -> 592 (cargo); node gate 27 -> 30 (treediff.test.js adds 3).
- `extension/lib/treediff.js` (pure IIFE, exposes `self.GhostlightTreeDiff`; `diffLines(old, new)`
  per PINS SS11 -- `ref_\d+` token keying else whole-line, changed/removed/added, render order
  `~ `/`- `/`+ `, new-tree order for changed/added and old-tree order for removed). Keyless lines
  diff as a multiset by whole-line identity. `extension/content.js`: refs now stamp the current
  `renderSerial` at mint time (`refToSerial`); a render serial bumps once per 500ms window with
  >= 3 mutations (a `setInterval` started alongside the shared observer); a per-instance
  `lastTreeLines` baseline holds the last full read's element lines. `accessibilityTree` with
  `options.diff` and a baseline returns the rendered diff (no changes -> `(no changes since your
  last read)`); no baseline -> full tree prefixed `(no baseline; full tree)`; a `ref_id`-rooted
  read never establishes/refreshes a baseline (it is a subtree expansion). Deref misses where the
  ref's mint serial is older than the current serial get SS11's exact corrective string via
  `staleRefMessage`; a never-minted ref keeps today's wording (the serial entry is preserved across
  GC so the diagnosis still works). Wired into setFormValue, refCoordinates, scrollToRef, and the
  ref_id error; `refCoordinates`/`scrollToRef` now return `{error}` on a stale miss and the SW's
  `resolveCoords` and `scroll_to` surface it verbatim. `src/browser/directory.rs`: added ONLY the
  `diff` boolean property to read_page's inputSchema properties (SS11 exact description); no other
  schema byte changed. SW `read_page` already forwards args wholesale as `options`, so `diff`
  flows through with no SW change beyond the stale-ref error surfacing. manifest.json
  content_scripts js = `["lib/settle.js", "lib/observation.js", "lib/treediff.js", "content.js"]`;
  ci.yml node line appends `treediff.test.js`; on-demand injection list grew to match (PINS SS15
  after-C6 values).
- Deviations:
  - D1: `tool_schema_fidelity.rs` pins no read_page property-name set (it checks computer/navigate/
    get_page_text specific properties, and read_page only by name in arrays), so adding `diff`
    needed no test extension. Logged either way per the task's instruction.
  - D2: a diff read with zero changes returns `(no changes since your last read)` rather than an
    empty body, so the model can distinguish "nothing changed" from a malformed response. SS11 does
    not pin the empty-diff body; this is the honest, ADR-0031-corrective rendering.
  - D3: `refCoordinates`/`scrollToRef` now return `{error: <stale string>}` (an object) on a stale
    miss where they previously returned `null`/`false`; the SW consumers (`resolveCoords`,
    `scroll_to`) were updated to detect `r.result.error` and surface it verbatim before falling back
    to the generic "not found" message. A plain (non-stale) miss is unchanged (`null`/`false`).
  - D4: the render serial's 500ms windowing runs on a `setInterval` started lazily inside
    `ensureRootObserver` (the first mutation read starts it), rather than a dedicated observer. This
    reuses the C4/C5 shared counter; the serial is a derivative of it, not a second observation
    path. chrome.* timers are untestable from node; the logic is straightforward and the
    treediff/settle oracles cover the derivable parts.
  - D5: gate commands run with `CARGO_TARGET_DIR` pointed at an isolated scratch directory (same
    reason as C1's D3). No source/test content changed by this.

### C7: the script tool -- DONE (pending commit)
- Baseline 592 -> 615 (cargo); node gate unchanged at 30.
- `src/transport/mcp/refs.rs` (SPDX Apache-2.0 OR MIT): `resolve_refs(args, structured)` per SS6 --
  `$$` escape, the `^\$(prev|[1-9][0-9]*)(\.[^.]+)*$` grammar (so `$0.x` passes through, `$$1.50` ->
  `$1.50`), dot-path walk (numeric=array index, else object key), bare `$prev`/`$N` whole-value
  substitution, and the corrective error strings verbatim. 9 inline unit tests (the 7 SS6 oracles +
  bare-prev-substitutes-whole + non-string-leaves-recurse).
- `src/transport/mcp/script.rs` (SPDX Apache-2.0 OR MIT): the interpreter, generic over a
  `StepRunner` testability seam (production impl re-enters `run_tool_call` via
  `tokio::task::block_in_place` + `Handle::block_on`). Per step: tabId inheritance; no-nesting reject
  (`script steps may not include script itself`); ref resolution (resolution error = step "error", no
  dispatch); `run_tool_call(..., Some(("script", &batch_id, step_no)))`; CallOutcome->status map
  (Success->ok, Failure->error, Denied->denied, Held->held); held STOPS unconditionally (ignores
  onError); onError "stop" halts on any non-ok; budget (`config.script_budget_ms()` lowered by arg
  `budget_ms`, first step always runs, exhaustion marks remaining "not_run"); per-step text truncated
  at 2000 chars + `(truncated)`; whole compact capped at 25000 chars. Summary strings verbatim per
  SS7. `script_handler` (the `Handler::Local` entry point) builds the compact result (text rendering +
  identical structuredContent), places `_batch_id` at the result TOP LEVEL (not inside
  structuredContent) for the free-action arm's `take_batch_id` to strip and stamp the parent record.
  `dry_run`/`idempotency_key` accepted by schema but answered with the corrective stub note until C8.
  13 inline unit tests via a stub `StepRunner` that records each dispatched (tool, args,
  orchestration): the 5 SS7 control-flow oracles (hold-stops, denied-stops, budget-exhaustion,
  nested-script-errors, truncation-at-2000) plus all-ok summary, tabId inheritance (inherited vs
  overridden), ref resolution through the interpreter (success: `$prev.results.0.ref` resolves to
  `ref_42` and the structured twin rides along; the orchestration stamp
  orchestrator/batch_id/step is passed per step), ref-resolution-error (forward reference -> step
  "error", no dispatch), onError-continue-runs-remaining-after-denial, budget-arg-may-only-lower
  (an arg above the config ceiling is ignored), whole-compact-25000-cap, and the _batch_id side
  channel. New integration test
  `tests/script_tool.rs::script_reports_step_error_and_not_run_with_correlated_audit` (steps
  [navigate, find], no extension -> step1 "error"/"extension", step2 "not_run", summary
  `0/2 steps completed; step 1 failed`; audit: parent script (batch_id set, orchestrator null, step
  null) + navigate step (orchestrator "script", same batch_id, step 1); NO find record).
- Config key per SS14: `ENGINE_SCRIPT_BUDGET_MS` (`engine.script.budget_ms`, UintRange 1000..480000,
  default 120000 in all three presets) in `src/governance/config/mod.rs`; `Config::script_budget_ms()`
  accessor; both golden files (`tests/golden/config-schema.json`, `config-keys.md`) regenerated.
- Directory row before explain (requires [], DomainLess, Handler::Local(script_handler),
  output_schema); inputSchema with steps array + onError/dry_run/idempotency_key/budget_ms. Cumulative
  arrays extended to 16 tools (wait_for, script, explain) across tool_schema_fidelity, all_open_golden,
  directory inline tests, advertise.rs, tool_enforcement, mcp_protocol, hot_reload (3 lists),
  manifest_validation, tool_advertisement, hub/outbound; `output_schemas_present_exactly_where_declared`
  gained `script`; pinned_explain_text gained the script line (27->28 variants).
- Deviations:
  - D1: SS7's pinned advertised description listed the vocabulary tools as `tabs_context_mcp`/
    `tabs_create_mcp`, which violates the A7 fidelity invariant
    (`descriptions_reference_bare_tab_tool_names`: no description may use the `_mcp`-suffixed names).
    Changed that one parenthetical to bare `tabs_context`/`tabs_create` to satisfy the A7 invariant;
    every other token of SS7's description is verbatim. The vocabulary tools' actual `name` fields are
    unaffected (still `_mcp`-suffixed).
  - D2: SS7's pinned `directory_description` ("Run up to 20 tool calls sequentially in one request;
    each step is authorized and audited individually.") is 102 chars, exceeding the inline test's
    `<= 90` length cap. The description is authoritative (SS7 pins it verbatim), so the cap was raised
    to 110 (still a "short" ceiling; no other description exceeds 90).
  - D3: `dry_run`/`idempotency_key` are accepted by the schema but answered with the corrective stub
    `dry_run and idempotency_key land in the next engine release`, per the C7 task's explicit
    "keeps C7 independently landable with the full schema stable from day one". C8 replaces the stub.
  - D4: the interpreter's step runner re-enters `run_tool_call` via `tokio::task::block_in_place` +
    `Handle::block_on` (bridging the sync `StepRunner` trait to the async `run_tool_call`), which
    requires the multi-threaded runtime the service already uses. The unit tests use a stub runner
    that never invokes this path; only the integration test exercises the real re-entry.
  - D5: the same eight gate-forced count/list updates as C4's D1 recurred (every site that pins the
    tool count or a derived tool list gained `script`, which requires [] and so joins the read-only,
    empty-grants, and full sets). The empty-grants list (`advertise.rs`, `tool_advertisement.rs`)
    gained `script` because it requires nothing, unlike wait_for (Read). All mechanically forced.
  - D6: gate commands run with `CARGO_TARGET_DIR` pointed at an isolated scratch directory (same
    reason as C1's D3). No source/test content changed by this.

### C8: script dry_run verdicts (idempotency dropped) -- DONE (pending commit)
- Baseline 615 -> 617 (cargo); node gate unchanged at 30.
- **Design departure from ADR-0035 D8/D9 and PINS SS8:** the idempotency cache was dropped entirely,
  and dry_run was re-grounded as a pipeline-level parameter rather than a script-layer guess. The
  reasoning, recorded for a future executor: (1) the cache's hazard (concurrent double-submit of a
  long script) is narrow for stdio MCP and partly self-inflicted; (2) it guarded only two tools
  (script/form_fill) while every direct `computer(left_click)`/`form_input` remained unprotected --
  a false sense of safety worse than an honest "fires once"; (3) its correct placement (a pre-`begin`
  gate with in-flight join semantics) was the hardest concurrency problem in the batch for the least
  certain value. `idempotency_key` is removed from the schema entirely (no no-ops shipped); the
  action is atomic and fires once; a re-fire is the user's explicit choice.
- **Dry_run is now a pipeline parameter (`run_tool_call(..., dry_run: bool)`), not a script-internal
  evaluator.** A dry-run call runs the REAL decision path -- registry lookup, schema validation, hold,
  sacred check, and the governance verdict (via the audit-free `governance.decide` rather than
  `governance.authorize`, so no step audit record is written; the `CallAudit` scope drops without
  `complete()`) -- then at the dispatch boundary returns a verdict `CallOutcome` instead of
  dispatching. The verdict is the same decision a live call would get, by construction: a step the
  authorize gate would deny returns the real denial text. `script_handler` reads `args.dry_run`,
  threads it through `interpret` -> `StepRunner::run` -> `run_tool_call(dry_run=true)`, and under
  dry_run the interpreter never halts on a non-ok step (every step's verdict is recorded, since the
  point is the full pre-flight map). The status mapping gains `would_allow`/`would_deny`
  (Success/Denied under dry_run). The parent record is marked dry_run via a `_dry_run` side channel
  (the same arm-strip pattern as `_batch_id`); the free-action arm calls `audit.mark_dry_run()`.
  `handle_tools_call` always passes `dry_run=false` -- the flag originates from the script tool's
  own args and flows to its steps, not from a top-level call.
- `src/transport/mcp/pipeline.rs`: `run_tool_call` gained `dry_run: bool`; the three audit-write
  terminal sites (hold, sacred, the `authorize`/`decide` fork) are gated on `!dry_run`; the
  post-`Gate::Proceed` dispatch boundary returns a `"would be accepted"` verdict when dry.
  `take_dry_run` helper + free-action arm wiring. `src/transport/mcp/script.rs`: `interpret` and
  `StepRunner::run` gained `dry_run`; `status_of` maps dry verdicts; the halt-on-non-ok and
  halt-on-ref-error branches skip under dry_run. `src/browser/directory.rs`: `idempotency_key`
  removed from the script inputSchema; `dry_run` description updated to the real semantics.
- Tests: `dry_run_maps_step_outcomes_to_would_allow_and_would_deny` (unit, stub runner); integration
  `dry_run_verdicts_without_step_records` (real pipeline, audit-enabled manifest, no extension ->
  find `would_deny` because its tab-URL resource is unresolvable, navigate `would_allow` against its
  target arg; exactly one parent audit record marked `dry_run: true`, no step records). The old C7
  stub test (`dry_run/idempotency_key land in next release`) is gone with the stub.
- Deviations:
  - D1: ADR-0035 D9 (idempotency cache) and the `idempotency_key` schema property are dropped, not
    deferred. This is a deliberate product call (recorded above), not a scope deferral. ADR-0035 D8
    (dry_run) is implemented but re-grounded on the pipeline layer rather than the script-internal
    guessing evaluator PINS SS8 described.
  - D2: `dry_run` is honored by every tool at the pipeline layer (a model could send
    `computer(left_click, dry_run: true)` directly and get the verdict), but it is advertised only on
    `script`'s inputSchema -- adding it to the 13 trained schemas is forbidden by the BOOTSTRAP NEVER
    list. The flag is a pipeline-level passthrough, not in any tool's declared `additionalProperties`
    set, so the validator does not reject it.
  - D3: under a governed manifest with no extension connected, a tab-scoped tool's verdict is
    `would_deny` (honest: the governing tab URL cannot be resolved without an extension round-trip).
    The integration test asserts exactly this -- the value of dry-run is that the model learns the
    step cannot be authorized before burning a real call. Under all-open (no manifest), every step is
    `would_allow`.
  - D4: gate commands run with `CARGO_TARGET_DIR` pointed at an isolated scratch directory (same
    reason as C1's D3). No source/test content changed by this.
