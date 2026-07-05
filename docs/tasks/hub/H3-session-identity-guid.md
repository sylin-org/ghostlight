# H3: Adapter-minted GUID identity + local peer-cred binding

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 4; also
> "Preserved invariants (and the pinned oracles the batch transcribes)"). One task = one commit.
> Facts below are as-of-authoring 2026-07-04 -- RE-READ the named files before relying on any line
> number or signature.

## Goal
Give every session an opaque, unguessable identity that the SERVICE routes and isolates by, while
the governance core stays PID/GUID-agnostic. The thin ADAPTER mints a CSPRNG UUIDv4 GUID and
presents it in the connection handshake (the first framed message). The LOCAL accept layer in
`src/hub` captures the connecting peer's OS credential and binds the GUID to that minting peer
(refusing a GUID re-presented by a different OS user, except the sanctioned same-user reuse path),
and holds that credential as the per-peer rate-limit key. Why: ADR-0030 Decision 4 ("identity model
(adapter-minted GUID; core stays PID-agnostic)") plus its "Amendment to the transport-side".

## Authority
1. docs/adr/0030-ghostlight-hub-orchestrator.md Decision 4 -- NORMATIVE. Cite it; never restate its
   semantics.
2. BOOTSTRAP.md ground rules.
3. This task file.
If they conflict, the higher wins.

## Current-tree facts (as-of-authoring; RE-READ before relying)

- CORRECTED 2026-07-04 (PINS.md SS9; H3's own first attempt BLOCKED on the stale version of this
  bullet -- see LEDGER.md's H3 log): `src/hub/` (H0-H2) holds `mod.rs` (the composition root:
  `run_mcp_server`, `run_as_service`, `run_as_adapter`, `ServiceContext`) and `handshake.rs`. It does
  NOT hold an accept loop or per-connection code -- the ADAPTER/CONTROL accept loop and the
  session-hello read live in `src/transport/native/ipc.rs` (`serve_adapters`,
  `handle_adapter_connection`; H2's two-endpoint re-authoring put them there, not in `src/hub`).
  RE-READ PINS.md SS9 in full before touching any of items 2-3 below; RE-READ `src/hub/mod.rs` and
  `src/transport/native/ipc.rs` to confirm this still holds at execution time.
- `src/hub/session.rs` -- NEW file this task creates.
- `src/transport/native/messages.rs` -- currently a DOC-ONLY module (no Rust types; "only the
  mcp-server constructs and parses them, so they are documented here"). It documents the
  binary<->extension vocabulary (`tool_request`/`tool_response`/`tool_error`, hold, `session_killed`,
  `tab_url_*`). H2 introduces the adapter<->service connection handshake: the SS1 "hello" frame
  (PINS.md SS1) that H2's adapter role already sends on the adapter/control endpoint. This task ADDS a
  documentation section for that hello frame's `guid` member; it does NOT invent a second or separate
  handshake frame. No Rust types are added here.
- `src/proc.rs` -- ADR-0029 process-liveness primitives (`ProcId {pid, created}`, `parent`,
  `is_alive`, `orphaned`, `terminate`). It STAYS (adapter lifecycle + doctor reap). The SERVICE core
  gains NO dependency on it for identity: the GUID carries no pid/ancestor/creation-time.
- `src/governance/dispatch.rs` -- `Governance` holds `mode`, `audit`, and `client: Mutex<Option<
  ClientInfo>>`; `set_client(&self, name, version)` (first-capture-wins) at ~line 386;
  `current_client` at ~line 402. The audit `identity` field is written as `None` today
  (`record_session_killed` ~line 417, `record_manifest_reload` ~line 433; also `audit/mod.rs`
  ~lines 194/213). Governance has NO subject/GUID concept and MUST NOT gain one (a7).
- Coupling that pins scope: Decision 2 places "the opaque subject GUID" in PER-SESSION state held
  in `src/hub` ALONGSIDE the `Governance` facade, NOT inside `Governance`. So the GUID lives in
  H2's `src/hub` per-session record; `Governance` is NOT modified by this task, which keeps the a7
  arch-test and the all-open byte-identity trivially green. The audit `identity` field STAYS `None`
  in H3 (stamping an authenticated subject into audit is Decision 9 / H8, not this task).

## Required behavior

### 1. Mint (ADAPTER side; ADR-0030 Decision 4 "minted by the thin ADAPTER")
Add to the NEW `src/hub/session.rs`:

```
/// An opaque, unguessable session identity minted by the adapter and presented to the service.
/// Canonical lowercase hyphenated UUIDv4 (36 chars). Secret material (ADR-0030 Decision 4:
/// "Treat the GUID as secret in logs/audit").
#[derive(Clone, PartialEq, Eq)]
pub struct SessionGuid(String);

impl SessionGuid {
    /// Mint a fresh CSPRNG UUIDv4. Uses `uuid::Uuid::new_v4()` (the crate is already a dep).
    pub fn mint() -> Self;
    /// Parse a presented string; `Some` iff it is a valid version-4 UUID in canonical form.
    pub fn parse(s: &str) -> Option<Self>;
    /// The raw canonical string (for the wire handshake and the routing-map key ONLY).
    pub fn as_str(&self) -> &str;
}
```

- The adapter role mints via `SessionGuid::mint()` ONCE per adapter PROCESS and reuses that same
  value for the process lifetime (Decision 4: "Same adapter process reuses its GUID (same group); a
  new adapter process mints a new one"). A new adapter process calls `mint()` again -> a different
  GUID -> a different group (D7).
- REVISED 2026-07-04 (PINS.md SS9, fresh-eyes review): the SERVICE's own directly-served stdio
  session (`run_as_service`, `src/hub/mod.rs`) ALSO calls `SessionGuid::mint()` for itself -- every
  session gets a real GUID (`serve_session`'s new parameter is `SessionGuid`, NOT
  `Option<SessionGuid>`; see item 3). This closes a real isolation gap an exempt/`None` lone session
  would otherwise leave open (H4's owned-tab map would never learn what the lone session touched, so
  a later adapter session could silently first-touch-adopt the SAME tabId). A genuinely lone session
  still "owns everything it touches" (Decision 6) because first-touch-adoption always succeeds when
  nothing else contests the tabId -- no special-casing needed downstream.
- `Display`/`Debug` for `SessionGuid` MUST render a REDACTED form that does NOT contain the raw
  canonical string, so the GUID never reaches a `tracing` log or audit sink verbatim (Decision 4:
  "Treat the GUID as secret in logs/audit"; if persisted for reuse it is owner-only, never client
  config -- at-rest persistence is OUT OF SCOPE for H3, see fences). The exact redacted string form
  is AUTHOR MUST PIN before execution; the TEST asserts only the non-leak invariant below.

### 2. Peer credential + binding (LOCAL accept layer; Decision 4 "Amendment to the transport-side")
Add to `src/hub/session.rs`:

```
/// The connecting peer's OS credential, captured by the LOCAL accept layer purely for admission
/// control and as the per-peer rate-limit key (ADR-0030 Decision 4 amendment). Lives in `src/hub`,
/// NEVER in `src/governance` (a7). `user` is the peer's OS user principal: the SID string on
/// Windows, the uid on Unix. `pid` distinguishes processes for logging; admission compares `user`.
#[derive(Clone, PartialEq, Eq)]
pub struct PeerCred { pub user: PeerUser, pub pid: u32 }

/// Opaque OS-user principal; same-user comparison is `==`.
/// `Hash` (FIXED 2026-07-04, PINS.md SS9): H5's per-peer quota table is
/// `HashMap<PeerUser, usize>`, which requires it -- do not land without it.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerUser(String);

/// The service's GUID -> bound-peer routing map (Decision 2: per-session state in `src/hub`).
pub struct SessionRegistry { /* map SessionGuid canonical string -> PeerCred */ }

pub enum Admission { Admitted, Refused }

impl SessionRegistry {
    pub fn new() -> Self;
    /// Admit a peer presenting a GUID. First presentation records the binding and returns
    /// `Admitted`. A re-presentation is `Admitted` iff the presenter is the SAME OS user as the
    /// bound peer (the sanctioned reuse path re-verifies same-user); a DIFFERENT user is `Refused`
    /// and the existing binding is left unchanged (ADR-0030 Decision 4: "refuse a GUID presented by
    /// a different peer, except the sanctioned reuse path which re-verifies same-user").
    pub fn admit(&mut self, guid: &SessionGuid, peer: &PeerCred) -> Admission;
}
```

- CORRECTED 2026-07-04 (PINS.md SS9): the real OS capture (Windows `GetNamedPipeClientProcessId` +
  token SID; Unix `SO_PEERCRED` / `getpeereid`) happens INSIDE `serve_adapters`
  (`src/transport/native/ipc.rs`, both platform variants), on the CONCRETE platform type
  (`NamedPipeServer` post-`.connect()` / `UnixStream` from `.accept()`) -- BEFORE the stream passes
  to `handle_adapter_connection<S>`, which is generic over `S: AsyncRead + AsyncWrite` and so CANNOT
  itself call a platform-specific capture function. Add a `capture_peer_cred` fn per platform in
  `ipc.rs`, called at the capture point above; thread the resulting `PeerCred` as a new plain
  parameter into `handle_adapter_connection` (new signature:
  `handle_adapter_connection<S>(ctx, stream: S, peer_cred: PeerCred)`). `admit` itself stays a PURE
  function of `(guid, peer)`, so its OWN tests drive it with synthesized `PeerCred` values (no real
  OS user needed) -- only the CALL SITE moves, not the admission logic.
- `PeerCred` is ALSO retained (via `ServiceContext.session_registry`, see item 3) as the per-peer
  rate-limit key. H3 only PROVIDES the key; the mint/group quota ENFORCEMENT is Decision 3 / H5 --
  do not add quota logic here.

### 3. Service routing (Decision 4 "routes and isolates by that opaque GUID only")
CORRECTED 2026-07-04 (PINS.md SS9):
- Add ONE new field to `ServiceContext` (`src/hub/mod.rs`):
  `session_registry: Arc<std::sync::Mutex<SessionRegistry>>`, built once in
  `ServiceContext::from_startup` alongside `browser`/`store`/`recorder` (`ServiceContext` is already
  `Clone`, so every session shares the one registry).
- In `handle_adapter_connection` (`src/transport/native/ipc.rs`), parse the hello's `guid` field via
  `SessionGuid::parse`. ADDED 2026-07-04 (fresh-eyes review, closing a gap the second review found):
  if parsing FAILS (absent, empty, or not a canonical v4 UUID), refuse the connection cleanly --
  mirroring the existing "unknown or absent role fails the connection cleanly, never a panic"
  handling H2 already built for the role field -- do NOT call `admit` with an unparseable value and
  do NOT surface the raw presented string in any log. Only once `guid` parses does admission run:
  call `ctx.session_registry.lock().unwrap().admit(&guid, &peer_cred)`. On `Refused`, drop the
  connection without creating a session (the exact refusal log/return string is AUTHOR MUST PIN; do
  NOT surface the GUID). On `Admitted`, call `crate::mcp::server::serve_session(stream, ctx, guid)`.
- SANCTIONED TEST FIX (the one exception to "do not touch H2's tests" in this task): H2's own
  `tests/hub_multiplex.rs::adapter_endpoint_two_phase_wire_round_trips` sends a hand-built hello with
  a PLACEHOLDER `"guid": ""` (PINS.md SS1 already anticipated this: "before H3 an empty placeholder
  guid is acceptable and H3 fills it"). That literal string does not parse as a valid `SessionGuid`,
  so once the parse-failure handling above lands, this test would hit a clean refusal instead of its
  intended successful wire round-trip -- defeating what the test exists to prove (the two-phase
  framed-hello-then-raw-JSON-RPC wire, not guid validity, which `tests/hub_identity.rs` covers
  separately). Update ONLY that one literal in that one test to a valid, well-formed v4 UUID string
  (e.g. a fixed literal like `"00000000-0000-4000-8000-000000000000"` or a freshly minted one) so it
  continues to exercise successful admission and the wire mechanics unchanged; do not otherwise
  modify that test file.
- `serve_session`'s signature (`src/transport/mcp/server.rs`) GAINS a parameter:
  `serve_session<S>(stream: S, ctx: ServiceContext, guid: SessionGuid) -> Result<()>` -- REVISED
  2026-07-04 (PINS.md SS9): NOT `Option<SessionGuid>`. This is NOT a violation of H1's
  byte-identical-signature pin (H1 pinned transport-genericity over the STREAM type and
  byte-identical OUTPUT, never an eternal 2-parameter arity) -- adding identity to the per-session
  scope IS this task's own Goal. `run_as_service`'s call site (`src/hub/mod.rs`, its own
  directly-served stdio session) mints its OWN `SessionGuid::mint()` and passes it (see item 1's
  revision -- every session gets a real GUID, closing an isolation gap an exempt lone session would
  otherwise leave in H4's owned-tab map); `handle_adapter_connection`'s call site (above) passes the
  admitted `guid`. Minting/threading a GUID is byte-identical to today's behavior/output (H3 does not
  stamp the GUID into audit or branch dispatch on it -- the parameter is inert this task, established
  for H4/H8 to consume).
- FIXED 2026-07-04 (PINS.md SS9, fresh-eyes review): `src/transport/mcp/server.rs::run` (the
  H1-era thin wrapper) is DEAD CODE as of H2's landing -- `run_mcp_server` calls
  `run_as_service`/`run_as_adapter` directly and never calls `run` (confirm via a repo-wide grep for
  `server::run(` before relying on this). It still calls `serve_session(stream, ctx)` with the OLD
  2-arg signature and will FAIL TO COMPILE once `serve_session` gains `guid`. DELETE `run` (do not
  thread a fake guid into dead code); optionally correct the stale doc comments that still describe
  it as live (`dispatch.rs`, `hub/mod.rs`, `tests/audit_recorder.rs`,
  `tests/manifest_validation.rs` -- comments only, not load-bearing, do not scope-creep into a
  broader cleanup).
- The `Governance` facade is NOT modified. Do NOT add a subject/GUID setter to `src/governance/**`.
  If GUID routing appears to need the governance core, STOP (see STOP preconditions) -- the mapping
  is in `src/hub`/`ServiceContext` and the core stays handle-agnostic (a7).

### 4. Wire handshake documentation (`src/transport/native/messages.rs`, doc-only)
Add a documentation section describing the adapter->service connection handshake's identity member:
the SS1 "hello" frame the ADAPTER sends (PINS.md SS1: `{ "hub": 1, "role": "adapter", "guid":
"<uuid-v4>" }`) carries the session GUID in its `guid` member, a canonical lowercase hyphenated
UUIDv4 (`SessionGuid`). The GUID rides in H2's existing hello frame; do NOT invent a second or
separate handshake frame. The hello frame's `hub`/`role` members are DEFINED BY H2 -- RE-READ
messages.rs and H2's `src/hub/handshake.rs` constants; document the `guid` member against that
existing SS1 hello frame. The EXTENSION link uses NO hello at all (it is on its own endpoint,
server-speaks-first; PINS.md SS1 as amended 2026-07-04), so there is no `ext` role and nothing about
the extension link to document here. Keep this section doc-only (no Rust types), matching the file's
existing style.

### 5. a7 scanner extension (SANCTIONED `tests/architecture.rs` edit)
H3 is the ONE task in this batch sanctioned to edit `tests/architecture.rs` (ADR-0030 "Preserved
invariants" as amended names H3 as the extender; it is the single sanctioned edit to that file in
this batch). EXTEND `governance_core_has_no_forbidden_back_edges` so its scan of `src/governance/**`
ALSO rejects the identifiers `tabId`, `token`, and `socket` (belt-and-suspenders for the type
discipline: the governance core must name no transport/handle/credential type). Keep every existing
back-edge rule in the scanner intact (the current browser/transport/mcp/native/url forbidden set);
this edit is PURELY ADDITIVE. Make no other change to `tests/architecture.rs`.

### 6. Role marker + governance-chokepoint assertion (ADR-0030 Decision 1 addendum; PINS.md SS8)

Added 2026-07-04 after H2 landed the two-endpoint split. Create `src/hub/role.rs` per PINS.md SS8's
PINNED shape (`Role`, `set_role`, `role`, `assert_role`, `assert_service_role`, `assert_adapter_role`,
verbatim panic message), and add `pub mod role;` to `src/hub/mod.rs`'s module declarations (RE-READ
its current ones, e.g. `pub mod handshake;`, and add the line in the same style) -- without this,
`crate::hub::role::*` does not resolve from `src/transport`. This is a fail-loud backstop, not a
substitute for H2's structural separation: it must be a no-op (no output, no behavior change)
whenever the role is already correct, so it does not touch the all-open byte-identity invariant.

Wire it at the two seams H2's landed code already makes obvious (RE-READ `src/hub/mod.rs` to confirm
these are still the actual function names/shapes before relying on them; H2 landed them as of this
writing):
- `run_as_service` (`src/hub/mod.rs`, the async fn entered when `ipc::claim_adapter_endpoint` returns
  `Ok`): call `hub::role::set_role(hub::role::Role::Service)` as the ABSOLUTE first line of its body,
  before the `Browser::with_debug` call.
- `run_as_adapter` (`src/hub/mod.rs`, the async fn entered on `Err(crate::Error::SessionBusy)`): call
  `hub::role::set_role(hub::role::Role::Adapter)` as the ABSOLUTE first line of its body, before the
  `ipc::relay_adapter` call.
- `serve_session` (`src/transport/mcp/server.rs`, the governance chokepoint every transport calls per
  ADR-0030 Decision 2): call `hub::role::assert_service_role("serve_session")` as the ABSOLUTE first
  line of its body, before any other setup.

Add `tests/hub_role_wiring.rs::governance_chokepoint_asserts_service_role` (PINS.md SS8): a text-scan
test (a7-style) asserting the source of `serve_session` in `src/transport/mcp/server.rs` contains the
literal substring `assert_service_role`. This guards the WIRING; `role.rs`'s own unit tests (below)
guard the assertion LOGIC. H6 later adds the symmetric adapter-side wiring test to
`tests/hub_lifecycle.rs` when it builds the spawn-on-demand function -- do not attempt that half here.

### 7. `relay_adapter` mints and embeds a REAL GUID (PINS.md SS9; added 2026-07-04 after fresh-eyes review)

`ipc::relay_adapter` (`src/transport/native/ipc.rs`) currently sends a PLACEHOLDER empty
`"guid": ""` in its hello frame (its own doc comment already flags this as "the H3 seam: an empty
placeholder before H3 mints a real adapter-minted session GUID" -- RE-READ it to confirm this is
still the case). Fix it: at the top of `relay_adapter`, before building the hello JSON, call
`let guid = crate::hub::session::SessionGuid::mint();` and embed `guid.as_str()` in place of `""`.
Because `relay_adapter` itself runs exactly once per adapter process (it is called once from
`run_as_adapter`, not in a loop), minting it as a local variable here already satisfies Decision 4
("same adapter process reuses its GUID; a new adapter process mints a new one") with no `OnceLock`
or extra plumbing needed.

## Tests (BY NAME; assertions pinned)

- Keep green: `tests/all_open_golden.rs`, `tests/audit_recorder.rs` (do not modify).
  `tests/architecture.rs::governance_core_has_no_forbidden_back_edges` stays green but is EXTENDED by
  this task (the single sanctioned edit to `tests/architecture.rs` in the batch -- see the a7 scanner
  extension item above); every existing back-edge rule in it must remain intact.
  A lone all-open session mints/binds a GUID but its OUTPUT and audit records stay byte-identical:
  the GUID is a routing key in `src/hub`, never stamped into audit in H3.

- Add: `tests/hub_identity.rs::guid_is_v4_csprng_and_bound_to_minting_peer`
  - Mint two GUIDs with `SessionGuid::mint()`. Assert each `as_str()` parses via
    `uuid::Uuid::parse_str` with `get_version() == Some(uuid::Version::Random)` (version-4) and the
    RFC-4122 variant. Assert the two GUIDs are NOT equal (CSPRNG, not a counter).
  - Non-leak invariant (transcribed from ADR-0030 Decision 4 "Treat the GUID as secret in
    logs/audit"): assert `!format!("{}", guid).contains(guid.as_str())` AND
    `!format!("{:?}", guid).contains(guid.as_str())` for a minted guid.
  - Binding: build `let a = PeerCred { user: PeerUser("user-A".into()), pid: 100 };`, a fresh
    `SessionRegistry`, and assert `registry.admit(&g, &a)` is `Admission::Admitted` on first
    presentation and `Admission::Admitted` again when the SAME peer `a` re-presents `g` (the reuse
    path).

- Add: `tests/hub_identity.rs::foreign_peer_presenting_a_guid_is_refused`
  - Mint `g`, admit it bound to `let a = PeerCred { user: PeerUser("user-A".into()), pid: 100 };`
    (assert `Admitted`).
  - Present `g` with `let b = PeerCred { user: PeerUser("user-B".into()), pid: 200 };` (a DIFFERENT
    OS user) and assert `registry.admit(&g, &b) == Admission::Refused`.
  - Assert the original binding is unchanged: `let a2 = PeerCred { user: PeerUser("user-A".into()),
    pid: 999 };` (same user, different pid -- the sanctioned same-user reuse path) admits:
    `registry.admit(&g, &a2) == Admission::Admitted`.

- Add: `tests/hub_identity.rs::relay_adapter_sends_a_real_guid_not_a_placeholder` (item 7, PINS.md
  SS9): connect to a running service's ADAPTER/CONTROL endpoint the way `relay_adapter` does (or
  drive `relay_adapter` itself against a test service), read the FRAMED hello frame it sends, and
  assert its `guid` field is NON-EMPTY and parses via `SessionGuid::parse` (i.e. a valid canonical
  lowercase-hyphenated v4 UUID) -- proving the `""` placeholder was actually replaced, not merely
  described as fixed.

- Add (the SANCTIONED `tests/architecture.rs` edit):
  `tests/architecture.rs::governance_core_rejects_tabid_token_socket_identifiers`
  - Assert the extended scanner FLAGS a synthetic `src/governance/**` source naming `tabId` (and
    likewise one naming `token`, and one naming `socket`) as a forbidden back-edge, and that a
    source naming none of the three passes. This proves the `tabId`/`token`/`socket` extension is
    live, not dead code, without weakening any existing rule.

- Transcribed oracles kept intact (asserted GREEN via the keep-green tests; do NOT re-derive):
  - Audit record field order, exactly 14 keys, in order:
    `event_id, ts, identity, client, tool, action, capability, domain, decision, grant_id,
    denial_id, duration_ms, manifest, held` (`tests/audit_recorder.rs`). H3 adds no key and leaves
    `identity` as `None`.
  - Session-event record field order, exactly 6 keys, in order:
    `event_id, ts, identity, client, event, manifest`. H3 stamps no GUID into it.

- Add (PINS.md SS8, transcribe verbatim; `src/hub/role.rs`'s own `#[cfg(test)]` module):
  - `adapter_role_hitting_the_governance_chokepoint_panics`:
    `#[should_panic(expected = "must only run when this process's role is Service")]`; calls
    `assert_role(Role::Adapter, Role::Service, "test")`.
  - `service_role_hitting_spawn_on_demand_panics`:
    `#[should_panic(expected = "must only run when this process's role is Adapter")]`; calls
    `assert_role(Role::Service, Role::Adapter, "test")`.
  - `matching_roles_do_not_panic`: calls `assert_role(Role::Service, Role::Service, "test")` and
    `assert_role(Role::Adapter, Role::Adapter, "test")`; a plain test asserting neither panics.

- Add: `tests/hub_role_wiring.rs::governance_chokepoint_asserts_service_role` (see item 6 above).

## Verification (literal commands)
```
cargo build --all-targets
cargo test --test hub_identity
cargo test --test hub_role_wiring
cargo test --test hub_multiplex
cargo test --lib role
cargo test --test all_open_golden
cargo test --test architecture governance_core_has_no_forbidden_back_edges
cargo test --test architecture governance_core_rejects_tabid_token_socket_identifiers
cargo test --test audit_recorder
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

(`hub_multiplex` is included because item 3's sanctioned fix touches its
`adapter_endpoint_two_phase_wire_round_trips` test literal -- confirm it and the other two tests in
that file all still pass.)

## STOP preconditions
- If `handle_adapter_connection` (`src/transport/native/ipc.rs`) has NO per-connection point at
  which the adapter's first framed message (the session-hello) can be read and carry the GUID,
  STOP -- the handshake seam is H2's to build; do not bolt one on here.
- If `serve_adapters` has NO access to the connecting peer's CONCRETE platform handle (before it is
  erased to generic `S`) to read its OS credential, STOP -- per PINS.md SS9, the capture point is
  inside `serve_adapters` itself, never inside the generic `handle_adapter_connection` body and
  never by reaching into `src/governance`.
- If GUID routing would require `use crate::transport::...` inside `src/governance/`, STOP -- put
  the key mapping in `ServiceContext`/`src/hub` and leave `Governance` untouched (Decision 4
  amendment: "This lives in `src/hub`, never in `src/governance`").
- If `ipc.rs`'s `serve_adapters`/`handle_adapter_connection` or `src/hub/mod.rs`'s `ServiceContext`
  no longer match PINS.md SS9's description (function names, the generic-vs-concrete split, or
  `ServiceContext`'s `Clone`-ability), STOP and reconcile against the ACTUAL landed shape -- do not
  guess a different call site or re-derive the location yourself.
- If H2 already introduced a session-identity type or a per-session GUID field, STOP and reconcile
  with it rather than duplicating (do not create a second identity type).
- If `run_as_service`, `run_as_adapter`, or `serve_session` no longer exist under those names or no
  longer cleanly separate the two roles (item 6), STOP and reconcile against H2's ACTUAL landed shape
  before wiring the role marker -- do not guess a different call site.
- If satisfying this task would require moving or weakening any NEVER-touch fence below, STOP.

## NEVER touch (this task)
- `src/transport/mcp/tools.rs` (TOOLS_JSON: the 13 trained schemas + `explain`), byte-frozen. No
  exception.
- `tests/tool_schema_fidelity.rs`. No exception; keep green untouched.
- `tests/all_open_golden.rs` + the all-open byte-identity invariant. No exception; the GUID
  mint/bind path MUST be a no-op for a lone all-open session's output and audit.
- `tests/architecture.rs::governance_core_has_no_forbidden_back_edges` (a7): `src/governance/**`
  names no browser/transport/mcp/native/url and no tabId/token/socket type, and gains NO PID/GUID
  concept. `SessionGuid`/`PeerCred`/`SessionRegistry` land in `src/hub` ONLY. H3 SANCTIONED
  EXCEPTION: H3 is the ONE task in this batch allowed to edit `tests/architecture.rs`, solely to
  EXTEND this scanner to ALSO reject the `tabId`/`token`/`socket` identifiers in `src/governance/**`
  (ADR-0030 "Preserved invariants" as amended names H3 as the extender). Every other back-edge rule
  in the scanner stays intact; no other edit to `tests/architecture.rs` is sanctioned.
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`,
  `encode`/`read_message`). No exception; H3 adds documentation only, not framing.
- The MCP JSON-RPC wire and the `notifications/tools/list_changed` line (`server.rs`); the adapter
  is a byte relay, never a rewriter.
- `Browser::attach` single-EXTENSION-link rejection (`AttachOutcome::AlreadyAttached`). Retained;
  H3 does not touch the extension link.
- `src/proc.rs` STAYS (adapter lifecycle + doctor reap) -- do NOT delete or repurpose it here; the
  SERVICE core gains no pid/ancestor/creation-time identity from it.
- At-rest GUID persistence (owner-only 0600 / DPAPI-per-user reuse across processes) is OUT OF
  SCOPE for H3 -- H3 reuse is in-process only. Do not add file/registry persistence here.
- Per-peer quota ENFORCEMENT (Decision 3 / H5) is OUT OF SCOPE -- H3 only provides `PeerCred` as the
  rate-limit key.
