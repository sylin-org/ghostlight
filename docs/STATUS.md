# STATUS -- where the project stands

Last updated: 2026-08-05. This file is a point-in-time snapshot maintained by whoever
finishes significant work. It exists so a fresh agent (or human) can orient without any
prior session context. **Trust the tree, `git log`, and the batch LEDGERs over this file
when they disagree**, and update it when you land something that changes the picture.

## Now

- **The public-documentation delight strategy is accepted and ready for a fresh-session epic
  pass.** ADR-0100 makes recognition, fit, first success, and recovery the public documentation
  contract for 0.8. It keeps trained tool names, parameters, types, enums, ordering, and structural
  contracts stable while authorizing richer descriptions, examples, annotations, output guidance,
  and package/directory metadata across all 25 tools. The six-pass execution batch lives at
  `docs/tasks/public-delight-0.8/`; its ledger is the durable handoff. Work in
  `sylin-org/website` is authorized on a non-publishing branch. Website deployment, store
  resubmission, directory posts, release publication, and other outward actions still require the
  standing explicit owner confirmation.

- **The v0.8.0 service candidate is locally release-ready without weakening the adapter gate.** The live
  Chrome Web Store listing was verified directly at adapter v0.7.1, updated 2026-08-02. Adapter
  v0.8.0 was submitted for review on 2026-08-05 with deferred publishing. The stale
  v0.6.0/pending-v0.7.1 canonical state and current runbooks are
  corrected. Online public-surface checks now compare the tracked adapter with Chrome's public
  update feed, and `reconcile-chrome-store.ps1` handles both accepted-review and became-public
  transitions without publishing anything. GitHub release assembly now takes `What's changed`
  from the existing versioned changelog instead of publishing only a static template. The
  changelog now has the release-owned `[0.8.0]` section without a premature date. Source adapter
  and service are now
  v0.8.0. Their compatibility row declares the 0.8 contract block: every 0.8 adapter patch covers
  every 0.8 service patch. Public adapter v0.7.1 remains capped at service v0.7.3, so preflight
  blocks the service release until adapter v0.8.0 is public. The store-ready v0.8.0 zip is 137,689
  bytes with SHA-256 `54119b6820f942071053927e0a309e7745629f1d7b992530d3386113d1f46535`.
  Google accepted the store submission. The pinned official MCP publisher validates `server.json`;
  its description was shortened to the registry's 100-character limit. Formatting, strict Clippy,
  the full workspace build and test suite, all 31 Lightbox process scenarios, 164 extension tests,
  seven npm launcher tests, five MCPB launcher tests, four e2e baseline tests, npm package dry-run,
  RustSec audit, cargo-deny policy, and an optimized Windows three-binary build all pass. The pure
  release preflight reports exactly two intentional blockers: public service status is still
  v0.7.3 and public adapter v0.7.1 does not cover service v0.8.0. Package-manager hashes remain
  placeholders until GitHub produces the immutable release assets. No website publication,
  release tag, or registry publication has occurred.
  The online audit now passes the live Chrome comparison and fails only because the public website
  still carries the old extension summary pending an explicitly confirmed refresh.

- **Browser-created tab continuity is implemented and live-verified in the working tree
  (ADR-0099).** Browser requests explicitly opt into bounded `tabDeltaV1` results. The extension
  passively adopts a new child only from an exact, uniquely owned opener and preserves Chrome's
  chosen window, group, and focus. The service validates and atomically claims still-open child
  ids before exposing the result, then adds concise follow-up guidance for an observed active
  child. A controlled `_blank` click returned composite tab id 5541182481 and an immediate
  `get_page_text` call on that id succeeded without an intervening context refresh. In the exact
  Google account-switch scenario, selecting `sylin`/`sylin.org` manually kept source tab
  5541182500 on the Developer Agreement page and opened dashboard child 5541182503 in the same
  window. The extension trace correlated native child 1246215207 to native opener 1246215204;
  the next context inventory exposed the child and a direct focus call succeeded. Because the
  Google selection was a manual browser action, no Ghostlight call falsely claimed it caused the
  child. The source adapter is now v0.8.0 with a 0.8 compatibility block. All 164 extension
  tests, changed JavaScript syntax checks, focused Rust tests, the full workspace suite, and strict
  workspace Clippy pass.

- **The official conformance runner stdio side quest is implemented and dogfooded locally.** The
  `sylin-org/conformance` fork now has branch `feat/stdio-server-runner` in the sibling local
  checkout. One target abstraction selects an existing HTTP endpoint or a runner-owned stdio
  command; the existing lifecycle selector, scenario registry, reporting, baselines, SDK runner,
  and composite action remain the control plane. Stdio uses newline-delimited JSON-RPC, captures
  `stderr` without treating it as failure, closes stdin before bounded process termination, probes
  `server/discover` before 2026 functional traffic, rejects HTTP-only scenarios explicitly, and
  runs transport-neutral initialization, tools, ping, sampling, elicitation, and MRTR paths. A
  real-process fixture proves that multi-round requestState traffic stays on one child process.
  Every observed request and response is validated against the runner's vendored 2025-11-25 or
  2026-07-28 schema from specification commit `71e306956a4959c9655e5036be215d41986596e6`.
  Live Ghostlight passes 2025 initialize, ping, tools/list, and a safe `explain` tool call. It also
  passes the expanded 2026 stateless checks: discovery, required per-request metadata rejection,
  optional clientInfo, response serverInfo, advertised capability/handler agreement, unsupported
  version errors, tools/list, and a safe `explain` tool call. All live wire-schema checks pass.
  The focused Ghostlight connector suite passes 48/48. Final full-workspace formatting, strict
  Clippy, and tests also pass. The runner's final `npm run check`, 46-file/518-test suite, and
  production build pass at local commit `f0273cf`.
  Post-gate live artifacts are under `F:\tmp\conformance-ghostlight-2025-*-post-gate` and
  `F:\tmp\conformance-ghostlight-2026-post-gate`. No Ghostlight defect was found and no
  Ghostlight runtime code changed during the side quest. The conformance fork remains local and
  unpublished pending owner review; no upstream PR or issue comment has been created.

- **Browser topology now belongs entirely to the extension in the working tree (ADR-0098).** The
  service owns logical `WorkspaceId` authority, exact tab ownership, governance, scheduling, and
  browser-profile routing. Its private tool instruction carries only the desired `groupTitle`; it
  stores and sends no native Chrome window or group ids. The extension owns one browser-session
  workspace topology record, derives current placement from live owned tabs, follows a tab or
  whole group that the user moves to another window, and reuses an exact-title visible group
  without merging workspace authority. The old asynchronous `group_request` path, stale-window
  retry, and separate window-qualified maps are removed. Formatting, diff hygiene, JavaScript
  syntax, all 158 extension tests, strict workspace Clippy, the complete Rust workspace suite, and
  all 31 real-process Lightbox scenarios pass. The repository release build is live and doctor
  reports its extension connection and three aggregate MCP edges healthy. After an explicit
  extension reload, a controlled call created tab 5541182382 in group 399144999. The user moved
  the whole group to another Chrome window; a creation-disabled context call returned that exact
  group and tab from its live location without creating or recovering anything. The candidate is
  complete in the working tree. A subsequent `navigate` routed to that same moved tab and loaded
  `https://example.com/`, proving ordinary addressed work follows it too.

- **Closed MCP transport recovery is explicit and verified in the working tree.** A live
  connector-name cutover showed that an MCP client can retain cached Ghostlight tools after its
  host-owned stdio connector is gone. `Transport closed` is real for that client even while the
  shared service, browser extension, and other MCP edges remain healthy. Both date-named MCP
  shores now instruct the model to stop, reconnect through its current client, avoid standalone
  connector workarounds, and inspect state before retrying effectful work. Install explains the
  same cache/liveness distinction. Doctor renders the existing aggregate live-edge count, treats
  zero as informational, and no longer attributes a persistent service to an exited launcher or
  historical MCP client. No process, endpoint, registry, persisted state, workspace rule, or tab
  cleanup behavior was added. Strict workspace Clippy and the full fast-tier workspace suite pass;
  focused results include 48 MCP-edge tests, 28 doctor tests, and 13 installer tests. A read-only
  live doctor probe confirmed one connected browser, two aggregate live MCP edges, and a truthful
  persistent-service row. The affected Codex connection still requires its client-owned MCP
  reconnect before live tool verification. The conformance-runner side quest has since resumed in
  the local `sylin-org/conformance` fork; no external PR or issue comment has been created.

- **The ADR-0096 three-executable cutover is implemented and fully verified in the working tree.**
  Its naming amendment uses `ghostlight-mcp-connector`, `ghostlight`, and
  `ghostlight-browser-connector`, with matching `crates/mcp-connector/` and
  `crates/browser-connector/` source boundaries. This is a rename of the two existing shores, not
  a fourth process or compatibility alias.
  MCP clients now launch `ghostlight-mcp-connector`, whose exact-date
  `mcp_2025_11_25` and `mcp_2026_07_28` modules own stdio, JSON-RPC lifecycle, revision metadata,
  correlation, cancellation, response rendering, and future-call reconnect. A typed owner-only
  bridge carries normalized catalog and work messages to the persistent `ghostlight` service.
  The service owns `WorkspaceId`, the canonical catalog, governance, audit, scheduling, browser
  coordination, and protocol-neutral outcomes; it does not parse MCP or retain JSON-RPC ids.
  `ghostlight-browser-connector` is browser-only and the agent role is gone. Executable entry points and crate
  dependencies enforce the split; the process-global role marker was removed. Browser frames use
  `WorkspaceId` in compatibility `guid` as their sole routing key, while human client labels are
  presentation/audit context only and current tool/group frames omit the former top-level
  presentation/routing `clientKey`. A nested scheduler resource may retain that legacy wire name
  while carrying `WorkspaceId` for covered adapter skew. Installers, doctor, demos, release
  archives, package-manager templates, npm, MCPB, the dev loop, Lightbox, and client configuration
  now use the three sibling executables. Older MCP revisions, pre-initialize calls, raw handshake
  replay, and replay/response recovery across an edge reconnect are intentionally removed. A
  surviving service still drains the same bounded per-call future after one outward
  `outcome_unknown`, so landing checks, audit, and leases settle without a result registry. The
  adversarial runtime pass also binds quarantine recovery to exact executor-generation proof,
  bounds browser writes, orders hold/panic against final enqueue, purges restarted-browser tab
  ownership, preserves replacement focus across stale detach, keeps active work alive past edge
  loss, and rechecks chunk negotiation on the exact final browser connection. Explicit input tab
  ids are verification-only; only exact successful context/create results establish membership at
  the browser shore. Browser relay dial, hello, and identity replay are one bounded reconnect
  attempt, closing the reproduced stale Windows-pipe race. Official MCP Tasks remain unadvertised
  and require a later ADR. Protocol verification uses immutable dated-schema/spec-driven review
  plus exact stdio transcript tests. Formatting, strict locked workspace clippy, the complete
  workspace suite, all 31 Lightbox process scenarios, 164 extension tests, npm/MCPB tests,
  Anthropic's pinned MCPB validator, public-surface checks, syntax checks, diff hygiene, and ASCII
  checks pass. The connector-name follow-up reran strict workspace Clippy, the full workspace
  suite, all 31 Lightbox process scenarios, 164 extension tests, npm/MCPB launcher tests, and
  Anthropic's pinned MCPB validator. The official conformance server runner now has a local stdio
  implementation in the `sylin-org/conformance` fork and passes the live checks summarized above.
  The batch ledger records the original gate evidence; do not treat this cutover as a published
  release.

- **The local directory packaging path is implemented but not externally submitted.** ADR-0095
  adds a self-contained Windows/macOS MCPB with a protocol-clean Node launcher, an installer mode
  that leaves MCP-client configuration to the package host, release assembly, and CI coverage.
  Anthropic's official validator accepts the manifest. The live Anthropic form also says submitted
  extensions must be MIT licensed; Ghostlight's complete bundle is open-core, so submission is
  correctly gated on an eligibility answer and a new released MCPB asset. OpenAI's public plugin
  form requires a public production HTTPS MCP endpoint and remains incompatible with ADR-0077.
  Ready copy and inquiry drafts live in `docs/business/DIRECTORY-SUBMISSIONS.md`. Formatting,
  strict workspace clippy, the full Rust workspace suite, five MCPB launcher tests, archive-layout
  packaging, PowerShell syntax checks, ASCII checks, and diff hygiene pass.

- **Ghostlight was submitted to mcpservers.org on 2026-08-04.** The free Development-category
  submission uses the canonical GitHub repository and `hello@sylin.org` contact. mcpservers.org
  confirmed receipt and quoted a 12-hour review window; the listing is not yet claimed live.

- **GitHub approved Ghostlight for inclusion in the GitHub MCP Registry on 2026-08-03.** The
  manually curated review of `org.sylin/ghostlight` is complete. GitHub will add the server to
  its catalog, and no further owner action is required.

- **Agent-readable tool definitions and standard MCP annotations shipped in v0.7.3
  (ADR-0094).** All 25 tools now publish display titles plus conservative read-only, destructive,
  idempotent, and open-world hints. The mixed `computer` tool publishes MCP's conservative
  whole-tool risk values while Ghostlight retains precise per-action enforcement. Focused
  descriptions now distinguish semantic and low-level interaction, dependent and fixed-input
  batches, form strategies, and the two upload paths.
  Guidance names the actual callable `_mcp` tab tools, and `tabs_create_mcp` advertises its
  stale-workspace recovery role. `update_plan` is now a truthful service-local informational
  echo, so this change requires no browser-adapter release. The compatibility boundary remains
  tool names, parameter names, parameter types, and enums; descriptions are deliberately
  improvable guidance. Formatting, strict workspace clippy, the full Rust workspace suite, and
  the focused 14-test fidelity suite pass. A guarded Windows dev-loop swap and real-relay probe
  also passed with the Chrome extension attached. PR #75 merged the change to `main`, and the
  official MCP Registry now carries v0.7.3. Glama's post-release card scores Ghostlight A for
  license, A for quality, and B for maintenance across 25 tools.

- **The license layout now supports conventional repository discovery without blurring the
  open-core boundary.** Root `LICENSE` contains the standard Apache-2.0 text so repository
  scanners can classify the permissive engine. The alternative MIT text and Ghostlight
  Commercial License live under `docs/licenses/`; `LICENSING.md` maps each source boundary to its
  governing text, and file-level SPDX identifiers remain authoritative. ADR-0027 carries the
  marked layout amendment. The layout shipped in v0.7.3, and Glama now scores its license metadata
  A. Other downstream services still depend on their next crawl.

- **Chrome adapters now version independently from the service (ADR-0093).** Historical adapter
  rows retain inclusive service ranges. From 0.8 onward, `compatibility.json` declares a
  major/minor contract block: any 0.8 adapter patch covers any 0.8 service patch. Release packaging
  names the extension artifact from `extension/manifest.json`; checks require the source adapter
  to cover the source service and the public adapter to cover the public service. Source and
  pending adapter v0.8.0 cover source service v0.8.0. Public adapter v0.7.1 covers published
  service v0.7.3.

- **v0.7.2 ships safe installed-engine activation (ADR-0092).** Windows identifies the
  exact adapter-pipe owner, verifies that its executable belongs to the managed Ghostlight install
  tree, quiesces lock-aware installed relays with owned deploy locks, replaces the predecessor,
  and verifies the selected executable claimed the endpoint. An external repository/dev engine
  is preserved. Linux explicitly restarts its updated user unit; macOS retains its forced kickstart.
  The same cleanup also makes `tabs_create_mcp` use one composite tab ID in its leading prose,
  embedded JSON inventory, and structured result. Formatting, strict workspace clippy, all 693
  core unit tests, the complete Rust workspace suite, 164 extension tests, 4 npm launcher tests,
  and all 34 Lightbox process scenarios pass on Windows. PR `#73` passed the complete GitHub CI
  matrix and merged to `main` before the release tag was cut.

- **End-user extension installation is store-only (ADR-0091).** The README, agent install guide,
  human guides, current design notes, test recipes, public status, and website source now point
  packaged users to the Chrome Web Store. Source builders retain one clearly labeled development
  extension workflow for immediate local testing. Repository and rendered-site checks reject
  alternate end-user installation language on public surfaces.

- **Stale Chrome workspaces now recover through explicit tab creation (ADR-0090).** A known
  Ghostlight `tabId` continues directly. Otherwise `tabs_create_mcp` retries the safe blank-tab
  operation once, selects an eligible normal window in the same browser profile, and replaces the
  dead session pin. Other calls never switch workspaces automatically. The agent-facing error is
  short and points directly to `tabs_create_mcp`.

- **The v1 visual-signature inventory is complete (ADR-0089).** Ref-based `computer.scroll_to`
  now settles three sky chevrons into its exact destination halo; `act_on.scroll_to` retains its
  semantic cue without repainting the same halo. Coordinate `upload_image` now settles a fixed,
  content-free photo tile into the target halo while the model-facing result separately reports
  page-signaled handling or dispatch without a signal. Same-origin iframe geometry is translated
  to the top viewport. Console and network buffer reads remain intentionally quiet because they do
  not manipulate the rendered page. Formatting, strict clippy, the full Rust workspace, all 164
  extension tests, JavaScript syntax, and diff hygiene pass. Visible Windows verification passed
  both new treatments on the public decision-aid page; the existing Windows signature matrix had
  already passed border, navigation, read, find, field, typing, key privacy, JavaScript, wait,
  screenshot, and dual-lane drag treatments. The Linux candidate also passed the automated gate
  (164 extension tests, 4 launcher tests, strict clippy, and the full Rust workspace including 683
  core tests) and the owner's manual visible-browser acceptance pass. The Linux environment is
  release-ready.

- **The repository-controlled OSS adoption readiness repairs are complete on `dev`.**
  `docs/public-status.json` now owns the release, platform, and extension-store truth used by the
  README and the website fallback. CI and release preflight reject local drift; the website
  publisher refreshes both install and status fallbacks, and an optional online check verifies the
  deployed site, GitHub release, npm, install guide, decision aid, and privacy route. Structured
  bug and install forms, a PR template, root support/governance routers, the README decision-aid
  path, a 1280x640 social preview, a greenfield cohort contract, and an honest OSPS Baseline
  2026.02.19 self-assessment are prepared. Website deployment, social-preview upload, Chrome Web
  Store review, non-author cohort evidence, and macOS live proof remain external gates. The OSPS
  review also records that direct pushes are not blocked, GitHub secret scanning/push protection
  are disabled, and privileged-account MFA still needs owner verification; no compliance badge is
  claimed.

- **Open-source publication-path research and a draft Ghostlight awareness plan are complete.**
  Research 20 establishes that GitHub Trending and social trend bots are downstream amplifiers,
  not submission channels, and traces six projects through creator audiences, problem communities,
  host ecosystems, MCP directories, technical media, and category-level demand. The draft plan in
  `docs/design/public-awareness-plan-2026-07.md` separates conversion readiness, targeted proof
  users, ecosystem seeding, a founder-present anchor launch, audience-native follow-ons, earned
  amplification, and privacy-compatible measurement. The derived project-agnostic guide at
  `docs/guides/open-source-publication.md` covers 14 project archetypes, repository and release
  readiness, proof, trust, community, channel strategy, measurement, funding, maintainer capacity,
  reusable templates, decision trees, and three risk-scaled publication tracks. Broad Ghostlight
  publication should wait for the Chrome Web Store path and clean greenfield install to be
  verified. No external publication is authorized by these documents.

- **Window-placed Chromium workspaces are implemented on `dev` (ADR-0085).** The first
  unaddressed tab-context, tab-create, or navigation call reuses Chrome's last-focused eligible
  normal window and pins that browser/window for the Ghostlight workspace. A new window is created
  only when no eligible normal window exists. Groups are keyed by browser window plus workspace;
  their human client label is presentation only. Moved tabs and groups stay where the user put
  them, and private native-window metadata never enters the MCP result. The visible group is
  organization, not the authority boundary; service tab ownership and the extension
  managed-surface guard remain intact. Extension tests and focused
  Rust pin/wire tests pass. Formatting, strict workspace clippy, the full Rust workspace suite,
  all 126 extension tests, JavaScript syntax, and every Lightbox process scenario pass. The
  fallback now distinguishes unknown inventory from a proven empty browser, consults live focus
  plus a validated browser-local MRU, and cannot create a window after an inventory failure.
  Visible Windows verification passed: first-touch work reused the last-clicked window, later
  work stayed pinned there after focus moved elsewhere, and the JavaScript workwheel remained
  visible during a two-second page-local operation. Visible Linux verification also passed in the
  ordinary graphical profile with a freshly rebuilt user candidate and explicitly reloaded
  development extension. A fresh real Codex session reused the last-focused one of two existing
  normal Chrome windows for first-touch work without creating a third window. Focus then moved to
  the other window through a natural no-Chrome-focus interval, but a later unaddressed tab-create
  stayed in the first session workspace. The normal-window count remained two and no product
  defect appeared.

- **Complete browser-window attention routing and multi-instance ergonomics remain accepted for
  v2 in ADR-0084.** New
  unaddressed work follows a service-owned move-to-front queue of eligible browser windows; focus
  carries the window ID, while connection and reconnect no longer count as attention. Tab owners,
  pinned workflows, and explicit selection remain stronger than recent attention, and ambiguity or
  capability mismatch never silently moves work into another authenticated browser context. The
  model-facing vocabulary separates `browserRef`, `browserName`, `engine`, `displayName`,
  `adapterMode`, and `state`, with compact browser provenance and a connected-browser directory.
  The complete implementation is deliberately parked for v2. The current narrow Chromium slice
  still uses the existing coarse browser-profile selector, has no global window MRU, browser
  directory, or explicit selection surface, and does not expose browser provenance.

- **Firefox and browser adapters now have a research baseline.** Research 19 maps all 25 current
  tools across Firefox extension-only and hybrid extension plus Marionette/WebDriver BiDi modes,
  inventories useful Firefox capabilities beyond Ghostlight, and identifies the pairing, trusted
  input, instrumentation, recording, and launch-security gaps. It proposes a typed semantic
  operation seam, connection-time capability negotiation, stable schemas plus dynamic adapter
  guidance, and tab-owner/session-affinity/focus/disambiguation routing. Firefox support, adapter
  refactoring, the proof of concept, and multi-browser selection are one deferred v2 workstream.

- **Target-aware privacy-safe key presentation is implemented on `dev` (ADR-0087).**
  `computer.key` observes only the structural class of each actual trusted keydown target after
  focus resolution. Ordinary printable keys remain literal; native password and platform-marked
  sensitive fields use an unlabeled glowing keycap. Unobservable targets fail private. Named
  navigation keys and real command shortcuts remain readable, and multi-chord sequences retain
  their distinct groups. The raw fallback token, event key, target element, and field value never
  enter the content observer or presentation message. Initial live verification passed the
  distinct ordinary and protected cues. Browser event execution is being reverified under
  ADR-0088.

- **Browser input event fidelity is repaired on `dev` (ADR-0088).** Pure keyboard and
  pointer domains now own complete CDP packets. Printable `computer.key` calls insert text while
  protected-field cues remain private; function keys and standalone modifiers carry correct
  identity; `computer.type` counts Unicode code points after CRLF normalization. Drag execution is
  dual-lane: ordinary pointer gestures stay on complete held-button packets, while native HTML
  drag and drop uses a bounded per-tab CDP interception/replay session. Content observation retains
  only trusted dragstart/cancellation booleans, and opaque drag data never leaves the worker. Empty
  `scroll_to` calls fail, and coordinate image placement distinguishes dispatch from page-signaled
  handling. All 160 extension tests, JavaScript syntax, Rust formatting, and diff checks pass. The
  keyboard, typing, click, hover, and shortcut live matrix passes. Native HTML drag live proof
  preserved the page-authored payload and observed dragstart, dragenter, dragover, drop, and
  dragend. Pointer-only proof moved a range from 10 to 84 through ten input events with a complete
  held-button sequence and no additional native drag lifecycle.

- **Unified action signature medallions are implemented on `dev` (ADR-0083).** One
  policy-free, signal-aware renderer now gives non-spatial work a consistent corner badge while
  avoiding the recent pointer, focused and touched regions, scroll direction, and active
  narration. JavaScript gets an active workwheel with light particles, typing gets a glowing
  keyboard without exposing typed values, waits show three calm dots for their real duration, and
  screenshots end with a camera confirmation alongside the capture frame. Start and finish events
  use the document-aware Presentation Broker and cannot replay into a later document. The complete
  25-tool coverage and review queue live in `docs/design/tool-visual-signatures.md`. Strict clippy,
  the full Rust workspace, 112 extension tests, 4 npm launcher tests, JavaScript syntax checks, and
  formatting pass. The full Windows visible matrix passed, and the owner accepted the final Linux
  visible candidate.

- **The README hero story is implemented, captured, and enabled.** `ghostlight demo-brief`
  drives `https://sylin.org/ghostlight/demo/brief/` through the ordinary relay: one visible page
  read, five exact paced ref writes, submit, and a held local completion state. The stage is
  warm-dark with no native blue or ambient motion, so Ghostlight's persistent
  border, scan, field, and click effects own the visual language. Its contract and recording recipe
  live in `docs/design/demo-brief.md`. A real-stack run against public website revision
  `20f2ce0a259b` completed in 8.44 seconds with a shortened 0.5-second final hold; the default
  three-second hold puts the active story at about 10.9 seconds. The final Chrome-visible capture
  is committed as `docs/assets/demo.gif`: 838 x 766, 11.63 seconds, 382 frames, and 3,111,002 bytes.

- **Branches**: `main` = releases, `dev` = trunk. Work lands on `dev`; the owner reviews
  `dev -> main` PRs and cuts releases.
- **Latest published release: v0.7.3** (2026-08-01). PR #75 merged at tag commit `8bf3b3f`.
  GitHub Actions run `30722958396` passed and published all 28 expected release assets. npm
  `ghostlight@0.7.3` is live at the `latest` tag, and its launcher fetched the integrity-pinned
  Windows binary and passed `doctor`. Homebrew tap commit `5055db1` and the official MCP Registry
  entry `org.sylin/ghostlight` also carry 0.7.3. Checksum fill `fc5b087`, trust restamp `2a26e1d`,
  and website fallback commit `bc0e022` are published; the online public-surface check passes.
  Chrome and Edge submission were skipped because the source adapter remains v0.7.2. Winget serves
  v0.7.2 after PR #410996 merged; the validated v0.7.3 update is open as PR #411087, mergeable with
  its CLA check green while Microsoft controls the remaining validation and merge.
- **v0.6.0 is an intentional greenfield boundary.** The unpublished 0.5.8 draft became this minor
  release because browser-control web ingress and its scaffolding were removed outright. Public
  setup starts from a local service, same-user OS IPC, and the interactive user's authenticated
  Chromium profile. There is no compatibility claim or migration path for the removed web
  transport.
- **v0.5.7 includes the expanded installer matrix**: Codex is a first-class lossless-TOML target
  (ADR-0067), and Windsurf, Zed, OpenCode, and Crush join Claude Code/Desktop, Cursor, and VS Code
  as explicit installer targets (ADR-0071). Strict JSON is merged idempotently. Commented JSONC is
  left intact and receives a copyable manual entry; `doctor` uses a tolerant registration check.
  The browser extension remains a separate user-visible install step. The current CLI help names
  all nine registered clients, with a registry-derived regression preventing future help drift.
- **MCP registry publishing is now automated** in `release.ps1` (the `registry` step, after `npm`):
  `mcp-publisher` DNS-auth publish, gated on `MCP_DNS_PRIVATE_KEY`. The one-time DNS proof is DONE
  (apex TXT `v=MCPv1; k=ed25519; p=...` on sylin.org via Cloudflare; ed25519 key in the env file;
  see `local/AUDIT-LOG.md`). The registry is immutable per version, so metadata fixes (like the
  websiteUrl) only land on the NEXT version.
- **v0.5.7 carries**: all v0.5.6 features plus expanded installer targets, bidirectional install
  handoff (ADR-0070), agent narration (ADR-0072), reliable memory-only GIF recording and bounded
  browser transport (ADRs 0073/0074), the cohesive Card Foundry demo story, and the live Foundry
  companion route at `https://sylin.org/ghostlight/demo/foundry/`.
- **The Chrome Web Store listing is public at v0.7.1; v0.8.0 is pending review with deferred
  publishing.** The owner
  completed the listing, Privacy practices, permission and remote-code justifications, data-use
  certifications, screenshots, video, and promotional tiles for the original v0.5.7 submission.
  After Google reinstated the `ghostlight-release` API project on 2026-07-15, the owner approved
  cancelling that pending review so the first public package would match the greenfield release.
  The v0.6.0 package uploaded successfully and Chrome accepted the new submission as
  `ITEM_PENDING_REVIEW`. Chrome approved and published v0.6.0 by 2026-07-31. The v0.7.1 release
  then uploaded successfully and the publish API accepted it with `[OK] OK.`. The public listing
  reports v0.7.1 updated on 2026-08-02. Adapter v0.8.0 was submitted on 2026-08-05; once approved,
  its staged publication expires after 30 days. Broad host permissions can trigger an in-depth review;
  that remains the intentional tradeoff for general-purpose automation across user-selected
  sites. Edge remains unsubmitted because no `EDGE_*` credentials are configured.
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
- **The four-phase public documentation freshness pass is complete on `dev`.** Trust
  material now follows SECURITY.md's best-effort solo-maintainer targets and names only live
  distribution channels. Present-facing guides use the current service/relay topology, 25-tool
  inventory, one-stack dev loop, shipped licensing behavior, and managed-tab boundary. The original
  SPEC is explicitly historical, recording privacy names the memory-only retention rules, and the
  sylin.org source carries the current version-agnostic fallback plus a product-first narrow hero.
  Ghostlight formatting, local-link and ASCII checks, the website clean build, all generated-site
  checks, and the rendered 390px overflow/navigation/order checks are green.
- **The July non-author experience closure is implemented on `dev` (ADR-0079).** An isolated
  denial is now a centered three-second sticker. Repeated enforced denials pause only the producing
  producing workspace at a synchronized service send boundary (3 matching/60 seconds or 5 total/120
  seconds), then show a closed-shadow overlay and popup controls. Compact narration drops the
  progress meter; screenshot and recording feedback are quieter and tied to real capture state.
  Attention transitions are content-free audit records. The README and install guide now expose
  the four-stage practitioner journey, no-account/free-core facts, visible extension step, and
  a read-only first proof. The full Rust suite, strict clippy, 93 extension tests, JS syntax checks,
  and formatting are green. Repository-actionable work is complete; a consented follow-up human
  review remains an owner-side evidence gate.
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
  screenshots are first; tab labels remain behind baseline evidence. The blocking Linux e2e job
  now executes the mechanical baseline after its ordinary browser smoke; the first CI result and a
  visible local repetition remain pending. The documented model-run recipe can be used from any
  visible browser. One Codex/Windows mechanical run confirmed two observations in each visual
  journey and 33 composite-id characters across three product tabs; it does not yet satisfy the
  repeated-model acceptance gate.

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
  green. The rebuilt service and reloaded development extension passed a real MCP browser verification:
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
v0.7.3 run completed the GitHub, Homebrew, npm, MCP Registry, checksum, trust-footer, and website
paths successfully. Its online public-surface check and published npm launcher smoke both pass.
Because this was a service-only release and the source adapter stayed at v0.7.2, the pipeline
correctly skipped Chrome and Edge submission.

CWS API credentials are working on this machine (see local/RELEASE-CREDENTIALS.md; values in
`~/.ghostlight-release.env`, written by `local/set-credentials.ps1`). Load them before a release:
`Get-Content "$HOME/.ghostlight-release.env" | % { if ($_ -match '^([A-Z0-9_]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1],$Matches[2]) } }`

Winget remains one manual PR per release. `scripts/prepare-winget.ps1` materializes the submission
tree from release manifests and runs `winget validate`. v0.7.2 PR #410996 merged and Winget serves
that version. The v0.7.3 manifest validates locally and PR #411087 is open; its CLA check passes
while Microsoft controls the remaining validation and merge. Store submission remains manual when
its API credentials or dashboard metadata are absent.

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
  mobile hierarchy, and public links. Linux live verification is complete, and the README now uses
  the captured Ghostlight hero. Remaining high-value external evidence is macOS live verification
  and the outcome of the pending CWS review.
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
- **ADR-0047 stage-2 user-supervised e2e re-run** still owed (needs the owner at a real
  browser).
- Parked (deliberately): audit TCP sink (UDP syslog is the standard; revisit only on ask);
  `socket.yml` capability acknowledgments for the npm package (draft-first, owner call).

## Owner-side gates (agents cannot do these)

- Chrome Web Store: wait for adapter v0.8.0 review, then publish the staged package before its
  30-day post-approval expiry and before service v0.8.0. Edge Add-ons remains unsubmitted.
- Trust center legal: vendor entity name in the MSA (blocked on forming the LLC), the
  cyber-insurance yes/no line, counsel skim of MSA/DPA and the commercial license before first
  EXECUTION (publication already happened by design; drafts are marked as drafts).
- Key backup + a second npm publisher; gather first-use evidence through public channels because a
  private greenfield cohort is not currently available.

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
