# ADR-0142: Per-member release version spaces

- Status: Accepted
- Date: 2026-08-27
- Amends: ADR-0093
- Builds on: ADR-0091

## Context

Ghostlight ships four members: the orchestrator/service, the MCP connector, the browser
connector, and the Chrome extension. Three of them are Rust executables built from one
workspace; the extension is browser-side JavaScript with its own manifest and its own store
listing.

The 1.1.0 release preparation treated the tree as one version space, and the machinery
enforced it. The bump commit restamped the unmodified extension manifest to 1.1.0 and declared
an adapter 1.1.0 compatibility row; two guards asserted the manifest version equals the
workspace version, which is what forced the restamp; and the candidate assembler renamed the
packaged ZIP to carry the service version. Two candidates were built, held, and retracted
(`f912a834`, `464f145a`) before the owner caught the disease: a service-only release had
manufactured an extension release that does not exist. The store still serves the approved
1.0.0 adapter, and nothing in `extension/` had changed.

On 2026-08-27 the owner stated the versioning model directly: every member has its own
version. The adapters and the extension should be as static as possible, and only the
service keeps updating. Lockstep Rust is acceptable if per-member Rust versions would add
bureaucracy without information. The extension is clearly a separate versioning space, and
compatibility maps in version blocks remain the coordination device between the spaces.

## Decision

- The workspace version is the service line. The orchestrator, the MCP connector, the browser
  connector, and every service-derived artifact (npm launcher, MCPB, NSIS and Debian packages,
  portable archives, component SBOMs, `server.json`, and registry records) carry it in
  lockstep. One Rust workspace, one version; splitting the connectors onto their own version
  lines would add release ceremony without a compatibility fact, because both connectors are
  demanded-start siblings of the same service install.
- The Chrome extension owns its version in `extension/manifest.json`. It changes only when
  extension source changes, and nothing outside the extension may write, restamp, or rename
  it. The extension packager names the ZIP from the manifest; release assembly preserves that
  name.
- Release tooling must never assert or manufacture equality between the service version and
  the adapter version. Adapter fitness is derived from `compatibility.json`: one row per
  adapter version, each covering a service version block or an explicit service version
  range. A service release with no extension change is a compatibility-registry edit, never
  an extension edit.
- When the extension does change, its new version joins the map with its own row, the store
  submission follows the established procedure, and `public-status.json` records the observed
  public adapter as it does today.

## Consequences

- A service-only release needs no store action and produces no extension candidate. The
  1.1.0 candidate binds the current 1.0.0-labeled extension ZIP for provenance only; that ZIP
  is deliberately not byte-identical to the store's approved 1.0.0 because the D1 stylesheet
  refactor (`f8bff79a`) landed after the store approval. That divergence is expected and
  recorded in the custody record, not a defect to fix by resubmitting.
- The standing gate is registry coverage in both directions -- the source adapter must cover
  the source service, and the public adapter must cover the public service -- checked by
  `adapter-compatibility.ps1`, which both public-surface and repository-integrity checking
  already run. A service minor or patch release extends the live adapter's range row; that
  one-line edit is the deliberate compatibility attestation, not bureaucracy.
- The removed equality guards are recorded so they are not re-proposed:
  `check-public-surfaces.ps1` and `check-repository-integrity.ps1` asserted manifest version
  equals the workspace version until 2026-08-27, and the assembler restamped the extension
  ZIP's name until the same day.
