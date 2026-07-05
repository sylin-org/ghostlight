# H7: Tab-group-per-session presentation

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 6;
> the policy-free-extension invariant restated in Decision 1's topology and in the "Preserved
> invariants" section; Migration line H7). One task = one commit. Facts below are as-of-authoring
> 2026-07-04 -- RE-READ the named files before relying on any line number.

## Goal

Using H4's per-session owned-tab set, the persistent SERVICE asks the policy-free EXTENSION to
place a session's owned tabs into a per-session Chrome tab group. The extension owns the durable
tab-group state and DECIDES nothing: it groups exactly the tabIds the service names, on request
only, with no domain/host/grant inspection. This lands ONE additive native-messaging message type
(a group request, binary -> extension); it must not alter the sacred tool wire or any existing
message shape. Same adapter process (same GUID, per ADR-0030 Decision 4 / Decision 7) reuses its
group. Why: ADR-0030 Decision 6 keeps cross-session isolation authoritative in the service and the
extension's per-group checks "defense-in-depth only"; ADR-0030 Migration line H7 is
"Tab-group-per-session presentation (extension owns the durable group; groups on request only)."

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 6; policy-free-extension invariant) is
   NORMATIVE. Cite it; never restate its semantics.
2. BOOTSTRAP.md ground rules.
3. This task file.
4. CLAUDE.md and docs/adr/0005-policy-free-extension.md pin the policy-free-extension invariant.
   If any two of the above conflict, the higher wins.

## Current-tree facts (as-of-authoring; RE-READ before relying)

- CORRECTED 2026-07-04 (PINS.md SS9; RE-READ it in full): `src/hub/` (`mod.rs`, `session.rs`,
  `handshake.rs`) is created earlier in this batch. `src/hub/session.rs` (H3) holds the PURE
  identity types (`SessionGuid`/`PeerCred`/`SessionRegistry`) ONLY -- it does NOT hold a per-session
  record or the owned-tab set. H4's binary-authoritative owned-tab tracking (`tabs_create_mcp`-
  created or legitimately adopted tabIds, keyed on the adapter-minted GUID) is a SHARED field on
  `ServiceContext` (`src/hub/mod.rs`): `owned_tabs: Arc<std::sync::Mutex<HashMap<i64, SessionGuid>>>`
  (tabId -> owning GUID). RE-READ `src/hub/mod.rs`'s `ServiceContext` before touching it; if
  `owned_tabs` is absent, this task's STOP precondition fires.
- `extension/service-worker.js` (RE-READ; ~1264 lines as-of-authoring) is the policy-free CDP
  executor + native endpoint + tab-group manager. It currently manages a SINGLE group:
  - `const GROUP_TITLE = "\u{1F47B}Ghostlight";` (line ~29; a ghost emoji written as an ASCII
    escape followed by the brand).
  - `let groupId = null;` (line ~32): one process-global group id.
  - `ensureGroup(create)` (line ~490), `groupTabs()` (~514), `inGroup(tabId)` (~517),
    `persistSessionState()` (~477), `rehydrate()` (~536): the single-group lifecycle + durable
    recovery via `chrome.storage.session`.
  - The `nativePort.onMessage.addListener` handler (line ~75) branches on `msg.type`:
    `tool_request`, `tab_url_request` (the mechanism-only tab-URL query, ~87), `hold_state`,
    `hold_error`. This is where an additive `group_request` branch belongs, alongside
    `tab_url_request` (which is the closest existing precedent: a binary -> extension request
    carrying a `tabId`, answered with mechanism only, no policy).
  - `handlers.tabs_create_mcp` (line ~1063) creates a tab and groups it into the single group.
- `src/transport/native/messages.rs` (RE-READ; ~91 lines as-of-authoring) is REFERENCE
  DOCUMENTATION ONLY (its header says the native-host relays objects verbatim and only the
  mcp-server constructs/parses them; nothing is modeled as a type here). It documents every wire
  message: `tool_request`/`tool_response`/`tool_error`; the hold vocabulary
  `get_hold`/`set_hold`/`toggle_hold` -> `hold_state`/`hold_error`; the `session_killed` event; and
  the "Tab-URL query (g13)" section `tab_url_request` -> `tab_url_response`. The additive
  group-request wire section is documented here, mirroring the "Tab-URL query (g13)" section's
  style. NO existing section changes.
- Coupling that pins scope: the SEND path from the hub to the extension is H2's plumbing. As-of-
  authoring the only code that frames and posts native messages to the extension is
  `src/transport/executor.rs` (`Browser::tab_url` uses `send_and_await`; `Browser` holds the one
  extension link). CORRECTED 2026-07-04 (PINS.md SS9): the code that reacts to "a session's
  owned-tab set changed" runs wherever H4's ownership-gate/adoption logic runs --
  `serve_session`'s read loop (`src/transport/mcp/server.rs`), NOT `src/hub/session.rs` (which
  holds only H3's pure identity types). That dispatch code reaches the extension THROUGH the shared
  `Browser` handle carried on `ServiceContext`. H7 does NOT build native-send transport; it emits
  the group request through that existing seam, from wherever H4's owned-tab update actually lands.
  If the seam is absent, STOP (see STOP preconditions).
- Extension tests are Node `node:test` files that `require` a PURE `extension/lib/*.js` module
  (see `tests/extension/geometry.test.js` requiring `extension/lib/geometry.js`).
  `service-worker.js` is NOT a pure module (it calls `importScripts` and registers chrome
  listeners at load), so the grouping DECISION must be factored into a pure, chrome-injected
  helper module the worker imports, exactly as `constants.js` / `geometry.js` / `keys.js` were
  extracted, so `grouping.test.js` can unit-test it with a fake chrome.

## Required behavior

Mandated by ADR-0030 Decision 6 ("cross-session isolation is authoritative in the SERVICE ... The
extension's per-group checks remain defense-in-depth only") and the policy-free-extension
invariant (ADR-0030 Decision 1 topology: "MV3 extension (POLICY-FREE; owns all durable browser
state: tabs, tab GROUPS, ...)"; CLAUDE.md; ADR-0005):

1. ONE additive native-messaging message type, binary -> extension, carrying the session's opaque
   GUID and the exact set of owned tabIds to group. It is ADDITIVE ONLY: it adds a new `type`
   value and its own branch; it changes NO existing message shape and touches NOTHING on the
   sacred MCP JSON-RPC wire (`tool_request`/`tool_response`/`tool_error` stay byte-identical). It
   is out of band from tool dispatch, exactly like `tab_url_request` and the hold vocabulary.
   - EXACT `type` string, field names, and reply: PINNED in docs/tasks/hub/PINS.md SS6. The
     message type is `"group_request"` with fields
     `{ "type": "group_request", "guid": <session guid>, "tabIds": [<i64>...], "title": <string> }`;
     the extension replies `{ "type": "group_response", "guid": <guid>, "ok": <bool> }`.
   - The GUID is secret material (ADR-0030 Decision 4): it MUST NOT be written to any log/audit
     sink from this path. Do not add logging of the GUID.

2. `extension/service-worker.js`: add the `group_request` branch to the
   `nativePort.onMessage.addListener` handler, alongside `tab_url_request`. On receipt the
   extension places EXACTLY the named tabIds into that session's group and returns/echoes nothing
   beyond what item 1 pins. Mechanism only:
   - The extension MUST NOT inspect any tab's url/host/domain/grant to decide membership. It groups
     the tabIds the service named; it filters none, adds none, drops none for any policy reason.
     (A named tab that no longer exists is a best-effort no-op, swallowed like the existing
     `chrome.tabs.get` failure paths; that is a liveness fact, not a policy decision.)
   - Per-session group state: replace the single process-global `groupId` model with a
     session-GUID -> groupId map so two GUIDs yield two groups (ADR-0030 Decision 7: "two adapters
     in one editor -> two GUIDs -> two groups"). Same GUID reuses its existing group
     (idempotent). Durable recovery (`persistSessionState`/`rehydrate` via
     `chrome.storage.session`) MUST be preserved for the per-session map.
   - The per-session group TITLE format: PINNED in docs/tasks/hub/PINS.md SS6 as
     `"\u{1F47B} Ghostlight <short>"`, where `<short>` is the first 8 chars of the GUID. Keep the
     ghost as the `\u{1F47B}` escape in source (ASCII source; no unicode literals in code).
   - The grouping DECISION logic (which tabIds get grouped and how, given an injected `chrome`)
     MUST live in a pure module the worker imports, so it is unit-testable. Module path + exported
     function signature: PINNED in docs/tasks/hub/PINS.md SS6, which pins the grouping as a pure
     module (`extension/lib/grouping.js`) imported by `service-worker.js` that groups ONLY on a
     `group_request` and makes no policy decision; the service side lives in `src/hub/session.rs`.

3. CORRECTED 2026-07-04 (PINS.md SS9): wherever H4's owned-tab update actually runs (RE-READ H4's
   landed shape -- `serve_session`'s read loop in `src/transport/mcp/server.rs`, right after the
   ownership-gate/adoption logic updates `ctx.owned_tabs`, NOT `src/hub/session.rs`), when a
   session's owned-tab set changes (a tab is created via `tabs_create_mcp` or legitimately adopted),
   emit the group request naming that session's GUID and its current owned tabIds (read from
   `ctx.owned_tabs`, filtered to this GUID), THROUGH the shared `Browser` seam on `ServiceContext`.
   This is the only new call site. It MUST be a no-op for a lone all-open session with respect to
   the sacred tool wire: grouping is out-of-band presentation and MUST NOT alter any `tool_response`
   bytes (ADR-0030 Preserved invariants: "every new session/isolation path is a no-op for a lone
   all-open session"). REVISED 2026-07-04 (PINS.md SS9): every session, including the service's own
   lone stdio session, now carries a REAL `SessionGuid` (H3's revision -- there is no `None` case),
   so this emit path fires the SAME WAY for a lone session as for any other -- there is no
   special-casing to skip it. Byte-identity is preserved anyway because `group_request` is an
   OUT-OF-BAND native-messaging branch, entirely separate from the sacred `tool_response` stream
   this invariant governs; it cannot alter those bytes regardless of whether it fires. Session/
   owned-tab/GUID code stays in `src/hub`/`src/transport`, NEVER in `src/governance` (the a7
   arch-test holds).

4. `src/transport/native/messages.rs`: add ONE doc section for the group-request wire, mirroring
   the "Tab-URL query (g13)" section (a `//!` doc block: the JSON shape, "binary -> extension",
   and a one-line "mechanism only; the extension groups the named tabIds and makes no policy
   decision"). Doc only; NO existing section is edited.

What MUST stay byte-identical: `src/transport/mcp/tools.rs` `TOOLS_JSON`; every existing native
message shape (`tool_request`/`tool_response`/`tool_error`, `get_hold`/`set_hold`/`toggle_hold`,
`hold_state`/`hold_error`, `session_killed`, `tab_url_request`/`tab_url_response`); the
`src/transport/native/host.rs` framing; a lone all-open session's `tool_response` bytes.

## Tests (BY NAME; assertions pinned)

- Keep green (do not modify): `tests/extension/geometry.test.js`, `tests/all_open_golden.rs`,
  `tests/tool_schema_fidelity.rs`.

- Add: `tests/extension/grouping.test.js::owned_tabs_are_grouped_on_service_request_only`
  (Node `node:test`; `require` the pure grouping module -- module path PINNED in
  docs/tasks/hub/PINS.md SS6, see
  Required behavior item 2 -- and drive it with a fake `chrome` that RECORDS every
  `chrome.tabs.group` / `chrome.tabGroups.*` call). Pinned assertions:
  1. GROUPS ONLY ON REQUEST: with the fake chrome constructed and NO group request issued, the
     recorded `chrome.tabs.group` call list is empty (the extension groups nothing on its own).
  2. GROUPS EXACTLY THE NAMED TABS: after one group request naming `tabIds: [101, 202]` for
     session `"S"`, `chrome.tabs.group` is called with exactly those tab ids `[101, 202]` -- none
     dropped, none added.
  3. MAKES NO POLICY DECISION: repeat assertion 2 with the fake chrome reporting one of the named
     tabs' `url` as a plausibly sensitive host (for example a bank or an internal domain). The
     SAME `[101, 202]` set is grouped, byte-for-byte identical to assertion 2: the helper never
     reads the tab's url/host and applies no filter. This operationalizes the ADR oracle
     transcribed below.
  4. SAME GUID REUSES ITS GROUP; DISTINCT GUID MAKES A NEW GROUP: a second request for session
     `"S"` reuses the same groupId returned/assigned for the first `"S"` request (no new group
     created); a request for a different session `"T"` creates a distinct group. (ADR-0030
     Decision 7: two GUIDs -> two groups.)

  ORACLE transcribed VERBATIM from docs/adr/0030-ghostlight-hub-orchestrator.md into this test's
  header comment (the source of assertion 3's "no policy decision"):
  - Decision 1 topology: "MV3 extension (POLICY-FREE; owns all durable browser state: tabs, tab
    GROUPS, debugger, console/network buffers, auth/cookies)".
  - Decision 6: "The extension's per-group checks remain defense-in-depth only."
  - Migration H7: "H7 Tab-group-per-session presentation (extension owns the durable group; groups
    on request only)."

  PINNED in docs/tasks/hub/PINS.md SS6, before an executor runs this task: (a) the group-request message `type` string
  and field names (Required behavior item 1); (b) the pure grouping module path and exported
  function signature (item 2); (c) the per-session group title format (item 2). Assertions 1-4 are
  written against those pinned names; the executor TRANSCRIBES them, never invents them.

## Verification (literal commands)

```
cargo build --all-targets
cargo test --test all_open_golden --test tool_schema_fidelity
node --test tests/extension/grouping.test.js tests/extension/geometry.test.js
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

(If the repo runs the extension tests through an npm script rather than `node --test` directly,
RE-READ `package.json` and use that script; the pinned assertions do not change.)

## STOP preconditions

- If H4's owned-tab tracking (`ServiceContext.owned_tabs`, per PINS.md SS9) is ABSENT or no longer
  matches SS9's description, STOP. H7 builds on it and must not invent it.
- If no seam exposes a hub-side path to send an out-of-band native message to the extension (the
  shared `Browser` handle on `ServiceContext`, reachable from wherever H4's owned-tab update runs),
  STOP. H7 MUST NOT build native-send transport itself; that is H2's plumbing.
- If grouping would require the extension to make ANY policy decision -- inspect a tab's
  url/host/domain/grant to decide membership -- STOP. The extension stays policy-free
  (ADR-0005; ADR-0030 Decision 6).
- If any change would alter the sacred MCP tool wire (`tool_request`/`tool_response`/`tool_error`)
  or any EXISTING native-messaging message shape, STOP. This task is additive only.
- If satisfying the task would require a lone all-open session's `tool_response` bytes to change,
  STOP (the all-open byte-identity invariant, transcribed below).
- If any AUTHOR-MUST-PIN value in this file is still unpinned, STOP -- do not invent the message
  `type`, the module/function names, or the group-title format.
- If honoring any instruction here would require moving a NEVER-touch fence below, STOP.

Transcribed oracle for the byte-identity STOP (verbatim, ADR-0030 "Preserved invariants"):
"All-open byte-identity: a lone all-open session's output stays byte-identical through H0-H8
(tests/all_open_golden.rs); every new session/isolation path is a no-op for a lone all-open
session."

## NEVER touch (this task)

- `src/transport/mcp/tools.rs` (`TOOLS_JSON`: the 13 trained schemas + `explain`), byte-frozen.
  No exception.
- `tests/tool_schema_fidelity.rs`. No exception; keep it green, untouched.
- `tests/all_open_golden.rs` and the all-open byte-identity invariant. No exception; the
  group-request path MUST be a no-op for a lone all-open session's wire output.
- The native-messaging framing in `src/transport/native/host.rs` (4-byte LE prefix,
  `MAX_MESSAGE_LEN`, `encode`/`read_message`). No exception this task.
- Every EXISTING native-messaging message shape (`tool_request`/`tool_response`/`tool_error`,
  `get_hold`/`set_hold`/`toggle_hold`, `hold_state`/`hold_error`, `session_killed`,
  `tab_url_request`/`tab_url_response`). Additive only: add a new `type`, edit no existing shape.
  No exception.
- The MCP JSON-RPC wire and the `notifications/tools/list_changed` line in
  `src/transport/mcp/server.rs`. The adapter is a byte relay, never a rewriter. No exception.
- `src/governance/**` and the a7 arch-test
  (`tests/architecture.rs::governance_core_has_no_forbidden_back_edges`): the core names no
  browser/transport/mcp/native/url and no tabId/token/socket type. All grouping/session/owned-tab
  code lands in `src/hub` and `extension/`. The H8-only `channels.webapi.from` allowlist exception
  is NOT sanctioned for this task.
- `Browser::attach` single-EXTENSION-link rejection (`AttachOutcome::AlreadyAttached`). Retained;
  H7 does not touch the single physical extension link.
- Task-specific: do not build the native-send transport (that is H2); do not weaken the existing
  single-group durable recovery for a lone session such that its wire output changes; do not write
  the session GUID to any log/audit sink from the grouping path (ADR-0030 Decision 4 secrecy).
