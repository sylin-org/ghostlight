# STATUS -- Ghostlight 1.0 source candidate

Last updated: 2026-08-13 (fourteenth pass).

This is the mutable implementation snapshot. Git history, the ADR index, dated research, and the
preserved `docs/0.8/` material carry history; this file does not rewrite it.

## Where the branches stand

Distances below are measured against the local remote-tracking refs, which are only as fresh as the
last fetch.

The repository carries exactly two branches, `main` and `dev`, as of 2026-08-13. The topology is
linear: `main` is an ancestor of `dev`, and nothing anywhere needs merging.

- `dev` is the working branch and the 1.0 source candidate. Workspace version `1.0.0`. It absorbed
  `ghostlight-1.0`, which was a fast-forward and has been retired.
- `main` carries the 0.8 line at `0116feca`. Promoting it is a deliberate release decision, not
  routine sync. The 1.0 line now carries adapted three-platform source, extension, process, and
  supply-chain CI; a manual Pages deployment; and bounded monthly dependency updates targeting
  `dev`. A manual build-only workflow now creates and inspects unsigned native-package candidates
  without publishing them. Signing, native operating-system validation, and publication remain
  owed. Do not promote `dev` before those live gates pass.
- No pull requests are open. Thirteen Dependabot bumps against the 0.8 line were closed as obsolete
  on 2026-08-13: the 1.0 tree either already carried the proposed version or had dropped the
  package outright (`clap`, `rustls`, `webpki-roots`, `color_quant`). Dependency updates are paused
  on `main` with `open-pull-requests-limit: 0` rather than by deleting the configuration. The 1.0
  config targets `dev`, runs monthly, groups non-major updates, and caps open work per ecosystem.
- The pre-1.0 worktree snapshot is preserved as the annotated tag `archive/0.9-pre-1.0`
  (`f5d43768`), pushed to the remote. It replaced a local-only branch that existed on one machine.
  It is history, never implementation authority for the 1.0 tree.
- The 0.8 recovery is now source-backed rather than implicit. `docs/0.8/HARVEST.md` distinguishes
  the released, reconciled, mature, and archived snapshots; `docs/0.8/test-inventory.json` records
  1,355 ordinary test declarations and 34 source-enumerated Lightbox scenarios; and the dated
  publication observation corrects WinGet to merged while recording Glama drift. The old ledger's
  unexplained claim of 37 Lightbox scenarios is preserved as a discrepancy, not repeated as fact.
- Release safeguards are active again on `dev`: Rust and extension CI cover all three operating
  systems; process journeys cover all three; dependency licenses, sources, wildcards, and
  advisories are gated; source and observed-public versions are checked separately; and the store
  extension package is built from an explicit runtime allowlist. The online public check passed
  against GitHub, npm, Chrome, the official MCP Registry, and sylin.org on 2026-08-13. These local
  workflow files have not run on GitHub until they are pushed.
- Packaged native-host lifecycle is restored without restoring the 0.8 resident supervisors
  (ADR-0115). The orchestrator now checks, installs, updates, and safely removes Chrome, Edge,
  Brave, and Chromium registrations; packages carry both connector sidecars; and narrow migration
  retires only recognized pre-1.0 Run/task, launchd, or systemd artifacts. The unsigned Windows
  NSIS candidate built locally and its payload inspection found exactly the three required
  executables. Linux and macOS package builds plus all clean-install, upgrade, reboot, and uninstall
  journeys remain required native-host evidence.
- The 0.8 test recovery is now dispositioned rather than merely counted.
  `docs/0.8/RECOVERY-MATRIX.md` maps all 1,389 entries through twelve current behavior areas;
  `docs/0.8/test-recovery.json` gives each of the 34 Lightbox process scenarios an explicit
  reexpressed, superseded, invariant-retained, or deferred state; and CI checks the map against the
  source-derived inventory. Two missing high-value proofs were added: sibling runtime discovery
  does not depend on Linux session environment, and an unreachable configured managed authority
  fails closed from cold start.

## Implemented

- One Rust 2021 workspace builds four roles: the shared typed bridge, `ghostlight` orchestrator,
  generic MCP connector, and opaque browser connector.
- The orchestrator owns the 22-tool model-facing catalog, workspace aggregate, one executor and
  completion path, immutable authority snapshots, runtime controls, content-minimized audit,
  browser port, and content-free presentation decisions.
- The page-context JavaScript tool is `browser_execute`, not `browser_evaluate`. The execute name
  states that it may read, mutate, or navigate. The unreleased old name has no alias. Internal
  `RunScript` and `EvaluateScript` mechanism names remain behind the language boundary.
- The stable browser fringe includes a policy-free Manifest V3 extension, durable native relay,
  operation-disposition recovery, one browser-wide exact-title group per client label, dedicated
  Ghostlight window placement, and the established visual language and product identity.
- Adapter 1.0.0 advertises end-to-end liveness (ADR-0113). The service sends a content-free
  heartbeat every 20 seconds and follows every physical dispatch with its own probe. Forty-five
  seconds without an acknowledgement makes an attached relay unavailable; an operation deadline
  with no post-dispatch acknowledgement quarantines it immediately, so the next call stops before
  dispatch. A healthy silent operation stays connected when the extension answers independently.
  Older adapters retain their capability-gated attachment behavior, and the opaque browser
  connector is unchanged.
- Browsers are plural (ADR-0114). The service keeps one adapter connection per persistent browser
  identity, so Chrome and Edge, or two profiles, are connected and worked in at once. A hello
  carrying an identity that is already registered replaces that entry and **closes the replaced
  stream**, which is what makes a duplicate connection collapse instead of lingering as a silent
  sink. Each workspace binds to one browser for its life; physical tab ids resolve as
  `(browser, physical_id)`, so one browser's tab 5 can never be governed as another's. A crossing
  with no binding uses an explicit `browser`, then reported attention, then the sole connected
  browser, and otherwise refuses while naming the candidates. Runtime control publishes to every
  connected browser.
- Extension native-host startup is single-flight. Concurrent bootstrap, installation, startup,
  and reconnect signals share one attempt, and ownership is rechecked after local-state
  initialization. One worker epoch therefore cannot strand multiple attached relays with only one
  active extension listener.
- Recording now has one owner (ADR-0108, extended by ADR-0109). The extension keeps a plural,
  workspace-namespaced, memory-only registry; owns capture ids, frames, fixed bounds, autonomous
  stop, five-minute retention, erase, and the GIF encode itself; and exposes only
  start/status/stop/export/discard physical requests. It folds byte-identical successive JPEGs into
  one retained visual span with an accumulated duration, so capture time and compressed bytes are
  the ordinary limits. During recording, presentation disables only the perpetual controlled-scope
  glow and keeps transient action feedback available. The old service coordinator, renewal loop,
  unsolicited frame events, and duplicate deadlines are gone.
- Recording frames no longer cross a process boundary (ADR-0109). `gif_output.rs`, the
  frame-returning `read` command, and `PhysicalRecordingFrame` are deleted. The orchestrator
  governs the save, names one of three destinations, and states an output budget; the extension
  encodes in an offscreen document (pinned MIT `gifenc` under `extension/vendor/`) and delivers.
  A page attach and a browser download finish inside Chromium; only a client return carries bytes,
  and the shape of `RecordingDelivery` is what makes that structural rather than a rule. Thinning
  exists once, in `extension/lib/recording.js`, and folds each dropped frame's time into the frame
  before it, so a thinned replay still plays for as long as the work took. A saved replay's
  sentence reports how long it plays; counts and bytes stay in the facts. The manifest gained
  `offscreen` and `downloads`, which is a published-surface change.
- Model-driven tab close is admitted by service authority and then checked by the extension's
  default-on preserve-tabs interlock. A refusal stays visible and returns a blocked no-effect
  result.
- The `ghostlight` executable hosts a Tauri 2 workbench inside the modular monolith, with a tray
  lifecycle, bounded global search, and content-free native notifications. It presents three
  destinations:
  - **Monitor**, the landing surface. The current action stands in full with its elapsed time,
    then settles and drops into a newest-first queue as the next one rises. Connected sessions and
    browser instances sit alongside it, and the last completed action stays on screen while
    nothing is running.
  - **MCP integrations**, which checks, connects, and disconnects Ghostlight's owned registration.
  - **Status**, which carries diagnostics, authority sources, and the end-session intent.

  Pause and resume live in the persistent header beside the connection state and match the tray.
- The orchestrator publishes a closed sequenced change vocabulary (`OperationStarted`,
  `OperationChanged`, `OperationSettled`, `RuntimeChanged`) through a best-effort
  `WorkbenchEventSink`. Snapshots carry the sequence they reflect; a surface that receives a gap
  resynchronizes from a fresh snapshot rather than trusting its cache. The WebView may listen and
  is not granted permission to emit. A projection with no sink attached publishes nothing, so
  headless runs and domain tests stay free of presentation.
- `OperationSummary` carries the governed capability, so live work is classified as plainly as
  completed history.
- Monitor has a presentation-only Clear view control. It hides completed actions for the current
  desktop surface, keeps running work visible, and never mutates or deletes the durable audit.
- The workbench follows the published sylin.org palette: Ghostlight's teal accent carried as
  `--a`/`--al`/`--argb`, the night-garden ground, and the five-step ink ramp. The in-page renderer
  deliberately keeps its trained sky signal. The two surfaces still share the spring curve and the
  ADR-0083 medallion vocabulary.
- Supported MCP client registrations are Codex, Claude Code, Claude Desktop, Cursor, Visual Studio
  Code, Windsurf, Zed, OpenCode, and Crush. Re-check is read-only. Connect and disconnect are
  explicit, serialized, ownership-checked, backed up, and preserve unrelated JSONC/TOML comments
  and configuration.
- `ghostlight --headless` retains the service-only execution path. Recoverable desktop startup and
  event-loop failures leave that service running.
- The shared bridge owns one demand-start seam used by both connectors after a failed service
  connection. It starts only the exact sibling `ghostlight` with no application arguments, honors
  a fresh deploy lock, and preserves each connector's established reconnect behavior.
- The orchestrator holds an operating-system lifetime lease before publishing runtime discovery or
  initializing Tauri. Concurrent launch attempts therefore converge on one authority and one tray.
- There is one normal desktop launch. It always creates the tray and shows its workbench minimized.
  A second direct launch restores and focuses the existing authenticated workbench. `--headless`
  remains the explicit presentation-free mode.
- `ghostlight call` is a second intake for scripts and programs (ADR-0105). It invokes one tool, or
  a batch of them over one session with `--stdin`, prints the outcome sentence or `--json`, and maps
  the terminal status to distinct exit codes where an uncertain effect is never zero. It demand-
  starts the authority like any connector, and it crosses the same executor, governance facade, and
  completion path, so there is no scripting bypass.
- Every session records the intake it arrived on, and every audit record carries it. `ghostlight
  call` work is attributed to the `cli` channel and grouped under its own browser tab-group name.
  The channel is attribution and is never an input to an authority decision.
- `ghostlight_bridge::client::ServiceClient` is the one place the service handshake lives, so a
  second edge does not grow a second copy of it.
- `--output <file>` writes bounded content, so a scripted capture lands as an image rather than as
  base64 in a terminal. Later captures in one session gain an index instead of overwriting.
- A policy layer may close an intake: `{"channels":{"cli":{}}}` refuses it, `{"enabled":true}`
  admits it, and an absent map restricts nothing, so all-open is untouched. Layers intersect, so a
  managed refusal cannot be undone locally, and an unknown channel name is a typo that fails closed.
  The refusal lands at admission with the stable `channel_denied` reason, before a workspace exists,
  so nothing is invoked and nothing is audited (ADR-0105 amendment).
- A command-line session is its caller, not its connection (ADR-0106). Every `ghostlight call` from
  one terminal, or from one program that shells out repeatedly, reaches the same workspace and the
  same tabs. Identity is the caller's process id plus start time, so a recycled pid running the same
  program does not inherit a dead session; the executable name rides along for attribution only.
  `GHOSTLIGHT_SESSION` pins a session explicitly for a caller whose own children are ephemeral, and
  is a claim rather than an observation, so it never reaches an authority decision.
- An owned workspace outlives its connection and is released when its owner is gone, handing back
  the tabs it held. The close it then asks for goes through the same interlock a model's close does,
  so with the default-on preserve-tabs setting those tabs are released but stay visible. Liveness is observed rather than guessed at, sweeping on admission so the cost follows
  use. Work in flight is never reaped, and a connection that sends no marker keeps the previous
  connection-bound behavior, which is what the MCP edge does.
- [`scripts/demo-brief.ps1`](../scripts/demo-brief.ps1) drives the ADR-0069-era launch-brief demo
  story entirely through the command line: open, scan, inventory controls once, three separately
  paced field writes, two checkbox clicks, submit, and a wait for the exact completion sentence.
  Verified live against the published Sylin stage: ten steps, one session, read/write/action
  capabilities classified per tool, and no typed value in the audit. `docs/design/demo-brief.md`
  specified this as a Rust subcommand; it does not need to be one, and the note now says so.
- [`scripts/browser-journey.ps1`](../scripts/browser-journey.ps1) is a complete PowerShell journey
  over the CLI: open, list, read, capture to a file, close, with a non-zero exit if any step fails.
  It holds one `--stdin` session open and writes a line at a time, so each step uses the handle the
  previous one returned.
- Monitor rows carry the intake between the tool and the description, resolved from the record when
  settled and from the still-connected session while running. A guard derives the row's grid track
  count and each width's hidden cells from the stylesheet and compares them to the cells the surface
  renders, so a new column cannot silently shift the ones after it.

## Verified in this workspace

Re-run on 2026-08-13 against the current tree:

- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace`: 184 Rust tests -- 149 in the orchestrator library, its launch-mode
  binary test, 30 in the shared bridge, and 4 in the MCP connector.
- `npm test --prefix extension`: 99 extension tests.
- Plurality contracts prove two browser identities stay connected at once and each answers its own
  request, a second connection from one identity collapses onto the first with the replaced stream
  reaching end-of-stream rather than hanging open, attention moves to front without duplicates and
  never routes to an absent browser, and resolution prefers selection, then binding, then attention,
  then a sole browser. Executor contracts prove work follows the attended browser once and then
  stays there when attention moves, an ambiguous bootstrap names both candidates with no dispatch
  and no binding, a named stranger is refused rather than substituted, and listing tabs answers
  truthfully with no browser connected at all.
- Browser-port contracts prove an attached socket without heartbeat acknowledgements becomes
  unavailable, an unanswered post-dispatch probe quarantines at the operation deadline, a legacy
  adapter keeps compatible attachment semantics, and a silent operation can outlast the liveness
  timeout while independent acknowledgements keep it available.
- Lifecycle tests prove demand-start supplies no application arguments and the executable has one
  normal desktop mode beside explicit headless and scripted intake. The real process journey still
  passes across service restart and connector renegotiation.
- Action-subject tests prove the Chrome receipt carries the physical role and name without a
  describe round trip, the role cannot author language, editable values cannot become names, names
  are normalized and bounded, and either authority layer can remove them monotonically.
- `node tests/cli-powershell-journey.mjs`: the shipped PowerShell script drives a real service and a
  scripted browser adapter through open/list/read/capture/close, exits zero, writes real JPEG bytes,
  and every step is audited as `cli` with the landing host and no page text. It then proves the
  session marker across processes: one `ghostlight call` opens a tab and a separate one lists it.
- `node tests/cli-journey.mjs`: the real executable's command line reaches a real service, returns a
  governed result, exits non-zero on refusal, is attributed to the `cli` channel in the audit file
  the service wrote, and keeps one workspace across a `--stdin` batch while separate processes get
  separate workspaces. A second service started with `{"channels":{"cli":{}}}` refuses the intake
  with `channel_denied`, exits non-zero, and writes no audit record.
- `node tests/process-journey.mjs`: stable MCP and browser relays reconnect through a service
  restart without replaying an interrupted effect, then complete open/read, an extension-owned
  recording start/save/discard with a real GIF content block, a second save to the browser's
  download mechanism that returns no bytes at all, and close. Its adapter advertises liveness and
  acknowledges every dispatch probe through the unchanged opaque browser connector. The journey uses
  a fresh deployment lock to isolate explicit restart recovery from demand-start. It also reads
  the audit file the real executable wrote and checks that the read records a host and a word
  count, and no page text.
- `cargo build --workspace --target-dir .target-ghostlight-1.0`.
- `node --check` on both journeys, the bundled workbench script, and the preview server.
- A live 35-second static Example Domain recording with the scope glow suppressed retained 15
  frames and 121,293 JPEG bytes. Its 211,458-byte GIF was valid GIF89a, carried 35,320 milliseconds
  of playback with a 33,720-millisecond folded static span, and repeated save was byte-identical.
- A live Foundry hover, click, and type sequence retained six distinct frames across 670
  milliseconds. Its 595,861-byte GIF saved twice with the same SHA-256 digest, after which the demo
  state and recording bytes were cleared.
- Live isolated demand-start proofs began with no service. The MCP connector started one exact
  sibling authority and completed MCP initialization. In a separate run, the browser connector
  reported `backend_unavailable`, started one exact sibling authority, and completed its adapter
  hello. Each run found exactly one service at the isolated executable path and removed only that
  test-owned process afterward.
- The repository's live Windows `target/release` stack was replaced from an isolated release
  build under the deploy lock. Stopping its one service authority caused the already-running
  browser connector to demand-start one replacement with a fresh runtime token. A direct launch
  then revealed that workbench and exited while the authority count stayed one.
- The workbench renders against the repository preview server, which now drives the real sequenced
  event path, and uses the byte-identical original Ghostlight artwork.
- Guard tests keep the surface and the orchestrator in step: every publishable change has a
  handler, every capability class has a visual treatment, every runtime intent stays reachable
  (guarded by an exhaustive match), the surface reads the one observed fact no sentence states and
  renders the sentence for the rest, every observed fact is documented where collectors read it,
  every outcome measurement agrees with its sentence, every readiness has a note, the published
  palette
  is present with the accent defined once, the workbench capability grants listen without emit,
  and every catalog tool has a medallion. Each of these was checked against a negative control:
  breaking the thing it guards makes it fail.
- Outcome-language oracles cover every success sentence, every refusal sentence, workspace reason
  mapping, number grouping, safe next steps, sentence/measurement agreement, and the unchanged
  `Observed` JSON round trip. Executor tests prove the browser seam records landing facts without
  counts and the completion path still combines host/readiness with the outcome measurement.
- The complete desktop-workbench change, from its starting revision through the live-monitor
  rebuild, has an empty diff under `crates/mcp-connector`, `crates/browser-connector`,
  `crates/bridge`, and `extension`. The later demand-start lifecycle intentionally changes the
  bridge and both connectors at their connection-lifetime seam; the extension remains unchanged.

## Visual language and monitor content

- Both surfaces share one motion vocabulary. The workbench names its beats as `--beat-*` tokens
  taken from the renderer's frozen `visualIdentity`, so a treatment meaning the same thing in the
  page and the window keeps the same tempo.
- The in-page effect registry (`TRANSIENT_EFFECTS`) owns both reduced-motion enrollment and each
  treatment's beat, and teardown derives from the beat. No effect lifetime is hand-picked.
- The renderer stylesheet is static CSS: identity arrives once as custom properties, leaving only
  the token block and the generated reduced-motion selector interpolated.
- A click describes itself end to end. `ClickShape { clicks, button }` rides on
  `PresentationSignal`, and the renderer draws one ring per click, dashed for a secondary button.
- Audit records and workbench history carry the Ghostlight-authored `summary` and a measured
  `duration_ms`, so every row states what happened and how long it took.
- Outcome language now leads with the action and names the governed place. Browser action receipts
  return the role and accessible name of the physical element in the same effect response, without
  a describe round trip. The orchestrator narrows raw roles to a closed noun, normalizes and bounds
  names to 80 characters, and produces sentences such as `Clicked the "Save" button on
  example.com`. `preserve_target_names` defaults to true; false in either authority layer removes
  names monotonically and leaves `Clicked a button on example.com`. Editable values never supply a
  name. A refused explicit navigation adds only its normalized host to the existing observation
  shape, never its path, query, fragment, or value. Rendered label whitespace is normalized before
  the name is retained, so visually separate label fragments cannot collapse in the audit sentence.
- Per-action observation is built, at the seam it was designed for. See
  [`design/action-observations.md`](design/action-observations.md).
  - `language/outcome.rs` owns `Outcome`, `Refusal`, `WorkspaceReason`, and
    `Observed { host, readiness, count, width, height }`. Every successful completion requires an
    `Outcome`, so its Ghostlight-authored sentence, safe next steps, and named measurements cannot
    drift into separate call-site strings.
  - `Executor::dispatch` remains exhaustive over browser outcomes and gathers host/readiness keyed
    by invocation. `Outcome::observed` supplies counts and capture sizes from the same value that
    authored their sentence. The one completion path merges the outcome over the seam and clears
    the registry.
  - The host and optional governed action-target name are the deliberate line. Never the path,
    query, fragment, selector, target handle, entered value, or arbitrary page text. A capture reports its
    pixel size, a wait reports how long it waited and which condition it waited on, and a read
    reports how many words it read.
  - A count is recorded only where the Ghostlight-authored sentence beside it names what was
    counted, so the count needs no per-tool wording table on the surface. Those summaries now state
    their measurement: "Read 1,240 words from example.com.", "Filled 3 fields on example.com and
    submitted the form.", "Found 7 matches on example.com.", "Captured the viewport at 1280x720."
  - Rows always render the outcome sentence and add a readiness note where a document never
    settled. They no longer guess between host and measurement, because the orchestrator already
    chose which register the sentence uses. The hero renders the same sentence and carries no host
    chip: the sentence names the host, so a chip would say it twice. Readiness is the one observed
    fact no sentence states, and it is the only one the surface reads structurally. The host is
    guarded where it is collected, in
    [`guides/siem-integration.md`](guides/siem-integration.md), because that guide is what a
    person configuring a collector reads.
  - The audit stays content-minimized. `InvocationResult::facts` still carries page text and full
    URLs to the model; the observation is a separate closed type so there is no shortcut between
    them. The bounded action-target name exists only inside Ghostlight's terminal summary and may
    be removed by governance.
    [`guides/siem-integration.md`](guides/siem-integration.md) now documents `summary`,
    `duration_ms`, and `observed`, and states the host exception where it used to claim that no
    host is ever recorded.

- A service session used to outlive its connection whenever that connection ended badly. The
  request loop propagated read errors, oversized frames, and malformed lines out of the handler
  before the release ran, and an unowned workspace has no owning process for the reaper to check,
  so nothing could collect it afterwards: the workspace and every tab it held survived until the
  service restarted. A live workbench showed 17 sessions against 5 connectors. The teardown now
  runs on every exit path, guarded by a test that fails when the old early return is put back.
- The workbench connections bar groups its chips by client label, with a tally when one client
  holds more than one session. The sessions array itself is untouched, because history attribution
  resolves a single workspace to its client by id.

- The workbench surface is hardened and, for the first time, actually executed by a test.
  `node tests/workbench-surface.mjs` runs `app.js` against a minimal DOM with one panel broken on
  purpose and asserts the window still comes up, the failure is visible, the rest of the pass
  continues, and the broken panel is retried rather than memoised as done. Every other guard over
  this window reads its source as text, and none of them could tell that the window never started.
- Four fragilities behind that failure are fixed: the element table is derived from the document
  instead of hand-listed; boot is one ordered sequence that installs its own recovery first;
  wiring is an isolated step rather than loose statements ahead of boot; and a render failure is
  reported as itself instead of as a lost connection.

- The workbench surface is rebuilt around its seams. It was one 1045-line file where vocabulary,
  cache, rendering, transport and wiring were the same thing; it is now `ui/lib/words.js` and
  `ui/lib/entries.js` (pure), `ui/lib/transport.js` (the only caller of the orchestrator),
  `ui/lib/store.js` (the cache and its only writer, announcing a closed set of seven change
  kinds), `ui/lib/view.js` (the only thing that touches the document), and `ui/app.js` as a
  296-line composition root. Data flows one way: transport brings a snapshot, the store folds it
  in and announces, the view draws what it is handed. A view that cannot fetch cannot fail on a
  missing snapshot; a store that never sees the document cannot be corrupted by a paint.
  Guards hold the seam: words, entries, store and transport fail the build if they contain
  `document.`, `window.` or `el[`, with the view as the negative control.

## Owed

- A row that never settled reads its readiness as a parenthetical. Colour would carry it better
  than words: the duration cell already has a running and a blocked treatment, and an unsettled one
  would be found while scrolling instead of read for.
- ADR-0105 stages 2 and 3 are blocked on an owner decision, recorded in that ADR's amendment.
  Identifying the socket peer and verifying a signature are both raw Win32 FFI, and the workspace
  sets `unsafe_code = "forbid"`, which no scoped `#[allow]` can override. The choice is to relax
  that invariant for one audited module or to depend on a wrapper crate on a security-sensitive
  path. Until then the channel stays attribution: the `channels` switch decides whether an intake
  may open a session, which is a weaker claim than knowing who is calling.
- `crates/mcp-connector` still has its own copy of the service handshake and did not adopt
  `ServiceClient`. One home exists now; the connector should move to it.
- The extension stylesheet could move to its own module now that it is static. Lowest value of the
  maintainability steps; needs about eight test assertions reworked.
- GIF quality remains deferred. The vendored encoder quantizes each frame to its own 256-colour
  palette with no dithering, which suits flat interface pixels and not photographs. Overlays,
  action tagging, and perceptual palettes are still unbuilt. Output size is no longer the pressure
  it was: a browser-local save may spend 16 MiB, and anything over its budget is thinned rather
  than refused.
- `origin/main` still carries 0.8. Deciding when the 1.0 line is promoted is a release decision.
- ADR-0084's complete browser-window attention routing remains deferred; only the narrow Chromium
  slice is implemented.

## Release gates still requiring an owner or release environment

- Produce and sign platform bundles, install the native-messaging registration, and verify upgrade
  and uninstall from a clean machine on Windows, macOS, and Linux.
- Complete interactive native-window, tray, and notification smoke tests on each platform. The
  automated environment verifies native build and failure containment but does not expose its GUI
  desktop to the test runner.
- Verify demand-start, direct workbench activation, and deploy quiesce from each clean signed
  platform installation.
- Run the accepted browser-job matrix against visible supported Chromium browsers, including
  screenshots, file upload, form input, dialogs, governed denial, reconnect, and local close
  interlock journeys.
- Reconcile release metadata, public status, store submission, compatibility, distribution, and
  the final public documentation only when the 1.0 artifacts exist.
- Keep the checked 0.8 recovery matrix aligned as current proofs move. Its remaining live-package
  and visible-browser rows are covered by the release gates above.

## Canonical 1.0 sources

- Product intent: [`1.0/INTENT.md`](1.0/INTENT.md)
- Model-facing language: [`1.0/LANGUAGE.md`](1.0/LANGUAGE.md)
- Architecture: [`1.0/ARCHITECTURE.md`](1.0/ARCHITECTURE.md)
- Acceptance: [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md)
- Desktop decision: [`adr/0102-integrated-desktop-workbench.md`](adr/0102-integrated-desktop-workbench.md),
  including its 2026-08-11 amendment for the live monitor, the published palette, and the
  three-destination workbench.
- Outcome language decision:
  [`adr/0103-language-owned-outcome-voice.md`](adr/0103-language-owned-outcome-voice.md).
- Demand-start and single-engine decision:
  [`adr/0104-demand-start-single-engine-and-workbench-activation.md`](adr/0104-demand-start-single-engine-and-workbench-activation.md).
- One minimized desktop-startup decision:
  [`adr/0112-one-minimized-desktop-startup.md`](adr/0112-one-minimized-desktop-startup.md).
- End-to-end browser availability decision:
  [`adr/0113-end-to-end-browser-adapter-liveness.md`](adr/0113-end-to-end-browser-adapter-liveness.md).
- Plural browser adapters and routing decision:
  [`adr/0114-plural-browser-adapters.md`](adr/0114-plural-browser-adapters.md).
- Packaged native-host lifecycle decision:
  [`adr/0115-packaged-native-host-lifecycle.md`](adr/0115-packaged-native-host-lifecycle.md).
