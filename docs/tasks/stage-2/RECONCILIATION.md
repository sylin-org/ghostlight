# Stage 2 reconciliation overlay

The 18 `g01`-`g18` task prompts were authored before two things were decided: the architecture
baseline (the `governance/` core + `browser/` plugin + `transport/` infra split with dependency-
inverted ports; see `docs/design/ghostlight-service-architecture.md`) and the delight-informed
decisions (hot-reload first-class, owned `Config`, org-policy fail-closed on reload). This document
adjusts the g-docs to that current vision.

**Authority order:** where a `g`-doc conflicts with this file, THIS FILE WINS. Where this file is
silent, the `g`-doc stands. Read `PLAN.md` first (order + workstreams), then this file, then the
specific `g`-doc for a task. The new Phase-A prompts (`a1`, `a2`, `a3`, `a5`, `a7`) are not adjusted
here; they already encode the current vision.

## 1. Module placement map

The g-docs name flat paths (`src/policy/mod.rs`, `src/dispatch.rs`, `src/tools/`). After Phase A's
reorg (`a1-module-reorg.md`), read those as the new locations:

| A g-doc says... | Put it in... | Why |
|---|---|---|
| `src/policy/mod.rs` (registry, resolver, Config) | `governance/config/` | domain-agnostic config core |
| `src/dispatch.rs` (the chokepoint) | `governance/` (the facade, `a3`) | the enforcement point |
| decision flow / `check_call` (g13), denial, mode | `governance/` (core) | the pure PDP |
| audit record + recorder + sinks (g06) | `governance/` (core) | domain-agnostic flight recorder |
| manifest parse/identity (g09, g12) | `governance/` (core) | generic over any policy doc |
| `src/policy/pattern.rs`, the URL/domain matcher (g07) | `browser/` | browser/URL-specific; the `url` crate lives ONLY here |
| sacred-domain list + enforcement (g08) | `browser/` (data/logic) + `governance/` (the always-on check wiring) | resource list is browser; the mode carve-out is core |
| r/w classification TABLE (g05) | `browser/` (the 13-tool table) behind the `Classifier` port; the observe/mutate axis type in `governance/` | table is the plugin, axis is core |
| `read_page` redaction (`redact.rs`) | `browser/` | page-content specific |
| tool implementations (`src/tools/`) | `browser/` | the plugin executor surface |
| `src/native/`, `src/mcp/`, `src/browser.rs` handle | `transport/` | I/O and protocol |

`install/`, `debug`, `doctor`, `origin`, `error`, and `main` stay at the crate root as shared infra.

## 2. The ports (from `a2-governance-ports.md`)

The g-doc logic plugs into the seam, it does not stand alone:

- g05 classification implements `Classifier` / `DomainPolicy::classify` (axis in core, table in plugin).
- g07 matcher implements `ResourceMatcher` / `DomainPolicy::matches` (the `url` crate lives here).
- g08 sacred implements `DomainPolicy::is_sacred`; the always-on carve-out is wired in the core.
- g13 `check_call` IS the pure `PolicyDecisionPoint::decide` over a serializable `DecisionRequest`.
- g06 recorder implements `AuditSink`; g12/g09 feed `DecisionRequest.grants` and the manifest identity.
- g14 advertisement uses `DomainPolicy::tool_surface`.

Adjust a port's exact shape minimally when its concrete g-doc lands, keeping the pure (`DomainPolicy`)
vs impure (`ResourceResolver`) split and the serializable `DecisionRequest` intact.

**`RwClass` naming (authoritative):** `RwClass` is the observe/mutate CLASSIFICATION axis from g05,
with variants `Observe` and `Mutate`. It is distinct from a grant's `access` field
(`read` | `write` | `all`), which is a separate concept applied during enforcement (g13). Where the
`a2` or `a3` prompt names `RwClass` variants `Read`/`Write`, or references `RwClass::Observe` without a
`Mutate` sibling, use `Observe` and `Mutate`. The all-open facade may pass any placeholder `RwClass`
because STEP-0 never reads it.

**Known integration point (resolve during g01/a1):** g01's `DomainPatternList` constraint validates a
value via a domain-pattern syntax checker, which is browser-domain (`browser/pattern.rs`). The generic
`governance/config` registry MUST NOT call `browser::` directly (the `a7` arch-test forbids it). Inject
the domain validator into the registry (a validator hook / function pointer supplied by the plugin at
composition time), or carry the domain-pattern keys and their validation in the browser key catalog
(`browser/keys.rs`). Do not hardcode a `governance -> browser` edge.

## 3. Hot-reload deltas

The g-docs assume "read once per session / takes effect on restart." The current vision is live
reload (see `PLAN.md` "Cross-cutting workstream" and `a5-hot-reload-substrate.md`). Per-concern:

- **g02 resolver:** return an owned, re-resolvable `Config` snapshot; the in-force snapshot is held
  behind the `a5` atomic swap. Resolution must be a pure function of the layer inputs so it can re-run.
- **g03 `config set`:** after writing the user layer, trigger an immediate re-resolve and swap. An edit
  takes effect now, not next session.
- **Key descriptions that say "Takes effect on restart"** (g01: `audit.destination`, `audit.file.path`):
  prefer live. On a swap that changes an audit destination or path, re-open the sink (close old, open
  new). Update those two description strings to drop "Takes effect on restart" once the sink re-opens
  on change. If a specific resource genuinely cannot be swapped safely, keep restart-only for that one
  key and say so explicitly in its description; the DEFAULT is live.
- **g12 manifest:** reloadable. On a manifest source change, re-resolve and swap; FAIL-CLOSED if the
  new manifest is invalid (keep the last-good, surface an error, never fall open).
- **g14 advertisement:** subscribe to the `a5` change signal and emit MCP
  `notifications/tools/list_changed` when a reload changes the permitted tool set. This REVERSES the
  g14 note that mid-session re-advertisement is unsupported; it is now supported and is a delight point.
- **Org policy reload is FAIL-CLOSED** (keep last-good org policy on an invalid push). See section 4.

## 4. Org policy loading

Per `PLAN.md` "Org policy loading": a single admin-writable-only machine file
(`%ProgramData%\browser-mcp\policy.json` / `/etc/browser-mcp/policy.json` /
`/Library/Application Support/browser-mcp/policy.json`); auto-loaded and non-bypassable; delivered by
MDM/GPO/Intune/Jamf; strict parse, fail-closed on load AND on reload; precedence
`org_mandatory > user > org_recommended > preset > builtin`; trust is OS ACLs + the deployment channel,
not signatures; the extension never sees policy. This refines g02/g12 where they describe org loading.

## 5. Which g-docs run inside Phase A

`g01` (typed key registry) and `g02` (layered resolution) are executed as part of Phase A (the config
core), placed per section 1 and made re-resolvable per section 3. `g03` (config CLI) and `g04` (schema
generation) are the tail of Phase A. `g05`-`g18` run in Phases B/C/D per `PLAN.md`.

## 6. Framing: governance is delight

Ship each feature as a confidence feature, not a restriction. `explain` (g16) renders policy in plain
language, not JSON. `simulate` (g17) is try-before-you-trust. Presets (g18) are one-click, not a
settings maze. Audit (g06) is the substrate for a session recap, not just a compliance log. Pause (g10)
and kill (g11) are instant and visible. Keep the copy and the surfaces in that spirit.

## 7. Line-number and fork caveats

The g-docs were verified against the tree BEFORE the release-1 tasks landed; line numbers have drifted.
Trust function names and prose over line numbers, and re-verify the target against the current tree
before editing. Any g-doc branch that forks on "did release-1 T04 land" resolves to YES: stage 1 is
complete and merged to `main`.
