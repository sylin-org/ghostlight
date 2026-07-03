# 0024. Tool registry and the generic ingest pipeline

- Status: Accepted
- Date: 2026-07
- Amends: the dispatch-chokepoint structure of stage 2/3 (`transport::mcp::server::
  handle_tools_call` and the `Governance` record API). Builds on ADR-0022 (the action
  directory becomes the registry's core) and ADR-0007/0022 Decision 7 (the sacred
  tool surface, unchanged by this ADR). ADR-0023 (one loader) is a sibling under the
  same theme; ADR-0025 (manifest hot-reload) builds on both.

## Context

A full-code survey (2026-07-03, four parallel deep-reads over transport, governance,
browser, and the composition root; 113 findings) measured the distance between the
current dispatch path and the project value of fewer, more meaningful moving parts:

- **Tool identity lives in about seven places.** The frozen `TOOLS_JSON` fixture; the
  ADR-0022 action directory; `is_known_tool` (which re-parses the entire fixture
  string on every call); five inline `if name == ...` branches in `handle_tools_call`
  (`computer` action extraction, `explain` server-side handler, `navigate`'s
  pre-dispatch/sacred-target/landing triple, `read_page` redaction);
  `resolve_governing_resource`'s per-tool match (three resource-shape classes chosen
  by name); `sacred_check`'s navigate branch; plus a stale `src/browser/tools/`
  doc-stub subtree (11 files, zero functions) still describing the deleted
  observe/mutate scheme.
- **Audit correctness is caller choreography.** Transport threads requires, domain,
  grant id, and duration through four mutables into five outcome-specific `record_*`
  methods; one of them (`record_navigate_landing_deny`) hardcodes a tool name inside
  the governance core. The capability lookup runs twice per governed call with
  divergent miss semantics (audit maps a miss to "none"; decide maps it to a deny).
  The `computer (<action>)` label rule is implemented twice (dispatch and
  enforcement). The mode switch (`apply_mode`) is invoked from two control paths.
- **One fact, two extension round-trips.** The sacred check resolves a tab's URL by
  internally calling `tabs_context_mcp` and parsing that tool's result shape, while
  the grant path resolves the same tab via the dedicated `tab_url_request` frame.
- **Dead-weight API.** `DomainPolicy`, `ResourceResolver`, `ToolId`, and
  `ResourcePattern` are declared-but-unwired seams in `governance::ports`.

The survey also confirmed what is already RIGHT and must not be disturbed: the
directory is a working proto-registry with fixture-mirror tests; advertisement is
already a pure derivation over it; `Browser::call` is fully tool-agnostic; the
pure/impure split (resources resolved BEFORE the pure decision; `DecisionRequest`
serializable for simulate replay and a future remote PDP) is deliberate and
load-bearing; the pinned stage ordering (hold before everything; sacred before grants
and never shadowable; free actions after sacred, before grants) is behavior, not
implementation detail.

## Decision

### 1. One `ToolDescriptor` registry, data plus hooks

The ADR-0022 action directory generalizes IN PLACE (same module home,
`src/browser/`) into the single per-tool authority. One descriptor per advertised
tool (14: the 13 trained tools plus `explain`), each carrying:

    ToolDescriptor {
        tool:        &'static str,
        action_key:  Option<&'static str>,   // Some("action") on computer only:
                                              // "this tool has sub-actions, keyed by
                                              // this argument"
        variants:    &'static [ActionVariant],// 1 per tool; 13 for computer
        resource:    ResourceShape,
        handler:     Handler,
        postprocess: Option<fn(&mut serde_json::Value, bool)>, // read_page redaction
        post_dispatch: PostDispatch,
    }

    ActionVariant { action: Option<&'static str>,
                    requires: &'static [Capability],
                    description: &'static str }        // ADR-0022 rows, unchanged

    ResourceShape { DomainLess,    // tabs_context_mcp, tabs_create_mcp, update_plan,
                                   // explain
                    TabScoped,     // read_page, computer, find, form_input, ...:
                                   // governed by the tab's current URL via tabId
                    TargetArg }    // navigate: governed by the url argument
                                   // (extension-mirrored normalization)

    Handler { ExtensionForward,    // the default: 13 of 14 tools
              Local(fn() -> String) }   // explain: answered in the binary; the
                                        // pipeline wraps the text in the MCP result

    PostDispatch { None,
                   NavigateLanding }     // the point-5 landing re-check + park marker

Design rule, decided: **descriptors are data; the pipeline owns behavior.** Hooks are
plain `fn` values only where the behavior is pure and self-contained (`postprocess`,
`Local`). Behavior that needs the `Browser` handle or `Governance` (the landing
re-check, the about:blank park, resource resolution round-trips) is expressed as an
enum MARKER the pipeline interprets. There is deliberately no per-tool trait with 14
impls: 13 would be identical boilerplate, which multiplies moving parts instead of
reducing them. A future special case becomes one field on one descriptor row.

Invariants carried over from ADR-0022, unchanged and still pinned by tests:

- **Absent means DENY; empty means ALLOW.** `requires` lookup returns `None` for a
  registry/variant miss (fail closed) and `Some(&[])` for a free action. The two
  states stay unconfusable.
- **The registry is validated against the fixture, never the reverse.** `TOOLS_JSON`
  remains the byte-frozen source of names and schemas (ADR-0007/0022 Decision 7);
  `tools/list` is still the fixture cloned and filtered. Fixture-mirror tests assert
  the registry covers exactly the fixture's names and the computer action enum, with
  no gaps, no stale entries, no duplicates.
- The governance core still receives browser facts as injected values (the a7
  arch-test stands). The `requires` fn-pointer contract is unchanged in shape.

Two consequences of registry-as-authority:

- `is_known_tool`'s per-call fixture re-parse is deleted; the validity check is a
  registry lookup.
- `explain_text`'s formatter generalizes to `{tool} ({action})` from row data (no
  hardcoded "computer"); its output stays byte-identical.

**The family seam (deliberate).** The descriptor shape is intentionally
plugin-manifest-like: name, actions, capability requirements, resource shapes,
handler kind, descriptions. After this ADR, `src/browser/` is a self-describing
plugin behind injected data, and the governance core is already plugin-agnostic by
arch-test. That is the exact seam the Ghostlight family/service direction
(ADR-0021; docs/design/ghostlight-service-architecture.md) needs: a future sibling
(desktop-mcp and others) declares the same kind of table to the same kind of
governor. No family API is built now; the decision here is only to keep
`ToolDescriptor`'s shape close to "what a plugin would declare", so extracting that
API later is mechanical rather than a rewrite.

### 2. The generic ingest pipeline

The pipeline moves to its own module (`transport/mcp/pipeline.rs`); `server.rs`
keeps the JSON-RPC protocol loop and the composition root, nothing else. The
chokepoint's inline ordering tests move with the pipeline. The pipeline keeps the
exact, test-pinned stage ORDER of `handle_tools_call`; every per-tool branch
becomes a descriptor read:

1. Config snapshot (one per call, torn never).
2. Params extraction (name, arguments).
3. Registry lookup. Miss: the existing "Unknown tool" invalid_request result,
   byte-identical.
4. Action extraction via `descriptor.action_key` (no `name == "computer"`).
5. Requires from the descriptor: THE one lookup per call (see Decision 3).
6. Hold check (unchanged position: before everything, including Local handlers).
7. Sacred check: the tab check (STEP B) is ARGUMENT-driven -- any call carrying a
   numeric tabId checks the tab's current host, independent of `ResourceShape`
   (arguments are not schema-validated, so shape must never gate a never-touch
   check); `TargetArg` additionally checks the target host via the shared navigate
   normalization (STEP C); a call carrying neither is skipped. The empty-list fast
   path (no frames, no allocation) stays.
8. Free-action short-circuit (empty requires: no resource resolution, no grant scan)
   and `Local` handler dispatch (explain), in the position pinned by stage 3.
9. Governance authorization (Decision 3), with resource resolution driven by
   `ResourceShape` and skipped entirely when ungoverned or requires is empty.
10. Bounded first-call wait; dispatch via `Handler` (ExtensionForward ->
    `Browser::call`, unchanged contract).
11. `PostDispatch::NavigateLanding`: the landing re-check and park-on-real-deny
    (never on shadow), driven by the marker instead of `name == "navigate"`.
12. Audit completion (Decision 3), then `postprocess` hook and wait-note, then the
    JSON-RPC envelope.

All-open byte-identity and the zero-cost paths are constraints on every stage: no
per-call fixture parse, no resource resolution under all-open, no frames for free
actions, shadow mode observably identical to allow.

### 3. Governance: two-phase `authorize` and the per-call scope (`CallAudit`)

The five public `record_*` methods and the caller-side record-variant state machine
are replaced by a two-phase API that makes governance the single owner of audit
correctness (the scope's Rust identifier is `CallAudit`; the task prompts own the
concrete signatures):

- Phase 0/1, before dispatch: transport opens the per-call scope (`CallAudit`,
  created by `Governance::begin` right after the registry lookup) and then calls ONE
  authorization gate (`Governance::authorize`) with the call context (the one
  requires lookup result, the resolved `GoverningResource` when applicable, config
  mode). The gate returns either a terminal denial (already recorded; transport just
  renders the text) or proceed.
- Phase 2, after dispatch: transport completes the scope through variant-specific
  completion methods (held, sacred denial, landing amendment/denial, and a final
  `complete` that selects allow vs shadow_deny); each consuming call records exactly
  one audit line. The navigate landing re-check amends the scope (new grant
  attribution and domain, or a landing denial) before completion.

Semantics pinned:

- Exactly ONE requires lookup per call feeds both the decision and the audit
  capability field; the divergent-miss-semantics defect class dies. Known defect
  this closes, deliberately: at the stage-3 head (`b4b2faf`) a directory miss on a
  known tool's sub-action (a governed `computer` call with an unknown `action`) is
  flattened to an empty requires slice by the transport's `unwrap_or(&[])` and then
  treated as a FREE action -- it dispatches ungoverned, and `decide`'s
  `unknown_action` arm is production-dead. That is a fail-open violation of
  ADR-0022's absent-means-DENY invariant (exposure is low: the extension rejects
  unknown action strings itself, and the sacred check still applies). This ADR
  restores the invariant as a NAMED, SANCTIONED behavior change: under a manifest, a
  registry miss produces the `unknown_action` denial through the mode switch, from
  the single remaining `apply_mode` call site; under all-open, a miss still
  dispatches (all-open stays byte-identical). A black-box test pins the governed
  denial.
- Sacred denials remain a separate, always-on, never-shadowable path ahead of grant
  evaluation; they record through the same scope machinery but bypass
  `Decision`/`check_call` exactly as today.
- The pure core is untouched: `check_call`, `DecisionRequest` (pure, serializable,
  replayable by simulate), the denial rule vocabulary, message templates, and
  denial-id derivation are all byte-stable. Audit record field order and values are
  byte-stable. Hold and kill semantics, and their positions, are unchanged.
- The `computer (<action>)` label convention collapses to one generic formatter
  (`{tool} ({action})` when an action is present) owned by governance; the dispatch
  and enforcement copies are deleted. `record_navigate_landing_deny` (the
  navigate-only method in the core) is deleted; the landing amendment is generic
  scope behavior.
- `Config.governance_mode` becomes a typed `EffectiveMode` at resolution time
  (parsed once when the snapshot is built) instead of a raw string converted at
  each consumer. The one stringly seam in the typed config registry dies with the
  same motion that reshapes its consumers.
- Governance stays synchronous and free of browser/transport knowledge; resource
  resolution stays outside it (the pure/impure split is load-bearing for simulate
  and the future remote PDP).

### 4. One tab-URL resolution per call

The sacred check and the grant path share one lazily-resolved, memoized tab URL per
call, obtained via the existing `tab_url_request` frame (`Browser::tab_url`).
`resolve_tab_host` (the internal `tabs_context_mcp` call plus result-shape parsing in
the ingest layer) is deleted. Per-stage failure semantics are preserved exactly: an
unresolvable tab means the sacred tab-check finds no host to match (no denial from
that step) while the grant path resolves `Indeterminate` (fail closed). Laziness
preserves the zero-frames guarantees: no probe when the sacred list is empty and the
call is ungoverned or free.

### 5. Dead-seam and stub deletions

Deleted outright: `DomainPolicy`, `ResourceResolver`, `ToolId`, and
`ResourcePattern` from `governance::ports` (unwired placeholders; the registry
realizes their intent on the browser side, and a future remote PDP re-introduces
what it needs via its own ADR), and the `src/browser/tools/` doc-stub subtree (11
files: zero functions, stale observe/mutate prose, a fourth copy of the tool-name
list). Module docs that referenced them are rewritten.

### 6. Explicitly unchanged

`TOOLS_JSON` bytes and the fidelity test (no sanctioned change in this stage);
`tools/list` output; the explain tool's pinned output; denial rules, messages, and
ids; the audit record shape; `check_call` semantics; simulate; presets and
templates; the extension (zero changes -- `tab_url_request` already exists); the
schema-3 manifest format (ADR-0022); all-open behavior, byte for byte.

Known constraint documented: the audit `capability` field renders
`requires.first()`; registry rows therefore keep singleton (or empty) requirement
sets. A future multi-capability row needs an audit-format decision first.

## Consequences

- Positive: one table drives validity, classification, enforcement input,
  advertisement, explain, sacred relevance, resource shape, dispatch kind, and
  result post-processing. Adding a tool (should ADR-0007 ever sanction one) is one
  fixture entry plus one descriptor row.
- Positive: the audit state machine, the double lookup, the duplicated label logic,
  the second mode-switch site, the per-call fixture re-parse, one extension
  round-trip per governed tab call, four dead seams, and eleven stale files are
  deleted. The deletions ledger IS the point.
- Positive: governance owns audit correctness; transport reports events. The
  misroute-a-record defect class dies at the type level.
- Negative: `handle_tools_call` and its ~10 inline ordering tests are rewritten
  against the same assertions; the black-box suites (tool_enforcement, shadow_mode,
  audit_recorder, mcp_protocol, all_open_golden) must pass with expectations
  byte-unchanged. This is the largest single-task rewrite since s05 and is staged
  accordingly.
- Negative: descriptor fn-pointer hooks are less discoverable than inline code; the
  registry module doc must carry the map (which hook fires where, in pipeline
  order).
- Risk, accepted: behavior-preservation across the rewrite rests on the existing
  test wall (459+ tests, byte-pinned oracles). Where a pinned oracle must move
  files, it is transcribed, never re-derived.

## Future work (explicitly not this ADR)

- Manifest hot-reload and re-advertisement: ADR-0025.
- A remote PDP transport (the `DecisionRequest` serialization contract is preserved
  for it); network-layer enforcement; capability qualifiers (ADR-0022 future work).
- Generating documentation from the registry (the descriptor descriptions already
  feed `explain`; a docs generator is sugar, later).

## Provenance

Direction set by the user in review 2026-07-03: generalize the ingest engine around
the existing tool directory ("check if the MCP client invoked a valid command can be
as simple as a lookup on the tool map; invoking the governance as simple as a single
call; the handler for each function a method of it"), with break-and-rebuild
explicitly sanctioned and user delight (including developer delight) as the north
star. User-decided: data-plus-hooks over trait-per-tool (chosen from presented
options); tab-URL unification, manifest hot-reload, and dead-seam deletions all in
scope. Recommended-and-accepted: the two-phase authorize/AuditScope shape (dispatch
happens mid-call, so one synchronous validate cannot own the whole record);
markers-not-closures for Browser-dependent behavior; registry-validated-against-
fixture (never generated); keeping sacred/hold as transport stages recording through
the scope. In the same review the user invited counter-suggestions ("don't take my
suggestions as gospel"); recommended-and-accepted from that round: the pipeline
extraction into its own module, the typed `governance_mode`, the family-seam
paragraph above, and an optional `ports.rs` split alongside the dead-seam deletions.
Declined by design (anti-recommendations, recorded so they are not relitigated):
typing the native-messaging wire vocabulary, buffering the audit writer, caching
compiled patterns, per-tool trait impls, and adopting an MCP SDK. Grounded in the
four-reader survey of 2026-07-03 (113 findings).
