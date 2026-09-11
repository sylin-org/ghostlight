# Adapter 1.1.4 custody -- 2026-09-10

Status: Published. On 2026-09-11 Google's API reported 1.1.4 PUBLISHED at 100
percent distribution, and the independent public update feed served 1.1.4.
The owner authorized renumbering the prepared fixes to 1.1.4 and pushing them
after personally rolling back the public extension.

## Version identities

- Google's update feed first confirmed public 1.1.2, then confirmed public 1.1.3
  after the owner reported rollback. The dashboard confirmation identifies the
  rollback source as 1.1.1 and its new public version as 1.1.3.
- Public rollback 1.1.3 follows the 1.1.1 compatibility range, service 1.3.3-1.3.5.
- The unsubmitted local 1.1.3 ZIP from September 9 contains newer fixes, not the
  rollback payload. Preserve it as historical evidence; never upload it.
- New fixes use 1.1.4 and retain their service 1.3.5 compatibility. Service and
  package versions are unchanged. Public metadata now reflects observed 1.1.3.

## New artifact

- Path: `dist/ghostlight-extension-v1.1.4.zip`.
- SHA-256: `dbc145b62bd2107b627588cdfcb24ac98b558db0cec136dfb4ee146ea73608db`.
- Built by the existing deterministic extension packager. No new permissions,
  extension identity, listing text or privacy declaration changes.
- The payload carries the human-tab ownership correction, document-bound typing
  correction, opt-in connection diagnostics/export and options-page fixes already
  recorded in the [fleet wrap](release-wrap-2026-09-09.md).

Validation: all 39 ZIP entries were compared to the prior local candidate; only
the manifest version differs. Formatting, Clippy with warnings denied, all 537
Windows Rust tests, all 222 extension tests and offline public/compatibility
checks pass. No runtime or extension JavaScript source changed in this revision.

## API submission evidence

The owner authorized API upload and submission after browser tooling rejected
developer-dashboard navigation. The owner completed production OAuth setup and
consent. The missing publisher ID was recovered from a recent dashboard URL in
browser history. Credential locations and recovery notes are recorded under
owner-authorized, gitignored `local/`; no credential values belong in this record.

The existing `scripts/publish-extension.ps1` used the exact ZIP and SHA-256 above
with the explicit local credential file. Google's API returned:

- Before upload: public 1.1.3 PUBLISHED, with no submitted revision.
- Upload: SUCCEEDED, draft version 1.1.4.
- Submit: PENDING_REVIEW, publish type STAGED_PUBLISH.
- Independent status read after submission: public 1.1.3 PUBLISHED and submitted
  1.1.4 PENDING_REVIEW, both with distribution percentage 100.

The item is `lejccfmoeogmhemakeknjjdhkfkgncdl`. On 2026-09-11 the API no longer
reported a submitted revision; it reported public 1.1.4 PUBLISHED at 100 percent.
`scripts/reconcile-chrome-store.ps1` independently observed 1.1.4 in Google's public
update feed and updated `docs/public-status.json`. Do not upload the historical local
1.1.3 ZIP.

The prior publication monitor remains deleted. No service package publication,
live binary swap or new Linux assignment is part of this renumbering.
