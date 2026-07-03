# G05: Read/write classification table

## Goal

Implement the pure, table-driven read/write classification function described in
`docs/tasks/stage-2/00-shared-format.md` section 8: every one of the 13 MCP tools, and
every one of the 13 `computer` sub-actions, maps to exactly one class, `observe` or
`mutate`. The mapping is exhaustive by construction, guarded by test-time checks against
the sacred schema fixture, and its exact contents are pinned by unit tests. Nothing is
enforced, blocked, or logged by this task; it produces the classification primitive that
later stage-2 tasks (audit wiring in G06, grant enforcement, tool advertisement
filtering) consume.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` (sections 3.4 semantics note, 6.1 `rw` field,
  8). Read it before writing any code; its names are authoritative.
- All release-1 (stage-1) tasks in `docs/tasks/release-1/` are assumed landed. No other
  stage-2 task is a prerequisite; G06 (audit wiring) depends on THIS task.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance layer per
ADR-0013 (separable overlay; all-open stays first-class), ADR-0018 (observe-then-enforce
sequencing), ADR-0019 (layered configuration and typed key registry), and ADR-0020 (org
policy experience). This task is the ADR-0018 step 1 prerequisite: before any audit
record can carry an `rw` field and before any grant can distinguish read from write
access, the binary needs one authoritative answer to "does this call observe or mutate?".

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc `docs/tasks/stage-2/00-shared-format.md` is the reconciled single source for
formats and names. Concretely for this task: SPEC sections 3.1, 3.3, and 5.4 contain an
OLDER classification (a three-tier Observe/Mutate/Manage model, and "scroll is mutate
because it dispatches input"). That text is superseded. The table in the shared format
doc section 8 is the only correct classification. Do not consult the SPEC for class
assignments; consult only section 8 of the shared format doc and the table reproduced
verbatim in Required behavior below.

Key files for this task:

- `src/policy/mod.rs` -- the governance module. Currently declares `pub mod redact;` and
  holds the typed key registry seed (`KeyDef`, `KEYS`, `Config`). Your new module is
  declared here.
- `src/policy/redact.rs` -- existing overlay example (style reference only; do not
  modify).
- `src/mcp/schemas/tools.json` -- the SACRED tool schema fixture, embedded via
  `pub const TOOLS_JSON: &str = include_str!("schemas/tools.json");` in
  `src/mcp/tools.rs`. Read-only forever. Your tests parse it to prove exhaustiveness.
- `tests/tool_schema_fidelity.rs` -- the existing guard for the sacred surface. It pins
  the 13 tool names in order and the 13-value `computer.action` enum in official order.
  It must pass unchanged.
- `src/dispatch.rs` -- the single dispatch chokepoint. `policy_check` and `audit` are
  documented no-ops today. You do NOT touch this file in this task; G06 wires
  classification into it.

## Current behavior

- No read/write classification CODE exists anywhere in the codebase. Grep for `RwClass`
  under `src/` finds nothing. Grep for `Observe` or `Mutate` finds only module doc
  comments in `src/tools/*.rs` that describe the OLD SPEC tier model (for example
  `src/tools/navigate.rs` line 1 calls navigate "Observe tier", and
  `src/tools/computer.rs` line 6 says "Observe = screenshot, wait, zoom"). Those
  comments are stale relative to the table below; leave them alone (see Out of scope).
- `src/policy/` contains exactly two files: `mod.rs` (key registry seed:
  `KeyDef { key, description, minimal_default }`, the `KEYS` table with one entry for
  `content.security.secrets.redact`, and `Config` with `Config::minimal()`) and
  `redact.rs` (the read_page secret-value redaction overlay).
- `src/mcp/server.rs` calls the no-op seams at lines 132-133 of `handle_tools_call`:
  `let _decision = dispatch::policy_check(name);` then `dispatch::audit(name);`. Every
  call is allowed; nothing is classified.
- `src/mcp/schemas/tools.json` advertises exactly these 13 tools, in this order (pinned
  by `tests/tool_schema_fidelity.rs`): `tabs_context_mcp`, `tabs_create_mcp`,
  `navigate`, `computer`, `find`, `form_input`, `get_page_text`, `javascript_tool`,
  `read_console_messages`, `read_network_requests`, `read_page`, `resize_window`,
  `update_plan`. The `computer` tool's `action` enum (tools.json line 59) has exactly
  these 13 values, in this order: `left_click`, `right_click`, `type`, `screenshot`,
  `wait`, `scroll`, `key`, `left_click_drag`, `double_click`, `triple_click`, `zoom`,
  `scroll_to`, `hover`.

## Required behavior

### 1. New module `src/policy/classify.rs`

Create `src/policy/classify.rs` and declare it in `src/policy/mod.rs` by adding
`pub mod classify;` on the line directly after the existing `pub mod redact;`. That
one-line addition is the ONLY change to `mod.rs`.

The module is pure: no I/O, no allocation beyond what slice iteration needs, no
dependencies beyond `core`/`std`. Module-level doc comment explains that this is the
authoritative read/write classification of the sacred tool surface (shared format doc
section 8, ADR-0018 step 1), that it supersedes SPEC 3.1/3.3/5.4, and that audit
(`rw` field) and grant enforcement (`access: read | write | all`) both consume it.

### 2. The `RwClass` type

```rust
/// Read/write class of a tool call: does it observe page/browser state or mutate it?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RwClass {
    /// Reads or reveals state without committing input that changes application state.
    Observe,
    /// Changes page, application, or browser state.
    Mutate,
}
```

Add a method `pub fn as_str(&self) -> &'static str` returning exactly `"observe"` for
`Observe` and `"mutate"` for `Mutate`. These strings are the audit `rw` field vocabulary
(shared format doc section 6.1); the doc comment must say so.

### 3. The classification tables

Two public `const` tables of `(&'static str, RwClass)` pairs, each with a doc comment:

`pub const TOOL_CLASSES: &[(&str, RwClass)]` -- one entry per tool EXCEPT `computer`
(12 entries). `computer` is deliberately absent: it is classified per sub-action, and a
test asserts its absence. Author the entries in tools.json advertised order:

| Tool | Class |
|---|---|
| `tabs_context_mcp` | Observe |
| `tabs_create_mcp` | Mutate |
| `navigate` | Mutate |
| `find` | Observe |
| `form_input` | Mutate |
| `get_page_text` | Observe |
| `javascript_tool` | Mutate |
| `read_console_messages` | Observe |
| `read_network_requests` | Observe |
| `read_page` | Observe |
| `resize_window` | Mutate |
| `update_plan` | Observe |

`pub const COMPUTER_ACTION_CLASSES: &[(&str, RwClass)]` -- one entry per `computer`
sub-action (13 entries), authored in the tools.json enum order:

| Action | Class |
|---|---|
| `left_click` | Mutate |
| `right_click` | Mutate |
| `type` | Mutate |
| `screenshot` | Observe |
| `wait` | Observe |
| `scroll` | Observe |
| `key` | Mutate |
| `left_click_drag` | Mutate |
| `double_click` | Mutate |
| `triple_click` | Mutate |
| `zoom` | Observe |
| `scroll_to` | Observe |
| `hover` | Observe |

Summary check (must match shared format doc section 8 exactly): observe actions are
`screenshot`, `scroll`, `zoom`, `wait`, `hover`, `scroll_to` (6); mutate actions are
`left_click`, `right_click`, `double_click`, `triple_click`, `type`, `key`,
`left_click_drag` (7).

Include the section-8 rationale as a comment near `COMPUTER_ACTION_CLASSES`: the observe
set reads or reveals page state without committing input that changes application state;
`scroll`, `hover`, and `scroll_to` dispatch input events but only move the viewport or
pointer, and a read-only grant that cannot scroll cannot read a page below the fold,
which would make read access useless in practice. This deliberately supersedes SPEC 3.3.

### 4. The classification function

```rust
/// Classify one tool call. `action` is consulted only when `tool` is `"computer"`;
/// for every other tool it is ignored. Returns `None` for a tool name not on the
/// sacred surface, and for a `computer` call whose action is absent or unknown.
pub fn classify(tool: &str, action: Option<&str>) -> Option<RwClass>
```

Exact behavior:

- `tool == "computer"`: look up `action` in `COMPUTER_ACTION_CLASSES`. `Some(class)` on
  a hit; `None` when `action` is `None` or not in the table.
- any other `tool`: look up `tool` in `TOOL_CLASSES`. `Some(class)` on a hit; `None`
  otherwise. The `action` argument is ignored for non-computer tools (a caller passing
  `classify("read_page", Some("left_click"))` gets `Some(RwClass::Observe)`).
- Lookup is a linear scan over the const slices (13 entries maximum; no HashMap, no
  lazy statics, no new dependencies).
- The function makes no policy decision. `None` is a classification miss, not a denial;
  what callers do with `None` is decided by the consuming tasks (G06 and later), not
  here.

Note for future consumers (as a doc comment, not code): grant-level `tools` /
`exclude_tools` checks match the literal tool name `"computer"`, never an action name
(shared format doc section 4.3); this function is for the observe/mutate axis only.

### 5. Exhaustiveness guarantee (test-time)

The compile-time source of truth for the tool surface is the sacred fixture, which must
never be edited, so exhaustiveness is proven at test time by parsing the embedded
fixture. Inline unit tests (`#[cfg(test)] mod tests` in `classify.rs`) use
`crate::mcp::tools::TOOLS_JSON` and `serde_json` to extract:

- the set of advertised tool names (`tools[*].name`), and
- the `computer.action` enum values
  (`tools[name=="computer"].inputSchema.properties.action.enum`).

Required tests, by name and assertion:

1. `tool_table_matches_the_sacred_surface`: the set of names in `TOOL_CLASSES` equals
   the set of tool names in tools.json MINUS `"computer"`. Both directions: every
   advertised tool except `computer` is classified (no gaps), and every classified name
   is advertised (no stale entries). Also assert `TOOL_CLASSES` does NOT contain
   `"computer"` and contains no duplicate names.
2. `computer_action_table_matches_the_sacred_enum`: the set of names in
   `COMPUTER_ACTION_CLASSES` equals the tools.json `computer.action` enum value set,
   both directions, with no duplicates, and `COMPUTER_ACTION_CLASSES.len() == 13`.
3. `classification_matches_the_shared_format_table`: assert the exact class for every
   entry, spelled out call by call (this is the pin against silent table edits):
   - `classify("tabs_context_mcp", None) == Some(RwClass::Observe)`
   - `classify("tabs_create_mcp", None) == Some(RwClass::Mutate)`
   - `classify("navigate", None) == Some(RwClass::Mutate)`
   - `classify("find", None) == Some(RwClass::Observe)`
   - `classify("form_input", None) == Some(RwClass::Mutate)`
   - `classify("get_page_text", None) == Some(RwClass::Observe)`
   - `classify("javascript_tool", None) == Some(RwClass::Mutate)`
   - `classify("read_console_messages", None) == Some(RwClass::Observe)`
   - `classify("read_network_requests", None) == Some(RwClass::Observe)`
   - `classify("read_page", None) == Some(RwClass::Observe)`
   - `classify("resize_window", None) == Some(RwClass::Mutate)`
   - `classify("update_plan", None) == Some(RwClass::Observe)`
   - `classify("computer", Some("left_click")) == Some(RwClass::Mutate)`
   - `classify("computer", Some("right_click")) == Some(RwClass::Mutate)`
   - `classify("computer", Some("type")) == Some(RwClass::Mutate)`
   - `classify("computer", Some("screenshot")) == Some(RwClass::Observe)`
   - `classify("computer", Some("wait")) == Some(RwClass::Observe)`
   - `classify("computer", Some("scroll")) == Some(RwClass::Observe)`
   - `classify("computer", Some("key")) == Some(RwClass::Mutate)`
   - `classify("computer", Some("left_click_drag")) == Some(RwClass::Mutate)`
   - `classify("computer", Some("double_click")) == Some(RwClass::Mutate)`
   - `classify("computer", Some("triple_click")) == Some(RwClass::Mutate)`
   - `classify("computer", Some("zoom")) == Some(RwClass::Observe)`
   - `classify("computer", Some("scroll_to")) == Some(RwClass::Observe)`
   - `classify("computer", Some("hover")) == Some(RwClass::Observe)`
4. `unclassified_inputs_return_none`:
   - `classify("no_such_tool", None) == None`
   - `classify("computer", None) == None`
   - `classify("computer", Some("no_such_action")) == None`
   - `classify("read_page", Some("left_click")) == Some(RwClass::Observe)` (action
     ignored for non-computer tools)
5. `rw_class_strings_match_the_audit_vocabulary`:
   - `RwClass::Observe.as_str() == "observe"`
   - `RwClass::Mutate.as_str() == "mutate"`

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or descriptions.
   `tests/tool_schema_fidelity.rs` must pass unchanged. This task does not touch tool
   advertisement at all.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task changes no extension file.
3. All-open stays first-class: with no manifest and default config, behavior is
   byte-identical to today. This task guarantees that trivially by not touching
   `src/dispatch.rs`, `src/mcp/server.rs`, or any tool code: only `src/policy/mod.rs`
   (one added line) and the new `src/policy/classify.rs` change.
4. ASCII only in ALL code, comments, and docs: no em-dashes, arrows, or curly quotes.
5. The engine is truthful: this task adds no user-facing messaging, so nothing to
   misreport; do not add speculative denial or logging text.
6. No new runtime dependencies. `serde_json` (already in Cargo.toml, with
   `preserve_order`) is used only in tests. Do not add `phf`, `once_cell`,
   `lazy_static`, `strum`, or any map crate; const slices with linear scan suffice.
7. Rust 2021 edition; doc comments on every public item (module, enum, variants,
   method, both consts, function); `cargo fmt` clean; `cargo clippy --all-targets
   -- -D warnings` clean. Unit tests inline in `classify.rs`; no new integration test
   file is needed for this task.
8. Do NOT copy code from other projects; implement from the behavior described here.
9. Use the shared format doc's names and vocabulary: classes are `observe` and `mutate`
   (never `read`/`write`, which is the GRANT access vocabulary of section 4.3, out of
   scope here); the audit field this feeds is `rw`.
10. The SPEC's older classification (SPEC 3.1, 3.3, 5.4) is superseded; do not "fix" the
    table toward the SPEC. If the table here and shared format doc section 8 ever
    disagree, stop and report; do not guess.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including:
   - all five new tests in `src/policy/classify.rs` listed above;
   - `tests/tool_schema_fidelity.rs` unchanged and passing;
   - every pre-existing test unchanged and passing.
4. `git status` / `git diff --stat` shows changes ONLY to `src/policy/mod.rs` (exactly
   one added line: `pub mod classify;`) and the new file `src/policy/classify.rs`.
   `src/mcp/schemas/tools.json` shows no diff.
5. Grep the new file for non-ASCII bytes (for example
   `rg -n "[^\x00-\x7F]" src/policy/classify.rs`); there must be none.

Build note: if `target/debug/browser-mcp.exe` is locked by a running MCP session, rename
it aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and
rebuild. No extension reload or MCP client restart is needed for this task since no
runtime behavior changes.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Enforcement of any kind. Nothing is blocked, denied, or held by this task.
  `PolicyDecision` in `src/dispatch.rs` keeps its single `Allow` variant; do not add
  `Deny`, do not edit `policy_check` or `audit`, do not edit `src/dispatch.rs` at all.
- Audit wiring. G06 consumes `classify` and `RwClass::as_str` to populate the audit
  record's `rw` field; do not create any audit record, file, or destination here.
- Grant machinery: no `access: read | write | all` mapping, no manifest parsing, no
  domain matching, no grant resolution. The read/write-to-observe/mutate enforcement
  mapping (shared format doc section 8, last paragraph) belongs to the enforcement task.
- Tool advertisement filtering. `tools_list_result` in `src/mcp/server.rs` stays
  untouched; the full surface remains advertised unconditionally.
- Config registry changes: no new `KeyDef` entries, no `Config` fields, no `KeyValue` /
  `KeyConstraint` types (those belong to the registry growth task). `classify` is not
  configurable; the table is fixed.
- Threading `classify` through `handle_tools_call` or extracting the `computer` action
  from call arguments in the server. Argument parsing at the chokepoint is G06's job.
- Any change under `extension/`, `tests/`, `docs/` (including the SPEC; the SPEC
  amendment for the superseded classification is tracked in the shared format doc's
  "SPEC updates needed" list, item 1, and is a separate docs task).
- "Fixing" the stale tier mentions in `src/tools/*.rs` module doc comments (navigate
  "Observe tier", computer "Observe = screenshot, wait, zoom", the table in
  `src/tools/mod.rs`, and similar). They describe the superseded SPEC model and are
  wrong relative to this table, but updating them belongs to the docs cleanup task;
  touching them here would break the two-file diff check in Verification.
- Any `Display`, `Serialize`, `FromStr`, or `TryFrom` impl for `RwClass`. `as_str` is
  the only conversion until a consumer proves the need.
