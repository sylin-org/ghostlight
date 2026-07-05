# 0030. Ghostlight Hub: the multi-client orchestrator service

- Status: Accepted
- Date: 2026-07-04

## Relationship to other decisions

- REPEALS ADR-0004 (reject-second-session) at the MCP-client layer. It does NOT repeal the
  single-physical-extension-link rejection in `Browser::attach`
  (`AttachOutcome::AlreadyAttached`), which is a different invariant and is retained.
- AMENDS ADR-0029 (process-lifecycle hygiene): the parent-death watchdog/reaper is re-scoped to
  the thin adapter; the persistent service is never parented to a client and shuts down on
  idle-grace, not parent-death.
- BUILDS ON ADR-0019/0020 (layered configuration + org policy locks), ADR-0022/0024/0025
  (capabilities, one-loader pipeline, hot-reload), and ADR-0027/0028 (open-core + tripwire
  licensing, zero behavioral gating). RECONCILES ADR-0020's "no web console" non-goal (2026-07-05
  amendment on that ADR) with Decision 9's local, loopback-pinned Console: that non-goal rejected a
  remote/hosted organization-policy-authoring portal, which stays rejected; the Console is a
  single-machine, read-mostly operational view, never an authoring or deployment surface for org
  policy. Decision 9's embedded HTTP server is also the revisit ADR-0019 Decision 5 anticipated
  ("if the product family needs a shared local dashboard"; see that ADR's matching amendment).
- Supersedes the "persistent service = deferred Phase B, single-session lease" stance in
  `docs/design/ghostlight-service-architecture.md` and resolves its open decisions (section 9).
- Preserves the sacred invariants (see "Preserved invariants").

## Context

Ghostlight today is a dual-role binary: an MCP server (stdio) and a native-messaging host,
coordinating over a single-owner local IPC endpoint. That single-owner model has repeatedly
produced the same class of pain: a second MCP client hits `Error::SessionBusy` and cannot serve
the extension (ADR-0004); when the owner dies the survivor does not promote, orphaning the
extension and forcing a manual restart; and lifecycle bugs (native-host zombies, doctor
pipe-busy, kill-on-close) all live on the process-lifecycle axis. Parallel agents are the 2026
norm, and the family vision is a matrix of input adapters (MCP-stdio, a local web API) times tool
adapters (browser now; shell, filesystem, network later) through one governed chokepoint. The
single-driver model cannot express that.

The move that every independent design converged on: make one persistent per-user SERVICE the
sole owner of the single Chrome-spawned extension link, and turn clients into multiplexed
SESSIONS rather than rival endpoint owners.

## Decision 1: four roles, one binary

Four process kinds, all the same portable binary (role selected by argv, extending main's
existing `chrome-extension://` detection):

```
 MCP client (Claude Code/CLI)     MCP client (2nd editor)        local app (.NET "Automate")
        |  stdio JSON-RPC               |  stdio                        |  HTTP/WS (TCP)
        |  (13+explain SACRED)          |                               |  (own vocabulary)
        v                               v                               |
  ghostlight mcp ADAPTER          ghostlight mcp ADAPTER                 |
  (thin; per-client; dies              (thin; per-client)               |
   with its editor; the                                                 |
   ADR-0029 watchdog lives here)                                        |
        |  owner-only pipe/UDS <- ADAPTER/CONTROL ENDPOINT (session-hello) |  web-API listener
        +----------------------+----------------------+                 |  (a session SOURCE
                               |                                        |   into the same core)
                               v                                        v
   +============================ ghostlight SERVICE (the Hub) ============================+
   |  persistent . per-user . NEVER admin/SYSTEM . detached (no client/Chrome job)        |
   |  SOLE owner of the ONE extension link (its own endpoint) + the correlation map        |
   |  ServiceContext (shared): Browser handle, ConfigStore, audit Recorder                |
   |  session table {GUID -> Subject, owned-tab set, per-session Governance}               |
   |  GOVERNANCE FIRST: one serve_session/handle_tools_call ALL transports call            |
   +======================================================================================+
                               ^  EXTENSION ENDPOINT: owner-only pipe/UDS, NO hello (server speaks
                                  first, exactly as today); relay re-dials, sends nothing first
                    ghostlight RELAY (Chrome-spawned native host; ephemeral; dumb byte pipe;
                                       ONLY connects, NEVER spawns the service)
                               ^  native messaging (4-byte LE frames)
                    MV3 extension (POLICY-FREE; owns all durable browser state: tabs, tab
                                    GROUPS, debugger, console/network buffers, auth/cookies)
                               ^  CDP
                    the user's real authenticated Chrome
```

The SERVICE owns only reconstructible coordination state, so its crash loses nothing durable
(the extension holds durable browser state and survives a service restart). The SERVICE runs
STANDALONE (supervisor-launched at login or user-launched via `ghostlight service`; Decision 8), so
it is never trapped in an editor's or Chrome's job object. The ADAPTER only connects and relays and
dies with its client (asking the supervisor to start the service if it is not yet up; it never
spawns an in-job child and never runs governance). The RELAY only ever connects. The EXTENSION stays
policy-free.

Two local endpoints, not one demux (amended 2026-07-04; see Provenance). The local core exposes TWO
owner-only pipe/UDS endpoints, distinguished by which door a peer arrives at rather than by a
discriminator byte on a shared door:

- the EXTENSION endpoint -- the existing well-known relay name -- keeps its exact server-speaks-first
  contract: the service accepts, `Browser::attach` claims the one physical link, and the service may
  write a queued tool_request before the extension has sent a byte. NO hello frame is added to this
  endpoint, so the sacred `host.rs` wire, the policy-free relay, and every fake-extension test stay
  byte-for-byte unchanged.
- the ADAPTER/CONTROL endpoint -- a separate well-known name, and the single-instance election target
  (Decision 8) -- carries the multiplexed, speak-first sessions. Its first frame is a session-hello
  (Decision 4); a `role` member distinguishes only the speak-first peers (`adapter` vs the reserved
  `control`). The extension is never one of these roles.

The endpoint is the peer's identity; the extension is a singleton, spoken-to, sacred-wire peer and
gets its own door rather than being forced to speak first behind a shared demux. This is fewer
meaningful moving parts than one endpoint plus a role-negotiation layer, and it is why nothing in
this design touches the extension's wire or its golden tests.

Role is a fact the process can never be wrong about (amended 2026-07-04). The role is decided by
ARGV at startup (the `service` subcommand makes this process the SERVICE; a bare invocation makes it
the ADAPTER) and recorded once in a single hub-owned marker for the process's life. The two seams
where a role mismatch would be a genuine defect -- the governance chokepoint (`serve_session`/
`handle_tools_call`, SERVICE only) and the supervisor-start / self-heal path (Decision 8, ADAPTER
only) -- each assert against that marker as their first action and panic immediately, by name, on a
mismatch. This is deliberately narrow: it is NOT "every feature checks a flag" (a runtime check a
future feature could simply forget to add); it is a fail-loud guard at exactly the two places a
violation would mean the SoC boundary already failed by construction elsewhere. The structural
separation (the ADAPTER's code never calls governance; the SERVICE's code never calls the
supervisor-start path) remains the primary guarantee; the assertion is the test-time, crash-loud
backstop if that separation is ever accidentally breached by a future change. Pinned in
`docs/tasks/hub/PINS.md` SS8.

## Decision 2: HubCore / ServiceContext vs per-session state

Extract the composition root into a free-licensed `src/hub` module hosting `HubCore`. Split state:

- SHARED per service (`ServiceContext`): the one `Browser` handle (its `Arc<AtomicU64> next_id`
  and `Arc<Mutex<HashMap>> pending` already correlate replies by id across clones, so multiplex
  needs no new correlation code), the `ConfigStore`, the audit `Recorder`.
- PER SESSION: the `Governance` facade (it already holds grants + client + manifest hash, and is
  cheap), the opaque subject GUID, and the owned-handle set.

The single governance chokepoint is ONE `serve_session` / `handle_tools_call` that EVERY transport
(MCP adapter, web API) calls. It is never re-implemented per adapter. Per-session `Governance` is
retained rather than a separate HubCore/SessionContext split (it is already session-shaped).

## Decision 3: D1 -- the honest singleton queue

The single MV3 service worker plus the single native port is an accepted, documented serialization
bottleneck, framed as an availability SECURITY property, not a hidden limitation: fair ordering,
truthful failure on a real drop, per-peer-identity mint/group quotas (never a single global cap,
which is itself a lockout DoS), and MANDATORY screenshot chunking so a large payload (up to the
existing `MAX_MESSAGE_LEN` cap) cannot head-of-line-block the shared port and starve honest
sessions. We do not engineer around the singleton; we queue honestly.

## Decision 4: identity model (adapter-minted GUID; core stays PID-agnostic)

Session identity is minted by the thin ADAPTER, not the orchestrator: a CSPRNG UUIDv4 GUID
presented as the `guid` member of the adapter/control endpoint's session-hello (Decision 1; never on
the extension endpoint, which sends no hello). The service routes and isolates by that opaque GUID only;
the governance core gains NO concept of pid / ancestor / creation-time. Same adapter process
reuses its GUID (same group); a new adapter process mints a new one (D7: two adapters in one
editor -> two GUIDs -> two groups).

Amendment to the transport-side (NOT the core): the LOCAL accept layer in `src/hub` captures the
connecting peer's OS credential purely for admission control -- to bind a GUID to its minting peer
(refuse a GUID presented by a different peer, except the sanctioned reuse path) and as the
per-peer rate-limit key. The GUID is treated as secret material in every log/audit sink; if
persisted for reuse it is stored owner-only (0600 / DPAPI-per-user), never in client config. This
lives in `src/hub`, never in `src/governance` (the a7 arch-test holds).

## Decision 5: authorization is policy; authentication is optional; adapters ship default policy

There is ONE decision mechanism: the policy engine. Whether a source may connect is the `channels`
axis of the grant grammar (Decision: governance schema), evaluated at the same chokepoint as every
other decision. There is NO separate, hardcoded authentication gate and NO transport-level override
of policy.

- Each adapter (input or tool) ships a DEFAULT POLICY FRAGMENT: the "if no overlay is present, use
  this" declaration. This is exactly the BUILTIN layer of the ADR-0019 five-layer config, now
  contributed per-adapter (federated). The web-API adapter's builtin default is
  `channels.webapi.from: [allow: localhost]`. That default value, on one axis, IS the
  catastrophe-avoidance -- not a code gate.
- OPEN MEANS OPEN. All-open (no manifest) opens the tool / resource / class axes fully; the channel
  axis simply resolves to the adapter's builtin default (web API: loopback). Anonymous is a
  first-class principal. Loopback + anonymous is zero-friction, no token.
- The machine owner enables remote deliberately: the Console's "Enable remote connections" (with a
  plain disclaimer) writes a USER-layer policy (`channels.webapi.from: [allow: "*"]` or specific
  hosts). Remote is then open, no auth, as intended. We are not babysitting the user; we ship a
  sensible default they consciously change.
- Authentication is policy-driven and optional: a token or client cert is needed ONLY when a policy
  names a principal (`from: [allow: "alice"]`). Issuing a token (Console / `ghostlight token`) is a
  convenience for naming and scoping a principal, never a prerequisite for use.
- Enterprise pushes an ORG-MANDATORY layer that locks `channels.webapi.from` (or denies the web
  adapter); it renders read-only in the Console and shuts remote down immediately.
- TLS for a remote bind is RECOMMENDED (so a token, when used, is not sniffable) but is the user's
  configuration choice, not an enforced precondition.

## Decision 6: cross-session isolation is authoritative in the SERVICE

The service tracks, per session (keyed on Decision 4's GUID), the set of tabIds that session
created (`tabs_create_mcp`) or legitimately adopted. Before routing any tab-scoped call OR
resolving policy for it -- i.e. BEFORE any `tab_url` probe -- the service refuses a tabId the
session does not own, returning a uniform "unknown tab" result that leaks neither the tab's
existence nor its host (closing the cross-session host-enumeration channel). Owned-handle sets live
in `src/hub` (opaque handles that may name a tabId); the governance core stays handle-agnostic. The
extension's per-group checks remain defense-in-depth only. A lone all-open session owns everything
it touches, so the all-open path stays a byte-identical pass-through.

## Decision 7: hold, panic-kill, and the kill audit are GLOBAL

Take-the-wheel hold and the panic kill switch are properties of the ONE shared browser, evaluated
once and applied to every session regardless of GUID -- never keyed per session. This is already
structurally true (`held`/`killed` latch on the shared `Browser` handle; a kill drains all pending
and gates all future calls). The one forced change: the single-consumer kill hook
(`Browser::on_session_killed`, "Registering a second hook replaces the first ... one Governance per
session") becomes a fan-out registry so every live session's subject gets exactly one
`session_killed` audit record; one group's extension reconnect must not clear a global kill for
other groups. A browser-originated hold (the context-free popup gesture) is likewise global; a
per-session pause is exposed only as a programmatic adapter/API verb.

## Decision 8: lifecycle -- always-ready standalone service, thin-only adapters (amended 2026-07-04)

The persistent service is a STANDALONE process the user always has running. It is started one of
two ways, both per-user and zero-admin, and NEVER by an editor as an in-job child:

- AUTO-START (the installed default): an OS supervisor registered at install (Windows Task
  Scheduler LeastPrivilege logon task; macOS launchd LaunchAgent; Linux systemd --user
  Restart=on-failure) starts `ghostlight service` at login and restarts it on crash. The installer
  ALSO starts it once at install time, so the first session is already up.
- DIRECTLY (dev / portable): the user runs `ghostlight service` themselves.

Because the OS (or the user) launches the service standalone, it sits in NEITHER an editor's NOR
Chrome's job object BY CONSTRUCTION. There is nothing to break away from. This DELETES the earlier
"the adapter spawns a detached, job-breakaway-verified child" mechanism entirely: Windows in-job
breakaway (`CREATE_BREAKAWAY_FROM_JOB`) is not reliably achievable (it fails inside a kill-on-close
job that lacks `JOB_OBJECT_LIMIT_BREAKAWAY_OK`), and it was the source of the H6 block (see
Provenance). The service runs as the logged-in user, NEVER admin/SYSTEM (a browser-driving service
must not exceed the medium integrity of the user's Chrome). It has NO client parent and runs NO
parent-death watchdog; it shuts down only on an idle-grace window (no live sessions AND the
extension link gone for the window). To defeat a process that squats the well-known endpoint name
without knowing the per-install secret, the service proves possession of that secret to the adapter
on connect (anti-squat) before any GUID/pairing flow proceeds (defense-in-depth: it stops a naive
or cross-user squatter; a determined same-user process can read any same-user file, so this is not
a same-user sandbox).

Every MCP invocation is ALWAYS a thin ADAPTER -- never a service, never a promoted service, never a
spawner of an in-job child. It connects to the service, sends the session-hello (Decision 4),
relays its stdio as a pure byte pipe, and dies with its editor (the ADR-0029 parent-death watchdog
and reaper live HERE, on the adapter). If the service is not reachable, the adapter asks the
supervisor to start it (an idempotent, out-of-job OS call: Windows `schtasks /run`, macOS
`launchctl kickstart`, Linux `systemctl --user start`), waits briefly, and connects; if that cannot
be done (no supervisor registered) it fails with a clear, actionable message. The adapter NEVER
spawns an in-job child service and NEVER runs a governance decision.

Role is selected by ARGV, deterministically, not by a race: the `service` subcommand is the
SERVICE; a bare invocation is the ADAPTER; a `chrome-extension://` positional is the RELAY
(connect-only, unchanged). The endpoint create-claim is NOT a role election; it is only the
service's own single-instance guard (a second `ghostlight service` loses the claim and exits
cleanly). The relay never spawns the service. Governance policy (the manifest / ConfigStore) is
loaded ONCE by the service (`ghostlight service --manifest ...`); a `--manifest` on a bare adapter
invocation is ignored (the running service's policy governs all sessions), with a one-line warning.

## Decision 9: web API transport (D2 as corrected)

The app-facing web API is HTTP/1.1 + WebSocket over TCP, exposed only through the service (never
adapter-direct), a SECOND session source that reuses the same multiplex (Decision 2), identity
(Decision 4), and isolation (Decision 6) -- it calls the SAME `serve_session`/`handle_tools_call`.
It has its OWN non-sacred, versioned REST/WS vocabulary; it NEVER re-serializes the 13 trained
schemas. The listener binds per the resolved `channels.webapi.from` policy: the web adapter's
builtin default is loopback (`127.0.0.1`, bound explicitly, never `0.0.0.0`); a remote bind happens
only because a user/org layer opened it (Decision 5). The WS upgrade validates `Origin` against the
policy and rejects an unexpected `Host` (DNS-rebind defense). The authenticated subject (or the
anonymous principal) is recorded in the EXISTING `identity` field of the audit record (position 3 of
the frozen 14-key order; `Option<Identity>`, today always built as `None`), which is distinct from
the self-reported `client` field -- so this adds NO new audit key and all-open stays byte-identical
(anonymous/local resolves to `None`). The batch pins this in `docs/tasks/hub/PINS.md` section 2. The adapter/control sessions and the
extension link are two SEPARATE owner-only pipe/UDS endpoints (OS same-user ACL; Decision 1): the
extension endpoint stays hello-free and server-speaks-first, the adapter/control endpoint carries the
session-hello over that TCP-style session source -- transport-as-a-port with two trust models; only
the app-facing web API is TCP.

The local Console (a loopback-pinned static site served from the same HTTP stack, embedded in the
binary) is the control plane: live sessions/groups, a provenance-aware config view (per key: value,
which of the five layers set it, and whether an org-mandatory lock makes it read-only), and token
mint/revoke. Config changes and token issuance are audited governance events. The Console is the
"Enable remote connections" surface that writes the user-layer `channels.webapi.from` policy.

## Governance schema section (normative)

The grant is a uniform recursive allow/deny model over axes (WHO x WHAT x WHERE x HOW), one matcher
per axis (exact for channels/tools, glob for hosts, set for classes):

```
AxisNode := { allow?: [member|pattern], deny?: [member|pattern], <memberId>?: Refinement }
Refinement := AxisNode & { do?: [RAWX class] }        // resource sub-axis + effect classes
grant := { id, channels: AxisNode, tools: AxisNode }   // channels members refine with `from`;
                                                       // tools members refine with on/except + do
```

For THIS batch the only realized channel selector is a minimal flat `channels.webapi.from`
allowlist, added as a resolved field on `DecisionRequest` (resolve-in-adapter, decide-in-PDP,
exactly as `resource` is stamped today). The full recursive federated grammar
(channels/tools/on|except/do) is DEFERRED to its own core-only ADR (with re-pinned denial-ids and
simulate goldens). The ADAPTER DEFAULT POLICY (Decision 5) is the ADR-0019 builtin layer,
contributed per-adapter and overlaid by preset/user/org-recommended/org-mandatory. Load-time
validation is federated: the core validates the skeleton; each adapter validates its own refinement
slice; fail-closed on an unknown selector.

`channels.webapi.from` is a governance POLICY (it governs authenticated SOURCES, never which tools
EXIST), so it is the single sanctioned addition under `src/governance/**` (commercial license per
ADR-0027). It unlocks no tool; D6 (all tools free for everyone) is preserved.

## Preserved invariants (and the pinned oracles the batch transcribes)

Never disturbed, in any phase:

- The 13 trained MCP tool schemas + `explain`, byte-frozen (`src/transport/mcp/tools.rs` TOOLS_JSON,
  pinned by `tests/tool_schema_fidelity.rs`).
- The native-messaging wire: 4-byte LE length prefix, `MAX_MESSAGE_LEN`, `encode`/`read_message`
  (`src/transport/native/host.rs`), shared with the policy-free extension.
- The extension-facing contract, not just the `host.rs` wire but the server-speaks-first ordering the
  relay and every fake-extension test double depend on. The extension link keeps its own dedicated
  endpoint with NO hello frame added (amended 2026-07-04); a lone client's extension path is
  byte-for-byte unchanged. (Adding a hello-first demux to this endpoint is exactly what a first H2
  attempt did, and it deadlocked the spoken-to extension against `tests/all_open_golden.rs` -- see
  Provenance.)
- All-open output-identity: a lone all-open session's CLIENT-VISIBLE output stays byte-identical
  through H0-H9; every new session / isolation / lifecycle path is a no-op for the client-visible
  bytes of a lone all-open session. `tests/all_open_golden.rs` asserts this; from H6 its harness
  drives the standalone-service + thin-adapter topology (the delight-relevant assertions -- redaction
  wired at the chokepoint, advertised surface == the sacred fixture -- are the invariant and are
  preserved verbatim; the spawn choreography is movable scaffold, per the "only delight is sacred"
  provenance entry).
- The a7 arch-test (`tests/architecture.rs::governance_core_has_no_forbidden_back_edges`):
  `src/governance/**` names no browser/transport/mcp/native type nor the `url` crate. All
  session/multiplex/isolation code lands in `src/hub`, so the core additionally names no
  tabId/token/socket type by construction; H3 extends the a7 scanner to enforce that too (the one
  sanctioned edit to `tests/architecture.rs` in this batch).
- Single portable binary, zero runtime deps, no dylib; the policy-free extension.

Pinned oracles (transcribed by the executor, never re-derived):

- Audit record field order, exactly 14 keys, in order:
  `event_id, ts, identity, client, tool, action, capability, domain, decision, grant_id,
  denial_id, duration_ms, manifest, held` (`src/governance/dispatch.rs` `build_record`;
  `tests/audit_recorder.rs`).
- Session-event record field order, exactly 6 keys, in order:
  `event_id, ts, identity, client, event, manifest` (`SessionEventRecord`).
- `TOOL_TIMEOUT` = 60s. The hop-attributed error strings (verbatim, `src/transport/executor.rs`):
  not-connected `"Browser extension not connected"`; kill
  `"The user ended the browser session (kill switch)"`; disconnect
  `"Browser extension disconnected before responding"`; timeout `"Tool request timed out after 60s"`.
  All render under the `[hop: extension]` prefix (kill/not-connected/disconnect) as their tests pin.
- The single-consumer kill hook to fan out: `Browser::on_session_killed`
  (`src/transport/executor.rs`, "Registering a second hook replaces the first").

## Migration (implemented by the docs/tasks/hub prompt batch)

Every prefix leaves a green, shippable tree; the process-lifecycle risk lands last on a proven base.

- H0 Extract `HubCore` composition root into `src/hub` (pure code move; single stdio session).
- H1 Make the MCP loop transport-generic: `serve_session<S>(stream, ctx)` + `ServiceContext`
  (byte-identical single-session refactor).
- H2 Persistent SERVICE + thin ADAPTER + genuine multiplex; repeals ADR-0004; fan out the kill hook
  (the one large coupled commit; old P1+P3 collapse here).
- H3 Adapter-minted GUID identity + local peer-cred admission binding.
- H4 Binary-authoritative cross-session tab isolation (ownership-before-probe).
- H5 Reconnect grace window + honest bounded queue (orthogonal; any time after H2).
- H6 Always-ready standalone service (`ghostlight service`) + thin-only adapters + supervisor
  self-heal + anti-squat + idle-grace; reaper re-scoped to the adapter; delete the core's
  proc-identity role AND the on-demand-spawn / job-breakaway mechanism (it is never built).
- H7 Tab-group-per-session presentation (extension owns the durable group; groups on request only).
- H8 Local web API = TCP; bind per policy (builtin loopback default); `channels.webapi.from` in the
  PDP; anonymous is a valid principal; the Console control plane.
- H9 Installer auto-start: register + start the per-user OS supervisor (Windows Task Scheduler /
  macOS launchd / Linux systemd --user) that keeps `ghostlight service` warm and restarts it on
  crash; unregister + stop it on uninstall. This is what makes "always-ready" true for the installed
  product; the H6 adapter self-heal targets the same supervisor identifiers.

## Consequences

- Positive: N clients multiplex through one governed door; the orphan/restart-dance/reject-2nd bug
  class is designed out; the matrix of input x tool adapters through one chokepoint becomes real;
  the largest core deletion the project has earned (proc-identity/liveness) becomes possible at H6.
- Negative: one large coupled commit (H2); the singleton browser link is an irreducible shared
  blast radius (Decision 3 makes it honest, not invisible); a new lifecycle surface (the standalone
  always-ready service + thin-only adapters) that H6 lands on a proven base, with the OS supervisor
  (NOT in-editor job breakaway) as the strong-lifetime guarantee; and a dependency on the H9
  installer registering that supervisor for the zero-friction "always up" experience (absent it, the
  user runs `ghostlight service` themselves, or the adapter's self-heal reports it clearly).
- Out of scope (future, each behind its own ADR): the authenticated REMOTE adapter as a product
  (mTLS/PoP, per-principal scoped manifests, threat-model ADR); the full recursive federated grant
  grammar; a governed shell/filesystem tool adapter; `upload_image`; OIDC/SAML/LDAP; a remote policy
  service; manifest signing; Firefox.

## Provenance (so decided questions are never reopened)

- The base topology and multiplex inversion come from the 2026-07-04 design pass (4 independent
  architectures + adversarial red-team + specialists), consolidated to the dedicated-service spine
  (Approach 2) grafted with the spawn-on-demand fallback (Approach 3) and all four specialists.
- User decisions D1-D7 (verbatim): honest singleton queue; the app-facing web API is TCP not pipes;
  cross-session isolation authoritative in the service; the adapter stands up/connects the service
  and the relay only connects; identity is an adapter-minted opaque GUID with a PID-agnostic core;
  no new open-core tool gating; two adapter processes -> two groups.
- Stress-test-forced hardening (accepted): the kill-audit fan-out; GUID CSPRNG + peer-binding +
  at-rest protection; ownership-before-probe with a leak-free "unknown tab"; the detached/anti-squat
  lifecycle; per-peer (not global) quotas + mandatory screenshot chunking; the authenticated subject
  as a distinct audit field.
- The auth reconciliation (RATIFIED by the user, correcting the stress-test's "auth-always-on
  transport gate"): OPEN MEANS OPEN. Authorization is policy (the `channels` axis); authentication
  is optional and only invoked when a policy names a principal; each adapter ships a builtin default
  policy fragment (web API: loopback) as the sensible, user-overridable default; enabling remote is
  a user-layer policy edit made from the Console, not a hardcoded gate. The stress-test's
  refuse-remote-bind / mandatory-auth position is explicitly rejected as a separation-of-concerns
  violation.
- The two-endpoint amendment (2026-07-04, correcting Decision 1 as first ratified): the original
  single "role-demuxed core endpoint" was split into a hello-free EXTENSION endpoint plus an
  ADAPTER/CONTROL session-hello endpoint after implementing H2 surfaced a deadlock -- a hello-first
  demux on the shared endpoint inverts the extension's load-bearing server-speaks-first contract (the
  extension is spoken-to; it has nothing to send until asked), which `tests/all_open_golden.rs`
  faithfully encodes, so the block was the test doing its job, not a stale double. The `ext` hello
  role is deleted and the extension is identified by its endpoint; the session-hello (with the GUID)
  rides the adapter/control endpoint only. Fewer meaningful moving parts, and no sacred file is
  touched. The batch (PINS.md SS1, H2, H3) is re-authored to match.
- The always-ready-service amendment (2026-07-04, ratified by the user, correcting Decision 8's
  spawn-on-demand + implicit in-process-service model): H2's scaffold made whichever process won the
  endpoint claim host the service IN-PROCESS and serve its own stdio as the first session. That welds
  the service's lifetime to the first client: when the first editor closes, `run_as_service` returns
  and `process::exit` tears down `serve_adapters`, orphaning every other client -- the exact orphan
  cascade this ADR exists to kill, latent in the scaffold and first exposed at H6. Decision 8's
  original fix (the adapter spawns a detached, job-breakaway-verified child) is not reliably
  achievable on Windows (`CREATE_BREAKAWAY_FROM_JOB` fails inside a kill-on-close job lacking
  `JOB_OBJECT_LIMIT_BREAKAWAY_OK`), which is what BLOCKED H6. The ratified resolution has fewer, more
  meaningful moving parts: the service is a STANDALONE, always-ready process (supervisor-launched at
  login + started at install, H9; or user-launched via `ghostlight service`), and EVERY MCP
  invocation is a thin adapter that only connects and relays. No promotion, no in-process service, no
  on-demand in-job child, no breakaway. Role is decided by ARGV, not by a race. The service is never
  in an editor/Chrome job by construction, so the strong lifetime guarantee needs no breakaway
  trickery; the OS supervisor (H9) provides it. If the service is down, the adapter asks the
  supervisor to start it (idempotent, out-of-job) and otherwise reports a clear message. The batch
  (PINS.md SS5, H6, and the new H9) is re-authored to match; H2's in-process-service sub-decision is
  superseded, while its multiplex, identity, isolation, and queue work all stand unchanged.
- The sacred surface is user DELIGHT (ratified by the user, 2026-07-04). The single sacred thing is
  the user's delight; every "never touch" fence exists only insofar as it protects that. This
  ELEVATES the 13+`explain` trained tool schemas (Claude's trained behavior on them IS the delight;
  breaking `TOOLS_JSON` degrades the model, so `tests/tool_schema_fidelity.rs` stays byte-frozen with
  no exception, ever) and the extension `host.rs` wire (breaking it breaks the extension). It also
  makes explicit that tests which merely encode the OLD process topology are non-sacred SCAFFOLD:
  their delight-relevant ASSERTIONS are preserved (and may be strengthened) while their harness is
  updated to track the architecture. Concretely, from H6, `tests/peer_death.rs`,
  `tests/all_open_golden.rs`'s spawn choreography, and `tests/mcp_protocol.rs`'s spawn helpers drive
  the standalone-service + thin-adapter topology, preserving every assertion (redaction wired at the
  chokepoint, advertised surface == the sacred fixture, a native host exits when its real peer dies).
  This is NOT license to weaken a genuine invariant: the all-open CLIENT-VISIBLE output stays
  byte-identical, the trained schemas and the native wire stay frozen, and the a7 core boundary holds.
