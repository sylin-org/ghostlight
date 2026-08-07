# Ghostlight 0.8 publication packet

Prepared: 2026-08-05 15:14 -04:00

Status: owner review required. This packet does not authorize any action below.

## Exact source state

| Surface | Commit | State |
| --- | --- | --- |
| Ghostlight 0.8 product and public-copy baseline | `57b4d24274942c03c4c6f8d7465ae7d2b31a7dd1` | Local `dev`; all candidate code, manifests, copy, and E1-E5 work are present |
| Public Ghostlight repository `main` | `6de59b9920320a2e4b7f52e049c14c448cdaca56` | Does not contain the 0.8 candidate |
| Website `main` | `1568538ba5ca217e46b917688b41d17b7e672488` | Live at `https://sylin.org/ghostlight/` |
| Website work branch | `1568538ba5ca217e46b917688b41d17b7e672488` | Matches website `main` |

The E6 evidence and packet commit is documentation-only. Record its final hash alongside the
baseline above before authorizing a product push or merge. After the adapter reconciliation and
public-service status edit described below, the resulting reviewed `main` commit becomes the exact
release commit. Do not tag an earlier commit.

## Current public truth

Observed again on 2026-08-05 immediately before this packet:

| Fact | Current state |
| --- | --- |
| Public service | 0.7.3 on npm, GitHub, and the official MCP Registry |
| Source service | 0.8.0, unreleased |
| Public Chrome adapter | 0.7.1 from the public Chrome update feed |
| Pending Chrome adapter | 0.8.0; an owner-provided dashboard screenshot on 2026-08-05 showed `Pending review` |
| Compatibility | Adapter 0.7.1 covers services 0.7.1 through 0.7.3; adapter 0.8.x covers services 0.8.x |
| Platforms | Windows and Linux live-browser verified; macOS build and full-suite CI verified, live-browser proof owed |
| MCP revisions | Source candidate has exact local stdio shores for `2025-11-25` and `2026-07-28` |
| Install command | `npx -y ghostlight install` |
| Canonical URL | `https://sylin.org/ghostlight/` |

`scripts/check-public-surfaces.ps1 -Online` passes this state. The live Ghostlight, install,
privacy, brief, foundry, Agyo, and Zen Garden routes return HTTP 200. The generated website assets
carry commit marker `1568538ba5ca`. Glama and mcpservers.org still ingest the former relay copy,
and search results still cache older Sylin text. Those are downstream cache facts, not permission
to change canonical claims.

## Test and live-proof evidence

### Candidate gates already passed

The 0.8 candidate record in `docs/STATUS.md` includes formatting, strict Clippy, the full workspace
suite, all 31 Lightbox scenarios, 164 extension tests, seven npm launcher tests, five MCPB launcher
tests, four e2e baselines, npm package dry-run, RustSec, cargo-deny, the optimized Windows
three-binary build, and the pinned official MCP publisher validation. E3-E5 repeated the relevant
workspace, package, public-copy, schema, website, link, mobile, and metadata gates after their
changes.

E6 reruns the complete repository gate before its local commit. The release dry run is deliberately
not green yet: from the current `dev` checkout it stops at the script's `main`-only guard. After the
owner-approved adapter and status reconciliation lands on `main`, rerun the dry run from a clean
`main == origin/main` checkout.

### Live Ghostlight recipes

| Recipe | Result | Evidence boundary |
| --- | --- | --- |
| Safe launch brief | PASS in this Codex client and visible Chromium. Filled the five authorized fields and submitted only the public synthetic form. The page reported `Moonlight Notes is ready for review.` | Tab `5541182605`; `https://sylin.org/ghostlight/demo/brief/` |
| Authenticated read | PASS. The owner selected the signed-in Microsoft Partner Center home. `get_page_text` returned its workspace state and `read_page` returned its navigation and controls. The earlier Chrome Web Store Dashboard attempt separately proved signed-in continuity and exact child adoption, but Chrome correctly blocked extension inspection there. | The successful proof was read-only. No private contact value was retained in project evidence. The Chrome dashboard screenshot separately showed publisher `sylin.org`, `Ghostlight in Browser` 0.8.0, two users, no rating, and `Pending review`. |
| Browser-created child | PASS in this Codex client and visible Chromium. A measured click on the temporary `Open child proof` link returned exactly one child in `tabDelta`; the next direct call read child tab `5541182622` at `https://example.org/` without a context refresh. The source tab remained at `https://example.com/`. | Temporary DOM change on the disposable source tab only; both public example tabs remain open. |
| Foundry diagnosis | PASS. Tracking began before one explicit reload. The page read showed the synthetic QA blocker, no console message matched `error|warning|foundry`, and all 11 captured `sylin.org` requests returned 200. | Read-only public synthetic page; no page mutation. |

Two recoveries are worth retaining. The first fresh-tab request hit Chromium's transient
`tabs cannot be edited` guard and changed nothing; the existing owned blank tab completed the safe
proof. In the child proof, semantic resolution initially omitted the new link, so the agent used
the tool's exact coordinate escape after measuring the visible link. The action receipt then
identified the link and exact child. Neither failure repeated three times.

## Owner decisions

Approval must name each allowed item. One approval does not cover the others.

- [x] Select and authorize one scriptable signed-in page for the full read-only recipe above. The
  owner-selected Microsoft Partner Center home passed without retaining private contact values.
- [x] Recheck the Chrome owner dashboard. The 2026-08-05 owner screenshot records adapter 0.8.0 as
  `Pending review`.
- [ ] If approved, publish the already-submitted adapter 0.8.0 with deferred publication. Do not
  edit its listing copy in the same action.
- [ ] Authorize the Ghostlight `dev` push, review path, and `dev -> main` merge.
- [ ] Authorize the v0.8.0 tag and release orchestration.
- [ ] Authorize each later directory, issue, pull request, showcase, Discussion, or social edit by
  destination.

## Store-review implications

- Adapter 0.8.0 is already submitted. The service release must invoke
  `scripts/release.ps1 0.8.0 -SkipExtension`; the ordinary extension step sees changed extension
  source and could upload or submit again.
- Do not change Chrome listing copy while 0.8.0 is under review. A listing edit can add review time
  or create uncertainty about which change Google reviewed.
- Once the approved adapter is intentionally published, poll the public update feed and run
  `scripts/reconcile-chrome-store.ps1 -ExpectedVersion 0.8.0`. Review and commit its status and
  README changes before preparing the service release.
- Adapter 0.8.x covers service 0.8.x, while public adapter 0.7.1 stops at service 0.7.3. Publish the
  service promptly after the 0.8 adapter becomes public and the release commit is ready. Do not
  describe the short channel transition as simultaneous.
- Edge Add-ons is not configured or submitted. Individual enrollment makes the contact address
  customer-visible, so the owner deferred the native listing rather than publish a home address.
  Microsoft supports installing Chrome Web Store extensions in Edge through its other-store path;
  that is Ghostlight's Edge end-user route after Chrome approval. Revisit a native listing only
  after a legitimate non-home public address exists.

## Ordered publication runbook

Each numbered mutation needs explicit owner confirmation.

1. The owner-selected authenticated read is complete. Recheck the Chrome dashboard after its state
   changes. It currently says `Pending review`, so stop here and keep service 0.8.0 unreleased.
2. Publish the already-approved Chrome adapter 0.8.0 through deferred publishing. Make no listing
   copy change.
3. Wait for the public update feed to report 0.8.0. Run:

   ```powershell
   pwsh -File scripts/reconcile-chrome-store.ps1 -ExpectedVersion 0.8.0
   ```

4. Set the canonical public service release and matching README claim to 0.8.0 immediately before
   release. Run the local public-surface check and the complete repository gates. Commit the
   adapter and service-status reconciliation on `dev`.
5. Push `dev`, open or update the owner-review PR, merge `dev -> main`, and record the exact merge
   commit. Confirm a clean `main` checkout equals `origin/main`.
6. Preview the exact orchestration without mutation:

   ```powershell
   pwsh -File scripts/release.ps1 0.8.0 -DryRun -SkipExtension
   ```

7. With a separate release confirmation, run:

   ```powershell
   pwsh -File scripts/release.ps1 0.8.0 -SkipExtension
   ```

   The script creates and pushes the tag, watches the release workflow, verifies assets, fills and
   commits package checksums, updates Homebrew, publishes npm, publishes the official MCP Registry
   record when its DNS credential is present, restamps Trust Center footers, refreshes website
   fallbacks, and reports remaining manual work.
8. Verify GitHub release assets, npm launcher smoke, official MCP Registry, Homebrew, public Chrome
   version, website version and adapter summary, install page, privacy page, and decision aid. Run:

   ```powershell
   pwsh -File scripts/check-public-surfaces.ps1 -Online
   ```

9. Open the separate Winget 0.8.0 PR. Decide separately whether to publish the prepared store,
   Glama, mcpservers.org, Cline, awesome-mcp-servers, GitHub catalog, mcp.so, PulseMCP, Codex, Zed,
   or Claude copy from `PUBLIC-COPY-DRAFTS-0.8.md`.
10. Record the release reception snapshot. Recheck cached directory and search surfaces after
    48-72 hours, then fill the 7-day and 30-day snapshots.

## Recovery by channel

Do not delete or reuse a release tag after it is public.

| Channel | Recovery |
| --- | --- |
| Chrome adapter | Store versions do not roll back to an older version number. If 0.8.0 is bad, prepare a corrected 0.8.1 adapter or a higher-version code rollback, submit it, and state the temporary compatibility limit. |
| GitHub release | Keep v0.8.0 immutable. Mark a broken release clearly if needed, preserve evidence, and publish a corrected patch release. |
| npm | Do not overwrite 0.8.0. Deprecate it with a direct reason when needed, then publish the fixed patch after its assets exist. |
| Official MCP Registry | Preserve the version record. Publish the fixed patch and change active status only through the registry's supported process. |
| Homebrew, Winget, and Scoop | Revert or supersede manifests with a normal reviewed commit or PR. Point to an existing immutable release only. |
| Website | Revert the exact website refresh commit or publish a small correction. Keep `docs/public-status.json`, README, and website fallbacks aligned. |
| Directories and showcases | Edit or withdraw only the affected destination. Record that project-authored distribution changed; do not present it as user reception. |

If a failure can expose data, misroute browser work, violate the trained tool identity, or break
the store/service compatibility gate, stop the broader publication sequence. Prefer a patch and a
clear current-status note over attempting to hide a broken immutable version.

## Remaining caveats

- macOS still lacks live-browser verification.
- The authenticated proof passed on Microsoft Partner Center. Chrome Web Store pages remain
  protected from extension inspection, so their owner-only state still needs dashboard evidence.
- Edge Add-ons is deferred because its individual enrollment makes the contact address
  customer-visible. No enrollment or store submission completed.
- The owner dashboard was rechecked by screenshot on 2026-08-05 and still says `Pending review`.
- Glama and mcpservers.org are stale; search caches still show older Sylin content.
- GitHub catalog approval is owner-recorded, but a public catalog entry was not located.
- No independent review, user-authored public workflow, or permitted user quote was located in the
  bounded evidence pass.
- Chrome Web Store pages block extension scripting. The public version check therefore uses the
  update feed, and public user/rating evidence comes from the listing HTML.
- The website E5 install reported one high-severity dependency advisory. E5 did not authorize a
  dependency or lockfile change.

## Reception and discussion

Use `../research/public-reception-loop-0.8.md` for the release, 7-day, and 30-day records. The
Discussion draft lives there too. Do not add telemetry, tracking parameters, automatic review
prompts, or a vendor reporting service to fill evidence gaps.
