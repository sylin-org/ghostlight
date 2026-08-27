# CONTINUATION: Ghostlight 1.0 published -- 1.0-plus batch, D2 next

Read AGENTS.md first, then docs/STATUS.md (the "1.0 is published" section), then
docs/tasks/1.0-plus/LEDGER.md (the progress authority), then this prompt. You are continuing in
F:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp, branch `dev`.

## Where things stand (verified 2026-08-26, tree clean)

**1.0 IS PUBLISHED on every channel.** Do not re-plan publication; it happened. GitHub release
`v1.0.0` (tag at candidate `b2c27993a223c220f8828736b125676ae6f9d027`, run 33020313866), npm
`ghostlight@1.0.0`, Chrome Web Store adapter 1.0.0 (approved review `3570494f...` published
DEFAULT_PUBLISH; live listing + CRX feed observed serving 1.0.0), MCP Registry
`org.sylin/ghostlight` 1.0.0, `main` fast-forwarded. `check-public-surfaces.ps1 -Online` reports
all surfaces in agreement. Evidence: docs/testing/candidate-custody-2026-08-26.md, STATUS.md.

**main sits at `28a5892e`; dev is ahead (docs/tasks/1.0-plus opening, D1, install-guide fix).**
Post-publication, work happens on `dev`; promoting `main` is a deliberate owner decision, not a
sync habit.

**The 1.0-plus batch is open** (docs/tasks/1.0-plus/): D1 complete (`f8bff79a`, presentation
stylesheet moved to `extension/lib/presentation-css.js`, byte-equivalence proven, 153/153
extension tests green). The debt ladder runs simplest-to-most-complex; evidence lanes and
owner-action externals follow. Read its BOOTSTRAP.md before doing anything -- one task = one
commit, green tree always, published 1.0.0 is immutable, recovery ships as a higher version.

## Your next task: D2 -- GIF palette quality

Authoritative task file: docs/tasks/1.0-plus/D2-gif-palette-quality.md. Summary of the shape:

- `extension/lib/recording.js` + pinned `extension/vendor/gifenc.js` encode recordings in the
  offscreen document; today every frame gets its own local palette with no dithering, so static
  photographic content shifts color frame to frame.
- Deliver: (1) a shared palette derived from a bounded sample of the replay's frames, falling
  back to per-frame when a frame exceeds a chosen error bound; (2) optional dithering bounded by
  the existing byte budget; (3) tests with COMPUTED ORACLES -- pin expected palette counts,
  per-frame deltas, byte-budget adherence, and the unchanged thinning invariant (a thinned
  replay still plays for as long as the work took) from fixtures you compute first. The
  executor transcribes oracles; it never derives them.
- Ownership contract is untouchable: frames never cross a process boundary (ADR-0109), thinning
  lives only in recording.js, save budget 16 MiB. If a change would move either, STOP and write
  an ADR first.
- Prove live afterwards: one real recording on the dev authority, save, inspect; the unpacked
  extension needs a manual reload in chrome://extensions after any extension JS change (owner
  action).

After D2: D3 needs an ADR first (mapping old ADR-0084 attention routing onto the
plural-browser/ADR-0126 world; several rows may already be satisfied by current means -- record
that instead of building a second mechanism). Then evidence lanes E1-E5, then owner-action
externals X1-X3 (parked; never act without the owner naming the action).

## Standing owner decisions (do not re-litigate)

- Publication is DONE and owner-directed; the GO decision row in docs/RELEASE-CHECKLIST.md
  records that G4-G8 stayed open by the owner's call. Evidence lanes still close honestly.
- Store mutations, npm publishes, website deploys, anything public: explicit owner authorization
  at the moment of the action. scripts/publish-extension.ps1 now has Plan/Upload/Submit/Cancel/
  Status/Publish actions (Publish = publish the approved staged revision publicly; it refuses a
  staged revision whose crxVersion does not match the ZIP manifest).
- Never: phone home (ADR-0028), copy from reference/, read /private/, saps/, or local/ contents,
  weaken trust-doc claims, discard docs/ADRs/history, weaken an over-claim guard.
- Windows releases have no Authenticode signing by design (trust model: checksums + GitHub
  provenance). SignPath application pending; on acceptance its follow-ups unblock ADR-0105 D3.

## Gotchas learned 2026-08-26 (do not rediscover)

- **Cross-OS packaging determinism**: `ConvertTo-Json` writes platform newlines into rewritten
  JSON, and .NET's ZipArchive stamps the central directory's host-system marker + Unix mode bits
  from the running OS. scripts/package-extension.ps1 pins both to the approved store shape; any
  packager change must keep `3570494f...` reproducible from Linux CI. Lesson in docs/MEMORY.md.
- The extension ZIP hash to preserve: `3570494faf580a2286d9f7a5f1cbb6f657864ee369b0f70b944b0c927e64770c`.
  Future extension-source changes make this the "one commit older" revision -- normal forward
  flow, a NEW version ships next, never a mutation of published 1.0.0.
- A CWS submission in review LOCKS the item; replacing it is `:cancelSubmission` then upload+submit
  (script-gated; owner-authorized pattern used three times).
- The 0.8 artifact-harvest layer was retired on 2026-08-27 (ADR-0143); no ledger regeneration
  exists anymore.
- check-repository-integrity.ps1 validates every local doc link; an ADR rename must update every
  index row that names it.
- The website (sibling repo sylin-org/website) fetches llms-install.md live from ghostlight main
  at build time via a CDN-cached raw URL: push browser-mcp main FIRST, wait ~90s, then rebuild.
  Its check-site.js pins the current story (orchestrator chain, five proof recipes).
- Custody copies live in repo-local .target-g2-custody{,-copy} (gitignored working state); the
  superseded 994b6c85 candidate is archived at .target-g2-custody-superseded-994b6c85.
- Scoop/WinGet metadata for 1.0.0 is prepared under .target-pkg-metadata; the external bucket
  submissions are owner actions.

## Verification commands

git status --short                                 # must be empty
git log --oneline -8                               # e440c0c3 or later
npm test --prefix extension                        # 153+ tests
node --check extension/lib/presentation-css.js     # new module from D1
pwsh -NoProfile -File scripts/check-public-surfaces.ps1 -Online   # all surfaces agree at 1.0.0
pwsh -NoProfile -File scripts/verify-custody.ps1 -IncludeProvenance -CandidateDirectory .target-g2-custody/release-candidate
