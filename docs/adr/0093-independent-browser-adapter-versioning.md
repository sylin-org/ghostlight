# ADR-0093: Browser adapters version independently

- Status: Accepted
- Date: 2026-08-01
- Amends: the coupled release convention in `docs/RELEASE.md`
- Builds on: ADR-0053 (thin extension) and ADR-0065 (normal version skew)

## Context

Ghostlight originally stamped the service version into every release surface, including the Chrome
extension manifest and extension artifact name. That made a service-only fix look like a browser
adapter release. It also implied that every service patch needed another store review even when
`extension/` had not changed.

The architecture already rejects that coupling. ADR-0053 keeps the extension thin so most work can
ship in the service. ADR-0065 says old and new extensions must tolerate service skew through an
additive wire contract. The release model should say the same thing plainly.

## Decision

1. The Ghostlight service and each browser adapter have independent versions. The workspace,
   package-manager, npm, and MCP Registry versions describe the service. The Chrome adapter version
   is the `version` in `extension/manifest.json`.
2. `compatibility.json` is the canonical machine-readable map. Each row states one Chrome adapter
   version plus the inclusive minimum and maximum Ghostlight service versions it covers.
3. A service release does not change the Chrome adapter version. When the wire remains compatible,
   the release change extends the current adapter row's maximum service version. A real adapter
   change increments the manifest version and adds a new compatibility row.
4. Release preflight and public-surface CI refuse a service release unless the source adapter, the
   public store adapter, and any pending store adapter all cover that service version.
5. GitHub release bundles name the extension artifact from the adapter manifest, not the service
   tag. Store submission remains conditional on an actual `extension/` change.
6. Runtime mismatch reporting is deferred until a future genuine adapter release adds its version
   to the additive browser identity frame. The current frame carries identity and capabilities but
   no adapter version. Doctor must not guess from browser files or claim it knows the live version.

## Initial map

- Chrome adapter 0.6.0 covers Ghostlight service versions 0.6.0-0.7.2.
- Chrome adapter 0.7.1 covers Ghostlight service versions 0.7.1-0.7.2.
- Chrome adapter 0.7.2 covers Ghostlight service versions 0.7.1-0.7.2.

Adapter 0.7.2 is the baseline already present in the immutable v0.7.2 GitHub release. This decision
does not rewrite that release or resubmit the store package. The versions begin diverging with the
next service-only release.

## Consequences

- A Rust-only release can ship without a meaningless manifest bump or store queue reset.
- People can read one direct support statement instead of inferring compatibility from matching
  version numbers.
- Every service release makes adapter support an explicit reviewed claim.
- Full runtime enforcement waits for truthful version evidence from the adapter itself.

## Amendment: compatibility blocks from 0.8 onward

- Date: 2026-08-05

Starting with 0.8.0, the compatibility contract is the major/minor version block. An adapter row
with `"serviceVersionBlock": "0.8"` covers every 0.8.x service patch. Adapter 0.8.0 therefore
covers service 0.8.0, 0.8.29, and every other 0.8 patch. It does not cover 0.7.x or 0.9.x.

Service and adapter patch versions remain independent. A compatible implementation fix increments
only the component that changed. A contract change increments the minor version and requires both
components to declare the new block. Historical published rows retain their explicit inclusive
ranges; the unpublished 0.7.3 bridge row is removed.

This replaces Decision 2, Decision 3, and the compatibility part of Decision 4 for 0.8 and later.
Release checks require the source adapter to cover the source service, and the public adapter to
cover the public service. A pending adapter must cover the candidate service.
