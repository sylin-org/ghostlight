# Extension store resubmission -- 2026-08-25

Status: SUBMITTED. The stale pending review (package `f7b9a6ad...`, submitted 2026-08-24 before
ADR-0137 changed the service worker) was canceled and replaced by the exact G2 custody candidate.
This remains long-lead review traffic, not publication: the public listing still serves adapter
0.8.0, and the staged submission cannot go live without an explicit owner publication action.

## Package

```text
date_utc: 2026-08-25
version: 1.0.0
source_revision: 994b6c85dcd7c8df74237cf329461d85ce49b13a (pinned by docs/release/freeze.json)
sha256: 9ae88e6729c830a9871802a39a2301c27c1d2baa00a2213332c310a7746a6db8
origin: .target-g2-custody/release-candidate/assets/ghostlight-extension-v1.0.0.zip
        (one of two custody copies verified the same day by
        scripts/verify-custody.ps1 -IncludeProvenance; see
        candidate-custody-2026-08-25.md)
```

## Sequence

```text
item: lejccfmoeogmhemakeknjjdhkfkgncdl (Ghostlight in Browser, publisher Sylin.org)
api:  CWS v2 publishers/{id}/items/{id}:cancelSubmission, :upload, :publish, fetchStatus
```

1. First upload attempt returned HTTP 400 `FAILED_PRECONDITION` / reason `NOT_UPDATEABLE`:
   "You may not edit or publish an item that is in review." `fetchStatus` showed why: the item's
   submitted revision was still the stale 1.0.0 (`f7b9a6ad...`) in the review queue, which locks
   the item against package edits. The 2026-08-24 submission had succeeded only because that
   day's earlier stale review had already completed.
2. Owner authorized cancel-and-resubmit after seeing the exact error and the documented remedy.
   Google's own guidance for replacing a pending submission is the Cancel-review feature
   (developer.chrome.com/docs/webstore/cancel-review) and its API form
   `publishers.items.cancelSubmission` (March 2025).
3. `POST :cancelSubmission` returned HTTP 200 `{}`. The stale review left the queue; the item
   returned to an editable draft state.
4. Upload of the custody ZIP returned `uploadState: SUCCEEDED`, draft version 1.0.0. The script
   printed SHA-256 `9ae88e67...` before the request, matching the custody manifest exactly.
5. Submit returned state `PENDING_REVIEW` with `publishType: STAGED_PUBLISH`.
6. Final `fetchStatus`: published channel unchanged at 0.8.0 (100%); submitted revision 1.0.0,
   `PENDING_REVIEW`.

All four requests ran through `scripts/publish-extension.ps1` paths (`-Action Plan` for the
hash preflight, inline authenticated calls for cancel and diagnostics, `-Action Upload/Submit
-Execute` for the mutations). Credentials came from `~/.ghostlight-release.env` by location
only; no credential values were read into any record.

## Owner authorization trail

Two explicit authorizations at the moment of action, per the standing store-mutation rule:

1. Upload + submit of the custody ZIP as the item's 1.0.0 draft (given before step 1).
2. Cancel the locked stale review, then upload + submit (given before step 3, after the
   `NOT_UPDATEABLE` diagnosis).

## Consequences

- The reviewed package now contains the ADR-0137 service worker (tab/group reuse), closing the
  drift called out in the custody record.
- Review approval will not publish anything: publication type is staged, and G10's store row
  stays an owner-authorization boundary.
- G3's upload, submit, and custody-ordering rows are checked in RELEASE-CHECKLIST.md. Its last
  two rows (reach the G0 distribution state; install the reviewed adapter on a lane machine)
  stay open until the review completes and the environment lanes run.
