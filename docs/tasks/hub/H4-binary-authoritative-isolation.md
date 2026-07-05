# H4: Binary-authoritative cross-session tab isolation

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 6;
> "Preserved invariants (and the pinned oracles the batch transcribes)"). One task = one commit.
> Facts below are as-of-authoring 2026-07-04 -- RE-READ the named files before relying on any line
> number.

## Goal

Make the SERVICE the authority on which session may touch which tab. Track, per session (keyed on
H3's adapter-minted GUID), the set of tabIds that session owns: a tab it created via
`tabs_create_mcp` or first legitimately adopted (touched while no other session owned it). Before
routing any tab-scoped call OR resolving policy for it -- i.e. BEFORE any `tab_url` probe -- refuse a
tabId the session does not own, returning a UNIFORM "unknown tab" result that leaks neither the
tab's existence nor its host. This closes the cross-session host-enumeration channel. The "why" is
docs/adr/0030-ghostlight-hub-orchestrator.md Decision 6 ("cross-session isolation is authoritative
in the SERVICE"). A lone all-open session owns everything it touches, so the whole gate is a
pass-through no-op for the single-session path.

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 6; "Preserved invariants") -- NORMATIVE.
   Cited by name; its semantics are NOT restated here.
2. docs/tasks/hub/BOOTSTRAP.md -- ground rules.
3. This task file.

If they conflict, the higher wins.

## Current-tree facts (as-of-authoring; RE-READ before relying)

STANDING ORDER: every path, line number, and signature below is as-of-authoring 2026-07-04. This
task lands AFTER H0-H3. `src/hub/` and its files (`src/hub/mod.rs`, `src/hub/session.rs`) DO NOT
EXIST in the 2026-07-04 tree -- they are created by H0 (`HubCore` composition-root extraction), H1
(`serve_session<S>` + `ServiceContext`), H2 (persistent service + multiplex), and H3 (adapter-minted
GUID identity + local peer-cred admission). RE-READ `src/hub/mod.rs` and `src/hub/session.rs` (and
whatever session table / GUID routing key H2/H3 actually landed) before writing a line. If they are
absent, a STOP precondition below fires.

CORRECTED 2026-07-04 (PINS.md SS9; H3's first attempt BLOCKED on this exact stale assumption before
this fix -- RE-READ SS9 in full before writing a line): the per-session GOVERNANCE DISPATCH (the
loop that calls `handle_tools_call` once per request) is `src/transport/mcp/server.rs::serve_session`,
NOT anything in `src/hub/mod.rs`. `src/hub/mod.rs` only builds the shared `ServiceContext` and spawns
the two endpoints; it holds no per-request dispatch loop. The ACCEPT/ADMISSION layer (where H3 mints
and admits a GUID) is `src/transport/native/ipc.rs` (`serve_adapters`/`handle_adapter_connection`).

- `src/transport/mcp/pipeline.rs` (as-of-authoring 1883 lines). The `tools/call` chokepoint is
  `pub(crate) async fn handle_tools_call(browser, store, governance, id, params)` at ~:50. The FIRST
  tab-URL probe machinery is `LazyTabUrl::new(browser, args.get("tabId").and_then(Value::as_i64))`
  at pipeline.rs:118; `LazyTabUrl::get` (~:477) issues the single `tab_url_request` frame (the
  extension's `Browser::tab_url`, executor.rs:251) on the stage that first calls `.get()` (the
  sacred check at ~:133, or `resolve_governing_resource` at ~:179). Decision 6's "BEFORE any
  `tab_url` probe" means the ownership gate must decide and (on refusal) return BEFORE `LazyTabUrl`
  can probe -- i.e. ahead of pipeline.rs:118 for this call's tabId. The gate does NOT live inside
  `handle_tools_call`'s governance stages; it is a check in `serve_session`'s read loop
  (`src/transport/mcp/server.rs`) that runs before its call into `handle_tools_call`, using the
  session's own `guid: SessionGuid` parameter (H3; REVISED 2026-07-04 PINS.md SS9 -- every session,
  including the service's own directly-served one, carries a REAL guid, not `Option<SessionGuid>`)
  and the shared owned-tab map below.
- `src/hub/session.rs` (created by H3; RE-READ): holds the PURE identity types
  (`SessionGuid`/`PeerCred`/`SessionRegistry`) only -- it does NOT hold a per-session record or an
  owned-handle set (H3 does not create one; see PINS.md SS9's forward guidance for H4, below).
- `ServiceContext` (`src/hub/mod.rs`; RE-READ H3's landed shape): the SHARED, `Clone`-able state
  every session clones (per Decision 2), already holding `browser`/`store`/`recorder` and (after H3)
  `session_registry`. This task ADDS the owned-tab set here as ANOTHER shared field -- PINS.md SS9's
  forward guidance: `owned_tabs: Arc<std::sync::Mutex<HashMap<i64, SessionGuid>>>` (tabId -> owning
  GUID). Ownership check: `map.get(&tab_id) == Some(&my_guid)`. Adoption:
  `map.entry(tab_id).or_insert_with(|| my_guid.clone())`. This needs NO per-session record and NO
  cross-referencing between sessions' own state -- one shared map answers both questions in O(1).
- Coupling that pins scope: the owned-tab map is OPAQUE-keyed (tabId -> GUID) and lives on
  `ServiceContext` in `src/hub` (ADR Decision 6: "Owned-handle sets live in `src/hub` ...; the
  governance core stays handle-agnostic"). `src/governance/**` must gain NO concept of tabId/
  token/socket (a7, extended). The extension's per-group checks remain defense-in-depth ONLY; do not
  touch them.
- `tabs_create_mcp` is a free action (`Handler::Local`-adjacent free dispatch; see pipeline.rs
  comment ~:157). Its result carries the newly created tabId. The session adopts that tabId into its
  owned set on success. First-touch adoption: a tab-scoped call naming a tabId that NO live session
  owns is adopted by the calling session (this is what makes a lone session own everything it
  touches). A tabId owned by a DIFFERENT live session is refused.

## Required behavior

Mandated by docs/adr/0030-ghostlight-hub-orchestrator.md Decision 6 unless noted.

1. SHARED owned-tab map (CORRECTED 2026-07-04, PINS.md SS9). Add a NEW field to `ServiceContext`
   (`src/hub/mod.rs`): `owned_tabs: Arc<std::sync::Mutex<HashMap<i64, SessionGuid>>>`, built once
   alongside `session_registry`. A tabId enters the map when: (a) `tabs_create_mcp` returns it
   successfully to this session, or (b) this session issues a tab-scoped call naming a tabId that no
   OTHER live session owns (first-touch adoption: `map.entry(tab_id).or_insert_with(|| my_guid.clone())`).
   Ownership is service-authoritative; the extension is never consulted to decide ownership. There is
   no per-session record and no per-session HashSet -- one shared map answers "do I own it" AND
   "can I adopt it" without cross-referencing other sessions' state.

2. OWNERSHIP GATE BEFORE PROBE (CORRECTED 2026-07-04, PINS.md SS9). In `serve_session`'s read loop
   (`src/transport/mcp/server.rs`), for a call carrying a numeric `tabId`, run the ownership check
   against `ctx.owned_tabs` using this session's own `guid: SessionGuid` (every session has a real
   one, per H3's revision -- there is NO `None` case and NO special-casing here) BEFORE calling
   `handle_tools_call` (hence before `LazyTabUrl` at pipeline.rs:118 can probe, and before any policy
   resolution). If the tabId is owned by a DIFFERENT guid in the map (first-touch always succeeds
   for an unowned tabId; refusal only fires for a tabId owned by a DIFFERENT guid), RETURN the
   uniform "unknown tab" result immediately; do NOT enter
   `handle_tools_call`, do NOT issue a `tab_url` frame. This ordering is the whole point of Decision 6
   ("BEFORE any `tab_url` probe ... leaks neither the tab's existence nor its host").

3. UNIFORM, LEAK-FREE RESULT. The refusal result MUST be byte-identical whether the tabId belongs to
   another live session (the tab EXISTS, on some host) or names no tab at all (does not exist). It
   MUST NOT contain the tab's host, the owning session's identity, or any existence signal. It is a
   successful MCP tool result carrying a single text block (same envelope shape as every other
   pre-dispatch text result in `handle_tools_call`, e.g. the hold/deny path at pipeline.rs:109/193).
   Exact uniform string: the string `unknown tab`, PINNED in docs/tasks/hub/PINS.md SS3 (identical
   whether or not the tab exists). The refusal IS recorded, as a deny (PINNED in docs/tasks/hub/PINS.md
   SS3): `decision = "deny"`, `domain = null` (the host is NEVER resolved for an unowned tab, so it
   cannot leak), `held = false`, `duration_ms = 0`, using the 14-key order transcribed under Oracles;
   the denial rule label is `cross_session/unowned_tab` and its `denial_id` follows the existing `"D-"`
   + 8 lowercase hex scheme (assert the shape, not a literal).

4. PASS-THROUGH FOR THE LONE / ALL-OPEN SESSION. With a single live session (the only case a lone
   all-open stdio client produces), first-touch adoption means the session owns every tab it names,
   so the gate NEVER refuses and NEVER alters the frames, bytes, or audit of that path. `tabs_context_mcp`
   (no tabId) and every non-tab call are untouched. This MUST keep `tests/all_open_golden.rs`
   byte-identical (STOP precondition below).

5. ISOLATION LIVES IN `src/hub`, NOT `src/governance`. No file under `src/governance/**` may be
   edited by this task; the core stays handle-agnostic and names no tabId/token/socket type (a7,
   extended -- transcribed under Oracles). If enforcing ownership seems to require editing a
   governance file, STOP and relocate to `src/hub`.

6. MUST STAY BYTE-IDENTICAL: the 13 trained schemas + `explain` (`src/transport/mcp/tools.rs`); the
   native-messaging framing (`src/transport/native/host.rs`); the `handle_tools_call` stage order and
   every existing pre-dispatch/deny/audit string in `src/transport/mcp/pipeline.rs`; the all-open
   output bytes.

## Tests (BY NAME; assertions pinned)

Keep green (do not modify):
- `tests/all_open_golden.rs` (all-open byte-identity; the lone-session pass-through must not disturb it).
- `tests/tool_enforcement.rs`.
- `tests/architecture.rs::governance_core_has_no_forbidden_back_edges` (a7).
- `tests/tool_schema_fidelity.rs`.

Add (new file `tests/hub_isolation.rs`):

- `tests/hub_isolation.rs::unowned_tab_is_refused_before_any_tab_url_probe`
  - Set up two live sessions (A, B) on the shared service, each with its own H3 GUID (RE-READ H3 for
    how a session/GUID is stood up in `src/hub`; use the same fake-extension pattern
    `src/transport/mcp/pipeline.rs` tests use -- `attach_fake_extension_with_tab_urls`, ~:597 -- so a
    `tab_url_request` for an unregistered tabId PANICS, which is how this test proves NO probe fired).
  - Session A creates/owns tab 5 (via `tabs_create_mcp` returning tabId 5, or the H3-established
    ownership path). Session B then issues a tab-scoped call (e.g. `read_page` with `{ "tabId": 5 }`).
  - Pinned assertions:
    - The result text for B's call EQUALS the uniform unknown-tab string `unknown tab` (PINNED in
      docs/tasks/hub/PINS.md SS3). It is a success result, never `isError: true`.
    - The fake extension recorded ZERO frames for B's call: the `seen` vector contains no
      `"tab_url_request:5"` and no `read_page` entry attributable to B's call (refused before the
      probe at pipeline.rs:118 and before dispatch). Pin the exact expected `seen` contents for B's
      call as an empty slice for that call.
- `tests/hub_isolation.rs::unknown_tab_result_leaks_no_host_or_existence`
  - Session B issues the SAME tab-scoped call twice: once naming a tabId owned by session A (the tab
    EXISTS, on a distinctive host such as `secret-host.example`) and once naming a tabId that no
    session owns and no extension knows (does NOT exist).
  - Pinned assertions:
    - `assert_eq!(text_for_existing_other_session_tab, text_for_nonexistent_tab)` -- the uniform
      message is IDENTICAL whether or not the tab exists (this is the leak-free property; the exact
      uniform string is `unknown tab`, PINNED in docs/tasks/hub/PINS.md SS3). Pin both texts to equal
      `unknown tab`.
    - Neither text contains the owning tab's host substring (`assert!(!text.contains("secret-host"))`),
      proving no host leak.

Oracles transcribed VERBATIM from docs/adr/0030-ghostlight-hub-orchestrator.md (transcribe into the
test as pinned comments; do not re-derive):

- Decision 6 (verbatim): "The service tracks, per session (keyed on Decision 4's GUID), the set of
  tabIds that session created (`tabs_create_mcp`) or legitimately adopted. Before routing any
  tab-scoped call OR resolving policy for it -- i.e. BEFORE any `tab_url` probe -- the service
  refuses a tabId the session does not own, returning a uniform "unknown tab" result that leaks
  neither the tab's existence nor its host (closing the cross-session host-enumeration channel).
  Owned-handle sets live in `src/hub` (opaque handles that may name a tabId); the governance core
  stays handle-agnostic. The extension's per-group checks remain defense-in-depth only. A lone
  all-open session owns everything it touches, so the all-open path stays a byte-identical
  pass-through."
- All-open byte-identity (verbatim, "Preserved invariants"): "All-open byte-identity: a lone
  all-open session's output stays byte-identical through H0-H8 (`tests/all_open_golden.rs`); every
  new session/isolation path is a no-op for a lone all-open session."
- a7 (verbatim, "Preserved invariants"): "The a7 arch-test
  (`tests/architecture.rs::governance_core_has_no_forbidden_back_edges`): `src/governance/**` names
  no browser/transport/mcp/native type nor the `url` crate; extended so the core also names no
  tabId/token/socket type. All session/multiplex/isolation code lands in `src/hub`."
- Audit 14-key order (verbatim, "Preserved invariants" -- transcribe ONLY IF the refusal is audited,
  per behavior item 3): field order, exactly 14 keys, in order: `event_id, ts, identity, client,
  tool, action, capability, domain, decision, grant_id, denial_id, duration_ms, manifest, held`.

Resolved (PINNED in docs/tasks/hub/PINS.md SS3):
- The exact uniform result string is `unknown tab` (identical whether or not the tab exists).
- The ownership refusal IS recorded, as a deny: `decision = "deny"`, `domain = null` (the host is
  NEVER resolved for an unowned tab, so `domain` cannot leak a host), `held = false`,
  `duration_ms = 0`, using the 14-key order above; the denial rule label is `cross_session/unowned_tab`
  and its `denial_id` follows the existing `"D-"` + 8 lowercase hex scheme (assert the shape, not a
  literal).

## Verification (literal commands)

```
cargo build --all-targets
cargo test --test hub_isolation
cargo test --test all_open_golden
cargo test --test tool_enforcement
cargo test --test architecture governance_core_has_no_forbidden_back_edges
cargo test -p ghostlight --lib transport::mcp::pipeline
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## STOP preconditions

- If `src/hub/session.rs`, `src/hub/mod.rs`'s `ServiceContext`, or `src/transport/mcp/server.rs`'s
  `serve_session` does not exist or no longer matches PINS.md SS9's description (in particular,
  `serve_session` taking a plain `guid: SessionGuid` parameter -- NOT `Option` -- and `ServiceContext`
  being `Clone`), STOP (H0-H3 have not landed, or landed differently; reconcile against the ACTUAL
  shape, do not guess).
- If H3's session identity (the adapter-minted or service-self-minted GUID, threaded into
  `serve_session` as a plain `SessionGuid`) is NOT available at the per-request dispatch point,
  STOP (Decision 6 keys ownership on Decision 4's GUID; without it there is nothing to key the owned
  map on).
- If enforcing ownership would edit any file under `src/governance/`, STOP and relocate the check to
  `src/hub`.
- If a lone all-open session's owned-set logic changes `tests/all_open_golden.rs` output bytes, STOP
  and make the single-session path a pure pass-through (first-touch adoption, no refusal, no probe
  change).
- If landing this would require moving any never-touch fence below, STOP.
- If a STOP precondition's assumption is absent, STOP -- do not improvise around a broken assumption.

## NEVER touch (this task)

- `src/governance/**` -- isolation lives in `src/hub`; the core stays handle-agnostic (names no
  tabId/token/socket). NO sanctioned exception in this task (H8 alone may add
  `channels.webapi.from`; not here).
- `src/transport/mcp/tools.rs` (TOOLS_JSON: the 13 trained schemas + `explain`), byte-frozen. No
  exception.
- `tests/tool_schema_fidelity.rs`. No exception; keep green untouched.
- `tests/all_open_golden.rs` and the all-open byte-identity invariant. No exception; the isolation
  path MUST be a no-op for a lone all-open session.
- `tests/architecture.rs` a7 (`governance_core_has_no_forbidden_back_edges`). No exception; session/
  isolation/ownership code lands in `src/hub`, never `src/governance`.
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`,
  `encode`/`read_message`). No exception this batch.
- The `handle_tools_call` stage order and every existing string in `src/transport/mcp/pipeline.rs`
  (the ownership gate is ADDED ahead of the call into it, in `src/hub`; the pipeline's own stages are
  not reordered or re-worded).
- The MCP JSON-RPC wire and the pinned `notifications/tools/list_changed` line in `server.rs`.
- The extension's per-group checks (defense-in-depth only; the service is authoritative, but the
  extension checks are not this task's to edit).
