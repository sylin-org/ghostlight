# R9: Replacement extension candidate handoff

## Goal

Build and verify the deterministic extension package that contains the restored capabilities, then
leave an exact owner-controlled Store handoff. Do not upload it.

## Required work

- Run the official extension packaging path from the R8-proven source revision.
- Build twice and prove byte identity. Record size, entry count, SHA-256, manifest version,
  permission set, icon set, and source revision.
- Compare the package with both the currently pending Store draft and the published 0.8 package.
  Explain every changed permission, file, and physical capability. Unchanged artwork and listing
  assets are not blockers.
- Re-run package inspection, repository integrity, release truth, privacy/disclosure alignment, and
  the extension-specific release checks.
- Update `docs/STATUS.md`, release checklist evidence, and the extension release record with the
  exact local candidate and the fact that the pending review is stale.
- Prepare the exact dashboard/API sequence for replacement, but stop before any Store mutation.

## Evidence

- Two independent package runs with the same SHA-256.
- Extracted package manifest and file allowlist match the source tree and ADR-0133.
- Store justification and privacy text cover the actual final permissions and data behavior with
  no development-process language.
- Ledger `RESUME HERE` names the exact artifact and the separate owner approval needed next.

## STOP conditions

- R8 is not complete against the exact source revision being packaged.
- The package is not deterministic or contains a development key, unreferenced artwork, source
  files, tests, maps, or private material.
- Completion would require upload, review submission, staged publication, push, tag, or any other
  external mutation.

## Commit

`chore(release): prepare restored extension candidate`

