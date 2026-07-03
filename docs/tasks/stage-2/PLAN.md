# Stage 2 Implementation Plan (Governance)

Status: Approved to execute. Branch: `stage-2` (off `main`, which now has stage 1 merged). Date:
2026-07-02.

This is the execution reference for the governance layer. The 18 task specs (`g01`-`g18`) and the
shared format (`00-shared-format.md`) define the WHAT in detail; this document defines the ORDER, the
resolved decisions, and the cross-cutting workstreams. Read this first, then the relevant g-doc.

## Principles

- **Observe before enforce (ADR-0018).** Audit first (pure observation), then small self-contained
  enforcement, then the full manifest engine. A wrong denial is never debugged at the same time as a
  wrong grant resolution.
- **Separable overlay; all-open stays byte-identical (ADR-0013).** A no-manifest STEP-0 short-circuit
  means an ungoverned engine behaves exactly as stage 1. Governance is additive, never required.
- **Build into the architecture seams (S1 + S4).** See `docs/design/ghostlight-service-architecture.md`.
  Each task's code lands in `governance/` (domain-agnostic core), `browser/` (the domain plugin), or
  `transport/` (infra), behind the ports, guarded by a fail-closed arch-test. The decision is a pure,
  serializable function; resource resolution stays impure and local.
- **Governance IS delight.** Each feature ships as a confidence feature, not a restriction: the reason
  a user can hand the agent their real, authenticated browser without fear. Explain over JSON,
  simulate before trust, one-click presets, a session-recap audit, an instant visible pause/kill.

## Resolved decisions (delight-informed)

- **Hot-reload: first-class.** Config, presets, sacred lists, and manifests take effect on change, no
  restart. See the cross-cutting workstream below. (Flips the old "read once per session" default.)
- **Config is owned, not `Copy`.** An owned, re-resolvable snapshot is what hot-reload swaps in;
  `Copy` + `&'static str` fights re-resolution. Resolves the G01/G06 contradiction toward owned.
- **Lay the seams now.** Stage 2 is a clean slate (no code written against the flat layout yet), so
  the module reorg is cheap now and makes hot-reload, the live options page, and the future service
  split affordable. Do it up front, not as a later refactor.
- **Identity is attribution, not authentication.** No manifest signing; org binding is the deployment
  channel plus file ACLs. Crypto ceremony is anti-delight for individuals; the web adapter later needs
  real auth, but that is a different context (safety of a remote surface), not this decision.
- **SPEC reconciliation is a docs pass after the code lands** (the ~13 amendments in
  `00-shared-format.md` section 10). Not a blocker for implementation.

## Cross-cutting workstream: hot-reload

Enforcement is per-call at the dispatch chokepoint, which makes reload clean: each call reads the
current resolved snapshot, so "reload" is just produce-a-new-snapshot-and-swap-it.

- The layered resolver returns an **owned `Config` snapshot**. The in-force snapshot is held behind an
  atomic swap (`ArcSwap` or `Mutex<Arc<Config>>`); a re-resolve replaces it in one store.
- **File-watch** (debounced, cross-platform) on the three sources: the user config file, the org
  policy file, and the active manifest source. A change triggers re-resolve.
- **Validate-then-swap.** Parse and validate a candidate fully before swapping. Never apply a
  half-written file.
- **Invalid-on-reload handling differs by source, and this is a security rule, not a preference:**
  - User config file: keep the last-good snapshot, surface a clear error. (Lenient, like initial load.)
  - **Org policy file: FAIL-CLOSED.** Keep the last-good org policy, surface an error, and never fall
    open to a weaker posture because an org push was malformed. An org policy that silently fails open
    is worse than a stale one.
- **`config set` and the extension options page trigger an immediate re-resolve/swap**, so an edit
  takes effect now, not next session. This is the core delight payoff.
- **Emit MCP `notifications/tools/list_changed`** when a manifest reload changes the permitted tool
  set, so the agent's visible tools update live (the g-docs skipped this under the read-once
  assumption; hot-reload brings it back).
- **Audit stamps the manifest identity per call** (G09), so a reload is visible in the trail: each
  record shows which policy was in force for that call.

## Org policy loading

- **A single machine-scope file at an admin-writable-only path**, per platform:
  `%ProgramData%\browser-mcp\policy.json` (Windows), `/etc/browser-mcp/policy.json` (Linux),
  `/Library/Application Support/browser-mcp/policy.json` (macOS). Exact names pinned in G02/G12; the
  invariant is that only an administrator can write it.
- **Auto-loaded and non-bypassable.** No CLI flag or env var disables or redirects it (unlike
  `--manifest`, the user/dev manifest source). The org file always applies on top.
- **Delivered by the org's existing channel** (GPO / Intune / Jamf / MDM writes the file). We do not
  build a policy server.
- **Strict parse, fail-closed.** Any malformed entry or invalid value is fatal on load, and on reload
  keeps the last-good org policy (see hot-reload rule).
- **Precedence:** `org_mandatory` (locked) > `user` > `org_recommended` (overridable default) >
  `preset` > `builtin`.
- **Trust model:** OS file ACLs plus the deployment channel, NOT cryptography. Manifest signing stays
  excluded (an honestly-labeled usage-surface guard).
- **The extension never sees policy.** The binary reads the org file; the extension stays policy-free.
- **Delight:** invisible when absent (no org file means a pure personal experience, zero enterprise
  cruft); transparent when present (locked keys render read-only with a "managed by your organization"
  badge in `config list` and the options page, and `explain` renders the effective policy in plain
  language); and with hot-reload an MDM push takes effect without a user restart, fail-closed on an
  invalid push.

## Phases

Phases are lettered to avoid confusion with the SPEC's numbered build phases and ADR-0018's "steps."
Phase A is foundations; Phases B/C/D are ADR-0018 steps 1/2/3.

### Phase A - Foundations (seams + config + hot-reload substrate)

The scaffold every later task fills. Fully additive to behavior: all-open stays byte-identical.

- **A1. Module reorg.** Regroup into `governance/` (core), `browser/` (domain plugin), `transport/`
  (infra). Move `dispatch.rs` + `policy/` into `governance/`; `tools/` and the CDP-facing pieces into
  `browser/`; `native/`, `mcp/`, and the `Browser` executor handle into `transport/`. One dedicated
  reviewable commit, guarded by the full test suite + the sacred tool-schema fidelity test + a new
  all-open golden test (tools/list byte-equality + a dispatch round-trip). Do this first, while there
  is no stage-2 code to churn.
- **A2. Ports.** `governance/ports.rs`: `PolicyDecisionPoint` (pure, serializable
  `DecisionRequest -> Decision`), `DomainPolicy` (pure: classify/matches/is_sacred/tool_surface),
  `ResourceResolver` (impure/async: governing resource from live state), `AuditSink`, `Classifier`,
  `ResourceMatcher`; plus the types `RwClass`, `GoverningResource`, `Denial`. Generics/concrete for
  single-impl ports; `dyn` only for `PolicyDecisionPoint` (Noop/Local/future-Remote) and `AuditSink`
  (file/stderr/syslog).
- **A3. Governance facade.** `dispatch.rs` holds a `Governance` facade and becomes the enforcement
  point (PEP). `Governance::all_open()` is a literal zero-cost `Allow` (STEP-0). Lock behavior with
  the all-open golden test.
- **A4. Config core.** Config `Copy` -> owned. `G01` typed key registry (KeyDef/KeyType/KeyConstraint,
  parse/validate) with the generic parts in `governance/config/` and the browser key catalog as data
  in `browser/keys.rs`. `G02` layered resolver (5-layer precedence, Source, `Resolved`, file loading,
  strictness matrix), returning an owned snapshot designed for re-resolution.
- **A5. Hot-reload substrate.** Atomic snapshot swap + a re-resolve function; debounced file-watch on
  the config/org/manifest sources; validate-then-swap with the source-specific invalid handling above.
- **A6. Config surfaces.** `G03` config CLI (list/get/set with lock refusal, wired to trigger an
  immediate re-resolve) and `G04` JSON-schema + doc generation from the registry (golden-pinned).
- **A7. Arch-test.** A fail-closed CI test: nothing under `governance/**` may `use`
  `browser`/`transport`/`mcp`/`native` or the `url` crate.

Verify Phase A: all-open byte-identical (golden test), full suite + clippy + fmt green, arch-test
passes, and `config set` takes effect immediately (hot-reload smoke test).

### Phase B - Audit flight recorder (ADR-0018 step 1; pure observation)

- `G05` r/w classification (`tool+action -> observe|mutate`; axis in core, table in the plugin).
- `G09` manifest identity (canonical SHA-256, for attribution).
- `G06` audit recorder + sinks (one JSONL record per call: identity, client, tool, action, rw, domain,
  `decision=allow`, timing; file/stderr/syslog). Audit destination keys are the first an org locks, so
  they exercise the layered config from Phase A.

Verify Phase B: every call emits one record; zero behavior change (all-open still byte-identical).
Delight framing: this is the session-recap substrate, not just a compliance log.

### Phase C - Sacred domains + pause + kill (ADR-0018 step 2; first enforcement, audited)

- `G07` domain matcher (exact + `*.` wildcard, WHATWG host normalization, CVE-2025-47241 / IP-literal
  / IDN / suffix-stitch hardening; the `url` crate lives ONLY here, in `browser/`).
- `G08` sacred never-touch domains (always-on regardless of mode/manifest; introduces `Denial` + the
  stable denial-id).
- `G10` take-the-wheel pause; `G11` panic kill switch.

Verify Phase C: sacred denials + pause + kill work and are recorded; empty sacred list = byte-identical
all-open. Delight framing: visible, instant control.

### Phase D - Manifest engine + trust UX (ADR-0018 step 3; the differentiator)

- `G12` manifest engine (parse/validate/source-select: org file > `--manifest`/env > none; `Grant =
  {domains, access: read|write|all, tools, mode}`; integrates G09; fail-closed on a broken selected
  manifest).
- `G13` grant enforcement (the pure `check_call`: first-match grant -> tool-list-before-access ->
  access-class; wired at the 5 enforcement points incl. navigate pre/post-redirect + tab-URL drift;
  page-less-tool union rule).
- `G14` tool-advertisement filtering (`tools/list` = grants x class union; emits `list_changed` on a
  manifest hot-reload).
- `G15` shadow mode (effective mode: per-grant > manifest > `governance.mode`; observe -> log-but-allow,
  enforce -> block; sacred always hard-deny).
- `G16` explain (plain-language render of a manifest); `G17` simulate (replay recorded audit through
  the same `check_call`; honest would_allow/would_deny/not_evaluable); `G18` presets + init templates.

Verify Phase D: a restrictive manifest -> the agent sees only permitted tools and gets clear denials;
shadow logs would-denies without blocking; simulate replays; explain renders; presets are one-click.

## Dependency order (partial, not linear)

```
G01 -> G02 -> {G03, G04, G12}
G05 -> {G06, G13, G14}
G07 -> {G08, G13}
G08 -> {G13, G15}
G09 -> {G06, G12}
{G06, G13} -> G17     G12 -> {G16, G18}
```

## Task-to-phase map

| Phase | g-docs |
|---|---|
| A Foundations | G01, G02, G03, G04 (+ seams, facade, hot-reload substrate, arch-test) |
| B Audit | G05, G09, G06 |
| C Sacred + pause + kill | G07, G08, G10, G11 |
| D Manifest engine + UX | G12, G13, G14, G15, G16, G17, G18 |

## Execution notes

- Start with A1 (reorg) since the tree is clean of stage-2 code; then A2/A3 additively, then A4-A7.
- Keep every step green: full suite + clippy + fmt, plus the all-open golden test and the sacred
  fidelity test on any change near the surface.
- Never change the tool schemas (ADR-0007). Governance shapes visibility and execution, not schemas.
- The g-docs are the detailed spec per task; this plan is the order and the cross-cutting rules.
