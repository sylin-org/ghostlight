# Public delight 0.8: LEDGER

Durable progress for ADR-0100. A fresh session resumes from this file and does not infer state from
conversation history.

## RESUME HERE

- State: E6 is in progress at the owner-review gate
- Current pass: E6
- Next action: adapter 0.8.0 remains in Chrome `Pending review`. Wait for that external state to
  change. The owner may also select one signed-in page that permits extension inspection to finish
  the full read-only proof. Any publication action still requires exact approval from
  `docs/business/PUBLICATION-PACKET-0.8.md`.
- External actions: website commit `1568538ba5ca217e46b917688b41d17b7e672488` was pushed to
  `main` and deployed under the owner's explicit website-push instruction. No product push, merge,
  release, store publication, registry publication, directory edit, Discussion, showcase update,
  or social post is authorized.

## Status

| Pass | Title | Status | Product commit | Website commit | Deviations |
| --- | --- | --- | --- | --- | --- |
| E1 | Truth and reception baseline | DONE | this pass's `docs(public): establish E1 public truth baseline` commit | - | none |
| E2 | Message and content architecture | DONE | this pass's `docs(public): define 0.8 message architecture` commit | - | none |
| E3 | Core public surfaces | DONE | this pass's `docs(public): reshape core first-success surfaces` commit | - | none |
| E4 | Agent guidance and metadata | DONE | this pass's `docs(tools): sharpen agent guidance` commit | - | none |
| E5 | Website and directory surfaces | DONE | this pass's `docs(public): prepare E5 discovery surfaces` commit | `1568538ba5ca` | none |
| E6 | Release reconciliation and reception loop | in-progress | this pass's packet commits | `1568538ba5ca` | Chrome review pending; authenticated agent read needs a scriptable source |

Status values: `pending`, `in-progress`, `DONE`, or `BLOCKED`.

## Seed observations from 2026-08-05

These observations reduce rediscovery. They are not accepted E1 evidence until reverified and
cited in the dated baseline.

- The official npm API reported `ghostlight` 0.7.3, 538 downloads for 2026-07-29 through
  2026-08-04, and 2,009 downloads for 2026-07-06 through 2026-08-04. Counts are directional and may
  include automation.
- Glama showed A license, A quality, and A maintenance, one favorite, and a B grade for `computer`.
  Its ingested project copy was stale.
- The live Sylin Ghostlight page was indexed with v0.5.7, the older relay topology, and a stale
  Chrome review statement. Other Sylin project routes repeated enough Ghostlight content to create
  search-result confusion.
- Exact Chrome extension-id searches returned no useful indexed listing. Store users and reviews
  were not independently captured.
- Searches found little independent review material. Project-authored Codex and Zed showcases,
  directory submissions, and registry approvals are distribution, not reception.
- Playwright MCP now documents an extension path into existing signed-in tabs. Chrome DevTools MCP
  emphasizes live Chrome debugging and performance. Ghostlight should differentiate through its
  complete local, visible, recoverable, governed, multi-client experience rather than claiming
  signed-in-browser access is unique.
- `docs/public-status.json` and `docs/STATUS.md` say the service candidate and source adapter are
  0.8.0, the public service is 0.7.3, public Chrome adapter is 0.7.1, and Chrome adapter 0.8.0 is
  pending with deferred publication. Re-read them before use.

## Decisions already settled

- ADR-0100 is the public-documentation decision.
- ADR-0094 owns tool guidance: identity stays stable; descriptions and metadata may improve.
- ADR-0093 owns service/browser-adapter compatibility.
- `docs/public-status.json` owns public release and store state.
- `CHANGELOG.md` owns release notes.
- `sylin-org/website` may be changed on a non-publishing branch. Deployment remains owner-gated.

## External gates

- Chrome adapter 0.8.0 approval and intentional publication.
- Ghostlight service 0.8.0 release, registry update, npm publication, and package-manager updates.
- Website push or merge that triggers deployment.
- Store description resubmission while 0.8.0 is under review.
- Directory edits or submissions, GitHub Discussion creation, showcase updates, and social posts.
- Permission to quote any user or identify any proof participant.

## Execution log

Append one section per completed or blocked pass. Include verified facts, files changed, gates run,
commit hashes, external drafts, and numbered deviations.

### E1 -- truth and reception baseline -- 2026-08-05

- Commit: see this pass's `docs(public): establish E1 public truth baseline` commit.
- Baseline: `docs/research/public-reception-2026-08.md`, observed 2026-08-05.
- Verified product facts: public service 0.7.3; source service 0.8.0; public adapter 0.7.1;
  source and pending adapter 0.8.0; 25 registry tools; nine installer clients; Windows and Linux
  live-browser proof; macOS CI proof with live-browser verification still owed; exact 2025-11-25
  and 2026-07-28 source-candidate MCP shores.
- Verified reception measurements: npm 538 downloads for 2026-07-29 through 2026-08-04 and 2,009
  for 2026-07-06 through 2026-08-04; Chrome Web Store two users with no ratings or written
  reviews; GitHub 0 stars, 0 forks, 0 open issues, 62 aggregate v0.7.3 asset downloads, and one
  project-authored discussion; Glama one favorite. The baseline records the required automation,
  aggregation, and absence-of-evidence caveats.
- Owner-access measurements: Chrome adapter 0.8.0 is accepted for review with deferred
  publication. GitHub's 14-day owner-only window ending 2026-08-04 reported 13 views from 10
  unique visitors, 848 clones from 157 unique cloners, and one view/one unique each from Google and
  github.com in qualifying referrers. Recheck the owner dashboard and traffic APIs before reuse.
- Distribution corrections: Glama is A/A/A with `computer` B but has stale copy; mcpservers.org is
  live but stale; Winget v0.7.3 merged on 2026-08-02; Cline issue 1989 and awesome-mcp-servers PR
  11306 remain open; GitHub MCP Registry approval is owner-recorded but public catalog visibility
  was not independently located.
- Discovery result: the canonical Sylin page and the `/agyo/` and `/zen-garden/` routes expose old
  adapter or topology text. Bounded searches located no independent written review,
  user-authored workflow, or case study. This was recorded as unavailable evidence, not zero.
- Defensible comparison: existing signed-in tabs, visible browser work, form actions, debugging,
  GIFs, and local multi-client access are not unique primitives. The supported distinction is the
  combined visible, recoverable, governed, audited, multi-client local experience.
- Files changed: the dated baseline, both distribution runbooks, this ledger, and `docs/STATUS.md`.
  No README, website, tool definition, or external surface changed.
- Gates: `cargo fmt --all -- --check`; strict workspace Clippy; full fast-tier workspace tests;
  ASCII scan; relative-link validation; `git diff --check`; and live external evidence-link fetches
  all passed. The three owner-only GitHub traffic links returned the expected anonymous 401 after
  their authenticated observations were captured. The pre-existing future `main` MCPB form URL
  remains 404 until that release gate lands.
- Deviation 1: the first Clippy attempt could not create Cargo's temporary sibling beside
  `F:\tmp\ghostlight-e1-target`. It was rerun successfully in the isolated workspace target
  `.target-e1`; this was an environment-path failure, not a code finding.

### E2 -- message and content architecture -- 2026-08-05

- Commit: see this pass's `docs(public): define 0.8 message architecture` commit.
- Copy kit: `docs/design/public-message-0.8.md`.
- Canonical story: one-sentence, short, medium, and long descriptions follow the ADR-0100 order of
  useful work, visibility, continuity, local ownership, optional governance, then technical depth.
- Architecture: six outcome pillars, four audience routes, fit and anti-fit language, a 13-claim
  evidence matrix, and one primary job for each public surface keep later adaptations coherent
  without making every page comprehensive.
- First-success contract: reusable blocks cover roughly 15-second recognition, 2-minute fit,
  5-minute first success, and symptom-led recovery. Exact 0.8 local stdio wording names protocol
  revisions `2025-11-25` and `2026-07-28` below the product story.
- Proof recipes: safe synthetic form, user-chosen authenticated read, exact browser-created child
  continuity, and page/console/network diagnosis each include a copyable prompt, visible result,
  success boundary, and evidence owner. The two Sylin demo routes and both example domains returned
  HTTP 200; the live brief labels and success text matched the recipe.
- Discovery copy: page title is 59 characters, search description is 147 characters, and the
  compact directory draft is 230 characters. The fuller directory draft and banned-claim list are
  internal drafts only; no external edit or submission occurred.
- Better-fit guidance: Playwright MCP, Chrome DevTools MCP, first-party browser integration, and
  hosted/headless browser use cases are stated directly. Signed-in access is not treated as a
  unique primitive.
- Files changed: the new copy kit, this ledger, and `docs/STATUS.md`. No public front door, package
  metadata, tool definition, website, store, directory, or other external surface changed.
- Gates: `cargo fmt --all -- --check`; strict workspace Clippy; full fast-tier workspace tests;
  ASCII scan; relative-link validation; external proof-page and exact-marker checks; and
  `git diff --check` all passed.
- Deviations: none.

### E3 -- core public surfaces -- 2026-08-05

- Commit: see this pass's `docs(public): reshape core first-success surfaces` commit.
- README: reduced from 354 to 184 lines. Recognition, fit, one store-only install path, a copyable
  example.com proof, visible/local experience, and recovery now precede compatibility and the
  three-executable architecture. The full tool table, policy example, CLI inventory, and extended
  procurement copy were removed in favor of links to their canonical owners. The opening keeps
  four release/availability badges and no external score badge.
- Installation: the human guide now opens on a green `ghostlight doctor` outcome, uses the same
  bounded first proof, and maps missing tools, disconnected extension, stale workspace, closed
  transport, denial, development, and bare-CLI symptoms to concrete next actions.
- Agent guidance: `llms-install.md` now presents personal use as complete, keeps the Chrome Web
  Store as the sole end-user extension path, gives a compact tool-choice ladder without duplicating
  the registry, and tells the agent how to recover without arbitrary tabs, standalone connectors,
  blind retries, or policy evasion.
- Comparison: mutable star counts and the binary feature grid are gone. Current primary-source
  guidance states when Playwright MCP, Chrome DevTools MCP, Claude in Chrome, Browser Bridge,
  agent-browser, browser-use, hosted/headless products, or generic governance are better fits.
- Compatibility and contributor truth: README state matches `docs/public-status.json`,
  `compatibility.json`, and exact source-candidate MCP dates. `CONTRIBUTING.md` now preserves the
  trained structural identity while allowing deliberate description and metadata improvements per
  ADR-0094. The solo and compliance guides now say compatible local stdio clients and no longer
  depend on a removed README tool table.
- Files changed: README, installation guide, agent install guide, comparison, contributing guide,
  solo-developer guide, compliance-team guide, this ledger, and `docs/STATUS.md`. No runtime code,
  tool declaration, schema snapshot, package metadata, website, store, directory, or other external
  surface changed.
- Gates: local `scripts/check-public-surfaces.ps1`; `cargo fmt --all -- --check`; strict workspace
  Clippy; full fast-tier workspace tests; ASCII scan; relative-link validation; stale/frozen phrase
  scan; external-link checks; primary-source comparison checks; and `git diff --check` passed. All
  fetched public links returned HTTP 200 except npm's human web page, which returned its automated
  access 403; the official npm registry API remains the package/version evidence.
- Deviations: none.

### E4 -- agent guidance and metadata -- 2026-08-05

- Commit: see this pass's `docs(tools): sharpen agent guidance` commit.
- Scope: all 25 canonical descriptors were reviewed. Eight legacy advertised descriptions changed;
  the other 17 were kept because they already state the job, nearest useful alternative, material
  side effects, and recovery where one is common.
- Shared guidance: an unavailable tab or workspace now directs the agent to `tabs_create_mcp`
  instead of guessing an id. Both protocol revisions already append the exact transport-closed
  recovery contract at initialization, so that guidance was not duplicated in the registry.
- Metadata review: parameter descriptions, display titles, standard annotations, examples,
  expected results, and output-field descriptions were kept for all 25 tools. They are accurate,
  callable-name checks remain green, mixed-tool annotations remain conservative, and changing
  them would have added words without improving tool choice.
- Fidelity: every advertised description now has an intentional fingerprint. A new structural
  golden fingerprints each trained tool's name and complete input schema after removing editable
  description members. The existing exact computer action-order and annotation tests remain.
- Revision projections: the 2025 test now proves `tools/list` preserves a canonical declaration
  exactly. The 2026 test proves descriptions, titles, and examples survive its sanctioned
  `workspaceId` projection. No runtime projection code changed.
- Files changed: the canonical directory guidance, both revision-specific test modules, tool
  schema fidelity tests, the trained identity golden, this ledger, and `docs/STATUS.md`. No tool
  name, parameter, type, enum, order, requiredness, structural schema, runtime result, browser
  behavior, extension code, package metadata, or external surface changed.
- Gates: `cargo fmt --all -- --check`; strict workspace Clippy; full fast-tier workspace tests;
  both revision-specific connector test suites; all tool-schema fidelity tests; ASCII scan; and
  `git diff --check` passed in isolated target `.target-e1`.
- Deviations: none.

#### Descriptor dispositions

| Tool | Disposition | Reason |
| --- | --- | --- |
| `tabs_context_mcp` | keep | Already distinguishes inventory from creation and names unavailable-workspace recovery. |
| `tabs_create_mcp` | keep | Already states creation, focus, recovery, and the `navigate` follow-up. |
| `navigate` | keep | Already separates top-level navigation from clicks and names unsaved-change risk. |
| `computer` | keep | The external B grade prompted review only; current guidance names exact alternatives and retry risk while preserving the official mixed-tool signature. |
| `find` | change | Added structure and prose alternatives plus stale-ref recovery. |
| `form_input` | keep | Already distinguishes one exact ref from `form_fill` and states page-event side effects. |
| `get_page_text` | change | Added `read_page` and `find` selection guidance plus bounded-output recovery. |
| `javascript_tool` | change | Added purpose-built-tool preference, unbounded side effects, and no-blind-retry guidance. |
| `read_console_messages` | change | Added first-use tracking, reload recovery, output bounds, hostname scope, and `clear` consumption. |
| `read_network_requests` | change | Added first-use tracking, reload/interaction recovery, output bounds, reset scope, and `clear` consumption. |
| `read_page` | change | Added prose and targeted-read alternatives, diff semantics, output focusing, and stale-ref recovery. |
| `resize_window` | change | Clarified window-wide scope, responsive rerender effects, and ref refresh. |
| `update_plan` | keep | Already prevents agents from treating informational planning as approval or authority. |
| `narrate` | keep | Already distinguishes meaningful phase changes from routine interaction. |
| `wait_for` | keep | Already explains settlement, conditions, matched refs, timeout evidence, and the dynamic-page job. |
| `script` | keep | Already distinguishes dependent steps from `browser_batch` and explains validation, authorization, references, and waits. |
| `form_fill` | keep | Already distinguishes semantic multi-field filling from exact-ref input and handles ambiguity safely. |
| `act_on` | keep | Already distinguishes semantic receipt-based action from coordinate work and refuses ambiguity. |
| `dialog` | keep | Already makes status and explicit user intent the safe recovery path. |
| `tab_control` | keep | Already limits scope to one owned tab and makes close explicit. |
| `file_upload` | keep | Already avoids the native picker and distinguishes client files from captured images. |
| `browser_batch` | change | Condensed fixed-step selection while adding partial-completion recovery and preserving coordinate limits. |
| `upload_image` | keep | Already distinguishes captured images from client files and makes target choice exclusive. |
| `gif_creator` | keep | Already explains the bounded lifecycle, automatic stops, memory ownership, and export choices. |
| `explain` | keep | Its ADR-pinned description already distinguishes permission explanation from page explanation. |

### E5 -- website and directory surfaces -- 2026-08-05

- Product commit: see this pass's `docs(public): prepare E5 discovery surfaces` commit.
- Website commit: `bda3791` (`feat(ghostlight): align public discovery story`) on
  `work/ghostlight-public-delight-e5`. The final E5 action is the owner-authorized push of that
  exact non-publishing branch. A matching remote ref completes this record without another
  product edit. No website deployment branch was changed.
- Website story: the hero now names useful signed-in Chromium work, visibility and takeover, local
  ownership, and optional policy in the settled order. Fit and anti-fit guidance names Playwright,
  DevTools, Firefox, hosted, headless, stealth, and scraping boundaries without a feature-count
  comparison. The current three-executable path appears below the product story.
- Website proof: one read-only first task and four accessible disclosure recipes cover a safe
  synthetic form, a user-chosen authenticated read, an exact browser-created child, and combined
  page/console/network diagnosis. Each recipe states an expected result and success boundary.
- Discovery: direct project routes now have qualified titles and descriptions, canonical and
  social metadata, social-image alt text, and optional structured data. The enhanced carousel
  updates title, description, canonical, Open Graph, Twitter, image, alt, and structured data when
  the selected project changes, while direct links remain complete without JavaScript.
- Synchronization: the website's public-status and install-guide fallbacks are byte-identical to
  `docs/public-status.json` and `llms-install.md`. No new fetcher, CMS, analytics, beacon, or client
  version lookup was added. Site checks reject the stale relay topology and governance-first demo
  copy on every generated HTML, Markdown, and text route.
- Package metadata: the local 0.8 npm description now identifies the artifact as an installer and
  launcher. The 87-character official MCP Registry description and 94-character MCPB description
  share the visible, local, signed-in-browser story. The MCPB long description names visible work,
  continuity, the exact local roles, complete personal use, and optional policy and audit.
- Draft packet: `docs/business/PUBLIC-COPY-DRAFTS-0.8.md` carries field-specific, owner-gated copy
  for npm, the official registry, MCPB/Claude, Chrome Web Store, Glama, mcpservers.org, Cline issue
  1989, awesome-mcp-servers PR 11306, GitHub MCP Registry, mcp.so, PulseMCP, and the Codex and Zed
  showcases. It records current format sources, review cost, release sequencing, and the difference
  between project-authored distribution and reception.
- Website verification: changed JavaScript syntax checks, `git diff --check`, production
  `npm test`, generated metadata inspection, all-route stale-copy checks, fallback byte parity,
  direct Ghostlight and Agyo routes without JavaScript, in-page next/back metadata lifecycle,
  desktop visual review, and an exact 390 CSS-pixel mobile review passed. The exact mobile page and
  body widths were 390, the proof section stayed within x=32..358, and four disclosure controls
  rendered without page overflow.
- Product verification: all changed manifests parse as JSON; description lengths are 127, 87, and
  94 characters; seven npm launcher tests and five MCPB launcher tests pass; the pinned
  `@anthropic-ai/mcpb@2.1.2` validator accepts the manifest with only its existing advisory to use
  a 512px icon; local public-surface reconciliation, relative links, ASCII, formatting, strict
  workspace Clippy, full fast-tier workspace tests, and `git diff --check` pass in isolated target
  `.target-e1`.
- External state: no Chrome copy was edited while adapter 0.8.0 is pending. No package, service,
  registry, directory, issue, pull request, discussion, tag, website deployment, or release was
  published. The required website branch push is source transfer only and does not deploy.
- Deviations: none. Sandbox restrictions required approved runs for website dependency/build
  output, headless visual verification, Git metadata, and the pinned npm-fetched MCPB validator;
  these were environment constraints, not product findings. `npm ci` reported one high-severity
  dependency advisory in the website tree; no dependency or lockfile change was authorized in E5.

#### E5 publication amendment -- 2026-08-05

- The owner subsequently instructed that the website be pushed. The E5 branch first reached
  `bda3791`, then the public copy was simplified in `1568538` to remove repeated guilt-framed
  wording. The owner approved that follow-up and instructed another push.
- Website `main` and `work/ghostlight-public-delight-e5` now both resolve to full commit
  `1568538ba5ca217e46b917688b41d17b7e672488`. The external host deployed it. Live assets expose
  marker `1568538ba5ca`, and the Ghostlight page and repeated carousel fallbacks carry the current
  public 0.7.3/adapter 0.7.1 story.
- This amendment supersedes E5's earlier no-deployment statement. It does not authorize any other
  external action.

### E6 -- release reconciliation and reception loop -- 2026-08-05 (in progress)

- Exact baselines: product `57b4d24274942c03c4c6f8d7465ae7d2b31a7dd1`; product public
  `main` `6de59b9920320a2e4b7f52e049c14c448cdaca56`; live website
  `1568538ba5ca217e46b917688b41d17b7e672488`.
- Evidence refresh: npm, GitHub, Chrome public listing, official MCP Registry, Glama,
  mcpservers.org, Cline, awesome-mcp-servers, Winget, bounded search, and primary comparison facts
  were rechecked. The counters and directory states did not change. The canonical website is now
  current; search and directory caches are not.
- Reconciliation: `scripts/check-public-surfaces.ps1 -Online` passes. All checked live website
  routes return 200, the Chrome update feed reports public adapter 0.7.1, and website assets expose
  exact commit marker `1568538ba5ca`.
- Live proof: the safe brief, exact browser-created child, and Foundry diagnosis recipes passed
  through this Codex client and the visible browser. The child was usable by its returned id on the
  immediate next call without a context refresh. For the authenticated recipe, the owner selected
  the Chrome Developer Dashboard and manually switched to `sylin`. Ghostlight immediately
  inventoried the exact browser-created dashboard child, but Chrome blocks extension inspection
  and debugger attachment on Web Store pages. The owner screenshot supplied the visible item and
  status evidence. The full authenticated read subsequently passed on the owner-selected signed-in
  Microsoft Partner Center home: `get_page_text` returned the workspace state and `read_page`
  returned the page controls. The proof itself was read-only, and no private contact value was
  retained in project evidence.
- Edge distribution: the owner authorized an Edge Add-ons enrollment investigation. No program
  enrollment or store submission completed. The individual-account form states that the contact
  address is customer-visible, so the owner deferred the native listing rather than publish a home
  address. Microsoft supports installing Chrome Web Store extensions in Edge through its
  other-store path; that remains Ghostlight's Edge end-user route after Chrome approval.
- Packet: `docs/business/PUBLICATION-PACKET-0.8.md` records exact commits, evidence, every proposed
  destination, store implications, caveats, ordered release steps, and channel recovery.
  `docs/research/public-reception-loop-0.8.md` holds the release/7-day/30-day manual template and
  owner-gated Discussion draft.
- Release preview: the authenticated dry run passed tool/authentication setup and stopped at the
  intended `main`-only guard because the candidate remains on `dev`. The packet requires a clean,
  reviewed `main == origin/main` candidate and `-SkipExtension` after Chrome adapter 0.8.0 becomes
  public.
- Gates: local and online public-surface reconciliation; `cargo fmt --all -- --check`; strict
  locked workspace Clippy; the complete locked workspace suite in isolated target `.target-e1`;
  164 extension tests; seven npm launcher tests; five MCPB launcher tests; ASCII; relative links;
  and `git diff --check` passed. Current public and owner-only evidence endpoints, website routes,
  directory pages, Chrome update/listing pages, and primary comparison sources were fetched during
  the dated recheck.
- External state: no product push, merge, release tag, store submission or publication, registry
  publication, directory edit, issue/PR comment, Discussion, showcase update, or social post
  occurred in E6. Edge enrollment stopped before agreement acceptance and completion.
- Deviations: one transient fresh-tab request changed nothing; the existing owned blank tab
  completed the safe proof. The child recipe's first semantic lookup omitted the newly injected
  link; an exact measured coordinate action completed the proof and returned the unambiguous child.
  The authenticated Chrome dashboard proof reached a browser-protected surface: topology and
  signed-in continuity were visible, while page reading was not available to extensions.
