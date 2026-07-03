# S07: the `explain` directory tool (the ONLY sanctioned tools.json change)

## Goal

Add one new tool, `explain`, to the advertised surface (ADR-0022 Decision 7). It takes no
arguments, requires no capabilities (always allowed, always advertised, under any manifest
and under all-open), never touches the extension, and returns the action directory: the
capability vocabulary plus, for every action, its required capability and description. It is
the map that lets an untrained model learn this surface. This is the single sanctioned
amendment to the sacred tool surface; `tests/tool_schema_fidelity.rs` is amended in this
task, and ONLY this task, to pin the new 13-plus-1 invariant.

## Authority

ADR-0022 Decision 7 is normative, including the relaxation it defines: the 13 trained tool
schemas stay byte-identical forever; exactly one additive, argument-less governance tool is
sanctioned on top; nothing else.

## Depends on

s05 and s06 landed (directory-driven enforcement/advertisement; `classify.rs`/`RwClass`
gone). If not, STOP.

## Current behavior (verify against the tree before editing)

- `src/transport/mcp/schemas/tools.json` holds exactly 13 tool objects;
  `tests/tool_schema_fidelity.rs` pins them (count, order, names, schema shapes).
- `src/browser/directory.rs` holds 25 rows (12 tools + 13 computer actions).
- `src/transport/mcp/tools.rs` (`TOOLS_JSON`, `is_known_tool`) derives from the fixture;
  `transport/mcp/server.rs::handle_tools_call` rejects names failing `is_known_tool` before
  dispatch, then runs hold/sacred/governance checks, then forwards to the extension.
- `tests/all_open_golden.rs::tools_list_is_byte_stable_through_the_move` and
  `tests/mcp_protocol.rs` pin the advertised list's current shape; check exactly what each
  asserts before editing anything.

## Required behavior

### 1. The fixture entry

Append ONE object to the END of the `tools` array in
`src/transport/mcp/schemas/tools.json` (after `computer` or whatever the current last entry
is; the first 13 entries' bytes, order included, must not change):

- `name`: `explain`
- `description` (exact text):
  `Returns this server's action directory: every available action, the capability it requires (read, action, write, or execute; some require none), and a short description of what it does, plus definitions of the capability vocabulary. Use it to learn what you are allowed to do in this session. It does not read, summarize, or explain web pages.`
- `inputSchema`: byte-for-byte the same shape as `tabs_create_mcp`'s inputSchema (the
  house style for a no-argument tool), formatted with the file's existing indentation.

### 2. The directory row (`src/browser/directory.rs`)

Add row 26: tool `explain`, no action, `requires: &[]`, description (curt, agent-targeted):
`Show every action available here and the capability each one requires.` Update the
directory's fixture-mirror tests (they must now expect 13 tools in the table's tool half,
`explain` included, and 26 rows total).

### 3. The handler (`src/transport/mcp/server.rs`)

`explain` is handled server-side, before any extension dispatch, positioned exactly like the
other pre-dispatch outcomes: the hold check (g10) and kill state behave for `explain` as for
any tool; the sacred check does not apply (no tabId, no URL argument); governance allows it
by `requires: []`. The response is a single text content block, deterministic ASCII, built
from the directory in fixture order (computer's 13 actions in enum order, each on its own
line), with this exact layout:

    Capabilities: read = retrieve and observe only; action = dispatch UI input whose effect
    the page decides (this can trigger writes); write = declared state-changing operations;
    execute = arbitrary code.

    {tool}: requires {capability or nothing}. {description}
    computer ({action}): requires {capability or nothing}. {description}

One blank line between the vocabulary block and the listing; one line per action; requires
renders the single capability name or the word `nothing`. Pin the full expected output in a
unit test (build it from the directory constants, then assert the handler returns exactly
that string).

An `explain` call is audited like any tool call: decision `allow`, `capability: "none"`,
`domain` null, real `duration_ms`.

### 4. The fidelity amendment (`tests/tool_schema_fidelity.rs`)

Amend ONCE, in this task only:

- The existing assertions over the 13 trained tools stay logically unchanged (byte-identical
  schemas, order, names). Any assertion of the form "exactly 13" becomes "exactly 14, the
  14th named explain, positioned last".
- New assertions: `explain`'s object matches section 1 exactly (name, the pinned
  description string, the no-argument inputSchema shape); no other tool was added.
- Add a comment at the amendment site citing ADR-0022 Decision 7 and stating the invariant:
  13 trained entries byte-identical + exactly one sanctioned addition; any further change to
  this file is unsanctioned.

### 5. Other guard tests

`tests/all_open_golden.rs` and `tests/mcp_protocol.rs`: run them FIRST; where an expectation
pins the tools/list bytes or count, update that single expectation to include `explain`
(this is the one sanctioned advertised-surface change; document each touched expectation in
the ledger entry as a deliberate, sanctioned edit). Do not restructure either test.
`is_known_tool` must now accept `explain` (it derives from the fixture; verify with its
existing unit tests, extending them to name `explain`). Advertisement needs no code change
(requires-empty variants are always kept, s05); extend the advertisement tests to assert
`explain` is present under all-open, under a read-only manifest, and under an empty-grants
manifest.

## Constraints

1. The first 13 fixture entries are byte-identical before and after: verify with
   `git diff src/transport/mcp/schemas/tools.json` showing ONLY an addition at the array
   tail (plus, if unavoidable, the preceding entry's trailing comma).
2. No extension file changes; `explain` never produces a native-messaging frame.
3. `tests/architecture.rs` passes (the handler lives in transport; the directory row in
   browser; nothing new in governance).
4. ASCII only; no new dependencies; one commit.

## Tests (minimum)

Fixture: amended fidelity suite green. Directory: 26 rows, explain requires-empty, mirror
tests green. Handler: exact pinned output; audited as allow/none; works with no manifest,
with an empty-grants manifest, and with a read-only manifest (integration test via the
`CARGO_BIN_EXE_browser-mcp` stdio pattern of `tests/mcp_protocol.rs`: initialize, tools/list
contains explain last, tools/call explain returns the pinned text without an extension
attached). Hold: while held, an explain call gets the ordinary pause text (extend an
existing hold unit test).

## Verification

`cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test` all green;
the git diff shows tools.json changed only at the tail; ASCII scan clean. Append one
BROWSER-TESTS.md entry: with a live client, confirm `explain` appears in the client's tool
list and returns the directory text, and that trained-Claude sessions do not spuriously call
it for page questions (observe a normal browsing session; note any spurious call as a rename
signal per ADR Decision 7).

## Out of scope

- Renaming the tool (the ADR records `explain` as decided; a rename is a future one-line
  decision if spurious invocation shows up live).
- MCP resources, structured/JSON output modes, arguments or filters on `explain`.
- Any other tools.json change, ever.
- Documentation outside code and tests (s08).
