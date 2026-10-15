# Break 2.0: shell milestone 1 ledger

Milestone 1 (M1) of the desktop shell track carries the 2026-10-15 amendment of
[ADR-0199](../../adr/0199-one-service-thin-shells.md): the Ghostlight web apps over loopback.
Slices m1-01 to m1-12 replace the private scheme and the page bridge with the existing workbench
served on a loopback port, with access decided in context, one live stream and a shared Controls
sheet. Tauri stays inside the service for the whole milestone and its window keeps working. The
milestone ends with the coordinator's live demo with the owner, in three parts: the workbench in
the Ghostlight window, the same URL watching in a Chrome tab, and watching turned off with the
About page still showing in full. The rest of the shell track is planned after that demo.

Gate, from the repo root on `dev`, in Git Bash, with
`CARGO_TARGET_DIR=E:/repo/github/sylin-org/ghostlight/.target-lane-shell-m1`:

- `cargo fmt --check -p ghostlight-service`, plus `-p ghostlight-desktop` when `crates/desktop`
  changed and `-p ghostlight` when `crates/edge` changed (fix only with `cargo fmt -p <package>`,
  never a bare `cargo fmt`).
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
- When anything under `crates/orchestrator/ui`, `tests/workbench-*.mjs` or
  `tests/workbench-preview-server.mjs` changed: `node --test tests/workbench-transport.test.mjs`
  (from m1-07), `node --test tests/workbench-controls.test.mjs` (from m1-08) and
  `node tests/workbench-surface.mjs`.
- `node --test "crates/desktop/guards/*.test.mjs"` when `crates/desktop`, any `Cargo.toml` or
  `Cargo.lock` changed.
- `node --test "tests/guards/*.test.mjs"` when `crates/edge`, any `Cargo.toml` or `Cargo.lock`
  changed.
- `node --check` on every changed `.js` and `.mjs` file.
- `pwsh -NoProfile -File scripts/check-repository-integrity.ps1`.

Never pass `--locked` to cargo build, clippy or test.

Rules:

1. The commit protocol is in [BOOTSTRAP.md](BOOTSTRAP.md).
2. One slice is one commit. Stage and commit only your own paths.
3. Never run a Ghostlight binary, a journey, an install, a carrier or `scripts/dev-loop.ps1`.
   The coordinator runs every live check; record each under Owed live checks.
4. Follow the tree when it differs from a spec, keep the spec's intent and record the difference
   under Notes. When the intent cannot be kept, record a blocker and stop.

## RESUME HERE

Next: m1-09-desktop-role.

## Landed

| Slice | What | Commit |
|---|---|---|
| m1-01-adr-amendment | The owner-approved amendment of 2026-10-15 is appended to ADR-0199 (the Ghostlight web apps over loopback), with its README row, this ledger, and the pointers from the shell-spike ledger and the task index. Docs only. | see `git log` |
| m1-02-harness-evidence | Harness evidence names a foreign program and its argument count, never its arguments; a configuration that cannot be parsed carries no evidence (owner answers Q3 and Q8). | see `git log` |
| m1-03-own-origin | Agents can never open or read the workbench's own loopback origin: `GovernanceFacade::protect_workbench_port` shares the port with every snapshot, and the landing decision refuses a loopback address on that port with `ProtectedHost` whatever the policy says. The authority id does not depend on the port. | see `git log` |
| m1-04-loopback-server | The service serves the workbench on 127.0.0.1 with access decided per route in three contexts (Control, Watch, Closed), each part projected for its audience, guards on Host, Origin and framing, and the bearer made at each start. `protect_workbench_port` runs between binding and answering. The Tauri window is unchanged. | see `git log` |
| m1-05-live-stream | One live stream, pushed to every screen. `GET /api/events` is served from an `EventHub` every screen subscribes to; the sequence advances whether or not anyone listens; a snapshot reads it first; `status_changed`, `connections_changed` and `part_changed` events keep the band, the sessions and the hinted parts current; the tray, the window, a watching screen and the browser toolbar change the runtime through `apply_runtime_control(intent, source)`, and the status says where a pause came from. Acts answer with `Ghostlight-Seq`. | see `git log` |
| m1-06-native-acts | The acts that need the operating system are service actions the window asks for over loopback: `WorkbenchNativeActs` (open a link, open a folder, pick a file) attached beside the presentation port, four facade methods on top of it, three Control-only routes, and a table of links in `/api/context` for every context. The Tauri window behaves as before; its four commands call the facade. | see `git log` |
| m1-07-page-transport | The page speaks loopback HTTP and renders its context. `transport.js` gains `overHttp` (one table maps each Tauri command name to a route) and the live stream with reconnect; `app.js` reads `GET /api/context` first and renders what it is told: Control as before, Watch as state with one Continue line per panel, Closed as one sentence per data view with the About page in full. The page drops the pull on every focus and the rescan on every focus; opening Integrations rescans once. The Tauri window keeps its path and gains `workbench_context`, `set_diagnostics` and `continue_in_window`. | see `git log` |
| m1-08-controls-sheet | One shared Controls sheet on the band of every Ghostlight web app (`ui/kit/controls.js`, `ui/kit/kit.css`): "On this screen" (Density, Motion, kept in the browser) and "For Ghostlight on this computer" (Pause and the session, "Others can watch", process diagnostics, the notification test). The service gains the `controls` part, `WorkbenchChange::ControlsChanged`, `WorkbenchFacade::set_watching` (the only way watching changes; while it is off every watching stream ends with `closed`), `connection_counts` and `service_line`; the Status page loses its session, notification and process-diagnostics strips. | see `git log` |

## Route table

Server: `S/workbench/web/routes.rs`. Pages answer every context (a wrong bearer is 401 even there).
Control carries the bearer, Watch has none while watching is on, Closed has none while it is off.
A watcher on a route marked "window" gets 401 `needs_window`; Closed gets 403 `watching_off` on
every route but the pages and `/api/context`.

| Path | Method | Control | Watch | Closed | Watch limit | Replaces |
|---|---|---|---|---|---|---|
| `/` and each file under `ui/` | GET, HEAD | page | page | page | none | (the private scheme) |
| `/api/context` | GET | full | projected | product only | none | (new; Tauri command in m1-07) |
| `/api/snapshot?parts=` | GET | all parts | projected | 403 | none | `workbench_snapshot` |
| `/api/events` | GET | stream | stream, projected | 403 | 4 window and 8 watching streams | (new; Tauri event `ghostlight://change`) |
| `/api/policy` | GET | full, with etag | projected, no etag | 403 | none | `workbench_policy` |
| `/api/search` | POST | yes | yes | 403 | 5 per second | `workbench_search` |
| `/api/policy/preview` | POST | yes | yes | 403 | 1 in flight, 10 per minute | `preview_user_policy` |
| `/api/integrations/rescan` | POST | scans | cached roster, one scan per 5 s | 403 | coalesced | `refresh_harnesses` |
| `/api/window` | POST | yes | yes | 403 | 1 per 5 s | (new; `ghostlight open`) |
| `/api/runtime` | POST | any intent | `hold` only | 403 | 6 per minute | `apply_runtime_intent` |
| `/api/policy/user` | PUT | with `If-Match` | window | 403 | none | `apply_user_policy` |
| `/api/policy/user` | DELETE | with `If-Match` | window | 403 | none | `remove_user_policy` |
| `/api/controls/diagnostics` | PUT | set | window | 403 | none | `toggle_diagnostics` |
| `/api/controls/viewing` | PUT | `{ watch }`, answers the controls part | window | 403 | none | (new; Tauri command `set_watching`) |
| `/api/integrations/setup-all` | POST | yes | window | 403 | none | `setup_detected_harnesses` |
| `/api/integrations/act` | POST | with `expected_state` | window | 403 | none | `manage_harness` |
| `/api/notifications/test` | POST | yes | window | 403 | none | `test_notification` |
| `/api/open` | POST | `{ destination }` or `{ product }`, exactly one | window | 403 | none | `open_destination`, `open_harness_download` |
| `/api/diagnostics/reveal` | POST | yes | window | 403 | none | `reveal_diagnostics` |
| `/api/integrations/locate` | POST | `{ id }`, answers the client result or `null` | window | 403 | none | `locate_harness` |

Not routed, by design: `quit_ghostlight` and `copy_harness_text` (m1-07 copies in the page).

`/api/events` answers `200 text/event-stream` with no length, ended by closing the connection. It
writes `event: hello` with `{ context, instance, seq }`, then `id: <instance>-<seq>` and
`data: <the projected event as one line of JSON>` for each change numbered above the hello's `seq`,
`: keep` after 15 s of quiet, and `event: reset` with `{ "reason": "behind" }` before it ends a
stream whose queue (256 changes) overflowed. Past the places it answers `503 too_many_screens`.
Every act that succeeds answers with `Ghostlight-Seq: <seq after the act>` (rescan, runtime, both
policy writes, diagnostics, setup-all, client act).

Bodies are JSON objects with exactly the named fields. Answers keep today's Tauri values: search
answers the array of hits, a preview answers the preview, an intent answers the intent result, a
rescan answers the projected roster, a client act answers the projected `HarnessActionResult`.
The three native routes answer `{}` (open, reveal) or the projected `HarnessActionResult` / `null` (locate, with
`Ghostlight-Seq`); a person who closes the dialog gets `null`. The dialog blocks its connection thread until the
person answers. Errors are `{ "error": { "code", "message" } }`; what a facade call refuses with is 422
`refused` and its text.

## Owed live checks

- m1-04, Windows and Linux: the About card's two orbs, whose colors are inline style attributes the
  new content policy blocks, look the same in the Tauri window and in a browser tab on the new
  address. Today the page in a tab shows "Not connected" until m1-07; open the About page in a tab
  on the logged address (`RUNTIME_PUBLISHED` carries `workbench=<url>`) and compare it with the
  window.
- m1-04, Windows: a service stopped and started again at once takes the same port (the unit test
  covers TIME_WAIT on loopback; confirm with a real restart).

- m1-05, Windows and Linux: with the Tauri window open, pause from the tray and from the browser
  toolbar; the band's word, lamp and wheel change within a second and the word's tooltip names the
  source ("Paused from the tray.", "Paused from the browser toolbar."). Pause from a tab on the
  watch address and the window's tooltip says "Paused from a screen that is watching.".
- m1-05, Windows and Linux: connect and disconnect an MCP client and a browser; the sessions and
  browsers counts in the band change in the window with no refresh.
- m1-05, Windows: leave the window open for a minute with a tab on the watch address; both stay
  current, and closing the tab frees its place within about 15 s (the next keep-alive write fails).
- m1-06, Windows and Linux, in the Ghostlight window: the About and Status pages' links open in the
  default browser; "Open log folder" shows the diagnostics folder (on and off); a client's Download opens
  its page; Locate opens the file dialog, a cancel changes nothing, a chosen file is remembered.

- m1-07, Chrome, on the logged watch address (`RUNTIME_PUBLISHED` carries `workbench=<url>`): the page
  shows live data with the Watching chip and no greyed control; Set up everything, End session and
  Send test are absent; Integrations and the diagnostics card each show one "Change this in the
  Ghostlight window." line with a Continue button that brings the window forward; Pause works and the
  band then shows the state, its note and the Continue line; the About card and links open in a new
  tab; stopping the service shows "Reconnecting" in the band and the page recovers on its own when
  the service is back.
- m1-07, Ghostlight window (Tauri), Windows and Linux: nothing changed but the chip ("Full control");
  opening Integrations looks for the clients once; the window's focus no longer pulls; "Turn on" and
  "Turn off" for process diagnostics still work (the command is now `set_diagnostics`).
- m1-07, Chrome, with watching turned off (a later slice adds the switch; until then the Closed run is
  covered by `tests/workbench-surface.mjs` only): every data view shows the one sentence and About
  shows the Version and License facts and every link.

- m1-08, in the Ghostlight window (Tauri), Windows and Linux: the Controls button sits on the band after the wheel; the
  sheet replaces the Status page's Runtime session and Notifications strips and its process-diagnostics card (Status
  keeps its four health cards, Refresh and the colophon). Pause, Resume, Start session, End session, Turn on and off
  for diagnostics, Open log folder and Send test all work from the sheet; Escape, the button and a click outside close
  it. Density Compact tightens the hero and the queue, Motion Reduced stops the lamp and the view animations, and both
  are remembered after the window is reopened.
- m1-08, Windows: with a Chrome tab on the logged watch address, press Turn off under "Others can watch" in the window.
  The tab shows the closed sentence at once, its About page still shows in full, and the window's chip tooltip reads
  "Watching is off: only this window shows Ghostlight." with no reload. Reload the tab: it still shows the closed
  sentence. Turn it on again in the window and reload the tab: it watches again. `state/watch.off` exists in the home
  only while it is off, and a service restart keeps the choice.
- m1-08, Windows: run `ghostlight diagnostics on` from a terminal while the sheet is open; its diagnostics row follows
  within about a second with no refresh.

## Blockers

None.

## Notes

- Owner approval: the owner approved the amendment text on 2026-10-15, as written.
- Precondition 1: `seams.md` has Landed rows for f08a-edge, f08b-console-attach and
  w01-windows-user-path, and its RESUME HERE names none of them as next.
- Precondition 2: no tracked file under `crates/`, `Cargo.toml` or `Cargo.lock` was modified or
  staged; `install/user_path.rs` and `win-peer/src/settings_change.rs` are tracked.
- Precondition 3: the orchestrator package is `ghostlight-service`; `relay::home` has
  `SERVICE_EXECUTABLE_NAME` and `EDGE_EXECUTABLE_NAME`; `install/deployment.rs` has `service_swap`,
  `edge_swap` and `placed_in_bin`; `intake/listener.rs` `judge` refuses `Role::Desktop` and
  `service/mod.rs` refuses it again; `desktop/mod.rs` still has its `tauri::generate_handler!`
  list. No name differs from the spec.

- m1-02: the foreign entry's `program` is the file name of `command` when `command` names an existing file; otherwise the command is cut at its first whitespace before a file name is taken, and every later word counts as an argument. A configuration that cannot be parsed names the line of the parse error (taken from the parser, never any file text) in its detail and carries no evidence (owner answer Q3); `HarnessError::Malformed` carries only that line.

- m1-04: the server is `S/workbench/web` (`mod.rs`, `http.rs`, `routes.rs`, `assets.rs`,
  `limits.rs`, `watching.rs`, `tests.rs`); the parts and their projections are
  `S/workbench/parts.rs` (tests in `parts/tests.rs`); the contexts and acts are
  `S/workbench/context.rs`; the shared input bounds moved to `S/workbench/input.rs`; the words are
  `S/language/workbench.rs`. The Tauri commands call the moved validators and the two copy
  confirmations, so the Tauri window says what it said.
- m1-04: the bind is two steps (`PendingWorkbench::bind`, then `serve`) so `ServiceHost::publish_ready`
  can call `protect_workbench_port` between binding and answering. `WorkbenchServer::start` does both
  for callers with no rule to put between them. The `Intake` holds the running server in a
  `OnceLock` (`workbench_server`); the desktop role (m1-09) and `serve_control` (m1-11) read the
  address and the bearer from it.
- m1-04: the server builds `/api/context` through `WorkbenchFacade::context_document`, as the spec says.
  The facade keeps the first `attach_web` it is given, so a facade is served by one server.
- m1-04: scope-down, as the spec allows: `POST /api/runtime` has no `If-Match`. Hold, resume, start
  and end are sets that converge, and the window's view of the runtime is pushed within a second (m1-05).
- m1-04: search reads only fields a watcher is shown (operations, sessions, browsers, history
  status and reason, diagnostics cards, client names and details, the runtime word), so no hit needs
  filtering for a watcher. A test pins that an excluded host appears in no hit.
- m1-04: ceilings. The policy's ceiling lines that name the person's never-touch sites are found by
  the hosts in the `content.security.sacred_domains` settings of the layers; a watcher gets one
  count line (`never_touch_count`) in their place. `effective.rs` is not this lane's, so its line
  format is not touched.
- m1-04: the policy etag hashes the user layer's document as the authority reports it, which is the
  document re-serialized by the policy parser, parsed again without its top-level `version` and
  written with sorted keys. A change of formatting or version keeps the tag.
  Only Control receives the etag; a watcher's projected policy carries none, so the tag cannot
  reveal the never-touch list.
- m1-04: `GovernanceFacade::owning_user_policy` (test-only) became `pub(crate)` so the server tests can
  give a facade a user policy file of their own; `HarnessRegistry::isolated` and
  `WorkbenchFacade::with_harnesses` (test-only) keep the tests off this user's client configuration.
- m1-04: the default port of a home is 41000 plus the FNV-1a hash of the lower-cased home text
  modulo 1000, and 41738 when that is 41737. Windows' std binds without `SO_REUSEADDR`; the restart
  test passes there, so TIME_WAIT on the server's port does not block binding.
- m1-04: docs/trust/data-flows.md says `ghostlight status` shows the port, as the spec's sentence
  does; `status` shows it from m1-11.

- m1-05: `WorkbenchProjection::attach_events` appends, so every attached sink hears every event in
  order. A `delivery` lock makes the sequence number and the hand-off to the sinks one step, so no
  sink sees events out of order; a sink must not publish from inside its own delivery.
- m1-05: `S/workbench/live.rs` (new, in the lane's `S/workbench/**`) holds the live parts: it
  gathers operations, sessions, browsers, authority and runtime once, builds the status and
  connections parts from that look, and publishes a part only when it differs from the last one
  published. The projection holds the refresh hook through a `Weak`, so the facade is not kept
  alive by it. A refresh that arrives while another runs marks the parts wanted again and returns;
  the running refresh looks once more before it lets go, so the last look is always published, in
  order, and nothing waits.
- m1-05: the hook runs after an operation starts, changes or settles and after a saving-health
  change. A composition receipt and a coverage change cannot alter either part, so they do not run
  it. The first refresh always publishes both parts, because nothing was published before it.
- m1-05: `StatusPart` and `ConnectionsPart` are owned types in `S/workbench/parts.rs`, built from a
  snapshot with `of`, and a snapshot carries `runtime_note` (skipped when absent), so the Tauri
  snapshot and the web part say the same thing. `Projection::event` projects a published event for
  an audience: a watcher's coverage event carries `excluded_host_count` and no host, and a
  watcher's policy hint carries no etag.
- m1-05: the policy hint's etag is the new user-layer tag after the person's own write, which
  is what `GET /api/policy` reports for the window. When the refresher finds a different published
  authority it announces the authority id instead, so a change that leaves the user layer alone (a
  managed policy, a file edited by hand) still differs from every tag the page holds. The facade
  remembers the id it last announced, so the refresher does not repeat an act's hint.
- m1-05: `locate_harness` also publishes the integrations hint, because it changes the roster; the
  spec named only refresh, setup-all and manage. `apply_diagnostics_toggle` (the Tauri act) publishes
  the diagnostics hint like `set_diagnostics`.
- m1-05: the Tauri tray's Pause and Resume pass `RuntimeSource::Tray`; the Tauri `apply_runtime_intent`
  command and `WorkbenchFacade::apply_runtime_intent` pass `Window`. `ServiceBrowserEvents` holds the
  facade instead of the governance facade and applies the toolbar's intent through
  `apply_runtime_control(intent, Browser)`, toggle included.
- m1-05: refresh points: after `intake.browser.attach(connection)` returns, after a browser
  `Disconnected` or `TabClosed`, at the end of `LiveSession::open` and `LiveSession::close`, and
  after each refresher tick (`WorkbenchFacade::authority_refreshed`). A browser connecting changes
  the parts at attach; `attach` returns once the connection is registered, so the refresh sees it.
- m1-05: the page: `lib/store.js` folds `status_changed` and `connections_changed` into its snapshot
  and ignores an event numbered at or below the snapshot's (a snapshot now reads the sequence
  first, so those are already in it). `part_changed` returns `hinted`; `app.js` re-reads the part
  when its destination is showing (policy through the policy read, integrations and diagnostics
  through the full snapshot, which is all the Tauri transport has until m1-07) and otherwise reads
  it when the person opens that destination. A tag already acted on is not acted on twice. The
  Status page's checkup cards are derived from the snapshot and are not pushed; they follow the
  ten-second heartbeat or the next hint.

- m1-06: `S/workbench/mod.rs` has `WorkbenchNativeActs`, `NativeActError` (`Workbench`, `Harness`,
  `DiagnosticsOff`, `LinkNotSecure`) and the facade methods `open_destination`, `open_harness_download`,
  `reveal_diagnostics` and `locate_harness_by_picking`. `WorkbenchPresentationError::ActFailed` carries the
  computer's own text unprefixed, so the window shows the error text it showed before. The facade refuses any
  address that does not begin `https://` before the adapter is asked.
- m1-06: the spec's test "reveal_diagnostics with diagnostics off opens nothing" does not hold in the tree:
  `diagnostics::observe` names the diagnostics folder even while diagnostics are off (retained logs stay
  readable there, and the page always offers "Open log folder"), so `report().directory` is never `None`. The
  `None` branch and its moved sentence (`DIAGNOSTICS_OFF`) stay as the spec says, pinned by a test of the error's
  text; the behavior test pins that the folder opens in both states.
- m1-06: the download address is added to each projected client entry (snapshot, rescan and act answers) by
  `parts::add_download_url` from the routes, because the address lives in the client registry and
  `HarnessSummary` (`S/install/mod.rs`) is not this lane's to change after m1-02. It is public, so Control and
  Watch both get it.
- m1-06: `WebAct` gained `open`, `reveal_diagnostics` and `locate`, Control only (answer Q20). `WebAct::ALL` has 17.

- m1-07: the words.js additions are the four the spec names (`CONTINUE_LABEL`, `COPIED_SETUP`,
  `COPIED_COMMAND`, `RECONNECTING`) plus `APPLY_IN_WINDOW_LABEL` (the Watch spec names the Apply
  button's label) and two count helpers, `neverTouchedCount` and `leftOutCount`. A watcher's policy
  and coverage carry only counts, so the page needs a sentence for a count; the service's own
  never-touch sentence lives in the ceilings and is shown as it arrives.
- m1-07: a refused HTTP request rejects with a frozen object whose text is the service's sentence
  (`String(error)` and `error.message`) and that also carries `status` and `code`. A bare string has
  no place for the `stale` code the page needs to keep a draft and read the rules again.
- m1-07: the sequence an act answers with is attached to its result as a non-enumerable `seq`
  (`GhostlightTransport.seqOf(result)`). `app.js` waits up to 2 s for the store to reach it and pulls
  a snapshot only when it does not. `store.seq()` and a repaint in `store.endHarness` were added for
  this; nothing else in the store changed.
- m1-07: the Tauri command `toggle_diagnostics` and `WorkbenchFacade::apply_diagnostics_toggle` are
  retired; the page now says `set_diagnostics(on)` on both paths. A source guard in
  `desktop/mod.rs` pins the three new commands and the absence of the toggle.
- m1-07: the `closed` event's data is read as `{ sentence }`; the server does not send the event yet.
  A stream that opens with 403 (`watching_off`) is treated the same way, using the envelope's
  message. The page also reads `GET /api/context` again and shows its Closed word.
- m1-07: a watcher's policy draft keeps the rules and settings it was drawn from but not the
  never-touch hosts (the service sends a count), so Check against my history for a watcher
  approximates the person's real list; the field is read-only and shows the count. Apply there
  is "Apply in the Ghostlight window" and keeps the draft.
- m1-07: `view.policy(view, { keepDraft })` keeps the draft when the applied rules moved on under a
  refused write. A hint about the rules while the page's own write is in flight is ignored; the
  write reads the rules itself when it is done.
- m1-07: the Closed About page leaves out the Sessions, Browsers and Recorded facts, which come from
  data routes, as the spec says.
- m1-07: `tests/workbench-surface.mjs` moved its injected About failure to the new
  `about({ version, license, counts })` signature. The preview server's fixture and the Tauri
  fixture in the surface test answer `workbench_context` with a Control context.

- m1-08: the controls part carries a `label` on each row (`runtime`, `viewing`, `diagnostics`, `notifications`), which
  the spec's key list does not name. The kit holds no sentence, so every row label comes from
  `S/language/workbench.rs` with the state words; the page's own labels (headings, Density, Motion, each button's verb)
  are `CONTROLS` in `words.js` and are handed to the kit. `runtime.note` is always present (null while not paused).
- m1-08: `ControlsPart` is the Control form. `ControlsPart::seen_by(context)` fills every act's availability and `you`
  for a context, so the snapshot, the event and the route all project from one value, and the service compares the
  Control form to decide whether anything changed. The log folder is made home-relative when the part is built.
- m1-08: the controls part is published by the live refresh, with status and connections, so a connection that comes or
  goes, a pause from the tray and `ghostlight diagnostics on` all reach the sheet. Process diagnostics report their
  log size, so while they are on the sheet is republished whenever the size changes.
- m1-08: an act that changes the controls answers with `Ghostlight-Seq` of the `controls_changed` it caused, which
  follows its part hint. `the_acts_that_change_a_part_hint_at_it_with_its_tag` now reads the controls event for the
  diagnostics act.
- m1-08: a watching stream lives only while watching is on. The hub reads the shared state: it refuses a watcher
  while watching is off (the route answers `watching_off`), and the first change of any kind after the flip ends every
  open watching stream with `closed` and sends it nothing. A closed tab does not poll; it shows the closed sentence
  until it is reloaded.
- m1-08: with no watch view served, the Controls part says so: the viewing row is off, has no address, its word is
  the not-served sentence and Turn on or off is unavailable with that reason, in every context.
- m1-08: the Copy address button shows only on a screen that may change who watches, and only where the page has a
  clipboard (the loopback path). The Tauri path has no clipboard command, so the address is selectable text there.
- m1-08: `app.js` wires only the band's wheel for `data-intent`; the buttons in the sheet carry their own attributes
  and are wired by the kit, so one press is one act. The sheet's Continue button is `data-controls-continue`, not
  `data-continue`, for the same reason.
- m1-08: `view.js` names the body for the runtime state without discarding the other classes it wears, because the
  personal settings are body classes too.
- m1-08: the diagnostics part no longer carries `process_diagnostics`, and neither do the snapshot and the page's
  flat shape: the Controls sheet's diagnostics row is the one place the page reads that state. `doctor` keeps its own
  observation.
- m1-08: `connection_counts` is tested over an in-memory workspace store with admitted MCP and command-line
  connections and a fake browser; it needs no home, so no `TestHome` is used.
- m1-08: `docs/1.0/ARCHITECTURE.md` still says process diagnostics can be toggled "from the workbench Status card"
  (the process-diagnostics paragraph). That paragraph is not one this slice may edit; whoever next edits it should say
  "Controls sheet".
- Owner question: Theme and "Hold this screen" are not part of this milestone's "On this screen" group. Should
  either be added once the demo has been seen?
