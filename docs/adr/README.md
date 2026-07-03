# Architecture Decision Records

Each ADR captures one significant decision (its context, the decision, and its consequences) in
[MADR-lite](https://adr.github.io/madr/) form. ADRs are immutable once accepted: a later decision
*supersedes* an earlier one rather than editing it, so the evolution stays legible. The main docs
(README, `docs/SPEC.md`, `CLAUDE.md`) present the current design as greenfield; the *why* and the
history live here.

| # | Decision | Status |
|---|---|---|
| [0001](0001-single-binary-thin-extension.md) | Single portable Rust binary + thin Chromium extension | Accepted |
| [0002](0002-dual-role-binary-local-ipc.md) | Dual-role binary bridged over local IPC | Accepted |
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
| [0021](0021-ghostlight-brand-and-family.md) | Ghostlight brand and product family | Accepted |
| [0022](0022-intent-calibrated-capabilities.md) | Intent-calibrated capabilities: epistemic classification, per-action requirements, host polarity | Accepted |

## Conventions

- Filenames: `NNNN-kebab-title.md`, zero-padded, monotonically increasing.
- Status is one of `Proposed`, `Accepted`, `Superseded by ADR-XXXX`, or `Supersedes ADR-YYYY`.
- A decision that changes an earlier one gets a **new** ADR; the old one is marked Superseded rather
  than rewritten. History is preserved, not edited.
