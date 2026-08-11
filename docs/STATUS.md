# STATUS -- Ghostlight 1.0 source candidate

Last updated: 2026-08-11 (fifth pass).

This is the mutable implementation snapshot. Git history, the ADR index, dated research, and the
preserved `docs/0.8/` material carry history; this file does not rewrite it.

## Where the branches stand

Distances below are measured against the local remote-tracking refs, which are only as fresh as the
last fetch.

- `ghostlight-1.0` is the working branch and the 1.0 source candidate. Workspace version `1.0.0`.
- `origin/dev` was fast-forwarded onto the 1.0 line on 2026-08-11 and again on 2026-08-11 to
  `89bcbe6b`, which carries per-action observation, the language-owned outcome voice, demand-start,
  and this documentation pass. `ghostlight-1.0` and `origin/dev` match exactly. Before the first
  fast-forward `origin/dev` sat at the 0.8 line (`3fb093eb`, 2026-08-07).
- `origin/main` still carries the 0.8 line at `95468758`, now 32 commits behind `origin/dev`.
  Promoting it is a deliberate release decision, not routine sync. (An earlier pass recorded the
  distance as 24 when `git rev-list --count origin/main..origin/dev` said 26. Re-measure rather
  than carry the number forward.)
- 13 pull requests are open. All 13 are Dependabot dependency and action bumps; none are human
  contributions awaiting review. They target the 0.8 line and have not been reconciled against the
  1.0 rebuild.

## Implemented

- One Rust 2021 workspace builds four roles: the shared typed bridge, `ghostlight` orchestrator,
  generic MCP connector, and opaque browser connector.
- The orchestrator owns the 24-tool model-facing catalog, workspace aggregate, one executor and
  completion path, immutable authority snapshots, runtime controls, payload-free audit, browser
  port, and content-free presentation decisions.
- The stable browser fringe includes a policy-free Manifest V3 extension, durable native relay,
  operation-disposition recovery, one browser-wide exact-title group per client label, dedicated
  Ghostlight window placement, and the established visual language and product identity.
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
  connection. It starts only the exact sibling `ghostlight --background`, honors a fresh deploy
  lock, and preserves each connector's established reconnect behavior.
- The orchestrator holds an operating-system lifetime lease before publishing runtime discovery or
  initializing Tauri. Concurrent launch attempts therefore converge on one authority and one tray.
- Direct/default launch and `--show` reveal the existing authenticated workbench or start it
  visibly. `--background` starts the full tray authority hidden. `--headless` remains the explicit
  presentation-free mode.

## Verified in this workspace

Re-run on 2026-08-11 against the current tree:

- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace`: 99 Rust tests -- 81 in the orchestrator including its launch-mode
  binary test, 15 in the shared bridge, and 3 in the MCP connector.
- `npm test --prefix extension`: 39 extension tests.
- `node tests/process-journey.mjs`: stable MCP and browser relays reconnect through a service
  restart without replaying an interrupted effect, then complete open/read/close. The journey uses
  a fresh deployment lock to isolate explicit restart recovery from demand-start. It also reads
  the audit file the real executable wrote and checks that the read records a host and a word
  count, and no page text.
- `cargo build --workspace --target-dir .target-ghostlight-1.0`.
- `node --check` on the process journey, bundled workbench script, and preview server.
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
  - The host is the deliberate line. Never the path, query, or fragment. A capture reports its
    pixel size, a wait reports how long it waited and which condition it waited on, and a read
    reports how many words it read.
  - A count is recorded only where the Ghostlight-authored sentence beside it names what was
    counted, so the count needs no per-tool wording table on the surface. Those summaries now state
    their measurement: "Read 1,240 words.", "Filled 3 fields and submitted the form.", "Found 7
    matches.", "Captured the viewport at 1280x720."
  - Rows always render the outcome sentence and add a readiness note where a document never
    settled. They no longer guess between host and measurement, because the orchestrator already
    chose which register the sentence uses. The hero renders the same sentence and carries no host
    chip: the sentence names the host, so a chip would say it twice. Readiness is the one observed
    fact no sentence states, and it is the only one the surface reads structurally. The host is
    guarded where it is collected, in
    [`guides/siem-integration.md`](guides/siem-integration.md), because that guide is what a
    person configuring a collector reads.
  - The audit stays payload-free. `InvocationResult::facts` still carries page text and full URLs
    to the model; the observation is a separate closed type so there is no shortcut between them.
    [`guides/siem-integration.md`](guides/siem-integration.md) now documents `summary`,
    `duration_ms`, and `observed`, and states the host exception where it used to claim that no
    host is ever recorded.

## Owed

- A row that never settled reads its readiness as a parenthetical. Colour would carry it better
  than words: the duration cell already has a running and a blocked treatment, and an unsettled one
  would be found while scrolling instead of read for.
- Refusal sentences were deliberately left unchanged when the voice moved into
  `language/outcome.rs`, so they are the last boilerplate on the monitor. "Authority blocked the
  browser job." could name the host the seam already recorded.
- The extension stylesheet could move to its own module now that it is static. Lowest value of the
  maintainability steps; needs about eight test assertions reworked.
- The 13 open Dependabot pull requests target the 0.8 line and need reconciling against the 1.0
  rebuild before they can land.
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
