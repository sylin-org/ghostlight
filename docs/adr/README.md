# Architecture Decision Records

Each ADR captures one significant decision (its context, the decision, and its consequences) in
[MADR-lite](https://adr.github.io/madr/) form. ADRs are immutable once accepted: a later decision
*supersedes* an earlier one rather than editing it, so the evolution stays legible. The main docs
(README, `docs/SPEC.md`, `CLAUDE.md`) present the current design as greenfield; the *why* and the
history live here.

| # | Decision | Status |
|---|---|---|
| [0001](0001-single-binary-thin-extension.md) | Single portable Rust binary + thin Chromium extension | Accepted (single-binary aspect superseded by [0046](0046-role-specific-executables.md)) |
| [0002](0002-dual-role-binary-local-ipc.md) | Dual-role binary bridged over local IPC | Accepted (dual-role-binary aspect superseded by [0046](0046-role-specific-executables.md)) |
| [0003](0003-tokio-native-ipc.md) | IPC transport: tokio-native named-pipe/UDS, single-session, no heartbeat | Accepted |
| [0004](0004-reject-second-session.md) | Reject a second concurrent session | Accepted |
| [0005](0005-policy-free-extension.md) | Policy-free extension; DOM reads in a content script | Accepted |
| [0006](0006-mcp-client-agnostic.md) | MCP-client-agnostic server | Accepted |
| [0007](0007-sacred-tool-surface.md) | Sacred tool surface: byte-parity with the official Claude-in-Chrome | Accepted |
| [0008](0008-not-a-port.md) | Not a port: harvest intent and techniques, not code | Accepted |
| [0009](0009-coordinate-model-devicescale.md) | Screenshot coordinate model: `deviceScaleFactor:1` normalization | Superseded by [0010](0010-coordinate-model-official.md) |
| [0010](0010-coordinate-model-official.md) | Screenshot coordinate model: official DPR-probe + downscale + rescale | Accepted |
| [0011](0011-truthful-engine-redaction.md) | Truthful engine + secret redaction as a governance-config key | Accepted |
| [0012](0012-ui-and-input-fidelity.md) | UI parity + input fidelity (phantom cursor, virtual key codes) | Accepted |
| [0013](0013-governance-overlay-all-open.md) | Governance as a separable overlay; no-manifest = all-open | Accepted |
| [0014](0014-v1-scope-exclusions.md) | v1 scope exclusions | Accepted |
| [0015](0015-idempotent-merge-installer.md) | Self-registering installer via idempotent value-level JSON merge | Accepted |
| [0016](0016-debug-mode-pinned-extension-id.md) | Debug/observability mode + pinned dev extension id | Accepted |
| [0017](0017-release-1-engine-hardening.md) | Release 1 engine hardening | Accepted |
| [0018](0018-governance-observe-then-enforce.md) | Governance ships observe-then-enforce | Accepted |
| [0019](0019-layered-configuration-model.md) | Layered configuration: typed key registry, presets, org locks | Accepted |
| [0020](0020-org-policy-experience.md) | Org policy experience: policy as code with explain, simulate, shadow | Accepted |
| [0021](0021-ghostlight-brand-and-family.md) | Ghostlight brand and product family | Accepted (whole-repo license stance narrowed by [0027](0027-open-core-business-model-and-licensing.md)) |
| [0022](0022-intent-calibrated-capabilities.md) | Intent-calibrated capabilities: epistemic classification, per-action requirements, host polarity | Accepted |
| [0023](0023-one-loader-for-the-policy-file.md) | One loader for the policy file | Accepted |
| [0024](0024-tool-registry-and-generic-ingest-pipeline.md) | Tool registry and the generic ingest pipeline | Accepted |
| [0025](0025-manifest-hot-reload.md) | Manifest hot-reload | Accepted |
| [0026](0026-release-maturity-and-externalities.md) | Release maturity and externalities sequencing (license, CI, spec currency, syslog + managed://, extension JS coverage, live-verify) | Accepted |
| [0027](0027-open-core-business-model-and-licensing.md) | Open-core business model and licensing (permissive engine, commercial source-available governance module) | Accepted (supersedes ADR-0021 whole-repo license stance) |
| [0028](0028-tripwire-licensing-and-continuity-promise.md) | Tripwire licensing, tiers, and the Continuity Promise (purely observational license keys; never phone home) | Accepted |
| [0029](0029-process-lifecycle-hygiene.md) | Process lifecycle hygiene: parent-death watchdog and doctor --fix reaper (re-scoped to the thin adapter by ADR-0030 Decision 8) | Accepted (amended by ADR-0030 D8) |
| [0030](0030-ghostlight-hub-orchestrator.md) | Ghostlight Hub: the multi-client orchestrator service (persistent service, multiplexed adapter sessions, local web API + Console) | Accepted (adapter lifecycle amended by [0045](0045-resilient-reconnecting-adapter.md)) |
| [0031](0031-agent-onboarding-contract.md) | Agent onboarding contract: tools.json as the single source of agent-facing truth (initialize.instructions workflow preamble, per-tool examples, corrective validation errors) | Accepted (amends ADR-0007) |
| [0032](0032-test-at-seams-and-inject-config-sources.md) | Test at pure seams; inject config sources at the composition root | Accepted |
| [0033](0033-inbound-outbound-manage-zones.md) | Inbound/outbound/manage zones: the honest SoC split (renames `channels`→`inbound`, separates the management plane from the web ingestion adapter, policy-controlled listeners) | Accepted (amends ADR-0030 D5/D9) |
| [0034](0034-capability-transport-registry.md) | The Capability & Transport Registry (ICapability/ITransport traits, tool declarations in code, capability manifest at handshake, deprecates ADR-0007's byte-frozen mandate) | Accepted (amends ADR-0007, ADR-0030) |
| [0035](0035-script-tool.md) | The `script` tool: sequential multi-tool composition with `$prev`/`$N` data flow over structured results, dry-run, idempotency, budget | Accepted (amended in place 2026-07-06, pre-implementation) |
| [0036](0036-form-fill-tool.md) | The `form_fill` tool: semantic form interaction by label (one semantic Write decision, hardened matcher, dedicated form-structure read) | Accepted (amended in place 2026-07-06, pre-implementation) |
| [0037](0037-page-state-awareness.md) | Page-state awareness: `wait_for`, consequence digests on mutating actions, `read_page` diff mode, stale-ref corrective errors | Accepted |
| [0038](0038-structured-results.md) | Structured results (`structuredContent` + `outputSchema`, per-tool vocabulary) and cost-aware guidance; substrate for ADR-0035 references | Accepted |
| [0039](0039-saved-scripts-governed-artifacts.md) | Saved scripts as governed artifacts: named, parameterized, hash-bound approved workflows | Accepted (ratified by [0041](0041-post-evaluation-response.md) D4; implementation deferred to its own batch) |
| [0040](0040-pipeline-idempotency-gate.md) | Pipeline-level idempotency gate: universal pre-decision dedup with in-flight join (supersedes ADR-0035 D9's two-tool cache, not taken) | Proposed |
| [0041](0041-post-evaluation-response.md) | Post-evaluation response: standards posture (alternatives, not competition), capability onboarding discipline, origin-flow focus, P1-P10 dispositions | Accepted |
| [0042](0042-origin-flow-provenance.md) | Origin-flow provenance: the `sources` audit key (in-band `$prev`/`$N` flow attestation, honesty fence, consumer-side host join); enforcement direction-pinned for a future ADR | Accepted (phase 1) |
| [0043](0043-webmcp-stance.md) | WebMCP stance: future governed consumer via the capability registry; no implementation during origin-trial flux; named re-evaluation triggers | Accepted |
| [0044](0044-named-instances.md) | Named instances: one `--instance`/`GHOSTLIGHT_INSTANCE` parameter derives all identity (endpoint, native-host name, MCP name, supervisor names, config/policy/log dirs); default byte-identical; non-default gets isolated dirs + a multi-call binary copy for the Chrome-launched native host; enables dev/prod coexistence on one machine | Accepted |
| [0045](0045-resilient-reconnecting-adapter.md) | The resilient reconnecting adapter: the thin adapter reconnects to a restarted service and replays the MCP handshake so the client rides through transparently (raw-stream relay + timeout baseline; native-host stays Tier-B extension reconnect); makes dev rebuilds and prod upgrades/crashes seamless | Accepted (amends [0030](0030-ghostlight-hub-orchestrator.md)) |
| [0046](0046-role-specific-executables.md) | Role-specific executables: split the single multi-role binary into `ghostlight` (CLI + service) + `ghostlight-adapter-agent` + `ghostlight-adapter-browser`, backed by a `ghostlight-transport` (stable) / `ghostlight-core` (churny) crate split so a service rebuild never relinks the adapters; fixes the exe-lock dev-loop fight and makes transparent upgrades real | Accepted (supersedes single-binary aspects of [0001](0001-single-binary-thin-extension.md)/[0002](0002-dual-role-binary-local-ipc.md); refines [0044](0044-named-instances.md)/[0045](0045-resilient-reconnecting-adapter.md)) |
| [0047](0047-unified-session-tab-identity.md) | Unified session and tab-surface identity: one extension-side "managed surface" predicate (global group + every session group), a stable per-process session guid the adapter re-presents on reconnect, the session guid on the native tool envelope (tabs birth directly into the calling session's group), client-name group titles, and dead-owner tab adoption; closes the e2e F4 group desync at the root and makes ADR-0045's ride-through real at the session layer | Accepted (amends [0030](0030-ghostlight-hub-orchestrator.md)/[0045](0045-resilient-reconnecting-adapter.md); supersedes the hub batch's SS6 title pin) |
| [0048](0048-development-override.md) | The development override: an UNPINNED client or browser resolves its instance at connect time, preferring a live `dev` service and falling back to the default (dev SHADOWS release while it runs; release takes over when it stops); one unified browser host allowing both shipped extension ids, `--extension-id` optional, dev install thinned to pinned client entries, doctor reports the live routing | Accepted (amends [0044](0044-named-instances.md)/[0046](0046-role-specific-executables.md); supersedes the extension's installType host selection) |
| [0049](0049-mcp-conformance-pass.md) | MCP conformance pass: negotiate `protocolVersion` over a supported set (latest `2025-11-25`), advertise `tools.listChanged`, answer `-32700` on a malformed frame (silent on a blank line), reject a JSON-RPC batch with `-32600` and a teaching message pointing at the `script` tool, and deliberately keep NO initialize-before-use guard (ADR-0045 reconnect replay depends on it) | Accepted (amends [0041](0041-post-evaluation-response.md) D5; supersedes landscape-1 L2's `2025-06-18` target) |

## Conventions

- Filenames: `NNNN-kebab-title.md`, zero-padded, monotonically increasing.
- Status is one of `Proposed`, `Accepted`, `Superseded by ADR-XXXX`, or `Supersedes ADR-YYYY`.
- A decision that changes an earlier one gets a **new** ADR; the old one is marked Superseded rather
  than rewritten. History is preserved, not edited.
