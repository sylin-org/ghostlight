# Adapter 1.1.6 custody -- 2026-09-13

Status: Prepared; upload and submission not yet performed.

The owner authorized a version bump and submission to Google Chrome for deployment.
The source version advances from local investigation build 1.1.5 to 1.1.6. The
service remains 1.3.6. This adapter retains the existing wire commands and capability
revisions and covers services 1.3.4-1.3.6. No browser permission or store identity
changes are included.

## Artifact and verification

- Path: `dist/ghostlight-extension-v1.1.6.zip`.
- SHA-256: `3779689cd12ccd2f3b0d012144710026c61fe6f65ec0d63cf1663f8829b22eef`.
- Implementation commit: `f82793ea`.
- The existing deterministic packager reproduced the same ZIP hash twice. All 39
  non-manifest entries match their source files byte-for-byte. The packager checks
  the complete allowed surface, license files, version, and removed development key.
- This release changes only the manifest version relative to the installed adapter
  acceptance source. The [installed evidence](controlled-tab-focus-installed-2026-09-13.json)
  records five retained DOM/model fields across 81.465 seconds in an inactive tab,
  return, click, and another 29.159 seconds, without submitting the form.
- All 265 extension tests pass. All extension JavaScript passes syntax checks.
  Rust formatting, warnings-denied Clippy and workspace tests pass using the
  isolated `.target-form-retention` build directory. Offline public-surface,
  repository-integrity and diff checks pass.

The fix retains emulated focus only during active control of retained tabs.
Pause, stop, disconnect and release restore ordinary browser behavior. Sites may
still reset unsaved drafts after control ends; permanent draft retention is not
a release promise. ADR-0168 records the mechanism and cleanup boundaries.

## Chrome API evidence

The existing release script uses the explicitly selected repo-local credential
file. No secrets belong in this record. Item: `lejccfmoeogmhemakeknjjdhkfkgncdl`.

- Plan: API ready; version and SHA-256 match the artifact above.
- Before upload: public 1.1.4 PUBLISHED at 100 percent, no submitted revision.
- Intended publish type: DEFAULT_PUBLISH, because the owner authorized deployment.
  Google review remains required; approval permits automatic public publication.

No service package, npm, GitHub release, or parallel local deployment is included.
Public delivery must be observed independently before updating public version claims.
