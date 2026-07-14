# STATUS -- where the project stands

Last updated: 2026-07-13. This file is a point-in-time snapshot maintained by whoever
finishes significant work. It exists so a fresh agent (or human) can orient without any
prior session context. **Trust the tree, `git log`, and the batch LEDGERs over this file
when they disagree**, and update it when you land something that changes the picture.

## Now

- **Branches**: `main` = releases, `dev` = trunk. Work lands on `dev`; the owner reviews
  `dev -> main` PRs and cuts releases.
- **Latest published release: v0.5.6** (2026-07-12), cut with `scripts/release.ps1 0.5.6`.
  Shipped and LIVE: GitHub Release (27 assets + attestations), npm `ghostlight@0.5.6`, homebrew
  tap, **MCP registry (`org.sylin/ghostlight`)**, scoop/winget/homebrew manifests committed to
  main, trust footers restamped, sylin.org website refreshed. Post-release PR #45 is merged on
  `main` at `d22db80`; `dev` is ahead with ADR-0071's installer-target batch. v0.5.5 was prepared
  but never published.
- **Unreleased installer work**: Codex is a first-class lossless-TOML target (ADR-0067), and
  Windsurf, Zed, OpenCode, and Crush now join Claude Code/Desktop, Cursor, and VS Code as explicit
  installer targets (ADR-0071). Strict JSON is merged idempotently. Commented JSONC is left intact
  and receives a copyable manual entry; `doctor` uses a tolerant registration check. The browser
  extension remains a separate user-visible install step.
- **MCP registry publishing is now automated** in `release.ps1` (the `registry` step, after `npm`):
  `mcp-publisher` DNS-auth publish, gated on `MCP_DNS_PRIVATE_KEY`. The one-time DNS proof is DONE
  (apex TXT `v=MCPv1; k=ed25519; p=...` on sylin.org via Cloudflare; ed25519 key in the env file;
  see `local/AUDIT-LOG.md`). The registry is immutable per version, so metadata fixes (like the
  websiteUrl) only land on the NEXT version.
- **v0.5.6 carries**: composable policy tiers + session overlay + `ghostlight demo` (ADR-0060),
  extension-owned browser identity (ADR-0061), browser-relay reconnect (ADR-0062), the
  deploy-quiesce lock (ADR-0063), explicit dev isolation then the one-stack model (ADR-0064
  amended by ADR-0065), the on-screen governance ribbon + `notify` tool, the field-splash FX
  pass, the SAPS security-hardening pass, and the full deploy-automation + store-publish tooling.
- **CWS publish is BLOCKED on listing completion (owner action)**: the v0.5.6 package is uploaded
  and staged as a draft. The Developer Dashboard still needs the Privacy practices answers,
  remote-code-use justification, and promotional video. Paste-ready, code-backed wording lives in
  `docs/legal/PRIVACY.md`, `docs/legal/PERMISSION_JUSTIFICATIONS.md`, and
  `docs/legal/STORE_LISTING.md`; its canonical public target is
  `https://sylin.org/ghostlight/privacy/`. After completing the dashboard, publish there or re-run
  `pwsh -File scripts/publish-extension.ps1` (the package is already uploaded). Edge was skipped
  because no `EDGE_*` credentials are configured.
- **CWS credential durability needs one owner-side change**: the Google OAuth consent screen is
  External/Testing, so its refresh token is short-lived. Move the consent configuration to
  Production or mint a fresh token before a later publish attempt. Credential locations remain in
  `local/`; no values belong in tracked documentation.

## Active work: reliable ephemeral GIF recording

- **The demo tour is deliberately paused.** The owner chose to solve recording architecture before
  returning to presentation work. The Capture Studio website block remains complete and live at
  `/ghostlight/demo/studio/`; no browser-side capability-tour implementation has started.
- **The failed export was a transport defect, not an encoder stall.** The preserved 12-frame
  fixture encoded in under one second. The seven-frame coordinate export exceeded Chrome's 1 MiB
  host-to-extension message limit, disconnected the native host, and then waited for the generic
  60-second timeout. Four ordinary frames were already enough to cross that boundary.
- **ADRs 0073 and 0074 are implemented on `dev`.** Recording is session/surface/
  generation-owned, memory-only, byte-bounded, transactionally started and finalized, protected by
  idle/hard deadlines plus an extension health lease, and erased on session/policy/panic/retention
  cleanup. GIF encoding is two-pass and one-frame-at-a-time. Large browser-bound tool requests use
  negotiated, SHA-256-verified, memory-only chunks; old extensions fail fast before an oversized
  write. Debug MCP/tool payload persistence has been removed.
- **The model flow is smaller.** Use `start_recording`, ordinary browser tools, then `export`.
  Export auto-finalizes. `status`, explicit stop, and clear are supporting actions. Download export
  requires Read; page placement by ref or coordinate requires Write. A timeout or disconnect after
  enqueue reports `outcome_unknown` and `retry_safe: false` instead of inviting a duplicate page
  effect. Formatting, strict clippy, all 72 extension tests, and the full Rust workspace suite are
  green. The rebuilt service and reloaded unpacked extension passed a real MCP browser verification:
  20 accepted frames (2,707,795 compressed bytes) encoded to a 7,046,417-byte GIF, crossed the
  bounded chunk transport, and returned `dispatched` with `unverified` acceptance and
  `retry_safe: false`. The test recording was cleared and its synthetic page overlay removed.

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

- **Public documentation was rebalanced around responsible delight**: the applied review lives in
  `docs/design/public-documentation-review-2026-07.md`. The README now leads with the real-session
  problem, fit and anti-fit, visible experience, one install journey, and candid platform state.
  It also corrects stale topology, audit-default, roadmap, and install-time-vs-runtime claims.
  Remaining high-value work: CWS publication, the hero GIF, and macOS/Linux live verification.
- **WebMCP participation can begin without product support**: research 15 records the current
  governance gaps, a bounded non-shipping origin-trial experiment, and a draft response for the
  WebMCP explainer. Owner actions: approve the outbound text, join Chrome's early preview program,
  and choose a controlled experiment origin. ADR-0043's no-implementation stance remains intact.
- **Agent journey evaluation artifacts are proposed** (ADR-0069): local, minimized evidence for
  comparing models and clients across a browser journey. Acceptance requires concrete journeys, a
  data inventory and threat review, a versioned artifact schema, lightbox production, and evidence
  from at least two client or model configurations.
- **Bounded delegation needs scenario validation before an ADR**: the release-candidate triage
  journey in `docs/design/bounded-delegation-scenario.md` exercises the ADR-0060 session overlay and
  identifies the unresolved approval, expiry, budget, intent, and digest questions.
- **Bidirectional installation handoff is implemented on `dev`** (ADR-0070): an explicit first
  `ghostlight install` opens the stable extension walkthrough once; `--no-open`, dry-run,
  CI, failed, and idempotent paths stay quiet. The canonical service-first page is live at
  `sylin.org/ghostlight/service/post-install/`; the website publication gate is complete.
- **Scoped MCP cancellation is proposed and deferred** (ADR-0068): first verify that supported
  clients emit `notifications/cancelled`. If demand exists, stop `script`/`browser_batch` only
  between steps, let the active step settle, preserve audit, and never claim rollback.
- **Content / URL consistency pass (owner-driven, mostly DONE)**: swept outward-facing content
  for stale/branded URLs and moved the post-install UX onto the site. What landed:
  - **github.io fully retired.** The canonical home is `sylin.org/ghostlight`. Every reference to
    `sylin-org.github.io/ghostlight` was repointed (extension onInstalled, homebrew/scoop/winget/npm
    homepage + walkthrough URLs, `scripts/get.sh`/`get.ps1`, npm launcher fallback). `site/index.html`
    and `site/install.html` became meta-refresh redirect stubs to sylin.org (index -> project page,
    install -> post-install page). Committed on `dev` (b55102e). The Pages deploy is path-scoped to
    `site/**` on `main`, so the redirect stubs go live at the next dev->main merge.
  - **Post-install page is LIVE**: `sylin.org/ghostlight/chromium-extension/post-install/`
    (website repo `src/ghostlight/chromium-extension/post-install.njk`, teal accent, base.njk layout).
    `extension/service-worker.js:374` now opens it. Website pushed to `main` (auto-deployed).
  - `server.json` websiteUrl was already FIXED to `https://sylin.org/ghostlight/` (applies on the
    next registry version, not 0.5.6 -- immutable).
  - README now lists the LIVE distribution channels (MCP registry + Homebrew badges, an "Other ways
    to get it" line). CWS (blocked), Edge, winget, and scoop are omitted until each actually ships.
  This workstream is now COMPLETE; the only distribution follow-up left is the owner-side CWS listing
  gate below.
- **CWS listing completion** (owner): confirm the public privacy URL is live, paste the privacy
  practices and remote-code justification, record and upload the promotional video using the
  proven `ghostlight demo` recipe in `docs/legal/STORE_LISTING.md`, then publish the already-uploaded
  package. The currently staged v0.5.6 package predates `narrate`; do not pair a narration video
  with that package. Cut and upload the next release first (or re-run the extension publish step
  after that release), then record. The earlier live rehearsal passed on 2026-07-13, including the
  session-policy denial finale.
- **Agent narration is implemented** (ADR-0072): additive `narrate` is domainless RAWX none,
  bounded and schema-validated, ordinarily audited, ownership/hold/sacred checked, and legal in
  `script`/`browser_batch`. The policy-free extension renders one timed responsive Agent ribbon per
  tab with deterministic replacement, remaining-time navigation replay, effects/capture handling,
  and tab/session/panic cleanup. Placement is `auto`/`top`/`bottom`; auto chooses one stable edge
  away from recent touched-control, pointer, and scroll activity. The separate central governance
  ribbon now has viewport-bounded sizing and wrapped, untruncated security text. `ghostlight demo`
  narrates its six story beats after each stage loads, holds each caption for its full six-second
  lifetime, and only then begins the visible actions. Rust and the 67-test extension suite are
  green. Live browser
  verification passed on 2026-07-13 through the real MCP `script`
  path: `shown: true`, timed placement, replacement, active-navigation replay, and audit
  `capability: "none"` with no grant attribution. After the responsive refinement reload, a
  top-area hover resolved `auto` to bottom and a bottom-area hover resolved it to top; both calls
  returned the effective edge and the user-visible wide ribbon. Existing MCP clients need one
  restart to add the new direct `narrate` schema to their callable tool list.
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
  - A1 demo GIF for the README hero slot (README has a commented placeholder): export it from the
    same `ghostlight demo` OBS recording used for the Store video, then write `docs/assets/demo.gif`.
- **tabs_create prose leaks the un-encoded native tab id** (found in the ADR-0061 live
  verify; pre-existing, non-regression). Small fix in the tabs_create response text.
- **ADR-0047 stage-2 user-supervised e2e re-run** still owed (needs the owner at a real
  browser).
- **FAQ Q17 follow-up**: no license-expiry scenario exists in lightbox; adding one would
  let the trust-center FAQ point at exactly what it claims.
- Parked (deliberately): audit TCP sink (UDP syslog is the standard; revisit only on ask);
  `socket.yml` capability acknowledgments for the npm package (draft-first, owner call).

## Owner-side gates (agents cannot do these)

- Chrome Web Store: complete the v0.5.6 draft listing, make OAuth credentials durable, and publish.
  Edge Add-ons remains unsubmitted.
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
