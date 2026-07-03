# S06: audit capability field; delete classify.rs and RwClass

## Goal

Finish the model switch on the audit consumer per ADR-0022 Decision 8: the audit record's
`rw` field becomes `capability`, derived from the action directory. After s05, the ONLY
remaining live consumer of `browser/classify.rs` and `governance::ports::RwClass` is the
audit `rw` derivation; this task removes that last thread and deletes both (BOOTSTRAP
ground rule 13).

## Authority

ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) is normative; where this prompt
and the ADR disagree, THE ADR WINS (record the deviation in the ledger). This task implements
Decision 8 (the audit consumer) using the Decision 1 vocabulary and the Decision 2 directory.

## Depends on

- s05 (the schema-3 switch), and transitively s01-s04. STOP if s05 has not landed. Landing
  check: `src/governance/manifest/document.rs::Grant` has `hosts`/`allowed` fields (no
  `access`, no `domains`), and `src/governance/enforcement.rs::check_call` takes
  `requires: &[Capability]` (no `RwClass` parameter). If either fails, do not start.

## Current behavior (verify against the tree before editing)

This section describes the tree AS OF AFTER s05, written before s01-s05 executed: verify
every claim by reading the files; where s05 left latitude, this prompt pins the end state
regardless of which intermediate s05 chose.

- `src/governance/ports.rs`: `RwClass { Observe, Mutate }` (snake_case serde, `as_str`)
  survives only for the audit record. `AuditRecord` has `pub rw: RwClass` between `action`
  and `domain`; field order is part of the format (`serde_json` `preserve_order`).
  `DecisionRequest` no longer carries `rw` (s05). `DomainPolicy::classify` still returns
  `Option<RwClass>`; the trait has NO impl anywhere in the tree. Inline tests pin the record
  key order including `"rw"` (`record_serializes_all_fields_in_shared_format_order`), the
  session-event absent-field list including `"rw"`, and `RwClass` wire names
  (`rw_and_mode_wire_names_are_lowercase`).
- `src/browser/classify.rs`: `TOOL_CLASSES` (12 entries), `COMPUTER_ACTION_CLASSES` (13),
  and `classify()`, alive post-s05 solely for the audit `rw` value (s05 pinned this and
  added a module-doc line saying so). `navigate` is `Observe` after s01.
- `src/governance/dispatch.rs`: at stage 2 the public record functions (`record_call`,
  `record_deny`, `record_navigate_landing_deny`, `record_shadow_deny`, `record_held`) do
  NOT take a class parameter; the private `build_record` derives `rw` internally via
  `(self.classify)(tool, action).unwrap_or(RwClass::Mutate)` from a `classify` fn pointer
  held by `Governance`. s05 replaced `Governance`'s `classify` field with `requires`; the
  audit `rw` mechanism s05 left behind is EITHER a retained second classify fn pointer on
  `Governance` OR an rw value threaded from the server -- read `dispatch.rs` and
  `server.rs` to see which, then replace it per Required behavior 2.
- `src/transport/mcp/server.rs`: imports `classify` from `crate::browser`, performs a
  directory lookup once per call (s05), and holds every `record_*` call site (hold,
  pre-dispatch deny, decide deny, navigate landing deny, shadow deny, allow). Inline tests
  assert `rec["rw"]` values (`tools_call_produces_one_audit_record_with_client_identity`,
  `computer_call_records_action_and_observe_class`; both expected `"observe"` after s01).
- `tests/audit_recorder.rs`: wires `Governance` as production does, pins the 14-key record
  order including `"rw"`, and asserts `rec["rw"] == "mutate"` for `computer`/`left_click`.
- `tests/tool_enforcement.rs` and `tests/shadow_mode.rs`: read audit JSON lines and pin
  `decision`/`grant_id`/`denial_id`/`duration_ms`. NEITHER file pins an `"rw"` value
  (verified at stage 2); this task ADDS `capability` assertions there.
- `src/governance/simulate.rs`: s05 already switched simulate to the directory
  (`requires_fn`); replay lines' recorded `rw` was ALWAYS ignored (`evaluate_line` reads
  only `tool`/`action`/`domain`; its doc comment says so). Simulate needs NO functional
  change here. `tests/fixtures/simulate/audit.jsonl` carries rw-era records and
  deliberately stays that way (old audit files must remain replayable, ADR Decision 8).
- `src/browser/mod.rs`: declares `pub mod classify;`; its module doc names `classify` and
  `DomainPolicy::classify`.

## Required behavior

### 1. The record field (`src/governance/ports.rs`)

Replace `pub rw: RwClass,` on `AuditRecord` with `pub capability: &'static str,` in the SAME
position (between `action` and `domain`). Doc comment, exactly:

    /// The action's directory requirement rendered as one string (ADR-0022 Decision 8):
    /// the required set's single element's wire name for a singleton set, "none" for an
    /// empty set and for a directory miss. Exactly one of "read", "action", "write",
    /// "execute", "none". Replaces the rw field of shared format doc section 6.1, which
    /// ADR-0022 supersedes.

If `Capability` has no `pub fn as_str(&self) -> &'static str` returning exactly `"read"` /
`"action"` / `"write"` / `"execute"`, add one to its impl in `ports.rs` (mirror the deleted
`RwClass::as_str` shape).

### 2. Threading (`src/governance/dispatch.rs`)

The derivation lives in exactly one place: `Governance::build_record`. Every public record
function gains `requires: &[Capability]` immediately after `action` and threads it through:

    pub fn record_call(&self, tool: &str, action: Option<&str>, requires: &[Capability], duration_ms: u64, domain: Option<&str>, grant_id: Option<&str>)
    pub fn record_deny(&self, tool: &str, action: Option<&str>, requires: &[Capability], denial: &Denial, domain: Option<&str>)
    pub fn record_navigate_landing_deny(&self, action: Option<&str>, requires: &[Capability], denial: &Denial, domain: Option<&str>, duration_ms: u64)
    pub fn record_shadow_deny(&self, tool: &str, action: Option<&str>, requires: &[Capability], denial: &Denial, domain: Option<&str>, duration_ms: u64)
    pub fn record_held(&self, tool: &str, action: Option<&str>, requires: &[Capability])

`build_record` gains the same parameter and derives:

    let capability = requires.first().map(Capability::as_str).unwrap_or("none");

Remove whatever audit-side rw mechanism s05 left (a retained classify fn pointer on
`Governance`, or an `RwClass` parameter): after this task `Governance` holds exactly one
browser-supplied fn pointer, `requires` (used by `decide`). In the record functions' doc
comments, the old "a classification miss records RwClass::Mutate" paragraph becomes: "a
directory miss maps to an empty requires slice at the call site and records `\"none\"`; the
`decision` and denial-rule fields carry the deny story".

### 3. Server call sites (`src/transport/mcp/server.rs`)

The single per-call directory lookup the server already performs (s05) is the source: at
every `record_*` call site pass `<lookup>.unwrap_or(&[])`. If the post-s05 code performs the
lookup after the held-call early return, move it ahead of that return (it is a pure static
table scan, no I/O); the server performs at most one lookup per call. Remove `classify` from
the `use crate::browser::{...}` import and every remaining reference to it.

### 4. Deletion

- Delete `src/browser/classify.rs` entirely. Remove `pub mod classify;` from
  `src/browser/mod.rs` and rewrite the module-doc sentence that names it (name the s03
  directory module instead).
- Delete `RwClass` (enum and impl) from `src/governance/ports.rs`.
- Replace `DomainPolicy::classify` with
  `fn requires(&self, tool: &str, action: Option<&str>) -> Option<&'static [Capability]>;`
  and update its doc comment to cite ADR-0022 Decision 2 (the trait has no impl; this is a
  shape-only change).
- Run `rg -n "RwClass|classify" src/ tests/` and eliminate every remaining live reference:
  imports, fn-pointer types, test stubs (including `tests/all_open_golden.rs`'s
  `no_classification` helper if it still names `RwClass`; retype it to the requires shape,
  never change any golden expectation). Doc comments describing the observe/mutate model
  are reworded to name capabilities and the directory; update
  `src/governance/simulate.rs::evaluate_line`'s comment naming recorded `rw` to name
  `capability` and note that old rw-era lines replay identically. The only sanctioned
  `classify` survivors are the two `src/install/` prose comments about manifest-removal
  ownership ("classify a removal", "classifying by ownership"); leave them.

### 5. Simulate

No code change. Verify `run_simulate` consumes `requires_fn` (s05) and never reads any
recorded `rw`/`capability` value; leave it and its fixtures alone.

## Constraints

1. Do not touch `src/transport/mcp/schemas/tools.json`, `tests/tool_schema_fidelity.rs`,
   `tests/fixtures/simulate/audit.jsonl`, `examples/`, `src/governance/templates.rs`, or
   the explain goldens (the capability field never appears in tool or explain output).
2. `tests/architecture.rs` (`governance_core_has_no_forbidden_back_edges`) must pass: the
   derivation in `build_record` uses only the core `Capability` type; nothing under
   `src/governance/` names `crate::browser`.
3. All-open stays byte-identical in tool RESULTS: `tests/all_open_golden.rs` and
   `tests/mcp_protocol.rs` expectations are unchanged (compile-necessary helper retyping is
   allowed; expectation edits are not). Audit lines are the only observable change.
4. ASCII only; no new dependencies; delete what you replace fully -- zero `RwClass`
   references survive anywhere.

## Tests (minimum)

- `src/governance/dispatch.rs` inline -- update every `rec.rw` assertion to
  `rec.capability` with directory-true values (`computer`/`left_click` -> `"action"`,
  `computer`/`screenshot` -> `"read"`, `read_page` -> `"read"`); delete
  `classification_miss_records_mutate` (its premise, internal derivation, is gone); rename
  `computer_action_classification_flows_into_rw` to
  `computer_action_requires_flows_into_capability`. Add two NEW tests, exactly named:
  - `requires_empty_records_capability_none`: `record_call("tabs_create_mcp", None, &[], 0,
    None, None)` -> last record has `capability == "none"` and `decision == "allow"`.
  - `deny_record_carries_the_capability_of_the_denied_call`: `record_deny("javascript_tool",
    None, &[Capability::Execute], &denial, None)` -> `capability == "execute"`,
    `decision == "deny"` (use the variant identifiers s02 defined; expected `Read`,
    `Action`, `Write`, `Execute`).
- `src/governance/ports.rs` inline: key-order list `"rw"` -> `"capability"`; session-event
  absent-field lists `"rw"` -> `"capability"`; `sample_audit_record` sets
  `capability: "none"`; drop the `RwClass` assertions from
  `rw_and_mode_wire_names_are_lowercase` and rename it `mode_wire_names_are_lowercase`
  (keep the `EffectiveMode` assertions; keep any `Capability` wire-name test s02 added).
- `src/governance/audit/mod.rs` inline: `sample_record`'s third parameter becomes
  `capability: &'static str`; pass `"read"` at every existing call site.
- `src/transport/mcp/server.rs` inline: `rec["rw"]` assertions become `rec["capability"]`
  with `"read"` for both `navigate` and `computer`/`screenshot`; rename
  `computer_call_records_action_and_observe_class` to
  `computer_call_records_action_and_read_capability` and fix its doc comment.
- `tests/audit_recorder.rs`: key-order list `"rw"` -> `"capability"`;
  `rec["capability"] == "action"` for `computer`/`left_click`; pass requires at both
  `record_call` sites via the real directory (mirrors production), e.g.
  `browser_mcp::browser::directory::requires("computer", Some("left_click")).unwrap_or(&[])`;
  session-event absent-field list `"rw"` -> `"capability"`.
- `tests/tool_enforcement.rs`: in the test that reads both an allow and a deny audit line
  (stage-2 name `permitted_call_passes_and_denied_domain_is_denied_with_matching_audit`;
  locate by its `read_audit_lines` usage if s05 renamed it), assert
  `allow_line["capability"] == "read"` and `deny_line["capability"] == "read"` (both lines
  are `navigate`; if s05 changed the driven tool, use its ADR Decision 2 table value). Add
  one NEW integration test `requires_empty_call_records_capability_none`: schema-3 manifest
  with `"grants": []`, drive `initialize` plus one `tabs_create_mcp` call; the response is
  an ordinary execution failure (`isError == true`, text contains `not connected`), never a
  denial; exactly one audit line with `decision == "allow"`, `capability == "none"`,
  `grant_id` null, `held == false`.
- `tests/shadow_mode.rs`: assert `["capability"]` on BOTH the enforce deny line and the
  observe shadow_deny line; the value is the required capability of the call the post-s05
  test drives, read from the test's own `tools/call` and the ADR Decision 2 table
  (`form_input` -> `"write"`, `javascript_tool` -> `"execute"`, computer input actions ->
  `"action"`); both runs drive the same call, so both assertions pin the same string.

## Verification

- `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test` all green,
  including `tests/architecture.rs`, `tests/all_open_golden.rs`, `tests/mcp_protocol.rs`,
  and `tests/tool_schema_fidelity.rs` (the last two byte-untouched).
- `rg -n "RwClass" src/ tests/` -> no output.
- `rg -n "classify" src/ tests/` -> only the two `src/install/` prose hits.
- `rg -n "\"rw\"" src/ tests/ --glob '!tests/fixtures/**'` -> no output.
- `git diff --stat` shows no change to `src/transport/mcp/schemas/tools.json`,
  `tests/tool_schema_fidelity.rs`, `tests/fixtures/simulate/audit.jsonl`, or `examples/`.
- ASCII scan on every touched file: `rg -n "[^\x00-\x7F]" <files>` -> no output.
- Manual: `cargo run -- policy simulate tests/fixtures/simulate/manifest-restrictive.json
  --replay tests/fixtures/simulate/audit.jsonl` prints the exact s05-pinned report and exits
  2 -- proving rw-era audit files remain replayable after the field rename.

## Out of scope

- The `explain` directory tool and any tools.json or fidelity-test change (s07).
- Documentation sync: shared-format supersession banner, CLAUDE.md, SPEC updates list,
  BROWSER-TESTS entries (s08). Code doc comments are the only prose this task writes.
- Any change to enforcement, advertisement, explain, simulate, examples, templates (s05).
- Path rules, network-layer enforcement, capability qualifiers (ADR Future work).
