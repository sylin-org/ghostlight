# Chrome store submission -- 2026-09-30

Status: 1.3.12 is `PENDING_REVIEW` with `STAGED_PUBLISH`. The public Chrome Web Store channel
remains on adapter 1.3.8. Review approval cannot publish this revision automatically.

## Package custody

```text
version: 1.3.12
source_revision: 03a086004afb94a869b1c384df24f9eb24d60ce2
artifact: dist/ghostlight-extension-v1.3.12.zip
sha256: 9d5521b3f34562051ea6b07d9d5d0e07267ff6d7bacf274bd38234d43659a25d
packaging_runtime: PowerShell 7.6.5
item: lejccfmoeogmhemakeknjjdhkfkgncdl
publish_type: STAGED_PUBLISH
```

The package was reproduced independently before and after the signed-off source commit, with the
same hash. The source commit was pushed to `main`. The service and source package versions remain
in lockstep with adapter 1.3.12, and `compatibility.json` records that exact pair. The protocol-3
adapter cannot be promoted publicly while the public service remains on 1.3.10/protocol 2.

## Verification

- Formatting, strict workspace clippy, and all 563 Rust tests passed.
- All 294 extension tests, 10 npm launcher tests, and four MCPB tests passed.
- Changed JavaScript syntax, repository integrity, and offline public-surface checks passed.
- The real Chromium credential/passive-feedback journey and workbench surface passed.
- A fresh workspace build and process journey used `.target-adapter-181/debug` and passed.
- [CI for the source commit](https://github.com/sylin-org/ghostlight/actions/runs/36675504565)
  passed all 11 jobs, including dependency audit/license/source gates and Rust, process, and real
  browser journeys on Windows and Linux. Local cargo-audit and cargo-deny were unavailable.

These are source and component results. No installed service or user browser extension was
replaced during this submission preparation.

## Store sequence

1. `Plan` verified version 1.3.12, the exact SHA-256 above, and available API automation.
2. Initial `Status` showed public adapter 1.3.8 at 100 percent and approved adapter 1.3.11 `STAGED`.
3. Upload returned HTTP 400 because the older submission still held the item. A status read
   confirmed that 1.3.11 remained staged; a diagnostic retry returned the store's in-review edit
   prohibition.
4. After identifying the older version as the candidate superseded by this owner-requested fix,
   `Cancel` returned HTTP 200. Independent `Status` confirmed 1.3.11 `CANCELLED`; public 1.3.8
   remained unchanged.
5. `Upload -Execute` returned `SUCCEEDED` and draft version 1.3.12.
6. After verifying the updated public privacy policy, `Submit -PublishType STAGED_PUBLISH -Execute`
   returned `PENDING_REVIEW`.
7. Independent final `Status` confirmed submitted version 1.3.12 `PENDING_REVIEW`, with public
   version 1.3.8 unchanged at 100 percent.

Credentials came from the authorized ignored repository-local credential file. No credential
value is recorded here.

## Privacy publication and store disclosures

The source policy now distinguishes explicitly authorized 1.3.12 credential input from earlier
public versions. Website commit `7900564a27cbe981e5c31c15664041d94415d0af` triggered a rebuild of
the unchanged production tree to fetch that canonical policy. Cloudflare Pages reported success,
and an independent HTTP read of the [public policy](https://sylin.org/ghostlight/privacy/)
confirmed its September 30 date and credential-authorization disclosure. Unrelated local website
commits and untracked work were left untouched.

Website commit `5cd6958e72a36928dea776ab5ec762dbadd66994` refreshed the final generic-input wording
before submission. Cloudflare Pages again passed, and the independent public HTTP read confirmed
that Ghostlight processes client-supplied text for the requested edit.

The current public listing declares Web history, User activity, and Website content. Those
declarations remain unchanged. The updated policy describes client-supplied form text and the
request-level acknowledgement for credential-class controls. Additional collection categories
are not inferred solely from what an arbitrary input string could contain.

The submitted version and state were independently verified. This is a staged review submission;
public delivery still requires review approval and coordinated publication of the matching service.
