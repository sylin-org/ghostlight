# 1.1.0 candidate custody -- 2026-08-27

Status: HELD. The 1.1.0 candidate (the ZCode harness integration) is assembled, attested,
downloaded, and verified from two independent local copies. Nothing has published or submitted.
The Chrome adapter is 1.0.0 in this candidate and on the store; there is no extension candidate.

## Build

- Workflow run: [33097568009](https://github.com/sylin-org/ghostlight/actions/runs/33097568009)
  ("Build release candidate", dispatched on `dev`), all seven jobs green.
- Candidate source revision: `655e2078ef1631e1d64e50c777c0ac12398a1196`, pinned by
  `docs/release/freeze.json`.
- Candidate version 1.1.0, 18 artifacts (six raw binaries, two native packages, two portable
  archives, the deterministic extension ZIP, the npm launcher, the MCPB, and five component
  SBOMs), each bound by SHA-256 in `SHA256SUMS` and attested by GitHub build provenance for the
  raw binaries.
- Supersedes two retracted same-day candidates: `f912a834` (run 33089681641, held briefly and
  retracted by the owner) and `464f145a` (run 33095536687, never held). Both restamped the
  unmodified Chrome adapter as 1.1.0. The f912a834 copies remain archived under the machine-local
  `.target-g2-custody-superseded-f912a834` directory.

## Journey to green (one defect class, six finds)

The 1.1.0 workspace bump surfaced every hand-stamped `1.0.0` copy that asserts against live
output, and then the owner caught the deeper disease: the release machinery had quietly
restamped the independently versioned Chrome adapter along with the service.

Version pins (fixed at the seam for each kind: test assertions derive; released artifact
identity is stamped; the guards that compare the two held):

1. The CLI journey pinned `ghostlight --version` to the literal banner. Fix (`45874122`): the
   journey derives the expected banner from the workspace version.
2. `scripts/check-debian-package-lifecycle.sh` pinned four literals: the version banner, the
   `dpkg-query` version, the MCP initialize `serverInfo.version`, and the reinstall banner.
   Fix (`a73533cf`): the smoke derives `expected_version` once from the checked-out
   `Cargo.toml`.
3. The candidate npm guard correctly refused `packaging/npm/package.json` at 1.0.0. Fix
   (`f912a834`): the client-facing packaging identity was stamped 1.1.0 (npm `package.json`,
   MCPB `manifest.json`, the npm checksums placeholder, a new Debian changelog stanza, and the
   three man pages' `.TH` version and date).

Adapter independence (the owner's correction, `1670e237`, `464f145a`'s replacement, and
`655e2078`):

4. The 1.1.0 bump had mechanically restamped `extension/manifest.json` and declared an
   adapter 1.1.0 compatibility row, but nothing in `extension/` changed. The adapter is
   independently versioned (ADR-0093) and the store serves approved 1.0.0 bytes. The manifest
   and package were reverted to 1.0.0, and `compatibility.json` now records the truth as one
   range row: adapter 1.0.0 covers service 1.0.0 through 1.1.0.
5. Two equality guards had forced that restamping and were removed:
   `check-public-surfaces.ps1` asserted manifest version equals the workspace version, and
   `check-repository-integrity.ps1` asserted the same in its own copy. The adapter's fitness
   is the registry-derived coverage gate in `adapter-compatibility.ps1`, which both scripts
   already run.
6. The assembler restamped the packaged ZIP's name to
   `ghostlight-extension-v<service-version>.zip`, so the 464f145a candidate shipped a
   v1.1.0-named ZIP whose inner manifest is 1.0.0. Fix (`655e2078`): assembly preserves the
   name `package-extension.ps1` derives from the manifest, so the roster binds the adapter's
   own version.

## Verification results

`scripts/verify-custody.ps1 -IncludeProvenance` against both local copies:

- freeze binding: PASS (manifest sourceRevision == `docs/release/freeze.json`);
- deep candidate checks: PASS (18 artifacts, exact roster and coordinates, every hash);
- SHA256SUMS recomputation: PASS (all 18 lines rehashed from bytes);
- GitHub provenance attestations: PASS for all six raw binaries.

Local custody: two verified copies under repo-local `.target-g2-custody/release-candidate` and
`.target-g2-custody-copy` (paths are machine-local working state; not secrets).

## Publish-relevant asset hashes

- Extension ZIP `ghostlight-extension-v1.0.0.zip`, SHA-256
  `3544590ad728250ec2cec3f5eef05134ef554c3bed900ce064f35105bb7f40a0`. This is the current
  1.0.0-labeled extension source, and it is deliberately NOT byte-identical to the store's
  approved 1.0.0 revision (`3570494faf...`): the D1 presentation-stylesheet module refactor
  (`f8bff79a`) changed extension source after that approval. The store listing remains the
  sole distribution authority (ADR-0091); no submission is planned or needed for 1.1.0, and
  the candidate binds the adapter bytes for provenance only.
- npm `ghostlight-1.1.0.tgz`, SHA-256
  `b4dee8b09fe39b5110ad77c183d4c3d68a9ff29fdbe560a03497e26e767e1267`.
- MCPB `ghostlight-v1.1.0.mcpb`, SHA-256
  `68e0671d511d67faf5348ba379dd1dbdd97bddf18da9ab84208ea5c8b891e89f`.

The GitHub release, npm publish, MCP Registry record, and website copy are owner-authorized
channel actions and have not been performed. The Chrome Web Store needs no action for 1.1.0.
