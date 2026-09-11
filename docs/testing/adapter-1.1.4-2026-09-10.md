# Adapter 1.1.4 custody -- 2026-09-10

Status: Prepared for store submission. No upload or submission completed.
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

## Submission boundary

The Chrome publisher API configuration is absent. The browser tool rejected
developer-dashboard access with `Not allowed`; no alternate control path was used.
No store upload, submission or publication occurred in this attempt.
The owner can upload the exact ZIP above to the existing Ghostlight in Browser item
`lejccfmoeogmhemakeknjjdhkfkgncdl` and submit 1.1.4 for review with deferred publication.
Do not upload the historical local 1.1.3 ZIP. Record Google's actual confirmation
before changing this status to submitted.

The prior publication monitor remains deleted. No service package publication,
live binary swap or new Linux assignment is part of this renumbering.
