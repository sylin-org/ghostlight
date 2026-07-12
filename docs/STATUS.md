# STATUS -- where the project stands

Last updated: 2026-07-12. This file is a point-in-time snapshot maintained by whoever
finishes significant work. It exists so a fresh agent (or human) can orient without any
prior session context. **Trust the tree, `git log`, and the batch LEDGERs over this file
when they disagree**, and update it when you land something that changes the picture.

## Now

- **Branches**: `main` = releases, `dev` = trunk. Work lands on `dev`; the owner reviews
  `dev -> main` PRs and cuts releases.
- **Latest published release: v0.5.6** (2026-07-12), cut with `scripts/release.ps1 0.5.6`.
  Shipped: GitHub Release (27 assets + attestations), npm `ghostlight@0.5.6`, homebrew tap,
  scoop/winget/homebrew manifests committed to main, trust footers restamped, sylin.org
  website refreshed. `dev` and `main` are in sync at the release commit (`5762c3a`). v0.5.5
  was prepared but never published; its content folded into the 0.5.6 changelog.
- **v0.5.6 carries**: composable policy tiers + session overlay + `ghostlight demo` (ADR-0060),
  extension-owned browser identity (ADR-0061), browser-relay reconnect (ADR-0062), the
  deploy-quiesce lock (ADR-0063), explicit dev isolation then the one-stack model (ADR-0064
  amended by ADR-0065), the on-screen governance ribbon + `notify` tool, the field-splash FX
  pass, the SAPS security-hardening pass, and the full deploy-automation + store-publish tooling.
- **CWS publish is BLOCKED on a listing gate (owner action)**: the v0.5.6 package UPLOADED to
  the Chrome Web Store successfully (staged as a draft), but the publish API returned 400 --
  the listing needs, in the Developer Dashboard (Privacy practices tab): mandatory privacy
  information, a remote-code-use justification, and a promotional video. Content to paste lives
  in `docs/legal/PRIVACY.md`, `docs/legal/PERMISSION_JUSTIFICATIONS.md`, `docs/legal/STORE_LISTING.md`.
  After filling those, publish from the dashboard OR re-run `pwsh -File scripts/publish-extension.ps1`
  (the package is already uploaded; it will re-attempt publish). Edge was skipped (no `EDGE_*` creds).

## Release pipeline (canonical map: `docs/RELEASE.md`)

`scripts/release.ps1 <version>` from `main` automates: tag, watch CI, verify assets, fill
package-manager sums, homebrew tap, npm publish + smoke, trust-footer restamp, extension publish
(Chrome Web Store + Edge; auto when `CWS_*`/`EDGE_*` creds are set), and the website refresh. The
v0.5.6 run proved every step end to end (only the CWS listing gate above stopped the final publish).

CWS API creds are set up on this machine (see local/RELEASE-CREDENTIALS.md; values in
`~/.ghostlight-release.env`, written by `local/set-credentials.ps1`). Load them before a release:
`Get-Content "$HOME/.ghostlight-release.env" | % { if ($_ -match '^([A-Z0-9_]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1],$Matches[2]) } }`

Still manual per release: a winget PR to `microsoft/winget-pkgs` (CLA), and the MCP Registry
`mcp-publisher` step (DNS auth on the sylin.org apex).

## Owed engineering work (in rough priority order)

- **Lightbox legacy-27 migration** (ADR-0056): the 27 `#[ignore = "e2e"]` spawn tests +
  `scripts/test-e2e.*` migrate scenario-by-scenario into the lightbox harness against a
  per-test parity ledger. Not started; CI runs both tiers until the ledger completes.
- **SAPS remediation remainder** (assessment lives in gitignored `saps/`; findings already
  remediated are in git history around 2026-07-11):
  - SEC-HIGH-03 enforce-half: a confirm-gate for irreversible actions (send/delete/
    purchase) needing out-of-band human confirmation. Design captured in
    `docs/design/managed-mode-network-features.md` (managed intent descriptors); build
    pending.
  - SEC-HIGH-02 full fix: token/auth for non-loopback sources once `enable-remote` returns
    (the action is currently disabled as the interim fix). Same design note; build pending.
  - A1 demo GIF for the README hero slot (README has a commented placeholder): drive
    `gif_creator` or `scripts/capture-readme-tour.ps1`, write `docs/assets/demo.gif`.
- **tabs_create prose leaks the un-encoded native tab id** (found in the ADR-0061 live
  verify; pre-existing, non-regression). Small fix in the tabs_create response text.
- **ADR-0047 stage-2 user-supervised e2e re-run** still owed (needs the owner at a real
  browser).
- **FAQ Q17 follow-up**: no license-expiry scenario exists in lightbox; adding one would
  let the trust-center FAQ point at exactly what it claims.
- Parked (deliberately): audit TCP sink (UDP syslog is the standard; revisit only on ask);
  `socket.yml` capability acknowledgments for the npm package (draft-first, owner call).

## Owner-side gates (agents cannot do these)

- Cut the v0.5.6 release (owner: scripts/release.ps1 0.5.6 from main). PR #42 is merged.
- Chrome Web Store: 0.5.0 zip was submitted 2026-07-10; resubmit after 0.5.6 (extension
  changed). Edge Add-ons: same zip, never submitted.
- MCP Registry: needs DNS TXT auth on the sylin.org apex + `mcp-publisher`.
- Trust center legal: vendor entity name in the MSA (blocked on forming the LLC), the
  cyber-insurance yes/no line, counsel skim of MSA/DPA/LICENSE-GOVERNANCE before first
  EXECUTION (publication already happened by design; drafts are marked as drafts).
- `security.txt` on sylin.org (founder-side, ~1h).
- Key backup + a second npm publisher; one non-author human through the install flow.

## Standing context worth knowing

- The trust center (`docs/trust/`, 13 docs) is PUBLIC on `main` since 2026-07-11 (PR #27)
  at `v0.5.4+dev` footers. Its claims were red-teamed against the tree; keep code and
  claims in lockstep.
- managed:// central policy distribution (ADR-0055) is fully implemented through Phase 5.
- The dev workflow is the one-stack model (ADR-0065): no dev install, no `-dev` host;
  `scripts/dev-loop.ps1` swaps the engine, `-Restore` hands back (and refuses pre-v0.5.5
  releases, which are lock-unaware and fight the swap).
- Machine-local state (which engine runs on a given dev box, install quirks) belongs in
  `local/MACHINE-STATE.md` (gitignored), not here.

## How to update this file

Keep it a snapshot, not a journal: overwrite stale facts instead of appending history
(git history is the journal). Update the date at the top. If an item moves from owed to
done, delete it here and make sure the durable record (ADR, LEDGER, CHANGELOG) carries it.
