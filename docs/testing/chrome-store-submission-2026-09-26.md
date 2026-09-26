# Chrome store submission -- 2026-09-26

Status: PENDING_REVIEW with STAGED_PUBLISH. The public Chrome Web Store channel remains on adapter
1.3.8. Review approval cannot publish this revision automatically.

## Package custody

```text
version: 1.3.11
source_revision: fa29779b
artifact: dist/ghostlight-extension-v1.3.11.zip
sha256: 56ebf23d472afd876f01ff2765ee89bb03ef3b1f90057a680d4ab07ca5122714
item: lejccfmoeogmhemakeknjjdhkfkgncdl
```

The deterministic package was rebuilt after the source commit and reproduced the same hash that
the publisher printed before upload. The package carries adapter protocol 3 and is compatible only
with service 1.3.11, so public publication must wait for coordinated service delivery.

## Store sequence

1. `Plan` verified version 1.3.11, the exact SHA-256 above, and available API automation without
   making a store request.
2. Initial `Status` showed public adapter 1.3.8 at 100 percent and no submitted revision.
3. `Upload -Execute` returned `SUCCEEDED` with draft version 1.3.11.
4. `Submit -PublishType STAGED_PUBLISH -Execute` returned `PENDING_REVIEW`.
5. Final `Status` showed public adapter 1.3.8 unchanged and submitted adapter 1.3.11 in
   `PENDING_REVIEW`.

Credentials came from the ignored repository-local credential file by location only. No
credential value is recorded here.

## Firefox

No Firefox package was uploaded, signed, or submitted. ADR-0183 withdrew the partial adapter and
removed its active package and publisher paths. Historical Firefox evidence remains in the
repository and Git history.

## Authorization and next action

The owner explicitly authorized extension-store publication work in this task. Staged submission
uses that authorization without exposing protocol-2 users to a protocol-3 auto-update. A later
public Chrome publication still requires the matching 1.3.11 service to be publicly available and
an explicit publication step under the release procedure.
