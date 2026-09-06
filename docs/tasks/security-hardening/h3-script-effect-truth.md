# H3: Execute supported scripts without unsafe replay

Status: Implementation and validation in progress; its separate commit is pending. The owner
accepted the proposed script-correctness work while requesting the epic, then directed execution.
See the [ledger](LEDGER.md) for current progress.

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
