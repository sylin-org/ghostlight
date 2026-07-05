# H8: Local web API = TCP; bind per policy; channels.webapi.from

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 5,
> Decision 9, "Governance schema section (normative)", and "Preserved invariants (and the pinned
> oracles the batch transcribes)"). One task = one commit. Facts below are as-of-authoring
> 2026-07-04 -- RE-READ the named files before relying on any line number.

## Goal

Expose an HTTP/1.1 + WebSocket listener over TCP as a SECOND session source into the same Hub, so a
local app (the ADR's ".NET Automate") drives the browser through the identical governed chokepoint
an MCP adapter uses. The listener reuses the H2 multiplex, H3 identity, and H4 isolation by calling
the SAME `serve_session` / `handle_tools_call` -- it invents no parallel path. It has its OWN
non-sacred, versioned REST/WS vocabulary and NEVER re-serializes the 13 trained schemas. The
listener BINDS PER RESOLVED POLICY: the web adapter ships a builtin default policy fragment
(`channels.webapi.from: [allow: localhost]`, the ADR-0019 builtin layer), so with no overlay it
binds `127.0.0.1` explicitly; a remote bind happens ONLY because a user/org layer opened it.
Authorization is the `channels` policy decided in the PDP; authentication is optional and anonymous
is a first-class principal. Why: ADR-0030 Decision 9 (web API transport, D2 as corrected) and
Decision 5 (authorization is policy; authentication is optional; adapters ship default policy).

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md is the single NORMATIVE design doc. CITE its
   Decisions/sections by name; never restate their semantics.
2. docs/tasks/hub/BOOTSTRAP.md ground rules (authority order, environment facts, per-task procedure,
   failure protocol).
3. This task file.
   If they conflict, the higher wins.

## Current-tree facts (as-of-authoring; RE-READ before relying)

This is the LAST task in the Hub batch (ADR-0030 "Migration": H0 through H8). It depends on the
seams that H0-H7 land. As of 2026-07-04 those seams DO NOT EXIST yet; the executor reaches this task
only after H0-H7 are green. RE-READ the tree and honor the STOP preconditions below before writing a
line.

- CORRECTED 2026-07-04 (PINS.md SS9; RE-READ it in full before relying on any location below): the
  ADAPTER's accept/admission layer -- minting nothing itself, but capturing `PeerCred`, reading the
  session-hello, and calling `SessionRegistry::admit` -- lives in `src/transport/native/ipc.rs`
  (`serve_adapters`/`handle_adapter_connection`), NOT `src/hub/mod.rs`. `src/hub/mod.rs` hosts the
  composition root and `ServiceContext` (SHARED: `Browser`, `ConfigStore`, `Recorder`, plus H3's
  `session_registry` and H4's `owned_tabs`, each a plain field added the same way). RE-READ
  `src/hub/mod.rs` and `src/transport/native/ipc.rs` for the real names before mirroring the
  pattern below.
- `serve_session` / `handle_tools_call` -- the ONE governance chokepoint EVERY transport calls
  (ADR-0030 Decision 2) -- is introduced by H1 (`serve_session<S>(stream, ctx)` + `ServiceContext`)
  and made genuinely multiplexed by H2; H3 adds a plain `guid: SessionGuid` parameter -- NOT
  `Option<SessionGuid>` (every session, including the service's own lone one, carries a real GUID;
  see SS9). RE-READ its real signature before calling it; H8 must call it UNCHANGED, adding only a
  second caller (the web listener) that passes its own minted `guid` for its own sessions, never a
  second implementation.
- H3 lands the adapter-minted opaque GUID identity + local peer-cred admission binding (ADR-0030
  Decision 4), wired in `ipc.rs`'s `serve_adapters`/`handle_adapter_connection` (SS9). H4 lands
  binary-authoritative cross-session tab isolation via `ServiceContext.owned_tabs`, gated inside
  `serve_session`'s read loop (ADR-0030 Decision 6). REVISED 2026-07-04 (PINS.md SS9 forward
  guidance for H8, fresh-eyes review): the web API does NOT call
  `ctx.session_registry.lock().unwrap().admit(...)` -- `SessionRegistry`'s binding model exists to
  stop a DIFFERENT local OS user from hijacking a reused GUID, which has no meaning for a remote TCP
  peer (there is no OS credential to bind, and no pinned formula for one; a raw IP:port breaks
  reuse, a bare IP over-collapses NAT'd clients). The web listener mints a fresh
  `SessionGuid::mint()` per accepted connection (mirroring ONLY the minting half of
  `handle_adapter_connection`'s pattern, not its admission half) and calls
  `serve_session(stream, ctx, guid)` directly, in its OWN `src/hub/webapi.rs` (a distinct TCP
  source, per Decision 9). Trust for a web session is decided entirely by the resolved
  `channels.webapi.from` policy (item 4 below), not by peer-cred binding.
- `src/governance/ports.rs` (RE-READ): `DecisionRequest` (as-of-authoring around lines 292-325)
  is the complete PURE serde-serializable input to a decision. `resource: GoverningResource` is
  stamped there by the enforcement point AFTER an injected concrete resolver runs (resolve-in-adapter,
  decide-in-PDP; see the module doc lines 1-12 and the `resource` field doc). H8 stamps
  `channels.webapi.from` onto `DecisionRequest` the SAME way `resource` is stamped. The `AuditRecord`
  struct (as-of-authoring around lines 192-234) has EXACTLY 14 fields in the frozen order (oracle
  below); `SessionEventRecord` (around lines 244-263) has EXACTLY 6. `Denial` (around lines 154-167)
  is `{ rule, grant_id, denial_id, domain, message }`; `denial_id` is `"D-"` plus 8 lowercase hex
  (`crate::governance::denial::denial_id`).
- `src/governance/dispatch.rs` (RE-READ): `build_record` (as-of-authoring around lines 636-666)
  assembles the 14-key `AuditRecord`; `Governance::authorize` (around lines 315-381) is the single
  PEP arm that consults the held PDP with the caller's `requires`/`resource`. The PDP is
  `crate::governance::ports::PolicyDecisionPoint::decide(&self, req: &DecisionRequest) -> Decision`,
  PURE (no I/O, no live state).
- `src/governance/mod.rs` (RE-READ): declares the governance submodules
  (`audit`, `config`, `denial`, `dispatch`, `enforcement`, `explain`, `manifest`, `ports`,
  `simulate`, `templates`). H8 adds exactly one: `channels`.
- `Grant` (`src/governance/manifest/document.rs`, RE-READ; as-of-authoring shape seen in
  `ports.rs` tests around lines 392-403): `{ id, hosts: HostRules { allow, deny }, allowed:
  Vec<Capability>, description, mode }`. It has NO `channels` field today. The full recursive
  `grant := { id, channels, tools }` grammar is DEFERRED (ADR-0030 Governance schema section); THIS
  batch realizes only the minimal flat `channels.webapi.from` allowlist selector.
- `tests/architecture.rs::governance_core_has_no_forbidden_back_edges` (RE-READ, as-of-authoring
  around lines 109-149): scans every `src/governance/**` `.rs` file and fails if it names
  `crate::browser`/`crate::transport`/`crate::mcp`/`crate::native` or the `url` crate. ADR-0030
  "Preserved invariants" says this test is EXTENDED so the core also names no `tabId`/`token`/`socket`
  type. Whether that extension has already landed by H8 or not, the H8 governance additions
  (`channels.rs`, the `ports.rs` field) MUST name none of those. The allowlist is `Vec<String>` of
  source patterns and the resolved source is a `String`; no socket/token/tabId type appears in
  governance.

The coupling that pins scope: the web listener is a NEW TCP socket and names transport/socket types,
so it lives ONLY in `src/hub/webapi.rs`, never in `src/governance`. The policy TYPE + matcher +
fail-closed validation + the resolved `DecisionRequest` field is the governance side and is the
SINGLE sanctioned `src/governance/**` addition (ADR-0030 Governance schema section: "`channels.webapi.from`
is a governance POLICY ... so it is the single sanctioned addition under `src/governance/**`").

## Required behavior

Cite the ADR decision that mandates each item; keep every listed invariant byte-identical.

1. NEW web listener `src/hub/webapi.rs` (ADR-0030 Decision 9). HTTP/1.1 + WebSocket over TCP,
   exposed ONLY through the service (never adapter-direct). It is a SECOND session SOURCE that
   reuses the same multiplex (Decision 2), identity (Decision 4), and isolation (Decision 6): it
   mints a GUID and registers a session the SAME way the MCP adapter path does, then routes every
   request through the UNCHANGED `serve_session` / `handle_tools_call`. It has its OWN non-sacred,
   versioned REST/WS vocabulary and NEVER re-serializes the 13 trained schemas (ADR-0030 Decision 9;
   Preserved invariants: TOOLS_JSON is byte-frozen). The web vocabulary is out of the sacred surface
   and is NOT governed by `tests/tool_schema_fidelity.rs`.

2. BIND PER RESOLVED POLICY (ADR-0030 Decision 9 + Decision 5). The web adapter ships a BUILTIN
   default policy fragment -- the ADR-0019 builtin layer, contributed per-adapter -- declaring
   `channels.webapi.from: [allow: localhost]`. The resolved bind address is a PURE function of the
   resolved `channels.webapi.from` allowlist:
   - No user/org overlay present: resolves to the builtin `[allow: localhost]`, and the listener
     binds `127.0.0.1` EXPLICITLY, never `0.0.0.0` (ADR-0030 Decision 9, oracle transcribed below).
   - A user/org layer that opened remote (`channels.webapi.from: [allow: "*"]` or specific hosts):
     the listener binds remotely. This is the ONLY way a remote bind happens (ADR-0030 Decision 5:
     "The machine owner enables remote deliberately ... writes a USER-layer policy"). There is NO
     hardcoded boolean, flag, env var, or code gate that opens remote independent of policy. If you
     find yourself adding one, STOP (see STOP preconditions).

3. ANONYMOUS is a first-class principal (ADR-0030 Decision 5: "Anonymous is a first-class principal.
   Loopback + anonymous is zero-friction, no token."). There is NO hardcoded authentication gate and
   NO transport-level ACL. Authorization is the `channels` policy (item 4); authentication is
   OPTIONAL and invoked ONLY when a resolved policy NAMES a principal (`from: [allow: "alice"]`),
   in which case a token or client cert is required for THAT principal. Under all-open + builtin
   loopback, an anonymous loopback connection is ALLOWED with no token. Token mint/revoke machinery
   is NOT part of this task and lives in `src/hub` / the Console, NEVER in `src/governance` (a
   `token` type in governance would violate the a7 extension).

4. `channels.webapi.from` decided in the PDP on the SUBJECT (ADR-0030 Governance schema section:
   "added as a resolved field on `DecisionRequest` (resolve-in-adapter, decide-in-PDP, exactly as
   `resource` is stamped today)"). NEW module `src/governance/channels.rs` owns:
   - the minimal flat `channels.webapi.from` allowlist policy type (a `Vec<String>` of source
     patterns: `"localhost"`, `"*"`, a host, or a named principal) and its membership matcher,
   - the fail-closed load-time validation of its own refinement slice (ADR-0030 Governance schema
     section: "each adapter validates its own refinement slice; fail-closed on an unknown selector"),
   - the PDP-side decision function that takes the resolved connecting SOURCE (the authenticated or
     anonymous subject) and returns Allow when the source is a member of the allowlist, Deny
     otherwise.
   `DecisionRequest` gains a resolved field carrying the connecting source, stamped by the web
   enforcement point BEFORE the pure decision runs, EXACTLY as `resource` is stamped (ADR-0030
   Governance schema section). The DECISION runs in the PDP (`PolicyDecisionPoint::decide`), never
   at the transport layer. `channels.webapi.from` governs SOURCES and unlocks NO tool: D6 (all tools
   free for everyone) is preserved -- if this field would gate which tools EXIST, STOP.
   AUTHOR-PINNED (this file): the `DecisionRequest` field name and the `channels.rs` public
   signatures are pinned by the executor's transcription of the test assertions below; the executor
   MUST NOT rename them to satisfy a compile error without re-reading this file.

5. WS upgrade validation (ADR-0030 Decision 9: "The WS upgrade validates `Origin` against the
   policy and rejects an unexpected `Host` (DNS-rebind defense)."). On the WebSocket upgrade the
   listener validates the request `Origin` against the resolved `channels.webapi.from` policy and
   REJECTS a request whose `Host` header is unexpected for the bound address (DNS-rebind defense).
   This lives in `src/hub/webapi.rs` (it names HTTP/socket types).

6. Subject stamped on every audit record, distinct from `clientInfo` (ADR-0030 Decision 9: "The
   authenticated subject (or the anonymous principal) is stamped on every audit record as a field
   distinct from the untrusted, self-reported `clientInfo`."). The self-reported client identity
   (`AuditRecord.client`, from the untrusted handshake) and the trusted subject (authenticated
   principal, or `anonymous`) are DIFFERENT things and must be distinguishable in the audit stream.
   CONSTRAINT: the `AuditRecord` key order is FROZEN at EXACTLY 14 keys (oracle below) and all-open
   output is byte-identical (Preserved invariants). A lone all-open MCP-stdio session's audit bytes
   MUST NOT change. RESOLVED (PINS.md SS2): the trusted subject does NOT add a 15th audit key; it is
   recorded in the EXISTING `AuditRecord` `identity` field (position 3 of the frozen 14-key order),
   `identity: Option<Identity>` where `Identity { principal, resolved_by }` already exists in
   `src/governance/ports.rs` and is today always built as `None` in `dispatch.rs::build_record`. A
   local adapter session, an anonymous web caller, or any all-open session resolves to `identity =
   None` (BYTE-IDENTICAL to today; the frozen 14-key order is preserved and the all-open bytes stay
   untouched). A web session whose policy named a principal sets `identity = Some(Identity {
   principal: <the named principal>, resolved_by: "webapi" })`. So "distinct from the self-reported
   `clientInfo`" means the EXISTING `identity` field, which is separate from the `client` field; the
   executor MUST NOT invent a 15th audit key and MUST reuse the existing `identity` field per PINS.md
   SS2.

Must stay byte-identical / untouched: TOOLS_JSON (the 13 + explain), the native-messaging 4-byte-LE
framing, the MCP JSON-RPC wire + `notifications/tools/list_changed`, the 14-key `AuditRecord` order,
the 6-key `SessionEventRecord` order, and a lone all-open session's output.

## Tests (BY NAME; assertions pinned)

- Keep green (do not modify): `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`,
  `tests/all_open_golden.rs`, `tests/tool_enforcement.rs`, `tests/tool_schema_fidelity.rs`.

- Add:

  - `tests/channels_policy.rs::webapi_from_is_decided_in_the_pdp_on_the_subject`
    Pinned assertion: build a `DecisionRequest` whose resolved `channels.webapi.from` source field is
    set to a subject, with a governed policy carrying `channels.webapi.from: [allow: "localhost"]`.
    Feed it to the LOCAL PDP's `decide`. A source that IS a member of the allowlist (e.g.
    `"localhost"`) returns `Decision::Allow { .. }`; a source that is NOT a member (e.g. a remote
    host string) returns `Decision::Deny(_)`. The decision is produced by `PolicyDecisionPoint::decide`
    (the PDP), NOT by any transport-layer check -- assert by driving the pure `decide` directly with
    no listener involved (mirrors resolve-in-adapter/decide-in-PDP, ADR-0030 Governance schema
    section). The Deny variant's `denial_id` matches `"D-"` plus 8 lowercase hex
    (`crate::governance::denial::denial_id` scheme, ports.rs `Denial::denial_id` doc). The exact
    channels denial RULE string is PINNED in PINS.md SS7 as the rule label `channel/webapi_from`,
    with `decision = "deny"` and `denial_id` the existing `"D-"` + 8 lowercase hex scheme (assert the
    shape, not a literal). Transcribe PINS.md SS7; do not invent a value.

  - `tests/webapi_auth.rs::webapi_builtin_default_is_loopback_only_with_no_overlay`
    Pinned assertion: with NO user/org overlay, the resolved `channels.webapi.from` equals the web
    adapter's builtin fragment `[allow: "localhost"]`, and the pure "resolved allowlist -> bind
    address" function returns the loopback address. Oracle (transcribed verbatim from ADR-0030
    Decision 9): "the web adapter's builtin default is loopback (`127.0.0.1`, bound explicitly, never
    `0.0.0.0`)". Assert the resolved bind IP is `127.0.0.1` and assert it is NOT `0.0.0.0`.

  - `tests/webapi_auth.rs::enabling_remote_is_a_user_policy_change_not_a_code_gate`
    Pinned assertion: apply a USER-layer policy `channels.webapi.from: [allow: "*"]` over the builtin
    fragment; the resolved allowlist now contains `"*"` and the same pure "resolved allowlist -> bind
    address" function returns a remote bind (NOT `127.0.0.1`-only). Assert there is no separate
    boolean/flag/env input to that function -- its ONLY input is the resolved allowlist -- so remote
    is reachable ONLY because the policy layer changed (ADR-0030 Decision 5: enabling remote is a
    user-layer policy edit, not a hardcoded gate). PINNED in PINS.md SS7: the bind is a resolved
    config value `webapi.bind` (string), default `"127.0.0.1"` (bound EXPLICITLY, never `0.0.0.0`),
    plus `webapi.port` (default `4180`). The Console "Enable remote connections" writes a user-layer
    `webapi.bind` (e.g. `"0.0.0.0"`) AND the matching `channels.webapi.from` entry -- both are
    ordinary policy/config writes, never a code gate. Transcribe PINS.md SS7; do not invent a value.

  - `tests/webapi_auth.rs::anonymous_is_a_valid_principal_under_all_open`
    Pinned assertion: under a lone all-open session (no manifest) with the builtin loopback fragment,
    an anonymous (no-token) loopback connection is AUTHORIZED -- the channels decision returns
    `Decision::Allow` for the anonymous subject on a loopback source, with NO authentication step
    invoked (ADR-0030 Decision 5: "Anonymous is a first-class principal. Loopback + anonymous is
    zero-friction, no token."). Assert NO denial is produced for the anonymous-loopback case.
    Additionally assert the all-open MCP-stdio audit bytes are unchanged: the subject representation
    chosen in Required behavior item 6 leaves a lone all-open `AuditRecord`'s 14-key serialized form
    byte-identical (cross-checked against the 14-key oracle below). PINNED: the trusted-subject
    audit-field representation is the EXISTING `identity` field (PINS.md SS2) -- `identity = None` for
    the anonymous/all-open case, byte-identical to today, never a new key; and if any denial path is
    exercised, its rule label is `channel/webapi_from` with `decision = "deny"` and a `"D-"` + 8-hex
    `denial_id` (PINS.md SS7, assert the shape). Transcribe PINS.md SS2 and SS7; do not invent.

### Oracles transcribed from ADR-0030 "Preserved invariants" (verbatim; do not re-derive)

- `AuditRecord` field order, exactly 14 keys, in order:
  `event_id, ts, identity, client, tool, action, capability, domain, decision, grant_id, denial_id,
  duration_ms, manifest, held`.
- `SessionEventRecord` field order, exactly 6 keys, in order:
  `event_id, ts, identity, client, event, manifest`.
- Web bind default (ADR-0030 Decision 9, verbatim): "the web adapter's builtin default is loopback
  (`127.0.0.1`, bound explicitly, never `0.0.0.0`)".

(`TOOL_TIMEOUT` = 60s and the hop-attributed error strings are pinned in the same ADR section but are
not exercised by this task; do not add or alter them here.)

## Verification (literal commands)

```
cargo build --all-targets
cargo test --test channels_policy
cargo test --test webapi_auth
cargo test --test architecture governance_core_has_no_forbidden_back_edges
cargo test --test all_open_golden
cargo test --test tool_enforcement
cargo test --test tool_schema_fidelity
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## STOP preconditions

- If the H2/H3/H4 multiplex + identity + isolation seams are absent (no `serve_session` /
  `handle_tools_call` single chokepoint, no adapter-minted-GUID session registration path, no
  binary-authoritative tab-ownership path in `src/hub`), STOP. The web API MUST create sessions the
  SAME way an adapter does; it may not stand up a parallel session mechanism. (ADR-0030 Decisions 2,
  4, 6.)
- If you find yourself implementing a hardcoded authentication gate, an ambient cookie, or a
  transport-level ACL instead of a `channels` policy decided in the PDP, STOP. This is the corrected
  D2/D5 (ADR-0030 Provenance: "the auth reconciliation ... correcting the stress-test's
  auth-always-on transport gate").
- If `channels.webapi.from` would gate which TOOLS exist (add, remove, or hide a tool), STOP. It
  governs SOURCES only; D6 (all tools free for everyone) is preserved (ADR-0030 Governance schema
  section).
- If honoring Required behavior item 6 would force a 15th `AuditRecord` key or otherwise change a
  lone all-open session's serialized audit bytes, STOP and escalate the item-6 AUTHOR-MUST-PIN
  question; do not invent a key.
- If any AUTHOR-MUST-PIN value in this file is still unresolved at execution time (the channels
  denial rule/message/denial_id; the remote bind representation; the trusted-subject audit-field
  representation), STOP and request the pinned value; do not derive or invent one.
- If satisfying this task would require moving or weakening any never-touch fence below, STOP.

## NEVER touch (this task)

Global fences (repeated; each names its single sanctioned exception if any):

- `src/transport/mcp/tools.rs` (TOOLS_JSON: the 13 trained schemas + `explain`), byte-frozen. No
  exception. The web API has its OWN vocabulary and NEVER re-serializes these schemas.
- `tests/tool_schema_fidelity.rs`. No exception; keep green untouched.
- `tests/all_open_golden.rs` + the all-open byte-identity invariant. No exception; the web listener
  and the subject field must be no-ops for a lone all-open session's output.
- `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`: `src/governance/**` names no
  browser/transport/mcp/native/url and no `tabId`/`token`/`socket` type; all session/isolation/
  registry/socket code lands in `src/hub`. SANCTIONED EXCEPTION for H8: this task MAY add the
  `channels.webapi.from` POLICY allowlist (`src/governance/channels.rs` + the resolved
  `DecisionRequest` field), which governs SOURCES, never which tools exist. That is the ONLY
  sanctioned `src/governance/**` addition in this task.
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`, `encode`/
  `read_message`). No exception.
- The MCP JSON-RPC wire + the pinned `notifications/tools/list_changed` line (`server.rs`). The MCP
  adapter is a byte relay, never a rewriter.
- `Browser::attach` single-EXTENSION-link rejection (`AttachOutcome::AlreadyAttached`). Retained; not
  touched by this task.

Task-specific fences:

- Do NOT route the web API over the adapter <-> service LOCAL boundary (the owner-only pipe/UDS in
  `ipc.rs`). The web API is a NEW TCP listener; the local boundary keeps its owner-only pipe/UDS
  trust model (ADR-0030 Decision 9: "transport-as-a-port with two trust models; only the app-facing
  web API is TCP").
- Do NOT gate which tools EXIST from `channels.webapi.from`. It governs SOURCES only.
- `channels.webapi.from` (the `channels.rs` module + the resolved `DecisionRequest` field) is the
  SINGLE sanctioned governance-side addition. Add no other type, field, or module under
  `src/governance/**`. Add no `socket`/`token`/`tabId` type there.
- Do NOT modify `serve_session` / `handle_tools_call`; add the web listener as a SECOND caller only.
