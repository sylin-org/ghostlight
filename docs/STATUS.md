# STATUS -- where the project stands

Last updated: 2026-07-15. This file is a point-in-time snapshot maintained by whoever
finishes significant work. It exists so a fresh agent (or human) can orient without any
prior session context. **Trust the tree, `git log`, and the batch LEDGERs over this file
when they disagree**, and update it when you land something that changes the picture.

## Now

- **Branches**: `main` = releases, `dev` = trunk. Work lands on `dev`; the owner reviews
  `dev -> main` PRs and cuts releases.
- **Latest published release: v0.6.0** (2026-07-15), cut with `scripts/release.ps1 0.6.0`.
  Shipped and LIVE: GitHub Release (28 files: 27 payloads including the CycloneDX SBOM, plus the
  canonical hash manifest and Sigstore attestations), npm `ghostlight@0.6.0`, Homebrew tap,
  **MCP registry (`org.sylin/ghostlight`, 0.6.0 is latest)**, filled Scoop/Winget/Homebrew
  manifests, trust footers, and the current sylin.org install guide. Winget PR
  `microsoft/winget-pkgs#402692` is open and passed local `winget validate`. Release PR #50 merged
  at `2d8cb0c`; checksum fill is `aa8d50a` and the trust restamp is `2308b0c`.
- **v0.6.0 is an intentional greenfield boundary.** The unpublished 0.5.8 draft became this minor
  release because browser-control web ingress and its scaffolding were removed outright. Public
  setup starts from a local service, same-user OS IPC, and the interactive user's authenticated
  Chromium profile. There is no compatibility claim or migration path for the removed web
  transport.
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
- **The Chrome Web Store listing is submitted at v0.6.0 and pending compliance review.** The owner
  completed the listing, Privacy practices, permission and remote-code justifications, data-use
  certifications, screenshots, video, and promotional tiles for the original v0.5.7 submission.
  After Google reinstated the `ghostlight-release` API project on 2026-07-15, the owner approved
  cancelling that pending review so the first public package would match the greenfield release.
  The v0.6.0 package uploaded successfully and Chrome accepted the new submission as
  `ITEM_PENDING_REVIEW`. Chrome again warned that broad host permissions may trigger an in-depth
  review; that is the intentional tradeoff for general-purpose automation across user-selected
  sites, not a rejected submission. Edge was skipped because no `EDGE_*` credentials are
  configured.
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
  gates and all 34 Lightbox scenarios pass. The visible Linux verification is complete: semantic
  success and ambiguity, dialog blocking and recovery, owned-tab lifecycle, unowned-tab refusal,
  provenance boundaries, and minimized audit records all passed in the ordinary Chrome profile.
- **Linux user-session discovery is implemented and live-proven (ADR-0082).** A relay launched
  with `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS` absent securely found `/run/user/1000`,
  started and reached the user service, and converged with Chrome's real native-host environment.
  `doctor` found the extension, and Codex 0.144.4 completed browser actions in visible Chrome
  150.0.7871.124. Linux-only imports and environment constants are now compile-gated away from
  macOS and Windows, and the ownership regression reaches a real mismatched-owner directory rather
  than passing on a missing path. The user-level candidate is 0.5.8; it is not a published release.
- **The Foundry demo is compatible with ADR-0078 provenance boundaries.** Its machine-result
  preprocessor validates structured page provenance plus matching origin and nonce markers before
  unwrapping geometry JSON. Raw fallback is enabled only after `tools/list` advertises the legacy
  contract; current, missing, and unnegotiated contracts fail closed. Consumers accept the ADR's
  full lowercase even-length nonce range of at least 96 bits instead of pinning today's 128-bit
  producer. A normal-paced visible run on 2026-07-15 completed the full story,
  enforced the off-domain denial, exported a 100-frame 23,141,963-byte replay, confirmed page
  receipt, and cleared the captured bytes. No trained schema or model-facing boundary changed.
- **Release publication now has a narrow privileged boundary.** A read-only assembly job generates
  the pinned SBOM, packages the extension, creates `SHA256SUMS`, and uploads one immutable bundle.
  The privileged job only downloads, verifies the exact file set and hashes, attests, and releases.
- **The public vulnerability-disclosure endpoint is live.** `https://sylin.org/.well-known/security.txt`
  publishes the contact, expiry, canonical URL, and Ghostlight security-policy link.
- **The four-phase public documentation freshness pass is complete in the working tree.** Trust
  material now follows SECURITY.md's best-effort solo-maintainer targets and names only live
  distribution channels. Present-facing guides use the current service/relay topology, 25-tool
  inventory, one-stack dev loop, shipped licensing behavior, and managed-tab boundary. The original
  SPEC is explicitly historical, recording privacy names the memory-only retention rules, and the
  sylin.org source carries the current version-agnostic fallback plus a product-first narrow hero.
  Ghostlight formatting, local-link and ASCII checks, the website clean build, all generated-site
  checks, and the rendered 390px overflow/navigation/order checks are green.
- **The July non-author experience closure is implemented on `dev` (ADR-0079).** An isolated
  denial is now a centered three-second sticker. Repeated enforced denials pause only the producing
  MCP session at a synchronized service send boundary (3 matching/60 seconds or 5 total/120
  seconds), then show a closed-shadow overlay and popup controls. Compact narration drops the
  progress meter; screenshot and recording feedback are quieter and tied to real capture state.
  Attention transitions are content-free audit records. The README and install guide now expose
  the four-stage practitioner journey, no-account/free-core facts, pre-release extension path, and
  a read-only first proof. The full Rust suite, strict clippy, 93 extension tests, JS syntax checks,
  and formatting are green. Visible Linux/browser verification remains owed.
- **Resource-scoped browser command scheduling is implemented (ADR-0080).** The service now owns
  bounded fair queues for concrete tab surfaces, client topology, and browser-wide work. Same-tab
  commands serialize while different tabs remain parallel. Configuration and policy publish as one
  atomic authority epoch; URL probes, dispatch, landing verification, compound helpers, and audit
  retain the admitted execution context. Static single-surface scripts and browser batches retain
  the tab lease and yield at a 60-second step boundary; dynamic and multi-surface batches schedule
  per step. The extension adds a bounded per-surface FIFO, command deduplication, acceptance and
  terminal acknowledgements, payload erasure, and separate presentation/control bypass. Unknown
  outcomes quarantine a tab until an exact terminal acknowledgement, confirmed tab destruction,
  or a changed browser-process generation proves recovery. Every asynchronous reply now retains
  the accepting native connection plus request and command identity, so a late completion cannot
  cross into a replacement connection that reused its numeric request id. Dialog guarding also
  precedes scroll ref resolution, page probes, cursor movement, and direct fallback. Strict clippy,
  the full Rust workspace, all 34 Lightbox scenarios, and 108 extension tests pass. Visible
  verification found and fixed a
  retained-intent defect: extension execution identity now includes the internal request ID, so
  separate subrequests under one retained lease cannot suppress each other. A live v0.5.8 Chrome
  probe submitted
  deliberately overlapping JSON-RPC calls through a raw relay: two same-tab waits completed at
  4.41 and 8.41 seconds, two different-tab waits completed at 2.07 and 4.00 seconds, and narration
  rendered in 19 ms while a 3.98-second page command remained active. One first-post-reload
  `tabs_create_mcp` call lost its terminal acknowledgement and correctly returned
  `outcome_unknown`; inspection proved no tab was created and a deliberate retry succeeded in
  42 ms. Keep that transient in reconnect/reload reliability coverage.
- **Node CI now enforces the complete JavaScript surface on all three operating systems.** The
  extension job discovers every direct test file, parses every extension JavaScript file as a
  whole, and runs the npm launcher's host-allowlist, SHA-256, and target-selection tests. The local
  parity gate is 108 extension tests plus 4 launcher tests.
- **The document-aware Presentation Broker is implemented (ADR-0081).** One policy-free extension
  domain service now owns managed-tab document readiness, exact channel/revision/document
  acknowledgements, on-demand packaged-renderer activation, timed state replacement and replay,
  bounded document-local effects, browser-session-only restoration, and capture barriers. An
  extension reload on an unchanged page no longer depends on navigation to reinstall signage.
  Ready signals and activation are gated to Ghostlight-managed tabs. The prior narration and
  attention stores are consolidated into the broker; the renderer keeps DOM/CSS ownership and
  governance authority remains in the Rust service. Strict clippy, the full Rust workspace, all
  34 Lightbox scenarios, extension syntax checks, and all 100 extension tests pass. A live Chrome
  probe acknowledged narration on an unchanged managed document, acknowledged it again immediately
  after navigation, and completed a screenshot capture. A raw-relay concurrency probe returned
  narration in 4 ms while a same-tab page wait completed in 4,203 ms; the tool connector, not
  Ghostlight, explained an initially serialized measurement. The owner then confirmed narration,
  the navigation pill, screenshot border/camera/frame, and read scan in Chrome. That gate clarified
  the border's semantics: it now follows managed-tab control scope as deadline-free replayable
  state, with a gentle four-second breathing pulse, rather than fading after individual actions.
  It remains across idle time, navigation, detachment, and worker restart; capture hides and
  restores it. Strict clippy, the full Rust workspace, all 34 Lightbox scenarios, extension syntax
  checks, and all 102 extension tests pass. Awaited delivery and readiness deadlines remain
  referenced while background expiry remains unreferenced. A focused live probe delivered
  narration in under one second while a same-tab page wait remained active for at least 3.5
  seconds. The owner-visible local gate also passes: after an
  explicit unpacked-extension reload, the idle Example Domain tab recovered its pulsing border
  without another tool call; navigation kept the message, border, and pulse; and screenshot
  capture showed its camera cue while suppressing and then restoring the border.
- **The agent-browser overlap map is current through v0.31.2 (2026-07-13).** Research 17 contains
  the requested one-to-one table. The recommendation is deliberate non-parity: retain the local
  live-user-context boundary, compose with testing runtimes for specialist breadth, and measure two
  small free-surface candidates next -- ref-linked annotated screenshots and optional owned-tab
  labels. Research 18 now defines deterministic journeys, payload boundaries, benefit thresholds,
  and fail conditions. The opt-in real-stack baseline harness and four-layout local fixture are
  ready under `tests/e2e`; its default smoke path and public schemas are unchanged. Annotated
  screenshots are first; tab labels remain behind baseline evidence. The automated baseline waits
  for the Linux host, while the documented model-run recipe can be used from any visible browser.
  One Codex/Windows mechanical run confirmed two observations in each visual journey and 33
  composite-id characters across three product tabs; it does not yet satisfy the repeated-model
  acceptance gate.

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
v0.6.0 run published GitHub, Homebrew, npm, the MCP Registry, checksum pins, and trust footers. The
release workflow's checkout-free publisher needed an explicit repository argument; the release was
recovered from the already verified and attested immutable bundle without a rebuild or retag, and
the workflow fix is prepared. The website fallback already matched after newline normalization, so
no rebuild was needed. Chrome initially stopped at a suspended OAuth project; after Google
reinstated it, the v0.5.7 review was cancelled and v0.6.0 was uploaded and resubmitted. Edge
remains unconfigured.

CWS API credentials are working on this machine (see local/RELEASE-CREDENTIALS.md; values in
`~/.ghostlight-release.env`, written by `local/set-credentials.ps1`). Load them before a release:
`Get-Content "$HOME/.ghostlight-release.env" | % { if ($_ -match '^([A-Z0-9_]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1],$Matches[2]) } }`

Still manual per release: a winget PR to `microsoft/winget-pkgs` (CLA). The repository now provides
`scripts/prepare-winget.ps1`, which materializes the correct submission tree from release manifests
and runs `winget validate`; v0.6.0 passed and is submitted as upstream PR #402692. Store submission
remains manual when its API credentials or dashboard metadata are absent.

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
- **Public repository metadata is live.** The owner-confirmed outward-facing pass added a
  practitioner-first GitHub description, the `https://sylin.org/ghostlight/` homepage, and ten
  discovery topics spanning MCP, browser automation, Chromium, local-first operation, Rust,
  developer tooling, and access control. Funding links stay deferred until the owner chooses the
  recipient/entity, provider, and accounting/tax handling.
- **ADR-0078 visible-browser verification is complete.** C1-C6, the automated gates, and the five
  visible journeys in `docs/tasks/closed-loop-core/LIVE-VERIFY.md` passed on the Linux host.
  Cross-origin frame refs remain deferred because they require a separate multi-origin governance
  decision. Headless, isolated, cloud, and remote browser execution remain out of scope.
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
  and choose a controlled experiment origin. A 2026-07-14 recheck against the official Chrome 149
  trial, security guidance, and current explainer found the draft still current; nothing was sent.
  ADR-0043's no-implementation stance remains intact.
- **Agent journey evaluation artifacts are proposed** (ADR-0069): local, minimized evidence for
  comparing models and clients across a browser journey. Acceptance requires concrete journeys, a
  data inventory and threat review, a versioned artifact schema, lightbox production, and evidence
  from at least two client or model configurations. The v0 design now completes the first four
  gates with three journeys, redacted-by-default field rules, an append-only directory format,
  compatibility policy, and threat review. Lightbox production and two-configuration evidence
  remain open; no capture tool or replay path is authorized.
- **Bounded delegation needs scenario validation before an ADR**: the release-candidate triage
  journey in `docs/design/bounded-delegation-scenario.md` exercises the ADR-0060 session overlay and
  identifies the unresolved approval, expiry, budget, intent, and digest questions. Personal travel
  research and organization-managed incident triage now add the two missing postures, and a
  six-state paper prototype plus rejection criteria is ready. Human comprehension evidence, client
  elicitation capability, and enforceable consequence vocabulary remain open.
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

- Chrome Web Store: monitor the pending v0.6.0 review and answer any reviewer questions. Edge
  Add-ons remains unsubmitted.
- Trust center legal: vendor entity name in the MSA (blocked on forming the LLC), the
  cyber-insurance yes/no line, counsel skim of MSA/DPA/LICENSE-GOVERNANCE before first
  EXECUTION (publication already happened by design; drafts are marked as drafts).
- Key backup + a second npm publisher; one non-author human through the install flow.

## Standing context worth knowing

- The trust center (`docs/trust/`, 13 docs) is PUBLIC on `main` since 2026-07-11 (PR #27)
  with footers restamped against v0.6.0. Its claims were red-teamed against the tree; keep code and
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
