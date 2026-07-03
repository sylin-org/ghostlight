# G14: Tool advertisement filtering and observe-only mode

## Goal

Make `tools/list` advertise only the tools an active manifest could ever
permit. With no manifest, advertise the full 13 exactly as today (byte for
byte). With a manifest, filter the advertised set by the manifest's grants and
the read/write classification: a read-only manifest advertises only the
observe-class tools plus `computer`; a tool that no grant could ever permit is
not listed. The SCHEMA TEXT of every advertised tool stays byte-identical to
the sacred fixture; only which tools appear in the list changes. Per-call
enforcement stays authoritative; advertisement is a visibility optimization,
never a security boundary.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference.
  Use its field names, its file locations, and its section-8 classification
  table verbatim. This prompt cites it as "shared format section N".
- G05 (read/write classification table, shared format section 8) -- G05
  creates `src/policy/classify.rs` with
  `pub fn classify(tool: &str, action: Option<&str>) -> Option<RwClass>` and
  `pub enum RwClass { Observe, Mutate }`. G14 consumes that function. Do NOT
  re-hardcode the section-8 table in this task; call
  `crate::policy::classify::classify`. If `src/policy/classify.rs` does not
  exist yet, stop and land G05 first.
- The stage-2 manifest-loading and grant-resolution tasks -- whichever G-tasks
  parse the active manifest and expose its resolved grants (the `Grant` type
  with `id`, `domains`, `access`, `tools`, `exclude_tools`; shared format
  section 4.3) to the mcp-server role. G14 reads those grants; it does not
  parse the manifest itself. If that machinery is not yet threaded into
  `src/mcp/server.rs`, stop and land the prerequisite first; do not invent a
  manifest parser inside this task.

## Project context

Browser MCP is a governed browser automation tool. A single Rust binary is
BOTH the MCP server (JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the
Chrome native-messaging host; a thin Manifest V3 extension executes CDP
commands. The two binary roles are separate OS processes bridged by
tokio-native named-pipe / UDS IPC.

This is stage 2, the governance layer. Governance is a separable overlay
(ADR-0013): with no manifest and default config, behavior is byte-identical to
the all-open engine. Enforcement follows observe-then-enforce sequencing
(ADR-0018). Configuration is a typed key registry with layered precedence
(ADR-0019). The org policy experience (ADR-0020) adds shadow enforcement,
denial ids, and manifest identity in audit.

The tool schemas in `src/mcp/schemas/tools.json` are SACRED and byte-frozen.
They are guarded by `tests/tool_schema_fidelity.rs`. Tool ADVERTISEMENT
filtering (which tools appear in `tools/list`) is explicitly allowed here.
Tool schema TEXT (names, parameters, descriptions, enum values) never changes.

The 13 preserved tools, in fixture order (from `tests/tool_schema_fidelity.rs`
`EXPECTED`, line 11, and verified against `src/mcp/schemas/tools.json`):

```
tabs_context_mcp, tabs_create_mcp, navigate, computer, find, form_input,
get_page_text, javascript_tool, read_console_messages, read_network_requests,
read_page, resize_window, update_plan
```

Read/write classification (shared format section 8, authoritative; ADRs win
over SPEC):

| Tool | Class |
|---|---|
| `tabs_context_mcp` | observe |
| `tabs_create_mcp` | mutate |
| `navigate` | mutate |
| `computer` | split (per sub-action) |
| `find` | observe |
| `form_input` | mutate |
| `get_page_text` | observe |
| `javascript_tool` | mutate |
| `read_console_messages` | observe |
| `read_network_requests` | observe |
| `read_page` | observe |
| `resize_window` | mutate |
| `update_plan` | observe |

`computer` is "split": its observe sub-actions are `screenshot`, `scroll`,
`zoom`, `wait`, `hover`, `scroll_to`; its mutate sub-actions are `left_click`,
`right_click`, `double_click`, `triple_click`, `type`, `key`,
`left_click_drag`.

Grant `access` vocabulary (shared format section 4.3): `read` authorizes
observe-class calls; `write` authorizes mutate-class calls; `all` authorizes
both. `write` does NOT imply `read`.

## Current behavior

`src/mcp/server.rs`:

- `run` (line 22) builds `let config = Config::default();` (line 28), the
  built-in Minimal preset, and threads it through `handle_line`.
- `handle_line` (line 55) routes `"tools/list"` (line 87) to
  `tools_list_result()`.
- `tools_list_result()` (lines 112 to 114) returns
  `serde_json::from_str(TOOLS_JSON).expect("embedded tools.json is valid")`.
  It takes NO arguments and does NO filtering. The full 13-tool fixture is
  advertised unconditionally.

`src/mcp/tools.rs` line 15 embeds the fixture:
`pub const TOOLS_JSON: &str = include_str!("schemas/tools.json");`. The parsed
value is the object `{ "tools": [ ... 13 tool objects ... ] }`.

`src/main.rs` `run_server` (line 230) receives `manifest: Option<String>`
(the `--manifest` / env source) and calls `browser_mcp::mcp::server::run(browser)`
(line 254). The manifest source is logged but NOT yet parsed or threaded into
the server loop; wiring that in is the prerequisite manifest-loading task.

`src/dispatch.rs` `policy_check` (line 23) and `audit` (line 30) are documented
no-ops today. G14 does NOT touch dispatch or per-call enforcement.

Tests today:

- `tests/mcp_protocol.rs` `initialize_tools_list_and_tool_call_over_stdio`
  (line 49) drives the binary with no manifest and asserts `tools/list` returns
  13 tools (line 71) and that `list["result"]` equals the parsed fixture
  exactly (lines 74 to 78). This is the all-open byte-identity invariant. It
  must keep passing UNCHANGED.
- `tests/tool_schema_fidelity.rs` guards the fixture text. It must keep passing
  UNCHANGED.

## Required behavior

Advertisement is computed once at connection time from the active manifest's
resolved grants. It is domain-independent: the current tab URL is unknown at
`tools/list` time, so filtering is the union over ALL grants, never a
per-domain decision. Per-call enforcement (a later G-task at the dispatch
chokepoint) remains the authoritative check.

### 1. No manifest: full surface, byte-identical

When no manifest is active (shared format section 4.5, all-open; enforcement
STEP 0 short-circuit), `tools/list` returns the parsed fixture unchanged: all
13 tools, in fixture order, each tool object identical to the fixture. Do not
rebuild, reorder, or re-key any tool object. The existing byte-identity test
in `tests/mcp_protocol.rs` (lines 74 to 78) must continue to pass with no
edits.

### 2. Manifest active: filter membership by grants and class

When a manifest is active, iterate the fixture `tools` array IN FIXTURE ORDER
and keep a tool T if and only if some grant G in the manifest could ever
permit T. Never edit a retained tool object; only drop non-members. Build the
result by cloning retained tool objects verbatim from the parsed fixture and
wrapping them as `{ "tools": [ ... ] }`.

A grant G could permit tool T when BOTH of these hold:

1. Access-class match. Determine T's class with G05's function:
   - T is `computer`: special-case it BEFORE calling `classify` (that function
     needs a sub-action for `computer`, and advertisement has none). Because
     `computer` has both observe and mutate sub-actions, ANY access class
     (`read`, `write`, or `all`) permits at least one of its sub-actions, so
     the access-class test passes for every grant.
   - any other T: let C be `crate::policy::classify::classify(name, None)`.
     C is `Some(RwClass::Observe)`: G permits T's class when `G.access` is
     `read` or `all`. C is `Some(RwClass::Mutate)`: G permits T's class when
     `G.access` is `write` or `all`. C is `None`: unreachable for fixture
     tools while G05's exhaustiveness tests pass; do not panic, treat the tool
     as not advertised.
2. Tool-list match. Given `G.tools` and `G.exclude_tools` (mutually exclusive,
   shared format section 4.3):
   - if `G.tools` is a non-null array: T is permitted only when T's name is in
     `G.tools`;
   - else if `G.exclude_tools` is present: T is permitted only when T's name is
     NOT in `G.exclude_tools`;
   - else (both null or absent): T is permitted.

T is advertised when at least one grant satisfies both. A tool that every
grant denies (its class is unauthorized everywhere, or every grant that would
otherwise include it excludes it) is NOT listed.

Grant-level tool checks match the string `"computer"`, never a sub-action name
(shared format section 8, SPEC 5.4 retained). Sub-action-level enforcement is
NOT part of advertisement; it belongs to per-call enforcement.

### 3. The `computer` advertisement choice (document this)

`computer` stays advertised whenever any of its sub-actions is permitted by
some grant, that is, whenever any grant includes `computer` in its tool list
(a read grant covers its observe sub-actions; a write grant covers its mutate
sub-actions; `all` covers both). Advertisement is coarse: it lists the tool if
ANY use of it is reachable, and per-call enforcement then denies the specific
sub-actions the grant does not permit. Write a code comment on the
`computer`-handling branch of the filter stating this choice and pointing to
this task and shared format section 8. `computer` is only dropped when every
grant excludes it (via `exclude_tools`) or no grant lists it (via a `tools`
positive list that omits it, or an empty grants array).

### 4. Read-only manifest: exact expected set

For a manifest whose every grant has `access: "read"` and no `tools` /
`exclude_tools` restriction (so all tools of the read class are permitted), the
advertised set is exactly these 8 tools, in fixture order:

```
tabs_context_mcp, computer, find, get_page_text,
read_console_messages, read_network_requests, read_page, update_plan
```

`computer` is present per the section-3 choice. The 5 mutate-only tools
(`tabs_create_mcp`, `navigate`, `form_input`, `javascript_tool`,
`resize_window`) are absent because no grant authorizes the mutate class.

### 5. Empty grants array: nothing advertised

A manifest with `grants: []` permits nothing anywhere (shared format section
4.5), so no tool satisfies the membership test and the advertised `tools`
array is empty. This is the correct, deterministic outcome; handle it
explicitly rather than falling through to the full surface. (Absence of a
manifest is different: that is all-open, section 1.)

### 6. Wiring

- Create a new module `src/policy/advertise.rs` and declare it in
  `src/policy/mod.rs` with `pub mod advertise;` next to the existing module
  declarations. The filter lives in the policy layer, not in the server loop.
- The public function is
  `pub fn advertised_tools(grants: Option<&[Grant]>) -> serde_json::Value`,
  returning the `{ "tools": [...] }` object. `None` means no manifest and
  returns the full fixture verbatim (section 1); `Some(grants)` filters
  (section 2). `Grant` is the type the prerequisite manifest-loading task
  defined (fields `id`, `domains`, `access`, `tools`, `exclude_tools`,
  `description`, `mode`; shared format section 4.3); import it from that
  task's module, do not define a second grant type here. Parse
  `crate::mcp::tools::TOOLS_JSON` inside the function (or accept the parsed
  fixture) and clone retained tool objects unchanged.
- Change `src/mcp/server.rs` `tools_list_result` to take the active grants (or
  the active manifest) and delegate to `advertised_tools`. Thread the grants
  from wherever the prerequisite manifest-loading task resolved them (the same
  place `config` is resolved in `run`, line 28) down through `handle_line`
  (line 55) to the `"tools/list"` arm (line 87). Do NOT reach back into
  `src/main.rs` `manifest` string; consume the parsed / resolved form the
  manifest-loading task provides.
- Add doc comments on the new module and public function (project rule: doc
  comments on public items). The module doc comment states that advertisement
  is a domain-independent visibility optimization computed as the union over
  all grants, that per-call enforcement stays authoritative, and that schema
  text is never altered (this file, shared format section 8 last paragraph).

### 7. Truthfulness and invariants

- All-open stays first-class: with no manifest, output is byte-identical to
  today (section 1). This is non-negotiable.
- Advertisement never mutates schema text. If a tool is listed, its object is
  the fixture object unchanged.
- Advertisement is not protection: it only hides tools from the list. It does
  not, on its own, deny anything. Per-call enforcement is authoritative
  (unchanged by this task).
- Audit is unaffected by this task. Do not add or change audit records here.

## Constraints

- NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or
  descriptions. `tests/tool_schema_fidelity.rs` must pass UNCHANGED. Only list
  MEMBERSHIP may change; schema TEXT never does.
- The extension holds mechanism only: no policy, access, or classification
  decisions in extension JS. This task is entirely in the binary. Do not touch
  `extension/`.
- All-open (no manifest, default config) stays byte-identical to today (shared
  format section 4.5). Preserve and test this invariant.
- The engine is truthful: advertisement filtering hides tools from the list,
  it does not block them, and nothing may present it as protection. Do not add
  status text, log lines, or comments claiming hidden tools are "blocked" or
  "denied"; denial reporting belongs to the enforcement tasks.
- ASCII only in all code and docs: no em-dashes, arrows, or curly quotes,
  including comments.
- No new runtime dependencies. Use the already-present `serde_json`. Do not add
  crates for this task.
- Rust 2021 edition; `thiserror` for any library error types; doc comments on
  public items; `rustfmt` clean; `clippy` with deny warnings. Unit tests
  inline (`#[cfg(test)]`), integration tests under `tests/`.
- Do not touch `src/dispatch.rs`, per-call enforcement, or the audit
  subsystem. Advertisement is connection-time only.
- Do not copy code from other projects; implement from the behavior described
  here.

## Verification

- `cargo test` from the repo root is all green.
- `tests/tool_schema_fidelity.rs` passes UNCHANGED (schema text intact).
- `tests/mcp_protocol.rs` `initialize_tools_list_and_tool_call_over_stdio`
  passes UNCHANGED: no-manifest `tools/list` still returns 13 tools and
  `list["result"]` still equals the parsed fixture (lines 74 to 78).
- New tests (prefer pure unit tests on `advertised_tools`, no I/O):
  - No manifest (`None`): result equals the parsed fixture value exactly (all
    13 tools, fixture order).
  - Read-only manifest (one grant, `access: "read"`, `tools` and
    `exclude_tools` absent): advertised names equal exactly the 8-tool observe
    set of Required behavior section 4, in fixture order.
  - A tool denied everywhere is omitted: e.g. a grant `access: "all"` with
    `exclude_tools: ["javascript_tool"]` yields a list that contains the other
    tools but NOT `javascript_tool`; and a grant `access: "all"` with
    `tools: ["read_page"]` yields exactly `["read_page"]`.
  - Empty grants array yields an empty `tools` array.
  - `computer` is present under a read-only manifest and under a write-only
    manifest, and absent when every grant excludes it.
- Audit unaffected: `git diff --stat` shows NO change to `src/dispatch.rs`,
  `src/mcp/schemas/tools.json`, or anything under `extension/`. The changed
  files are `src/policy/mod.rs` (module declaration), the new
  `src/policy/advertise.rs`, `src/mcp/server.rs` (threading only), plus any
  new test code.
- `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  clean.
- No non-ASCII bytes in changed files (for example
  `rg -n "[^\x00-\x7F]" src/policy/advertise.rs src/mcp/server.rs`).
- If `target/debug/browser-mcp.exe` is locked by a running session, rename it
  aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`)
  and rebuild. Binary changes need an MCP client restart to observe manually;
  the automated tests above do not require one.

## Out of scope

- Changing any schema text in `src/mcp/schemas/tools.json`, any tool name,
  parameter, description, or enum value. Membership only.
- Dynamic re-advertisement mid-session (MCP `notifications/tools/list_changed`).
  The server loop in `src/mcp/server.rs` resolves configuration and the
  manifest once at startup (line 28), advertises statically, and never emits
  server-initiated notifications; there is no mid-session manifest reload, so
  dynamic re-advertisement is NOT trivially supported. Note it as a follow-up
  in a code comment on `advertised_tools`; do not build it here. In
  particular, do NOT add `"listChanged": true` to the `capabilities.tools`
  object in `initialize_result` (`src/mcp/server.rs` line 105): advertising a
  capability the server does not have would violate the truthfulness rule.
- Per-call enforcement, denials, denial ids, shadow-deny, and audit records.
  Those live at the dispatch chokepoint in other G-tasks. Advertisement does
  not deny; it only shapes the list.
- Sub-action-level advertisement (hiding individual `computer` actions). The
  `computer` schema, including its 13-action enum, is sacred and unchanged;
  sub-action limits are enforced per call, not advertised.
- Domain-specific advertisement. Advertisement is the domain-independent union
  over grants; the current tab URL is not consulted at `tools/list` time.
- Manifest parsing, grant resolution, domain matching, and config layering.
  Those are prerequisite G-tasks; G14 consumes their outputs.
