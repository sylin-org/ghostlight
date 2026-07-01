# The Eight Forks — Decided (North-Star Re-Lensed)

**Date:** 2026-07-01 · **Status:** DECISIONS LOCKED · **Governed by:** [NORTH-STAR.md](NORTH-STAR.md)

Outcome of re-deciding the eight design forks through the corrected engine-vs-overlay / layered-
delight lens. Produced by a multi-agent workflow (8 analysts + 3 adversarial verification lenses
per fork + a synthesis→critique→revise loop that surfaced and resolved 15 issues), then reviewed
by the project owner.

**Owner decisions:** seven forks accepted as re-lensed. **Fork 1 (`extract`) is deferred to v2**
(see §Fork 1). Consequently **v1 preserves the 13 Claude-in-Chrome schemas byte-identical and
makes NO schema-preservation amendment** — the §3.4 amendment is deferred and becomes a
prerequisite for any future additive tool.

The north star governs: every v1 decision is re-lensed so **L0 (engine) is complete for the
all-open user with zero overlay present**, overlay concerns are provably inert in all-open, and no
token/correctness delight is mis-sourced from tool-filtering.

---

## 1. Fork table (final)

| # | Fork | Layer | Decision |
|---|---|---|---|
| 1 | Caller-directed `extract` | Engine | **v2 (DEFERRED)** — convenience + least-privilege niche; costs the sacred-schema amendment; not load-bearing |
| 2 | Snapshot-first / token efficiency | **Engine** | **v1** — lean refs + screenshot discipline; compression = engine-config |
| 3 | Risk annotations / HITL | Engine (annot.) + Overlay (gate) | annotations **v1**, enforced gating **v2** |
| 4 | Self-registering installer | **Engine (packaging)** | **v1** (Phase 1/2) — single ID mechanism |
| 5 | Audit standards | **Overlay** | **v1** core split; off in no-manifest; OCSF/CEF/hash-chain for enterprise |
| 6 | Positioning / tool advertisement | **Positioning/correctness** | **v1** — invert §5.1 to full-surface baseline |
| 7a | Committed-origin reporting | **Engine (correctness)** | **v1** — per-frame origin truth |
| 7b/7c | Domain enforcement / pipe-oracle | Overlay + opt-in | enforcement **v1**; oracle **v2**, must reuse real session or be cut |
| 8 | Policy resolution semantics | **Overlay** | **v1** — all-open floor = full mutate |

---

## 2. Per-fork final decisions

### Fork 1 — Caller-directed `extract` — **DEFERRED to v2**

Proposed as a deterministic, caller-directed projection
(`extract(fields:[{name, ref|selector|xpath, attr, many}])`) — the caller supplies the DOM→field
mapping; the engine pulls and type-coerces, no embedded LLM.

**Why deferred (owner decision after discussion):**
- **Convenience + a niche, not a missing capability.** Everything `extract` does is already
  achievable with `read_page`/`find`/`get_page_text` (+ `javascript_tool` when granted). Its real
  value is (a) efficiency — one compact typed object vs. blob-and-parse — and (b) least-privilege:
  structured reads *without* granting arbitrary JS (useful where a grant excludes
  `javascript_tool`). Both are genuine but modest.
- **The novel version is off-mission.** Stagehand's power is LLM-driven schema *inference*; that
  would embed inference in the engine, violating "keep intelligence in the caller." Our
  deterministic version is effectively "read-only, constrained `javascript_tool`."
- **It costs the sacred rule and eats its own token argument.** A 14th, never-trained tool forces
  the §3.4 amendment, and adding its schema to the advertised surface costs tokens every session
  (to save tokens only on calls that use it).

**v1 consequence:** the 13 schemas stay byte-identical; **no §3.4 amendment in v1.** `extract` is a
reasonable v2 addition, at which point the §3.4 sacred-schema amendment is taken as a deliberate,
evidence-driven decision. `observe`/`act` self-healing remain v2/v3.

### Fork 2 — Snapshot-first / token efficiency — **Engine, v1**

- **Firewall note** (§6.3/§3.1): token efficiency is an **L0/engine** property from lean refs +
  screenshot discipline, present identically in all-open; **not** sourced from §5.1 tool-filtering.
- **Compression is a pure engine-config knob, not governance-overridable.** Split §4.1 into an
  **engine-config** block (`screenshot_quality=55`, `screenshot_fallback_quality=30`,
  `screenshot_max_bytes=512000`, `page_load_timeout_ms`, `max_concurrent_tabs`) and a
  **governance-config** block. Engine-config is settable by *any* posture including the all-open
  user, produces the **same pixels** in every posture, and is **never** gated on grants/identity.
  Governance-config cannot reference engine-config keys.
- **Leanness contract** (§3.1): `read_page`/`find` return interactive elements + refs + on-screen
  text; off-screen/hidden de-prioritized; representation posture-invariant. Origin metadata
  (Fork 7a) is **O(frames), not O(elements)** — one `{securityOrigin,url}` pair per represented
  frame, never per node.
- `scroll` returns a screenshot but is Mutate-classified → works out-of-box in all-open (Fork 8
  full-mutate floor). Ref-stability contract (lifetime, invalidation on nav/DOM-mutation/frame-
  detach, frame-binding) defined jointly with Fork 7a.

### Fork 3 — Risk annotations (v1) / HITL gating (v2) — **Engine + Overlay**

**Annotations (engine correctness, v1):** publish the full annotation quad per tool. An
advertised-but-unannotated tool is a spec violation.
- observe-reads (`read_page`/`get_page_text`/`find`/`read_console_messages`/
  `read_network_requests`/`tabs_context`) → `readOnlyHint:true, openWorldHint:false`
- `navigate` → `readOnlyHint:false, destructiveHint:false, openWorldHint:true`
- `tabs_create`, `javascript_tool`, `form_input` → `destructiveHint:true, openWorldHint:true`
- `resize_window`, `update_plan` → `readOnlyHint:true`
- **`computer` → `openWorldHint:true` + title/description REQUIRED, with
  `readOnlyHint`/`destructiveHint` INTENTIONALLY omitted** (it spans observe `screenshot` and
  mutate `click`; a static superset would mislabel the highest-frequency action). This omission is
  the **sanctioned exception, documented in §3.3** — it is *annotated*, not *unannotated*.
  Per-action tier stays authoritative in the binary (§5.4).

**Gating (overlay, v2, inert in all-open):**
- Transport plumbing (elicitation/MRTR negotiation) → **§2.2 (engine, dormant)**; the HITL trigger
  lives at **§5.9**.
- **v1 parses-and-ignores `approval.*`** (validates shape, ignores with a startup WARNING) for
  forward-compatibility — v1 never rejects a manifest for containing approval semantics. The
  "reject if `approval:"required"` and no channel" rule lands in v2 with the gating.
- **Posture-split timeout (v2):** enterprise (L2) fail-closed (auto-deny on timeout/no-channel,
  `on_no_channel` overridable); user-chosen (L1) delight-first (**resumable, no auto-deny** by
  default). The overlay UX must be delightful, not merely correct.

### Fork 4 — Self-registering installer — **Engine (packaging), v1**

- **Pinned `key` in `extension/manifest.json` is the sole ID mechanism** → deterministic build-time
  ID; installer writes `chrome-extension://<id>/` into `allowed_origins` (no wildcards) without
  reading user profiles.
- **Constraint:** the **same public key** MUST be used for the `key` field (dev/unpacked ID) and
  for CRX packing/signing (packed/force-installed ID). When the deployed ID cannot be pre-pinned
  (Web-Store re-sign, enterprise self-sign), the installer accepts the real ID via
  **`--extension-id <id>`** — a **first-class enterprise input**, not a doctor-only fallback.
  Installer input: `--extension-id <id>|auto` (`auto` = consent-gated profile-discovery for
  personal installs).
- Absolute paths (host manifest, Windows registry `(Default)`, MCP client config); product-
  namespaced host name; idempotent install/uninstall at HKCU (personal) and HKLM `--system`
  (enterprise) with channel retraction; `verify`/`doctor` diagnoses the transport chain only.
- **Promote to Phase 1/2.** Every installer change: "no MCP tool schema added/removed/altered."

### Fork 5 — Audit standards — **Overlay, v1 (core split)**

- **Engine-neutral core record** (emitted only when audit is active):
  `{timestamp, event_id, tool, url, result, duration_ms, parameters?, screenshot?, output?}`.
  `grant_id`, `access_tier_required/granted`, delegation `sub`/`act.sub` live in an optional
  **`governance` sub-object present only when the overlay is active.**
- **No-manifest all-open: success logging OFF by default** (`log_successful_calls:false`); only
  denials/errors may surface on stderr (and all-open has no denials → effectively silent). Audit is
  opt-**in** in all-open (achieved by writing a manifest = an L1 action), never opt-out.
- **No-manifest ≠ explicit all-open manifest for audit:** the full-mutate short-circuit is identical
  for both, but audit follows a manifest whenever one is *explicitly present* (even an all-open
  one). The truly-zero-overlay posture is **no-manifest only**.
- Add `include_tool_output_in_log` (routes `get_page_text` output; `extract` output when it lands
  in v2). Rewrite §7.2 `url` paragraph: full-fidelity `url` when logging active; in-binary query
  redaction is a **governance-gated** sensitive-field (`redact_url_query: off|hash|drop`, default
  `off`), never global. Add `format: json|ocsf|cef` (sink-side formatters over canonical flat
  JSON), `hash_chain` (default false; "tamper-EVIDENT not tamper-proof"), `result:denied` with
  winning `grant_id`/reason.

### Fork 6 — Positioning / tool advertisement — **Positioning/correctness, v1**

- **Invert §5.1:** **Step 0 (all-open / no grants): advertise the COMPLETE engine surface (the 13
  preserved schemas) unconditionally; skip steps 1–4.** Steps 1–4 apply only when a grant exists
  (the overlay *subtracts*).
- **All-open advertisement invariant (§1.2/§5.1):** *"In all-open, advertised == callable == the
  complete engine surface (the 13 preserved schemas)."* (When a v2 additive tool lands, this reads
  "13 preserved + that additive tool" — a v2 change tied to the §3.4 amendment.)
- Filtering is an OVERLAY behavior active in L1+L2, inert in all-open — never the source of token
  efficiency (that is L0, Fork 2).
- Reframe §3 "tiers" as engine-intrinsic read/write classification consumed by the overlay; fix
  §3.1's "Only available when the grant specifies mutate" → "Always available in all-open; under
  governance, available when a matching grant specifies mutate."

### Fork 7a — Committed-origin reporting — **Engine (correctness), v1**

- **"Page-truthful committed-origin reporting":** faithful to what an untampered extension reports
  (immune to page-spoofing signals), explicitly NOT ground truth vs. a *tampered* extension.
- **Reads carry origin as faithful population of EXISTING reference fields where they exist, else
  via an additive metadata channel — never injected into a trained response body.** Verify per
  tool against the reference: `tabs_context` already returns tab `url` (faithful population, no
  schema change); for `read_page`/`get_page_text`, if the reference schema already carries a
  per-frame `url`/origin field, populate it; if not, emit on a sibling metadata channel outside the
  preserved envelope (keeps the trained body byte-identical). Record which case applies in §5.0.
- **Re-home the committed-origin state model to the engine chapter (§5.0)** — per-frame
  `frameNavigated.securityOrigin`+`loaderId`, dual-event, `Target.getTargetInfo` cross-check; §5
  consumes it by reference. Raw committed URL is the authoritative reported/audited value; the
  NFKC+IDNA+`inet_aton` matching-key is **overlay-only**.
- Origin metadata is **O(frames)** (shared with Fork 2's leanness contract).
- **Correct §7.4:** "audit is as trustworthy as the **weakest of {binary, extension}**; the
  committed-origin signal is extension-relayed and forgeable by a tampered extension; the opt-in
  pipe-oracle is the only extension-independent path."

### Fork 7b/7c — Domain enforcement (v1) / pipe-oracle (v2) — **Overlay + opt-in**

- **§4.3/§4.2 edits:** §4.3 reads the committed `securityOrigin`; `domains` matched against the
  canonical origin. **Opaque-origin (`null`) deny-by-default is gated behind manifest-present** —
  in all-open, `null` is reported truthfully with no deny path.
- **Enforcement points (renumbered):** pre-nav; post-commit `frameNavigated`; **new-target
  (`Target.createTarget`) → §5.6**; **fail-safe containment to `about:blank` → §5.7**;
  **host-canonicalization for matching only → §5.8**. §5.4 gets an explicit all-open pass-through
  clause.
- **Pipe-oracle (7c) — v2, and MUST reuse the user's real authenticated session, or be CUT.** A
  `--user-data-dir` fresh profile violates Principle 3 by construction. If a clean profile is
  technically required for the oracle to work, **cut it entirely** — do not defer. Positioning: the
  extension-mediated default is first-class; the oracle is a hardening lens over the *same
  authenticated session*, never the "real/secure" mode.
- Extension-tampering residual risk documented honestly (identical to official Claude in Chrome),
  not surfaced as an in-product alarm.

### Fork 8 — Policy resolution semantics — **Overlay, v1**

**The most important correction — applied at EVERY occurrence:**
- **No-manifest / empty-grants all-open path short-circuits to ALLOW at FULL MUTATE (complete engine
  surface), not observe.**
- **Change the default at every SPEC location:** built-in default `unlisted_domains:"observe"` →
  `"mutate"` at **§4.4** AND **§8.2 step 5** AND **§5.3 STEP 0**. A full-SPEC grep for `"observe"`
  used as a *default* was performed; every occurrence changed. (Observe remains a valid
  user-chosen grant value — only its use as the silent floor is removed.)
- **§5.3 STEP 0 (new, first line of enforcement):** *"If no manifest is present (all-open), return
  ALLOW immediately — no URL query, no grant resolution, no canonicalization, no enforcement code
  path executes."* Guarantees zero overlay code runs in all-open and makes §5.1's "per-call is
  authoritative" Note correctly vacuous in all-open. No grant lookup may precede STEP 0.
- **Falsifiable invariant (§1.3):** "The all-open posture exposes the complete engine surface at the
  mutate tier; any reduction is by definition a user-chosen (L1) or enterprise (L2) restriction."
  Test: `javascript_tool`/`form_input`/`computer:left_click` must succeed with zero manifest.
- **For manifested personas:** order-independent **deny-wins + most-specific-match + allow-wins-on-
  tie**, a top-level always-wins **`deny`** block, separated domain-scope/capability-scope
  (`domain_groups` optional sugar; inline form stays valid). Host canonicalization consumed from
  Fork 7b (§5.8). Schema-version bump + `--migrate`.

---

## 3. Behavior-across-postures matrix

**Claim (proven): every fork the all-open user *needs* is Engine/L0 and fully active with zero
overlay; no L0 capability is gated behind the overlay; no overlay artifact runs by default in
no-manifest.** (`extract` is v2 and was never load-bearing, so all-open stays complete without it.)

| # | Fork | Layer | All-open (no manifest) | User-chosen (L1) | Enterprise (L2) |
|---|---|---|---|---|---|
| 2 | Snapshot-first / tokens | Engine | **ACTIVE** — lean refs + screenshot discipline + engine-config compression (user-settable) | identical | identical |
| 3a | Risk annotations | Engine | **ACTIVE** — full quad; `computer` risk-hint-omitted but annotated | identical | identical |
| 3b | Enforced HITL gating | Overlay | **INERT** — engine dispatches; `approval.*` parsed-and-ignored | opt-in; resumable, no auto-deny | `approval:"required"`, fail-closed |
| 4 | Self-registering installer | Engine (pkg) | **ACTIVE** — pinned-ID install, `verify`/`doctor` | identical | `--extension-id` first-class; `--silent --system` |
| 5 | Audit | Overlay | **INERT** — success logging OFF; effectively silent | opt-in via manifest | full: OCSF/CEF/syslog, hash-chain, redaction |
| 6 | Advertisement | Positioning | **FULL SURFACE (13 preserved) advertised = callable** | overlay subtracts | default-deny subtracts |
| 7a | Committed-origin reporting | Engine | **ACTIVE** — true per-frame origin; raw URL preserved; O(frames) | identical | identical; feeds audit |
| 7b | Domain enforcement | Overlay | **INERT** — STEP 0 pass-through, no deny | deny/contain vs chosen origins | default-deny, 3 points, containment |
| 7c | Pipe-oracle | Opt-in | **ABSENT** — never default; same authenticated session or cut | absent | opt-in only, same session |
| 8 | Policy resolution | Overlay | **INERT** — STEP 0 → **FULL MUTATE** allow | order-independent resolution | default-deny + deny-block + specificity |
| 1 | `extract` | Engine (v2) | *(v2 — not in v1)* | *(v2)* | *(v2)* |

---

## 4. Cross-fork dependencies & sequencing

```
LAYER 0 — ENGINE FOUNDATION (must precede everything)
  Fork 4  Self-registering installer ───► (nothing runs until install works)
          [same signing key for `key` + CRX; else --extension-id first-class]
  Fork 7a Committed-origin state model ─┬─► Fork 2 (refs bound to committed frame)
          [verify reference schema      ├─► Fork 5 (audit consumes true URL)
           before touching read bodies] └─► Fork 8 (resolution needs true host)
  Fork 2  Snapshot-first / ref system ────► Fork 7a (ref-stability ↔ frame-binding, mutual;
          [compression = engine-config,       origin metadata O(frames) caps token cost)
           user-settable, non-governance]

CORRECTNESS / POSITIONING GATE
  Fork 6  Advertisement invert (full surface = 13 preserved) + tier-vocabulary
  Fork 8  All-open FULL-MUTATE floor — §4.4 + §8.2 step 5 + §5.3 STEP 0 (SAME PR as Fork 6 §5.1)

LAYER 1/2 — OVERLAY (all inert in all-open; build after engine is excellent)
  Fork 7b Domain enforcement (needs 7a origin + shared canonicalization §5.8)
  Fork 8  Resolution semantics for manifested personas (needs 7b canonical host §5.8)
  Fork 5  Audit governance sub-object (needs 8 grant_id; success-logging opt-in)

V2 (deferred)
  Fork 1  extract (caller-directed) + the §3.4 sacred-schema amendment as a deliberate decision
  Fork 3b Enforced HITL gating (transport stubbed dormant in v1 §2.2; posture-split timeout)
  Fork 7c Pipe-oracle (opt-in; MUST reuse real session or be cut)
  Fork 1  observe/act self-healing; manifest signing; dynamic grant refresh
```

**Hard ordering rules:** (1) Fork 7a before Fork 2/5/8 — origin-truth is the substrate; its per-tool
schema-impact verification precedes any read-body change. (2) Fork 6 §5.1-invert and Fork 8
all-open-floor ship together in one PR. (3) Fork 7b/8 canonicalization (§5.8) decided jointly, owned
by 7b. (4) Fork 3b transport plumbing lands dormant in §2.2 before any §5.9 trigger references it.
(5) Fork 4's pinned-key == signing-key constraint is validated before CRX packaging, or
`--extension-id` is wired as a first-class input.

---

## 5. Recommended v1 scope

**Tier A — Engine, base delight (L0 must be complete here):**
- Fork 4: single-mechanism self-registering installer + `verify`/`doctor` (Phase 1/2).
- Fork 2: lean ref snapshots, screenshot discipline, engine-config (non-governance) compression,
  ref-stability contract, O(frames) origin cap.
- Fork 3a: complete per-tool annotation quad; `computer` risk-hint-omitted-but-annotated.

**Tier B — Engine correctness (the load-bearing truth layer):**
- Fork 7a: page-truthful committed-origin state model; reads carry true per-frame origin via
  verified existing fields or an additive metadata channel; raw URL preserved; §7.4 corrected.
- Fork 6: §5.1 advertisement invert to full-surface baseline (13 preserved) + tier-vocabulary.
- Fork 8 (all-open half): §4.4 + §8.2 step 5 + §5.3 STEP 0 → `mutate`/short-circuit; falsifiable
  invariant.

**Tier C — Overlay (inert in all-open; delightful, not merely correct):**
- Fork 8 (manifested half): order-independent deny-wins + most-specific + deny-block + scope
  separation.
- Fork 7b: 3-point enforcement (§5.6/§5.7), containment, opaque-origin deny (manifest-gated),
  shared canonicalization (§5.8).
- Fork 5: core/governance record split, output-audit field, success-logging opt-in (OFF in
  no-manifest), governance-scoped redaction, OCSF/CEF/hash-chain.

**v2 (deferred, explicitly cautioned):** Fork 1 `extract` + the §3.4 sacred-schema amendment;
Fork 3b enforced HITL gating (transport dormant in v1 §2.2; `approval.*` parsed-and-ignored in v1;
posture-split timeout); Fork 1 `observe`/`act`; Fork 7c pipe-oracle (must reuse the real session or
be cut); manifest signing; dynamic grant refresh.

---

## 6. Consolidated SPEC-change map

Rows marked **[v2]** are deferred with `extract`; all others are v1.

| Section | Concrete change |
|---|---|
| **§1.2 / §1.3** | All-open invariant: *"advertised == callable == complete engine surface (**the 13 preserved schemas**) at mutate tier."* Annotation completeness = engine-correctness; grant-resolution = overlay (STEP 0 short-circuit to full-mutate allow). Falsifiable all-open invariant (Fork 8). |
| **§2.2** | Add elicitation/MRTR capability negotiation as a **dormant engine** protocol property (Fork 3b transport home). |
| **§3 / §3.4** | **[v2]** Amend schema-preservation scope for additive tools. **v1: no change — the 13 stay byte-identical.** |
| **§3.1** | Add leanness contract (refs + on-screen text; off-screen de-prioritized; **origin metadata O(frames)**; posture-invariant). Fix Mutate-tier line to "always available in all-open; under governance, gated." **[v2]** add `extract`. |
| **§3.3** | Publish full annotation quad per tool. **`computer`: `openWorldHint` + title/description REQUIRED; `readOnly`/`destructive` intentionally omitted — sanctioned exception, documented here.** Per-action tier authoritative in binary. **[v2]** add `extract` quad. |
| **§4.1 / §4.2** | Split `defaults` into **engine-config** (screenshot_* + timeouts — user-settable, never governance-gated, same pixels every posture) and **governance-config**. Add always-wins `deny` block + optional `domain_groups`, `redact_url_query`, `format`, `hash_chain`, `include_tool_output_in_log`, `approval.on_no_channel` (**v1: parsed-and-ignored w/ warning**), delegation `sub`/`act.sub`. `domains` matched vs canonical committed origin. Version bump + `--migrate`. |
| **§4.3** | Replace first-match-wins with: **all-open short-circuit → full-mutate allow**; else deny-block first, collect-all + most-specific, allow-wins-on-tie, capability-scope, `unlisted_domains` fallback. Read committed `securityOrigin`. Delete "Grant order matters." |
| **§4.4** | **Built-in default → `unlisted_domains:"mutate"`.** Audit = denials/errors only (success logging OFF). Engine-config screenshot/timeout constants noted (user-settable). |
| **§5.0 (engine chapter)** | Committed-origin state model **relocated to the engine chapter**, consumed by §5 by reference. **Per-tool verification recorded:** existing reference field vs. additive metadata channel (trained bodies stay byte-identical). Raw URL authoritative; canonical matching-key overlay-only. |
| **§5.1** | **Invert:** Step 0 = advertise complete surface (**13 preserved**) unconditionally in all-open; steps 1–4 only when a grant exists. Filtering active L1+L2, inert in all-open; never the token lever. |
| **§5.3** | **Add STEP 0 (first line):** all-open → return ALLOW immediately; no grant lookup may precede it. Reconciles §5.1's "not a security boundary" Note. |
| **§5.4** | Explicit all-open pass-through clause; per-action tier authoritative under governance. |
| **§5.5** | Denial Response Format (unchanged number). |
| **§5.6 (new)** | New-target enforcement (`Target.createTarget`). |
| **§5.7 (new)** | Fail-safe containment (to `about:blank`, never crash). |
| **§5.8 (new)** | Host-canonicalization for matching only (NFKC+IDNA+inet_aton), overlay-only, owned by Fork 7b. |
| **§5.9 (new)** | Overlay HITL trigger (references §2.2 plumbing). **[v2]** reject `approval:"required"` if no channel; `on_no_channel` default deny (L2) / resumable-no-auto-deny (L1); posture-split timeout. |
| **§6.1 / §6.3** | Screenshot compression constants (55/30/512000) = **engine-config, user-settable, identical every posture, never governance-gated**. Firewall note: token efficiency is engine-sourced, NOT tool-filtering. |
| **§7.1 / §7.2** | Engine-neutral core record + optional `governance` sub-object (present only when overlay active). Add `output` slot + `include_tool_output_in_log`. **No-manifest: success logging OFF (opt-in).** Rewrite `url` paragraph: full-fidelity when logging active; in-binary `redact_url_query` governance-gated, default off. Represent `result:denied`. Distinguish no-manifest (zero audit) vs explicit all-open manifest (follows manifest). |
| **§7.4** | Correct: "audit as trustworthy as the **weakest of {binary, extension}**; committed-origin is extension-relayed and forgeable; oracle is the only extension-independent path." |
| **§8.1 / §8.2 / §8.4 (new)** | Self-registering `browser-mcp install` (absolute paths, pinned-ID `allowed_origins`, namespaced host). **Same public key for `key` field AND CRX signing**, else **`--extension-id <id>` first-class**; `--extension-id id|auto`. Idempotent HKCU + HKLM `--system` + retraction. **§8.2 step 5 default → `mutate`.** `verify`/`doctor` diagnoses transport chain only. Each edit: "no MCP tool schema changed." |
| **§9.2** | Extension in the CDP trust path can forge origin signals; message-signing can't fix a compromised source. Mitigation = force-install + pin ID/hash + CRX integrity; residual risk accepted (identical to official Claude in Chrome). |
| **§2.4 / extension manifest** | Pin stable `key` field → deterministic build-time ID (sole ID mechanism), **matching the CRX signing key**. |
| **§10 / §11** | Move enforced HITL + `observe`/`act` + pipe-oracle + **`extract`** to cautioned v2. **Pipe-oracle: opt-in profile that MUST reuse the user's authenticated session; if a clean profile is required, CUT it (Principle 3), do not defer.** Replace vague "Conditional HITL" bullet with annotation(v1)/gating(v2) split + posture-split timeout. |
| **Appendix A** | A.2 (personal) `unlisted_domains:"mutate"` matches the no-manifest floor; keep `include_tool_parameters:true`, add `include_tool_output_in_log:true` — noting these apply because A.2 is an *explicit* manifest (opt-in L1 audit), unlike no-manifest. PHI-omission (`false`) stays in A.1 only. |

---

## 7. Adversarial findings (workflow critique)

The synthesis pass was critiqued adversarially; 15 findings were raised. In v1: **#2** (default→mutate
at §4.4 *and* §8.2 step 5 *and* §5.3, full-SPEC grep), **#3** (compression = user-settable
engine-config), **#5** (`computer` annotated-with-risk-hints-omitted vs unannotated), **#6** (reads
populate verified existing fields or an additive channel), **#7** (v1 parses-and-ignores
`approval.*`), **#8** (all-open success logging OFF), **#9** (§5.3 STEP 0 short-circuit), **#10**
(origin metadata O(frames)), **#11** (same signing key or `--extension-id` first-class), **#12**
(oracle reuses real session or is cut), **#13** (clean renumber §5.6–§5.9), **#14** (no-manifest vs
explicit-manifest audit), **#15** (posture-split HITL timeout) are all resolved. **#1** (§3.4
additive-schema) and **#4** (`extract` `attr:"value"` leak) are **moot in v1** — they concerned
`extract`, now deferred to v2, where they must be resolved as prerequisites.

---

## Provenance

Produced by workflow `relens-eight-forks` (run `wf_68eea25c-c61`): 8 fork analysts × 3 adversarial
verification lenses (north-star-guardian, posture-coverage, concern-separation/feasibility), then
synthesis→critique→revise. Owner reviewed; `extract` deferred by owner decision. Feeds the SPEC
§1–§11 revisions.
