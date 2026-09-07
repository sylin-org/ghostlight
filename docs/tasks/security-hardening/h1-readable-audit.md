# H1: Readable bounded audit

Status: implemented and verified locally; deployed locally on 2026-09-07, not published.
Owner decision: 2026-09-06. See the [deployment record](../../STATUS.md#local-deployment-2026-09-07).

## Accepted experience

Readable history is the default. Details appear on demand; policy controls retained information.
Keep concise explanations and the existing monotonic target-name control. Additional prose must
help recovery. State uncertainty when it changes the next step. Say "Script failed during partial
execution." only when evidence establishes that execution began and failed.

Display detail and retention are separate concerns. Expanding history does not authorize more
capture. Defer a Minimal/Descriptive selector until it offers a meaningful choice beyond existing
controls. Richer diagnostic capture remains a separate discussion. Child history and aggregate
recovery remain H4 and H2b; their examples do not authorize implementing undecided semantics here.

## Implementation task

One logical commit: require a language-owned audit projection at terminal construction, replace
arbitrary failure-facts copying with closed failure metadata, and render durable failure summaries
without arbitrary browser text. Keep permitted details in the requesting client's result. Preserve
governed hosts, normalized bounded target names, measurements, attribution, and readable history.
Audit ingestion must still accept older records; do not modify or delete historical files.

The existing orchestrator owns the correction. No new policy setting, connector contract, capture
mode, retention service, or website rollback is required.

## Acceptance

- Reproduce the failed-flow Read sentinel in serialized audit before the correction.
- Prove page text, entered values, script results, paths, selectors, handles, arbitrary tool names,
  and exception text stay out of new records across success, failure, and composition.
- Prove intended client results retain permitted details.
- Preserve readable host/count/target-name explanations and target-name removal by policy.
- Keep historical audit ingestion compatible without re-emitting arbitrary legacy failure facts.
- Run formatting, warnings-denied workspace Clippy, workspace and extension tests, and a fresh
  executable process journey. Record evidence and physical-lane limits before completion.

## Implementation and evidence

`language/audit.rs` owns a projection with no raw-result constructor and a closed `AuditRefusal`
vocabulary. Every terminal now requires that projection. The common completion path sends it to
audit alongside the existing measurements, policy attribution, and effect/status. Arbitrary client
facts and browser error descriptions remain available in permitted client results. Unrecognized
request tool names become `unknown_tool` in retained audit and completion diagnostics.

Recovered pages, already-closed tabs, and asynchronous browser landing records now choose typed
outcomes too, preserving their existing wording. Workbench history consumes the safe retained
sentence without a new presentation mode. Historical audit ingestion retains recognized closed
failure metadata and drops arbitrary legacy details. It neither rewrites files nor sanitizes
their historical summaries.

Before the fix, new regressions reproduced the successful Read nested in failed-flow audit,
browser text in both summary and facts, and a caller-authored unknown tool name in audit. They now
pass. Eight new Rust tests cover those paths, script result and invalid-input context, historical
read compatibility, typed failure round trips, catalog names, useful measurements, and governed
target labels. Existing policy removal and action-subject tests still pass.

Verified on Windows:

- `cargo fmt --all -- --check`.
- `cargo clippy --workspace --all-targets --target-dir .target-ghostlight-1.0 -- -D warnings`.
- `cargo test --workspace --target-dir .target-ghostlight-1.0`: 447 tests, including 370
  orchestrator library tests.
- Extension `npm test`: 183 tests. Extension source is unchanged.
- Changed process-journey JavaScript syntax and repository integrity: 915 tracked files, local
  links, version alignment, fixed ASCII exceptions, and the capability matrix pass.
- Fresh workspace build and process journey against `.target-ghostlight-1.0/debug`.
  Added real MCP/relay/service/JSONL checks for a failed Read/script flow and a primitive browser
  error. Both preserve permitted client detail while excluding it from the actual audit file.

The process journey uses a synthetic browser adapter. No installed MV3, live-browser deployment,
or Linux lane was run for H1. Existing generic adapter error reports do not prove partial script
execution; H1 does not infer that state from `effect: unknown` or exception wording. The accepted
short script sentence remains conditional on that evidence.

This record accompanies `fix(audit): retain bounded language instead of result payloads`; use Git
for the commit hash. H2b ideation is next. H4 child-history expansion, richer capture, and new
retention profiles remain outside this completed cycle.
