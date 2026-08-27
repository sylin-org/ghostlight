# 1.1.0 candidate custody -- 2026-08-27

Status: HELD. The 1.1.0 candidate (the ZCode harness integration) is assembled, attested,
downloaded, and verified from two independent local copies. Nothing has published or submitted.

## Build

- Workflow run: [33089681641](https://github.com/sylin-org/ghostlight/actions/runs/33089681641)
  ("Build release candidate", dispatched on `dev`), all seven jobs green.
- Candidate source revision: `f912a834a30eb704269c21dc25d7fe3f76ec3d31`, pinned by
  `docs/release/freeze.json`.
- Candidate version 1.1.0, 18 artifacts (the same roster as 1.0.0: six raw binaries, two native
  packages, two portable archives, the deterministic extension ZIP, the npm launcher, the MCPB,
  and five component SBOMs), each bound by SHA-256 in `SHA256SUMS` and attested by GitHub build
  provenance for the raw binaries.
- Supersedes the published 1.0.0 candidate at `b2c27993` only in the sense of being the next
  candidate; 1.0.0 remains the published release on every channel.

## Journey to green (one defect class, three finds)

The 1.1.0 workspace bump surfaced every hand-stamped `1.0.0` copy that asserted against live
output. Fixed at the seam for each kind: test assertions derive the expected version from the
workspace; released artifact identity is stamped, and the guards that compare the two held.

1. The CLI journey pinned `ghostlight --version` to the literal banner. Fix (`45874122`): the
   journey derives the expected banner from the workspace version.
2. `scripts/check-debian-package-lifecycle.sh` pinned four literals: the version banner, the
   `dpkg-query` version, the MCP initialize `serverInfo.version`, and the reinstall banner.
   Fix (`a73533cf`): the smoke derives `expected_version` once from the checked-out
   `Cargo.toml` and uses it in all four places.
3. The candidate bundle's npm guard refused the build: `packaging/npm/package.json` still
   declared 1.0.0, so `prepare-npm-package.ps1` threw
   `npm package identity does not match release version 1.1.0`. The guard worked as designed --
   it exists to stop a mismatched tarball from being staged. Fix (`f912a834`): the release-bump
   commit had missed the client-facing packaging identity, so the payloads were stamped 1.1.0
   (npm `package.json`, MCPB `manifest.json`, the npm checksums placeholder, a new Debian
   changelog stanza, and the three man pages' `.TH` version and date). `compatibility.json`'s
   adapter-1.0.0 row is the service-block-1.0 compatibility record, not a stale stamp, and was
   left alone.

## Verification results

`scripts/verify-custody.ps1 -IncludeProvenance` against both local copies:

- freeze binding: PASS (manifest sourceRevision == `docs/release/freeze.json`);
- deep candidate checks: PASS (18 artifacts, exact roster and coordinates, every hash);
- SHA256SUMS recomputation: PASS (all 18 lines rehashed from bytes);
- GitHub provenance attestations: PASS for all six raw binaries.

Local custody: two verified copies under repo-local `.target-g2-custody/release-candidate` and
`.target-g2-custody-copy` (paths are machine-local working state; not secrets).

## Publish-relevant asset hashes

- Extension ZIP `ghostlight-extension-v1.1.0.zip`, SHA-256
  `32e7c21b881613518b2ca20a414304f97e700420e43e992bd75c9a81d02a3460`. The store serves the
  approved 1.0.0 revision, so these new extension bytes require a fresh store submission before
  the 1.1.0 publication completes G3; same replacement procedure as the 2026-08-24/25 records,
  via `scripts/publish-extension.ps1`. The packager is the cross-OS-deterministic one pinned
  since `bd3bffe4`/`9ee05666`.
- npm `ghostlight-1.1.0.tgz`, SHA-256
  `856a4ff80e6436ee8b5eb21698932259bc06ac23bb4ce7e0b76357f45730da8e`.
- MCPB `ghostlight-v1.1.0.mcpb`, SHA-256
  `4f23d3650f8273c53cd75a92fea42a596bc04808dc7bad00f3ad5476eb8252df`.

The GitHub release, npm publish, store submission, MCP Registry record, and website copy are
owner-authorized channel actions and have not been performed.
