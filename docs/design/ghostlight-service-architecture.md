# Ghostlight Architecture: Service Topology and Family Baseline

Status: Design discussion. NOT ratified. Date: 2026-07-02.

This is a working design document. It records a discussion and the analysis behind it so we can
return to it later. Nothing here is a committed decision yet. The pieces that get ratified should
graduate into numbered ADRs (this document points out which).

## Why this document exists

The trigger was an IPC bug (`ghostlight doctor` reporting a pipe-busy error during a live session).
The question quickly grew past the bug into an architecture question: should the browser session
state live in its own persistent process, decoupled from the ephemeral MCP-client process? And past
that, into the real ask: a meaningful architecture baseline for the whole Ghostlight family that
centralizes complexity in the right place, promotes DDD and separation of concerns, allows growth,
and supports a plugin model where every MCP service benefits from one central policy engine.

Two adversarial design panels were run in-session to pressure-test the options (see Sources at the
end). Their conclusions are summarized here.

## TL;DR (the emerging baseline)

- The code architecture is the same regardless of process topology. Build it as a dependency-inverted
  governance core plus a browser domain plugin, in one crate, with the dependency direction enforced
  by a fail-closed CI test. Defer physical crate extraction to product #2 (rule of two, ADR-0021).
- The runtime target the user has chosen is a persistent local service that clients connect to via
  thin, spawnable adapters. This is a deliberate move away from the ephemeral-stdio-server convention,
  justified by the family vision and by a future authenticated web adapter.
- The policy engine runs on both the inbound path (allow/deny decision) and the outbound path
  (response transforms plus audit). Modules are self-describing (they declare their tools and their
  config keys). Config is federated across modules and overlaid with OS/org/user policy layers,
  including an immutable (org-locked) level.
- Sequence the work so the process-lifecycle risk lands last, on a proven base: build the pipeline
  in one process first, then split the persistent service out as its own tested step, then add the
  web adapter only after governance is mature and behind its own security ADR.

## 1. The runtime-topology sub-decision

### The bug that started it (BUGFIX-06)

The Windows `serve()` accept loop in `src/native/ipc.rs` keeps exactly one spare pipe instance, then
calls `browser.attach(connected).await`, which blocks the whole loop for the connection's entire
lifetime (the real, long-lived native-host session). A short-lived third-party probe (like `doctor`,
which opens then immediately drops the pipe) consumes that one spare, and nothing replaces it until
`attach()` returns. Every later `doctor` call then sees ERROR_PIPE_BUSY (231) until the real session
ends. The live session is never affected; only repeated health checks fail. The bug is cosmetic.

The naive fix (spawn `attach()` instead of awaiting it) is unsafe as-is, because `Browser::attach`
unconditionally sets and clears the single shared `outgoing`/`connected` state, so a concurrently
spawned stray connection's near-instant close would wipe out the live session's sender.

### Four models

- Model A: minimal in-place fix. Keep the topology; make the accept loop accept-ahead (spawn the
  handler, keep a spare ready) and give `attach` an atomic single-slot claim so a stray connection is
  dropped without touching the live session's state.
- Model B: persistent broker daemon. A third role of the binary owns the state and all sockets; the
  MCP-client process becomes a thin shim. This is the daemon / decouple-lifetime option.
- Model C: self-electing broker. Whichever mcp-server wins the endpoint race is the broker; losers
  become shims that forward to it; on broker death a surviving shim re-elects. No dedicated daemon.
- Model D: control-plane / data-plane split. A second, read-only IPC listener inside the mcp-server
  answers health/status/governance verbs, so the browser data pipe is never touched by out-of-band
  operations.

### Verdict: three adversarial judges

Totals (higher is better), across a lean-internals purist, a stage-2-governance architect, and a
reliability/lifecycle skeptic:

| Model | Lean | Governance | Reliability |
| --- | --- | --- | --- |
| A minimal fix | 25 | 24 | 24 |
| D control/data split | 22 | 23 | 24 |
| B daemon | 16 | 18 | 17 |
| C self-electing broker | 16 | 16 | 16 |

### Conclusion

Ship D (control-plane split) as the near-term answer: it fixes the bug at its root (doctor stops
touching the data pipe), adds no new process-lifecycle surface, and is the natural out-of-band home
for the stage-2 governance verbs that must not ride the tool-call path (kill switch, take-the-wheel
pause, policy explain/simulate, manifest identity, audit counters). Pair it with A (the attach-gate)
when any process other than doctor could connect to the data pipe. Defer B (daemon) behind a
reversible flag until real warm-session or multi-client demand appears. Reject C for now (re-election
sits on the process-lifecycle axis that has burned this project twice).

Key insight: the control plane is exactly the surface a future daemon inherits verbatim. So
"PDP in-process now, PDP out-of-process later" is a wiring change, not a rewrite. Note: the user has
since elected to make the persistent service the baseline direction, not a deferred option (see
section 5); the topology analysis still holds and shapes how that service is built.

## 2. The architecture baseline (DDD / SoC)

### Four structures

- S1: seam-only modular monolith. One crate; DDD achieved through in-crate module boundaries and
  dependency-inverted traits; no crate split, no daemon, no plugin runtime yet.
- S2: hexagonal ports-and-adapters with workspace crates now. Extract core/mcp/browser/bin crates
  before product #2 (consciously bends rule-of-two for compiler-enforced purity).
- S3: runtime governed host plus provider plugins, staged (vocabulary and modules now, host later).
- S4: PDP/PEP policy-as-a-service. A first-class policy decision point plus audit sink with a stable
  contract, consumed in-process now and out-of-process later without domain-code changes.

### Verdict: four adversarial judges plus scores

Totals across a DDD purist, a maintainer/evolvability lens, a YAGNI/rule-of-two pragmatist, and a
family-extensibility architect:

| Structure | DDD | Maintainer | YAGNI | Family |
| --- | --- | --- | --- | --- |
| S1 seam-only monolith | 29 | 27 | 29 | 28 |
| S4 PDP/PEP as-a-service | 27 | 26 | 28 | 28 |
| S3 runtime host staged | 24 | 25 | 26 | 23 |
| S2 crates now | 23 | 22 | 24 | 24 |

### Conclusion: S1 chassis + S4 contract

All four judges converged on the same hybrid: adopt S1 as the chassis (one crate, in-crate ports,
generics for single-impl ports, dyn only where a second impl exists today, a fail-closed arch-test),
and fold in S4's one load-bearing idea: make the decision a pure, serializable function, and split
pure decision from impure resolution. Defer S2's crate extraction to the moment product #2 actually
begins (the arch-test makes that extraction a mechanical move). Defer the runtime host / daemon /
provider registry to real demand.

## 3. Bounded contexts

| Context | Responsibility | Layer |
| --- | --- | --- |
| Policy Decision | The domain-agnostic decision skeleton: grant resolution, classification axis, denial-id, mode/shadow switch. Names no browser type. | core |
| Configuration | Typed key registry (single source of truth), 5-layer resolver, strictness, locked-key refusal. Drives CLI, schema/doc gen, options page. | core |
| Audit | The flight recorder: one record per call, the recorder, pluggable sinks (file/stderr/syslog). | core |
| Manifest | Parse/validate/source-select the active manifest; canonical SHA-256 identity hash (attribution, not authentication). | core |
| Browser Capability | The domain plugin: URL/domain matcher (CVE-hardened), the 13-tool classification table, tab-URL resolver, sacred-domains list, redaction, the extension wire. Depends on core; core never depends on it. | domain |
| Runtime / Transport | Composition root and I/O: MCP session, native messaging, IPC, executor handle, control-plane listener, the dispatch chokepoint. | infra |

Dependency direction is strictly inward: infra -> {domain, core}; domain -> core; core -> std/serde
only. Enforced mechanically by a CI test that fails if anything under the core names `browser`,
`transport`, `mcp`, `native`, or the `url` crate. That test is what turns "domain-agnostic core"
from a hope into a fact and makes a later crate extraction a move with zero back-edges.

## 4. The seams (Rust trait sketches)

These are sketches, not final signatures. The point is the shape.

```rust
// The decision is PURE and serializable. Same function in-process today, out-of-process later.
#[derive(Serialize, Deserialize)]
pub struct DecisionRequest {
    pub grants: Vec<Grant>,
    pub tool: String,
    pub rw: RwClass,
    pub resource: GoverningResource,
    pub mode: EffectiveMode,
}
pub enum Decision { Allow { grant_id: Option<String> }, Deny(Denial), ShadowDeny(Denial) }

pub trait PolicyDecisionPoint: Send + Sync {   // dyn: Noop (v1) / Local (stage 2) / Remote (daemon)
    fn decide(&self, req: &DecisionRequest) -> Decision;
}

// A generic resource, so the decision core stays domain-agnostic.
// Browser fills Resource(host); a filesystem module would fill Resource(path).
#[derive(Clone, Serialize, Deserialize)]
pub enum GoverningResource {
    Resource(String),
    AlwaysAllow,      // browser: about:blank
    OutOfScope(String),
    None,             // resource-less call
    Indeterminate,    // could not resolve -> fail closed under a manifest
}

// The plugin's PURE half. Travels WITH the decision (can relocate out-of-process).
pub trait DomainPolicy {
    fn classify(&self, tool: &str, action: Option<&str>) -> Option<RwClass>;
    fn matches(&self, pattern: &ResourcePattern, resource: &GoverningResource) -> bool;
    fn is_sacred(&self, resource: &GoverningResource) -> bool;
    fn tool_surface(&self) -> &[ToolId];
}

// The plugin's IMPURE half. Stays enforcement-point-side forever (needs live state; never relocates).
#[async_trait]
pub trait ResourceResolver {
    async fn governing_resource(&self, tool: &str, args: &Value) -> GoverningResource;
}

pub trait AuditSink: Send + Sync { fn record(&self, r: &AuditRecord); }  // dyn: file/stderr/syslog

// A self-describing capability module (see section 5).
pub trait Module {
    fn descriptor(&self) -> ModuleDescriptor;      // id, tools[], config_keys[] (namespaced + metadata)
    fn domain_policy(&self) -> &dyn DomainPolicy;
    fn resolver(&self) -> &dyn ResourceResolver;
    async fn execute(&self, call: &ToolCall) -> ToolResult;   // browser: send to the extension over CDP
}
```

## 5. The persistent-service model (chosen direction)

The user elected the persistent service as the baseline, not a deferred endgame. The reasoning: the
family vision and a future authenticated web adapter both require a service whose lifetime is not
owned by any single client process. This is a deliberate departure from the ephemeral-stdio-server
convention, made with eyes open about the lifecycle cost it takes on.

### The flow

```
MCP Client
   |  stdio JSON-RPC (sacred tool surface, byte-identical schemas)
   v
Thin Adapter          spawnable, per client; dies with the client; ~byte relay
   |  local socket (owner-only ACL, no TCP)
   v
Ghostlight Service    PERSISTENT. owns singleton state + module registry + resolved policy + audit
   |
   |-- Inbound PDP    hold/kill -> sacred -> classify -> resolve resource -> grant-check -> mode  => Allow | Deny
   |
   |-- Dispatch -> Module     browser module = pure policy (in service) + Executor = Chrome extension (CDP)
   |
   |-- Outbound PEP   response transforms (secret redaction, post-nav re-check) -> Audit (terminal) => result
   |
   v  result back to client
```

Only the adapter dies with its client. The service is an independently-lived, user-session service
that clients connect to. It is not spawned as a job-object child of the client (which is what would
make a Windows kill-on-close job take it down). The correction here to an earlier claim: the
"Windows job kills the server" problem only applies to the naive "adapter forks the service as its
own child inside the client's job" strategy, which we simply do not use.

### Self-describing modules

Each module declares its own capabilities (tool schemas) and its own config keys with metadata
(allowed values), namespaced, for example `org.sylin.ghostlight.security.secrets.redaction` with
values true/false. The service aggregates. This turns the single static key registry into a federated
registry: every module contributes its keys; the service overlays the policy layers on top. Adding a
module makes its tools and keys appear automatically. For the browser module the declared tool
schemas must remain byte-identical to the trained contract (ADR-0007); the fidelity test moves to
"the browser module's declared schemas match the golden fixture." Tool names must be globally unique
across modules, since they share one MCP surface.

### Inbound / outbound policy engine

- Inbound (the decision): hold/kill short-circuit, sacred check, classify, resolve governing resource,
  grant check, mode (enforce vs shadow) -> Allow or Deny. Pure decision plus impure resolution.
- Dispatch: the module executes the tool (browser -> extension via CDP).
- Outbound (transforms plus audit): response transforms (secret redaction in read_page,
  post-navigation final-URL re-check and parking), then audit emission.

Audit is terminal, not strictly outbound: a denied call never reaches dispatch, so its record is
emitted on the inbound-deny path. Every call, whichever branch it exits on, emits exactly one record.

### State ownership (what is singleton)

- The service holds coordination state: the module registry, resolved policy layers, per-connection
  subject/manifest context, the browser session lease (the single-active-session token), the request
  correlation map, the audit stream.
- The extension keeps durable browser state, unchanged and policy-free: open tabs, tab groups,
  debugger attachment, console/network buffers, auth. This survives a service restart. If a durable
  browser fact ever moved into the service, a service crash would lose it. Keep executor state in the
  executor. This is the separation of concerns that makes service crashes survivable.

### Module boundary

In-process, compile-time modules for the first-party family: the service is one binary that links the
browser module (and later a desktop module); adding one is a recompile, not a dynamic plugin loader
(honors single-binary / zero-deps / no dylib assumption, ADR-0001). The browser module is a special
case: its policy brain (classify, matcher, sacred, resolver, all pure or local) lives in the service,
but its executor is the Chrome extension, which is inherently out-of-process. So a module is
{ descriptor, DomainPolicy (pure), ResourceResolver (impure/local), Executor (whatever transport the
domain needs) }. A true out-of-process third-party module protocol is a later option, deferred until a
third party needs it.

### Config federation and org locks

Immutability equals the org-mandatory lock already in ADR-0019: a key set with value plus
immutable=true is the top precedence layer (org_mandatory > user > org_recommended > preset >
builtin), rendered read-only with "managed by your organization". The user's model and the existing
layered-config design are the same thing.

New question the persistent model creates: hot-reload. ADR-0019 assumed config and manifest are read
once per process, trivially correct when the process dies with the client. A long-lived service
outlives sessions. Proposed default: load OS/org/user layers at service start; load the per-subject
manifest per adapter connection (the adapter passes its manifest source); treat org-file hot-reload
as an explicit later feature, not implicit. This is an open decision.

## 6. The web adapter (future, high stakes)

### The idea

The service is fronted not only by local stdio adapters but by an authenticated web adapter: a remote,
properly authenticated principal connects to the local stack to run operations that use the user's
local authenticated browser session. The service becomes a local capability broker with pluggable
front-end transports.

### Why it makes governance load-bearing

For a purely local single-user tool, heavy governance (identity-bound grants, sacred domains, audit,
kill switch) is nice-to-have. The moment a remote principal can drive the user's authenticated
browser, governance becomes the load-bearing security boundary. The web adapter is what turns the
governance layer from a feature into a moat. This is a coherent product story and it retroactively
justifies the governance investment.

### The threat-model reversal and the security bar

This is the highest-stakes decision in the system. Today the threat model is current-OS-user only
(owner-only pipe ACL, no network). A web adapter flips it to "anyone who can authenticate over the
network", attached to a tool that drives sessions holding real money and identity. Requirements
before it ships:

- Keep the local transport exactly as decided: named pipe / UDS, owner-only ACL, no TCP. The
  ADR-0003 rationale still holds for the local surface. The two transports have different auth models
  and do not conflict.
- The web transport needs mutual authentication (mTLS or equivalent, not a bearer token in a config
  file), transport encryption, per-adapter capability scoping (a remote principal gets a narrower
  manifest than the local user by default), rate limiting, and tamper-evident audit.
- It is a conscious reversal of an explicit spec exclusion (no network reachability), so it gets its
  own ADR with the threat model written down, not architectural drift.
- Sequence it after governance is mature. The web adapter riding on top of stage-2 governance is
  defensible; before it, it is how someone gets an account drained.

### Transport as a port; identity per connection

Transport becomes an explicit abstraction: a local IPC adapter today, an authenticated web adapter
later. The service core does not care how a call arrived, only about the authenticated subject
attached to that connection. A local adapter's subject is the current OS user with the full manifest;
a web adapter's subject is the authenticated remote principal with its own scoped manifest. This is
the same identity-bound-grants model governance already uses, and it makes per-subject attribution a
first-class requirement. A local in-process fallback stays available regardless, so the local
experience is bulletproof even if the service is disabled or absent.

## 7. Sequencing (how to build without getting burned)

The code architecture is identical whether the PDP runs in-process or in a service; only deployment
differs. So build in an order that lands the process-lifecycle risk last, on a proven base:

- Phase A: the pipeline, in one process. Governance core (pure serializable PDP), the Module trait
  and registry, the inbound/outbound pipeline, federated config, all inside today's single process
  (adapter is the service). This is S1 + S4 plus the module registry. The IPC bug is fixed here for
  free via the control-plane split. Everything is unit-testable with no browser.
- Phase B: split the adapter from the service. Spawn-on-demand persistent service; adapter becomes a
  thin relay; the in-process mode from Phase A remains as the fallback. This is the only phase that
  touches the lifecycle axis, and it does so on top of a working pipeline, so a bug is unambiguously a
  lifecycle bug. This is where the persistent-service lifecycle model (start, single-instance,
  idle-shutdown, crash-restart, upgrade) is designed and the Windows job-object behavior is verified.
- Phase C (later): the authenticated web adapter, only after governance is mature and behind its own
  security ADR.

"Permanently alive" means session-scoped persistence: alive across client restarts, exits on a
genuine idle grace window. Not "orphaned forever holding the authenticated browser".

## 8. Do now vs defer

Do now (additive, cheap, reversible):
- Introduce the ports (PolicyDecisionPoint, DomainPolicy, ResourceResolver, AuditSink, plus RwClass
  and GoverningResource); generics for single-impl ports, dyn only where a second impl exists today.
- Make the decision input serde-serializable from day one (needed by simulate/shadow anyway; also the
  seam that lets the PDP relocate later).
- Turn dispatch into the enforcement point holding a Governance facade; all_open() is a literal
  zero-cost Allow so the no-manifest path stays byte-identical.
- Split the typed key registry: generic parts in the core, the browser key catalog as data; resolve
  the Config Copy-versus-owned contradiction toward owned.
- Add the fail-closed arch-test to CI.
- Stand up the read-only control-plane listener in the existing owner process (fixes BUGFIX-06 at its
  root; the surface a future daemon inherits).
- Land each stage-2 task into a seam as it is written.

Defer:
- Crate extraction (until product #2 begins).
- The persistent-service split (Phase B) until the pipeline is proven and the lifecycle is designed.
- The web adapter (Phase C) until governance is mature and its security ADR is written.
- Provider registry, subprocess/third-party modules, extra audit sinks beyond file/stderr.

## 9. Open decisions

- Admission policy for the shared session: single active subject with a lease and clean handoff
  (preserves ADR-0004, better UX than a hard SessionBusy) versus genuine multi-client multiplex.
- Module boundary: confirm in-process compile-time registry is acceptable, or whether out-of-process
  third-party modules are a near-term requirement.
- "Permanently alive" semantics: confirm idle-shutdown (session-scoped persistence) and treat Windows
  job-object breakaway as a Phase-B verification gate.
- Hot-reload of the org policy while the service runs (persistence reopens this; ADR-0019 assumed
  read-once).
- GoverningResource as an opaque string may not fit a future structured resource (path plus
  ancestry). Serialize only the decision boundary; keep the resource an enum; re-litigate at product
  #2.
- Whether and when to ratify: which of these graduate into ADRs (the topology decision, the S1 + S4
  baseline, the persistent-service model, the web-adapter security decision).

## Sources

Two adversarial design panels were run in-session on 2026-07-02:
- Topology panel: four models (A minimal fix, B daemon, C self-electing broker, D control/data split)
  scored by three judges (lean-internals purist, stage-2-governance architect, reliability/lifecycle
  skeptic).
- Architecture-baseline panel: four structures (S1 seam-only monolith, S2 crates now, S3 runtime host
  staged, S4 PDP/PEP as-a-service) scored by four judges (DDD purist, maintainer/evolvability,
  YAGNI/rule-of-two pragmatist, family-extensibility architect).

Grounded in the current module structure and ADRs 0001-0021, especially: 0001 (single binary),
0002 (dual-role binary), 0003 (tokio-native IPC), 0004 (reject second session), 0005 (policy-free
extension), 0007 (sacred tool surface), 0013 (governance overlay all-open), 0018 (observe then
enforce), 0019 (layered configuration), 0021 (Ghostlight brand and family). Also grounded in a digest
of the 18 stage-2 governance task docs (the generic-versus-browser fault line that any plugin split
must follow).
