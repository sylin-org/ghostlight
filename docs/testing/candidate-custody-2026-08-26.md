# 1.0 candidate custody -- 2026-08-26

Status: HELD. The G2 candidate is assembled, attested, downloaded, and verified from two
independent local copies. Its extension ZIP is byte-identical to the approved Chrome Web Store
revision, so the store review covers the candidate without a resubmission.

## Build

- Workflow run: [33020313866](https://github.com/sylin-org/ghostlight/actions/runs/33020313866)
  ("Build release candidate", dispatched on `dev`).
- Candidate source revision: `b2c27993a223c220f8828736b125676ae6f9d027`, pinned by
  `docs/release/freeze.json`.
- Supersedes the 2026-08-25 custody at `994b6c85`: the ADR-0140 relicensing changed product bytes
  (focused-typing fix `b26c3ecf`, governance license headers, workbench About view) and packaging
  payloads after that candidate was assembled. The superseded local copies remain archived under
  the machine-local `.target-g2-custody-superseded-994b6c85` directory.

## Journey to green (three defects found and fixed at their seams)

1. **0.8 artifact relationships drifted.** The relicensing pass and the 2026-08-25 restorations
   changed twelve tracked artifacts (PR template, oss-adoption and business records, trust MSA,
   DPA, and sub-processors pages, the deleted commercial license text and PRICING.md, restored
   icon generator and website publisher) without regenerating the disposition ledgers. Fix
   (`89bed6c6`): the ledgers were regenerated from the harvest against the recorded 0.8 revision;
   ordinary CI, red since the relicensing landed, is green again.
2. **A broken ADR index link.** The ADR-0137 index row still pointed at ADR-0106's pre-rename
   filename (`0106-command-line-session-identity.md` instead of `0106-caller-owned-sessions.md`).
   Fix (`ded44e2d`).
3. **The extension ZIP was not cross-OS deterministic.** The Linux CI build hashed differently
   from the approved store revision built on Windows, in three independent layers:
   - `ConvertTo-Json` separates lines with the platform newline, so the rewritten
     `manifest.json` carried CRLF on Windows and LF on Linux. Fix (`bd3bffe4`): the packager
     pins the serialization to CRLF, the exact form of the approved revision.
   - Even with identical entry content, the archives still differed by 99 bytes. The central
     directory's host-system marker and, on Unix, the file-mode attribute bits are written from
     the running platform by .NET (`0`/no attributes on Windows, `3`/`0o100644` on Linux; 33
     records, one host byte plus three attribute bytes each). Fix (`9ee05666`): the packager
     walks the central directory from the end-of-central-directory record and pins both fields
     to the Windows shape after archiving. On Windows the patch writes only values already
     present; the local rebuild stayed byte-identical to the approved revision.
   - Proof: the Linux CI build at this revision hashes to the approved store bytes exactly.

## Verification results

`scripts/verify-custody.ps1 -IncludeProvenance` against both local copies:

- freeze binding: PASS (manifest sourceRevision == `docs/release/freeze.json`);
- deep candidate checks: PASS (18 artifacts, exact roster and coordinates, every hash);
- SHA256SUMS recomputation: PASS (all 18 lines rehashed from bytes);
- GitHub provenance attestations: PASS for all six raw binaries.

Local custody: two verified copies under repo-local `.target-g2-custody/release-candidate` and
`.target-g2-custody-copy` (paths are machine-local working state; not secrets).

## Extension ZIP vs the store review

The candidate extension ZIP is `ghostlight-extension-v1.0.0.zip`, SHA-256
`3570494faf580a2286d9f7a5f1cbb6f657864ee369b0f70b944b0c927e64770c` -- byte-identical to the
revision Google approved on 2026-08-26 (dashboard state "Ready to publish"; API
`submittedItemRevisionStatus.state: STAGED`, version 1.0.0). No upload or resubmission is
needed; publication from the staged state is the remaining G10 store row.
