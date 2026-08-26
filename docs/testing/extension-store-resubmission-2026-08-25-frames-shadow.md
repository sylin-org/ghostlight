# Extension store resubmission -- 2026-08-25 (frames + shadow package)

Status: SUBMITTED. The pending review of the custody candidate (`9ae88e67...`) was canceled and
replaced by the current-source package containing the frame-transparent semantic layer
(ADR-0138), the shadow-complete interaction (ADR-0139), and the tab/group reuse worker. This
remains staged review traffic, not publication: the public listing still serves 0.8.0, and
publication stays an explicit owner action.

## Package

```text
date_utc: 2026-08-25
version: 1.0.0
source_state: dev working tree at 73ef34a8, clean
sha256: 3570494faf580a2286d9f7a5f1cbb6f657864ee369b0f70b944b0c927e64770c
size: 97,946 bytes, 33 entries (was 89,441 bytes / 32 entries at 9ae88e67; adds
      lib/frames.js and the frame/shadow service-worker and content-script changes)
determinism: two consecutive scripts/package-extension.ps1 runs produced byte-identical
      archives; development key stripped; manifest all_frames true verified in the archive
artifact: dist/ghostlight-extension-v1.0.0.zip (the packager's sanctioned gitignored output;
      an earlier scratch copy outside this location was removed at owner direction)
```

Permission surface is unchanged from the reviewed 0.8.0 and prior 1.0.0 drafts: no new
permissions, host permissions unchanged; `all_frames` is injection behavior, not a permission.

## Sequence

```text
item: lejccfmoeogmhemakeknjjdhkfkgncdl (Ghostlight in Browser, publisher Sylin.org)
api:  CWS v2 publishers/{id}/items/{id}:cancelSubmission, :upload, :publish, fetchStatus
```

1. `fetchStatus` before: published 0.8.0 (100%); submitted 1.0.0 `PENDING_REVIEW` (the
   `9ae88e67...` package), which locks the item.
2. Upload attempt returned HTTP 400 -- the expected in-review lock, same as the 2026-08-25
   morning resubmission.
3. Owner directed the replacement ("replace the Chrome Web Store submission with the new
   fixed package"), which covers the documented cancel-and-resubmit remedy.
4. `POST :cancelSubmission` returned HTTP 200 `{}`.
5. Upload returned `uploadState: SUCCEEDED`, draft version 1.0.0.
6. Submit returned `PENDING_REVIEW` with `publishType: STAGED_PUBLISH`.
7. `fetchStatus` after: published channel unchanged at 0.8.0 (100%); submitted 1.0.0
   `PENDING_REVIEW` (package `3570494f...`).

All calls ran through `scripts/publish-extension.ps1`, which gained first-class `Cancel` and
`Status` actions in this change so the documented remedy no longer requires inline
authenticated one-offs. Credentials came from `~/.ghostlight-release.env` by location only;
no credential values were read into any record.

## Owner authorization trail

One explicit authorization at the moment of action: "replace the Chrome Web Store submission
with the new fixed package" (covers cancel of the locked pending review, upload, and staged
resubmission, per the standing store-mutation rule and the 2026-08-25 precedent).

## Consequences

- The reviewed package now contains the frame- and shadow-complete extension; the store
  bytes match the development tree this document was written from.
- Review approval still publishes nothing: staged publication keeps G10's store row an
  owner-authorization boundary.
- The orchestrator-side focused-typing fix (`b26c3ecf`) is service bytes, not store bytes;
  it ships through the normal installed-package path, not this review.
