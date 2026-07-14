# STATUS -- where the project stands

Last updated: 2026-07-14. This file is a point-in-time snapshot maintained by whoever
finishes significant work. It exists so a fresh agent (or human) can orient without any
prior session context. **Trust the tree, `git log`, and the batch LEDGERs over this file
when they disagree**, and update it when you land something that changes the picture.

## Now

- **Branches**: `main` = releases, `dev` = trunk. Work lands on `dev`; the owner reviews
  `dev -> main` PRs and cuts releases.
- **Latest published release: v0.5.7** (2026-07-13), cut with `scripts/release.ps1 0.5.7`.
  Shipped and LIVE: GitHub Release (27 assets including the CycloneDX SBOM + attestations), npm
  `ghostlight@0.5.7`, homebrew tap, **MCP registry (`org.sylin/ghostlight`, 0.5.7 is latest)**,
  scoop/winget/homebrew manifests committed to main, trust footers restamped, and the sylin.org
  install-guide fallback refreshed. Release PR #48 merged at `96d1e02`; checksum fill is
  `49c4c5a` and the trust restamp is `4ddb5af`. v0.5.5 was prepared but never published.
- **v0.5.7 includes the expanded installer matrix**: Codex is a first-class lossless-TOML target
  (ADR-0067), and Windsurf, Zed, OpenCode, and Crush join Claude Code/Desktop, Cursor, and VS Code
  as explicit installer targets (ADR-0071). Strict JSON is merged idempotently. Commented JSONC is
  left intact and receives a copyable manual entry; `doctor` uses a tolerant registration check.
  The browser extension remains a separate user-visible install step.
- **MCP registry publishing is now automated** in `release.ps1` (the `registry` step, after `npm`):
  `mcp-publisher` DNS-auth publish, gated on `MCP_DNS_PRIVATE_KEY`. The one-time DNS proof is DONE
  (apex TXT `v=MCPv1; k=ed25519; p=...` on sylin.org via Cloudflare; ed25519 key in the env file;
  see `local/AUDIT-LOG.md`). The registry is immutable per version, so metadata fixes (like the
  websiteUrl) only land on the NEXT version.
- **v0.5.7 carries**: all v0.5.6 features plus expanded installer targets, bidirectional install
  handoff (ADR-0070), agent narration (ADR-0072), reliable memory-only GIF recording and bounded
  browser transport (ADRs 0073/0074), the cohesive Card Foundry demo story, and the live Foundry
  companion route at `https://sylin.org/ghostlight/demo/foundry/`.
- **The Chrome Web Store listing is submitted and pending compliance review.** On 2026-07-13 the
  owner completed the listing, Privacy practices, permission and remote-code justifications,
  data-use certifications, screenshots, video, and promotional tiles, then submitted v0.5.7 for
  review. Chrome warned that broad host permissions may trigger an in-depth review; that is the
  intentional tradeoff for general-purpose automation across user-selected sites, not a rejected
  submission. No action remains unless the reviewer asks for clarification. Edge was skipped
  because no `EDGE_*` credentials are configured.
- **CWS credential durability needs one owner-side change**: the Google OAuth consent screen is
  External/Testing, so its refresh token is short-lived. Move the consent configuration to
  Production or mint a fresh token before a later publish attempt. Credential locations remain in
  `local/`; no values belong in tracked documentation.
- **The ADR-0056 Lightbox consolidation is complete.** All 27 legacy ignored spawn tests have named
  parity scenarios, the originals and dual shell wrappers are retired, and CI runs the 34-scenario
  Lightbox suite as the sole service-side process-boundary gate. The repaired Playwright job stays
  as the separate real-extension/Chromium proof under ADR-0056 Decision 4.
- **Browser control is local-only (ADR-0077).** The `inbound.web` HTTP/WebSocket transport and all
  of its policy, configuration, remote-enable, and test scaffolding are removed. MCP clients enter
  through the same-user OS pipe. The Console is a separate read-only loopback HTTP listener and
  rejects WebSocket upgrades. ADR-0076 is superseded; any future remote design starts from zero.
- **The closed-loop browser core is implemented (ADR-0078).** The additive 25-tool surface now
  includes semantic `act_on`, explicit JavaScript dialog control, and exact owned-tab
  focus/reload/close. Actionable observations, bounded interaction receipts, service-authored
  untrusted-output provenance, and final response budgets reduce model roundtrips without moving
  policy or page content into the extension. The 13 trained schemas remain byte-stable. All fast
  gates and all 34 Lightbox scenarios pass. Visible-browser verification remains pending on the
  Linux lifecycle host.
- **The Linux lifecycle test recipe is ready.** `docs/testing/linux-live-lifecycle.md` pins Ubuntu
  Desktop 24.04 LTS, visible Chrome Stable, VS Code first and Codex second, one ordinary OS user,
  and clean install through uninstall evidence. The owner is preparing the host and SSH access.
- **Release publication now has a narrow privileged boundary.** A read-only assembly job generates
  the pinned SBOM, packages the extension, creates `SHA256SUMS`, and uploads one immutable bundle.
  The privileged job only downloads, verifies the exact file set and hashes, attests, and releases.
- **The public vulnerability-disclosure endpoint is live.** `https://sylin.org/.well-known/security.txt`
  publishes the contact, expiry, canonical URL, and Ghostlight security-policy link.
- **The four-phase public documentation freshness pass is complete in the working tree.** Trust
  material now follows SECURITY.md's best-effort solo-maintainer targets and names only live
  distribution channels. Present-facing guides use the v0.5.7 service/relay topology, 25-tool
  inventory, one-stack dev loop, shipped licensing behavior, and managed-tab boundary. The original
  SPEC is explicitly historical, recording privacy names the memory-only retention rules, and the
  sylin.org source carries a v0.5.7 fallback plus a product-first narrow hero. Ghostlight formatting,
  local-link and ASCII checks, the website clean build, all generated-site checks, and the rendered
  390px overflow/navigation/order checks are green.
- **The July non-author experience closure is implemented on `dev` (ADR-0079).** An isolated
  denial is now a centered three-second sticker. Repeated enforced denials pause only the producing
  MCP session at a synchronized service send boundary (3 matching/60 seconds or 5 total/120
  seconds), then show a closed-shadow overlay and popup controls. Compact narration drops the
  progress meter; screenshot and recording feedback are quieter and tied to real capture state.
  Attention transitions are content-free audit records. The README and install guide now expose
  the four-stage practitioner journey, no-account/free-core facts, pre-release extension path, and
  a read-only first proof. The full Rust suite, strict clippy, 93 extension tests, JS syntax checks,
  and formatting are green. Visible Linux/browser verification remains owed.
- **The agent-browser overlap map is current through v0.31.2 (2026-07-13).** Research 17 contains
  the requested one-to-one table. The recommendation is deliberate non-parity: retain the local
  live-user-context boundary, compose with testing runtimes for specialist breadth, and measure two
  small free-surface candidates next -- ref-linked annotated screenshots and optional owned-tab
  labels.

## Released in v0.5.7: reliable ephemeral GIF recording

- **The cohesive Card Foundry tour is released in v0.5.7.** It replaces the old
  capability checklist with one simulated foil-card QA story: inspect and rotate the proof, mark
  defects, request Revision B, attach screenshot evidence, fill the release packet, prove a real
  off-domain policy denial, export the GIF into the page, and clear captured bytes. The companion
  site route is `/ghostlight/demo/foundry/`; its design and acceptance contract live in
  `docs/design/tcg-foundry-demo.md`.
- **The failed export was a transport defect, not an encoder stall.** The preserved 12-frame
  fixture encoded in under one second. The seven-frame coordinate export exceeded Chrome's 1 MiB
  host-to-extension message limit, disconnected the native host, and then waited for the generic
  60-second timeout. Four ordinary frames were already enough to cross that boundary.
- **ADRs 0073 and 0074 are released in v0.5.7.** Recording is session/surface/
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
- **The new story has passed compressed and normal-paced live local rehearsals.** The final normal
  run captured 100 frames, delivered a 21,466,581-byte GIF through the bounded chunk transport,
  observed `Replay ready` in the page, cleared the recording, and proved that the session overlay
  denied `example.com`. Its enclosing build-and-run command took 113.3 seconds, including a
  3.98-second build and pre-recording setup, so capture remained inside the 120-second hard lease.
  The runner inventories controls once per stable page phase: two meaningful read scans replace
  the prior scan before each click, type, and screenshot. Screenshot and drag geometry follow the
  live viewport using the extension's canonical coordinate constants, so an accidental resize does
  not invalidate the run. Formatting, strict clippy, website checks, responsive checks, and the
  full fast-tier Rust workspace suite are green.

## Release pipeline (canonical map: `docs/RELEASE.md`)

`scripts/release.ps1 <version>` from `main` automates: tag, watch CI, verify assets, fill
package-manager sums, homebrew tap, npm publish + smoke, trust-footer restamp, extension publish
(Chrome Web Store + Edge; auto when `CWS_*`/`EDGE_*` creds are set), and the website refresh. The
v0.5.7 run proved every automated step end to end. The owner later completed the dashboard-only
CWS metadata and submitted the item manually; Edge remains unconfigured.

CWS API creds are set up on this machine (see local/RELEASE-CREDENTIALS.md; values in
`~/.ghostlight-release.env`, written by `local/set-credentials.ps1`). Load them before a release:
`Get-Content "$HOME/.ghostlight-release.env" | % { if ($_ -match '^([A-Z0-9_]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1],$Matches[2]) } }`

Still manual per release: a winget PR to `microsoft/winget-pkgs` (CLA). The repository now provides
`scripts/prepare-winget.ps1`, which materializes the correct submission tree from release manifests
and runs `winget validate`; v0.5.7 passed that preparation locally. Store submission remains manual
when its API credentials or dashboard metadata are absent.

## Owed engineering work (in rough priority order)

- **The first retrospective non-author review is captured and its repository-actionable response
  is implemented.** The owner
  reconstructed a pre-release developer review from a video call with no transcript or notes;
  `docs/design/non-author-experience-review-2026-07.md` preserves the method limits, install and
  messaging friction, and the strong post-install delight signals. The proposed response is split
  into `docs/design/visual-language-next-2026-07.md`,
  `docs/design/developer-first-entry-2026-07.md`, and prior-art research 16. ADR-0079, the ADR-0072
  and ADR-0073 amendments, the service/extension behavior, and the developer-first repository entry
  are now implemented. A late note naming OpenCode as a developer-friendly example is recorded and
  reflected as fast install orientation, without copying its one-command product shape. Next: run
  the revised journey on the Linux host and collect a consented, observed follow-up review.
- **Public repository metadata is the next small distribution task.** Add a useful GitHub
  description, homepage, and topics in one owner-confirmed outward-facing pass. Funding links stay
  deferred until the owner chooses the recipient/entity, provider, and accounting/tax handling.
- **ADR-0078 visible-browser verification is owed.** C1-C6 and the automated gates are complete.
  Run `docs/tasks/closed-loop-core/LIVE-VERIFY.md` against the visible Linux Chrome host once SSH
  access is available. Cross-origin frame refs remain deferred because they require a separate
  multi-origin governance decision. Headless, isolated, cloud, and remote browser execution remain
  out of scope.
- **Public documentation was rebalanced around responsible delight**: the applied review lives in
  `docs/design/public-documentation-review-2026-07.md`. The README now leads with the real-session
  problem, fit and anti-fit, visible experience, one install journey, and candid platform state.
  A follow-up four-phase freshness pass aligns trust commitments, distribution state, topology,
  tool count, recording privacy, roadmap, current guides, website copy, machine-readable surfaces,
  mobile hierarchy, and public links. Remaining high-value work: macOS/Linux live verification and
  the outcome of the pending CWS review. The optional hero GIF remains intentionally deferred until
  a proper capture is worth publishing.
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
- **Bidirectional installation handoff is released in v0.5.7** (ADR-0070): an explicit first
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
    to get it" line). CWS remains omitted until review completes and the listing is public; Edge,
    winget, and scoop are omitted until each actually ships.
  This workstream is now COMPLETE. The CWS listing was submitted on 2026-07-13 and has moved from
  an owner-side completion gate to an external review wait.
- **Agent narration is implemented** (ADR-0072): additive `narrate` is domainless RAWX none,
  bounded and schema-validated, ordinarily audited, ownership/hold/sacred checked, and legal in
  `script`/`browser_batch`. The policy-free extension renders one timed responsive Agent ribbon per
  tab with deterministic replacement, remaining-time navigation replay, effects/capture handling,
  and tab/session/panic cleanup. Placement is `auto`/`top`/`bottom`; auto chooses one stable edge
  away from recent touched-control, pointer, and scroll activity. The separate central governance
  ribbon now has viewport-bounded sizing and wrapped, untruncated security text. `ghostlight demo`
  narrates its six story beats after each stage loads, holds each caption for its full six-second
  lifetime, and only then begins the visible actions. Rust and the 72-test extension suite are
  green. Live browser
  verification passed on 2026-07-13 through the real MCP `script`
  path: `shown: true`, timed placement, replacement, active-navigation replay, and audit
  `capability: "none"` with no grant attribution. After the responsive refinement reload, a
  top-area hover resolved `auto` to bottom and a bottom-area hover resolved it to top; both calls
  returned the effective edge and the user-visible wide ribbon. Existing MCP clients need one
  restart to add the new direct `narrate` schema to their callable tool list.
- **SAPS remediation remainder** (assessment lives in gitignored `saps/`; findings already
  remediated are in git history around 2026-07-11):
  - SEC-HIGH-03 enforce-half: ADR-0075 proposes a signed managed descriptor, MCP form elicitation,
    one-time in-memory pending action, and stale-sensitive final dispatch. Acceptance needs client
    evidence, schema/privacy review, and Lightbox plus real-browser proof; build is not authorized.
  - SEC-HIGH-02 is closed by removal: ADR-0077 deletes the browser-control web listener, remote
    policy keys, remote-enable route, and WebSocket machinery. There is no remote browser-control
    transport to authenticate. Future remote work requires a new threat model and ADR.
  - A1 demo GIF for the README hero slot (README has a commented placeholder): export it from the
    same `ghostlight demo` OBS recording used for the Store video, then write `docs/assets/demo.gif`.
- **ADR-0047 stage-2 user-supervised e2e re-run** still owed (needs the owner at a real
  browser).
- Parked (deliberately): audit TCP sink (UDP syslog is the standard; revisit only on ask);
  `socket.yml` capability acknowledgments for the npm package (draft-first, owner call).

## Owner-side gates (agents cannot do these)

- Chrome Web Store: monitor the pending v0.5.7 review and answer any reviewer questions; make the
  OAuth credentials durable before the next release. Edge Add-ons remains unsubmitted.
- Trust center legal: vendor entity name in the MSA (blocked on forming the LLC), the
  cyber-insurance yes/no line, counsel skim of MSA/DPA/LICENSE-GOVERNANCE before first
  EXECUTION (publication already happened by design; drafts are marked as drafts).
- Key backup + a second npm publisher; one non-author human through the install flow.

## Standing context worth knowing

- The trust center (`docs/trust/`, 13 docs) is PUBLIC on `main` since 2026-07-11 (PR #27)
  with footers restamped against v0.5.7. Its claims were red-teamed against the tree; keep code and
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
