Ghostlight 1.0.0

The clean-room rebuild around one orchestrator that owns every product decision, a stable
protocol-versioned MCP edge, a browser-only relay, and a policy-free Chromium adapter. The
catalog grows to 24 tools, governance becomes a policy a person can read, and the whole
model-facing surface is revoiced to teach.

Requires the local Ghostlight application: https://sylin.org/ghostlight/

Source and documentation: https://github.com/sylin-org/ghostlight

Added
-----

- Ghostlight 1.0 contracts. docs/1.0/ defines the current intent, language, architecture, and
  acceptance boundary without replacing the inherited project documentation.
- Integrated desktop workbench (ADR-0102). The orchestrator hosts a Tauri 2 tray workbench with
  at-a-glance plural activity, payload-free history, checkup, runtime controls, global search,
  supported-harness management, and high-signal native notifications.
- Supported-harness management. Codex, Claude Code, Claude Desktop, Cursor, Visual Studio Code,
  Windsurf, Zed, OpenCode, and Crush registrations can be checked, installed, or removed through
  explicit ownership-checked operations that preserve JSONC and TOML comments. The roster grows
  to 18 products and 21 concrete targets with offline visual marks, status-sorted cards, and a
  lossless YAML seam (ADR-0125).
- A live monitor. The workbench's landing surface carries the current action in full with its
  elapsed time, then settles it into a newest-first queue as the next one rises. The orchestrator
  publishes sequenced changes rather than being polled, and a surface that misses one
  resynchronizes from a snapshot instead of trusting its cache.
- Rows that say what happened. Every record carries a Ghostlight-authored sentence and a measured
  duration: "Opened example.com.", "Read 1,240 words.", "Filled 3 fields and submitted the
  form.", "Stopped at step 3 of 5."
- Per-action observation (ADR-0103). Audit records gained a closed, content-free observed
  projection: the host an action landed on, the readiness the browser reported, a count, and a
  capture size. Never a path, query, or fragment; never page text.
  https://github.com/sylin-org/ghostlight/blob/v1.0.0/docs/guides/siem-integration.md
- Demand-start and one local authority (ADRs 0104, 0112, and 0127). Launching a connected MCP
  client or Chromium starts the local authority when it is absent. A lifetime lease admits
  exactly one authority, so concurrent launches converge on one engine and one tray instead of
  racing. Launching Ghostlight starts the one desktop authority with its workbench backgrounded;
  launching it again focuses the running workbench.
- Behavioral capability restoration (ADR-0133). Every published 0.8 browser job is reachable
  again through current seams: REPL-mode browser_execute with promise waiting, user gesture, and
  by-value returns; modified, repeated, focused, timed, and stroke-sequence input; duration waits
  and coordinate wheel scrolling; semantic selectors as alternatives on click, typing, and
  per-field fill; boolean and numeric form values with contained-form submit verification;
  optional postconditions that report truthfully when an expectation fails; article-first reading
  with a visible-text mode and bounded diffs; document-scope inspect trees with snapshot handles
  and structural diffs; uploads from absolute paths, bounded inline files, or one captured image;
  and browser_flow, the twenty-third tool, composing one to twenty steps whose arguments may
  reference earlier results through bounded JSON Pointers.
- Guarded beforeunload navigation. browser_navigate accepts beforeunload:discard, which accepts
  only that navigation's own beforeunload prompt and then follows the ordinary commit and landing
  path; the default still stops and reports a blocking prompt.
- The policy a person can read and author (ADR-0122). Policy is the workbench's fourth
  destination: one situation sentence, one plain line per capability naming the layer that
  decided it, the rules behind those lines, and the permanent ceilings in every situation
  including all-open. The workbench authors one user policy through validated, atomic writes;
  organization ceilings disable the controls they govern and name who set them.
- RAWX and managed policy (ADR-0121). The action directory is an independent set that drives
  enforcement, audit, catalog projection, explanation, and simulation. Schema-3 ordered grants,
  host polarity and specificity, layer intersection, sacred destinations, stable denial ids, and
  grant attribution are live. An organization can deliver signed monotonic bundles from its own
  file or HTTPS source, with required Ed25519 and optional mandatory-both ML-DSA-65, verified
  cache, bounded retry, and rollback refusal. The workbench shows a Policy Passport and a lamp
  band distinguishing all-open, applied policy, retained-policy warning, and fail-closed states.
- Reference experience (ADR-0126). One product across every machine a person uses: a
  second-machine extension state with one route back, a closed per-platform environment table
  shared by install and doctor, a first-class CLI (doctor --json, manual pages, shell
  completions, plain-word states), pinned human-control directives, and a policy attention hold
  separate from a person's pause.
- Lean Linux installation (ADR-0123). One XDG Applications entry, ghostlight open activation,
  native-package provenance with Snap and Flatpak refusal, and the release Linux artifact built
  on Ubuntu 22.04 with Debian 12 and Ubuntu 24.04 lifecycle smokes.
- Tab and group reuse with self-healing (ADR-0137). Duplicate same-title groups merge into the
  canonical one, a plain open adopts the nearest unbound same-host tab, and the summary says
  "Reused the ... tab." when it happens. A refused release-close unbinds the tab so it becomes
  adoptable, and workspace release tells the extension to forget released tab ids.
- Frame-transparent semantic layer (ADR-0138). Content scripts run in every frame; locators are
  frame-scoped at minting; document-wide reads aggregate across frames in stable order; fill_form
  groups fields by owning frame and proves a contained submit before clicking; pointer geometry
  over embedded targets composes through parent-side embed boxes with no debugger attachment.
- Shadow-complete interaction (ADR-0139). Focused-control discovery walks the shadowRoot
  activeElement chain and point-action subjects cross the shadow boundary through
  getRootNode().host. Closed shadow roots stay closed: never pierced, never patched.

Changed
-------

- Fully open-source licensing (ADR-0140). The whole product, including the governance module, is
  now Apache-2.0 OR MIT. Every paid option -- the tier table, founding program, prices,
  commercial license text, PRICING.md itself, and the governance-module CLA -- was withdrawn.
  Packaging ships only Apache-2.0 and MIT texts. The runtime never enforced licensing before and
  nothing changed there: the Continuity Promise holds unchanged.
- 1.0 implementation rebaseline. The active implementation is the clean-room orchestrator, shared
  typed bridge, stable MCP and browser connectors, and policy-free browser adapter. The branch
  now descends from the complete post-0.8 project history rather than an orphan root.
- Documentation continuity. Root documentation, licenses, ADRs through ADR-0101, research, trust
  material, design records, task ledgers, public surfaces, and product identity are retained.
- Fringe stability. The complete workbench and harness-management feature is implemented without
  changes to the MCP connector, browser connector, shared bridge, or extension. The later
  demand-start work changes the bridge and both connectors at the connection-lifetime seam, which
  is the one reason those fringes are allowed to change; the extension remains untouched.
- One place authors what Ghostlight says (ADR-0103). Completed-action sentences, safe next steps,
  and content-free measurements now come from one typed outcome in the language module instead
  of string literals spread through the executor.
- A language that teaches. Every validation message, tool description, refusal, and result
  guidance was revoiced: validators name the allowed set and the received value, refusals lead
  with the recovery action, and invalid-input results carry the specific expectation as their
  next step instead of a circular instruction.
- Dialogs are probed, not trusted. browser_dialog attempts Page.handleJavaScriptDialog directly
  and reports a typed dialog_absent outcome, so a dialog that opened while the debugger was
  detached is handled instead of being invisible forever.
- Per-client MCP revisions. The edge negotiates the revision each harness needs, including
  2025-03-26 for Junie and the 2026-07-28 discovery fallback for Antigravity, and serves portable
  top-level schemas where a downstream model API rejects composed ones.

Fixed
-----

- Release frees tabs for reuse. Workspace release notifies the extension to forget released tab
  ids, so the reuse ladder works end to end instead of leaving reaped tabs bound in topology.
- Drag keeps its held-button lane. The drag path restored ADR-0088's bounded two-lane seam:
  explicit held-button packets, action-scoped native interception, opaque drag-data replay, and
  cleanup on every terminal path.
- Packaged Windows desktop regressions. Release launches expose no console, and the workbench is
  created when the native event loop is ready instead of being lost just after startup.
- Extension reload no longer leaves false errors. The disconnect handler consumes Chrome's
  callback-scoped runtime.lastError before its stale-port ownership guard, so an expected
  replacement cannot leave a false extension error.
- Harness roster compatibility. Zed's zeditor launcher on CachyOS/Arch is detected beside zed,
  and Kiro/Bedrock receives portable schemas the typed decoder still validates exactly.

Removed
-------

- Superseded implementation code. The old root binary, core, transport, lightbox, extension
  mechanisms, test suites, release scripts, and packaging launchers remain in Git history but no
  longer coexist with the 1.0 implementation.
- Removed command forms. ghostlight service and ghostlight --headless are gone; every supported
  orchestrator start initializes the desktop authority.
- 0.8 behaviors deliberately not restored (ADR-0133). Old tool names, narration prose,
  destructive diagnostics, client plan mutation, and direct UDP syslog audit stay retired.

Trust model
-----------

Ghostlight never phones home. There is no telemetry, activation service, update ping, or hidden
network dependency. Artifacts are checksum-bound and carry keyless GitHub build-provenance
attestations; there is no platform code-signing certificate to trust. Every artifact's exact
length and SHA-256 are bound in the release-candidate.json and SHA256SUMS files attached to this
release.

License: Apache-2.0 OR MIT.

Full changelog: https://github.com/sylin-org/ghostlight/blob/v1.0.0/CHANGELOG.md
