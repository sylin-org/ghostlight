# H3: Execute supported scripts without unsafe replay

Status: Implemented and verified locally, not deployed or published. This evidence accompanies
the implementation commit `fix(script): select script form before page execution`. The owner
accepted the work while requesting the epic, then directed execution. The [ledger](LEDGER.md)
owns remaining epic work.

## Execution record (2026-09-06)

The explore workflow mapped the evaluator, worker error forwarding, operation journal,
`BrowserFrame::Error`, browser error mapping, `work/forms.rs::run_script`, and the single terminal
path. Existing effect uncertainty and packaging seams were sufficient. No new wire type, policy
decision, connector behavior, or Rust production code was needed. The optional constants catalog
under `.agentic/` is absent; current files and types supplied the authority.

The implementation removes exception-text retry and exception-class no-effect classification.
Pinned Acorn 8.18.0 parses an async function body without executing it. Exact AST boundaries
reject wrapper escapes. Returns outside nested functions select the explicitly awaited wrapper;
ordinary source keeps its REPL scope. Syntax errors name coordinates in the supplied source and
send no evaluation. After the sole effectful evaluation, browser exceptions and transport loss
remain uncertain. The operation engine preserves that uncertainty after recovery.

The real Chromium lane exposed an additional defect in the old mock expectations: an async
wrapper's promise was returned as `{}` through the REPL completion envelope. Explicitly awaiting
the selected wrapper restores the intended returned value. Await/resource syntax and hashbangs
are accounted for in local parsing without changing unwrapped source.

| Evidence | Result |
| --- | --- |
| New SA-04 regressions before the production change | Both failed: two mutations from the marker exception; false no-effect certainty after a runtime `SyntaxError`. |
| `node --test extension/tests/evaluate.test.js` | All 20 evaluator tests passed, including real synthetic effects, wrapper escapes, nested returns, source coordinates, import order, and operation-engine recovery. |
| `npm test --prefix extension` | All 183 extension tests passed. |
| `cargo fmt --all -- --check` | Passed. |
| `cargo clippy --workspace --all-targets --target-dir .target-ghostlight-1.0 -- -D warnings` | Passed. |
| `cargo test --workspace --target-dir .target-ghostlight-1.0` | Passed, including all 362 orchestrator library tests and the other workspace suites. |
| `cargo build --workspace --target-dir .target-ghostlight-1.0` and `node tests/process-journey.mjs` | Fresh executables; process reconnect and composed-operation journey passed. |
| `node tests/script-browser-journey.mjs` | All 19 cases passed on Windows with Chrome 152.0.7977.82. Actual DOM mutations and receipts cross the real relays/orchestrator and MCP edge. |
| Changed JavaScript syntax and `git diff --check` | Passed. |
| Extension packaging | The archive includes the parser, MIT license, provenance, evaluator, and worker; their extracted bytes match source. |
| Repository integrity | ASCII, local links, existing version alignment, permission justifications, and capability evidence checks passed. |

The Chromium lane starts a separate headless profile and synthetic local site. It uses the
shipped evaluator with real CDP and a test native-framing adapter. It does not prove an installed
MV3 service worker or machine native-host registration, and it does not deploy or reload the
owner's extension. No Linux/macOS browser lane was run. Published versions remain unchanged.
The vendored parser's license, exact archive integrity, transformation, and source digest live in
[its provenance record](../../../extension/vendor/acorn.PROVENANCE.md).

The original task and acceptance below remain the record of the approved scope. H2a is a separate
commit, `8103c69b`; H1 and H2b remain ideation items and are not completed by this fix.

## Problem and accepted behavior

SA-04 reproduced two defects in the actual evaluator with a Node VM sender. A runtime exception
containing `Illegal return statement` makes Ghostlight execute the script again. A runtime-thrown
`SyntaxError` after an effect is reported as if parsing failed before any execution.

Choose a supported script form without replaying possibly effectful code. Do not infer safe retry
or a known no-effect outcome from exception text or class. Once execution may have begun, retain
uncertainty unless independent evidence establishes the effects. Preserve supported expressions,
bare returns, top-level await, and REPL behavior. Earlier effects remain; there is no rollback.

The September 6 amendment to [ADR-0133](../../adr/0133-behavioral-capability-restoration.md)
records this correction to Decision 6's fallback. The mechanism remains implementation work.

## Current placement to recheck at cycle start

- [extension/lib/script-evaluator.js](../../../extension/lib/script-evaluator.js): owns the
  CDP evaluation flags, wrapper, retry decision, and effect uncertainty on exceptions.
- [extension/tests/evaluate.test.js](../../../extension/tests/evaluate.test.js): closest existing
  tests. Some tests currently pin the unsafe retry mechanism; replace those expectations while
  retaining the supported user behavior. Queueing fake CDP replies alone does not prove effects.
- [extension/service-worker.js](../../../extension/service-worker.js): invokes the evaluator and
  relays its result/error. Trace its receipt through bridge types and orchestrator completion so
  uncertainty reaches the model accurately.
- [extension/lib/engine.js](../../../extension/lib/engine.js): consumes effect uncertainty for
  engine state. Check that the correction does not turn an uncertain failure into a safe replay.

Before production changes, apply the explore workflow, read current contracts and DEV-LOOP, map
the whole error path, and check actual browser parsing/evaluation semantics. Do not infer that an
arbitrary `SyntaxError` or message proves a pre-execution parser failure. Use official browser
evidence if selecting a new CDP mechanism. Keep policy in the orchestrator and browser mechanics
in the adapter; add no product decisions to the connectors.

## Bounded task and acceptance

One logical fix should replace the unsafe interpretation/retry and uncertainty classification
at their owning seam, with meaningful regression tests and current contract updates.

1. Turn both assessment probes into repository regressions. Count actual synthetic effects as
   well as effectful evaluation entries; a side-effect-free compilation probe is not an execution.
2. A script that increments a sentinel and throws an error containing the bare-return marker
   increments once and never receives an automatic effectful second run.
3. A script that increments and then throws `SyntaxError` increments once and does not report a
   known no-effect parse failure. A forged description/class has no authority over retry safety.
4. Genuine supported bare-return input still works, including an await and a subsequent runtime
   failure. Expressions, top-level await, return values, and supported REPL semantics remain intact.
5. Genuine parse-only failure performs no effect. Claim that certainty only when the selected
   mechanism provides evidence that execution did not begin; otherwise preserve uncertainty.
6. Exercise the cases in real Chromium against a disposable synthetic fixture and verify the
   service/model-facing effect account. Keep the isolated evaluator and live-browser evidence
   distinct. Do not use a production site or real secrets for the mutation sentinel.

No general automatic retry framework or new tool signature is part of this fix. If no mechanism
can preserve an existing promised behavior, bring that concrete compatibility tradeoff to ideation
before changing the product contract. Routine implementation selection remains authorized.

## Verification and completion

Run focused evaluator regressions, the extension suite, and the applicable real browser journey.
Before a commit, run all AGENTS.md gates and syntax-check changed JavaScript. Use isolated fresh
binaries where needed and point journeys at that build. Follow DEV-LOOP for any later live swap.

Record changed paths, before/after evidence, actual commands, commit, missing proof, and any
deferred compatibility concern in the ledger. Keep H2a's existing local fix a separate logical
change. This package does not complete H1 audit confidentiality or H2b aggregate reporting.
