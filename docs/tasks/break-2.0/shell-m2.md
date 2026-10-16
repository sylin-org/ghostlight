# Break 2.0: shell milestone 2 ledger

Milestone 2 (M2) of the desktop shell track carries ADR-0199 and its amendments of 2026-10-15
("Settings and the tray", "shell milestone 2"). It follows [shell-m1.md](shell-m1.md). The
milestone's goal: the Ghostlight window, its tray icon and its notifications all come from
`ghostlight-desktop`, and the service brings it up by itself. Settings, a tab with a left rail
before About, replaces the Controls button and holds Quit. The tray icon shows state, its tooltip
counts connections, and the first close of the window explains the icon once. The service links no
UI toolkit, shows as "Ghostlight service" in Task Manager, starts with no display, and exits after
10 minutes when nothing is connected to it.

Slices, in order. The coordinator deploys each one in place (development channel, one live
Ghostlight on the owner's machine) and runs its owed live checks with the owner before the next
one starts.

1. m2-01-settings-tab: Settings replaces the Controls sheet, the page can quit in Control, every
   tab has an address, the connection sentence moves to Status.
2. m2-02-shell-carries-the-desktop: the service asks the shell to show the window at a section,
   show a notification, open a link or folder and pick a file.
3. m2-03-shell-lifecycle: one window at a time, the Start menu starts Ghostlight, the shell's four
   screens, the window reopens on the new build after an update.
4. m2-04-the-switch: the service brings up the shell, the shell draws the service's tray with a
   state icon, the first close shows a one-time hint, Tauri stops drawing.
5. m2-05-tauri-out: no UI toolkit in the service, "Ghostlight service" in Task Manager, a console
   program with no console window, and a guard that keeps it so.
6. m2-06-idle-exit: a service with nothing connected for 10 minutes exits.

Gate, from the repo root on `dev`, in Git Bash, with
`CARGO_TARGET_DIR=E:/repo/github/sylin-org/ghostlight/.target-lane-shell-m2` for every cargo
command, without `--locked` (a slice may change `Cargo.lock`):

1. `cargo fmt --all -- --check`. Fix with `cargo fmt -p <package>` for the packages the slice
   touched only.
2. `cargo clippy --workspace --all-targets -- -D warnings`.
3. `cargo test --workspace` (in the background or with a 600000 ms timeout).
4. When `crates/orchestrator/ui`, `tests/workbench-*.mjs` or `tests/workbench-preview-server.mjs`
   changed: `node --test tests/workbench-transport.test.mjs`,
   `node --test tests/workbench-settings.test.mjs` (it was `tests/workbench-controls.test.mjs`
   until m2-01) and `node tests/workbench-surface.mjs`;
   `node --test "crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js"`
   only if page assets changed.
5. `node --test "crates/desktop/guards/*.test.mjs"` when `crates/desktop`, any `Cargo.toml` or
   `Cargo.lock` changed.
6. `node --test "tests/guards/*.test.mjs"` when `crates/edge`, `crates/relay`, any `Cargo.toml` or
   `Cargo.lock`, or `tests/golden` changed.
7. `node --test packaging/npm/test/*.test.js` when `packaging/npm` changed.
8. `node --check` on every changed `.js` and `.mjs` file that no test runs.
9. `pwsh -NoProfile -File scripts/check-repository-integrity.ps1` (ASCII and links).
10. Goldens move only on purpose: `GHOSTLIGHT_DESKTOP_GOLDEN_UPDATE=1` for `tests/golden/desktop`.
    The catalog golden (`the_catalog_matches`) and the result goldens (`work::tests::golden`) must
    not move in any slice; if they do, the slice is wrong.

No agent runs a Ghostlight binary, journey, install or dev loop. The coordinator deploys after
every slice and runs that slice's owed live checks with the owner.

Never pass `--locked` to cargo build, clippy or test.

Rules:

1. The commit protocol is in [BOOTSTRAP.md](BOOTSTRAP.md).
2. One logical change is one commit. Stage and commit only your own paths, by path.
3. Never run a Ghostlight binary, a journey, an install, a carrier or `scripts/dev-loop.ps1`.
   Record each live check under Owed live checks.
4. Follow the tree when it differs from a spec, keep the spec's intent and record the difference
   under Notes. When the intent cannot be kept, record a blocker and stop.
5. Read the owner answers below before building anything a question covers.

## RESUME HERE

m2-01-settings-tab, m2-02-shell-carries-the-desktop and m2-03-shell-lifecycle have landed (see
Landed). The shell now keeps one window at a time (the service's one desktop place), starts the
service on a person's launch, shows Starting, Updating, Restarting and Stopped, reopens its window
on the new build after an update, and hears the end of the session through the relay's shared
listener. `--tray` (m2-04's presence) and `--updating` are accepted. While Tauri is still in the
service, one route (`ShellRoute`) sends a request to the attached shell and otherwise to the Tauri
window. Next: the coordinator deploys m2-03 and runs its live checks with the owner, then
m2-04-the-switch starts. m2-04 deletes the Tauri branch of the route
(`ShellRoute::install_fallback`, `ServiceHost::install_native_fallback` and its one call in
`desktop/mod.rs`), makes the service start `ghostlight-desktop --tray`, and gives the Start-menu
shortcut the AUMID `Sylin.Ghostlight`.

## Landed

| Slice | What | Commit |
|---|---|---|
| m2-01-settings-tab | Settings, a tab with a left rail before About, replaces the Controls button and sheet. The rail has This screen, Sharing, Browser work, Notifications and Diagnostics, each with its state word, and `kit/controls.js` became `kit/settings.js`. Every tab has an address from the closed list `ADDRESSES` (`#settings/sharing`, and so on), written in place with `history.replaceState`. In Control the page can quit Ghostlight: `WebAct::Quit`, `POST /api/quit` (202), a quit port the service attaches to the facade, and `quit_ghostlight` on both transports. Watch shows the Continue line in each section and Closed shows the watching-off line. The connection sentence moved to Status. The UI-surface guard tests moved out of `desktop/mod.rs` into `workbench/surface_tests.rs`. The Policy page's ceilings end with the line "Ghostlight's own control page on this computer is never automated.", authored once. Forced-colors rules cover the band, the tabs, the rail and the state words. | see `git log` |
| m2-02-shell-carries-the-desktop | The service asks the shell, over the `desktop` stream, to show the window at a section, show a notice, open a link or a folder and pick a file. New frames `show`, `notice`, `act` (service to shell) and `act_result` (shell to service) with goldens in `tests/golden/desktop/`. `PageAddress` (`workbench/address.rs`) is the Rust copy of the page's address list, `POST /api/window` takes `{view, address?}` (unknown address: 400) and a Watch Continue sends its section or screen. `service/shell.rs` holds the attached shells and `ShellRoute`, the one adapter the facade gets for presentation and native acts. The shell (`crates/desktop/src/native.rs`) carries them out with `notify-rust`, `open` and a file dialog (`rfd` on Windows, GTK on Linux), names itself `Sylin.Ghostlight`, and skips any frame it cannot read. The blocked and checkup notice words moved to `language/workbench.rs`. | see `git log` |
| m2-03-shell-lifecycle | One shell at a time: the service holds one desktop place, and a second shell is sent `already_open` (new frame, golden `already-open.json`) and exits 0 while the attached shell is sent `show`. The shell takes `--tray` and `--updating` (`Launch`). Its life is a pure state machine (`machine.rs`) with four screens in one built-in document (`states.rs`): a person's launch starts the service once and shows Starting, the update hand-over shows Updating or relaunches the shell on the new build, a loss without goodbye shows Restarting and starts the service up to three times, and Stopped offers Start and Diagnostics. A new workbench frame rebuilds the webview. The shell adopts the relay's session listener and drops its own signal sources. | see `git log` |

## Owner questions

Each has a default the work uses until the owner answers.

1. Tray icon states and the offline look. Default: the plain ghost when ready plus a small dot in
   the band's palette for the other states (teal working, amber paused, red needs attention), no
   new artwork; with no browser connected the icon looks ready and the tooltip says so.
2. Quit placement and confirm. Default: the button sits at the end of Browser work and asks
   inline, with no dialog: "Quit now? Agents lose the browser until Ghostlight starts again." with
   Quit and Cancel.
3. Theme or Hold this screen in This screen. Default: neither.
4. Icon only, or the window, when the service brings up the shell by itself. Default: only the
   icon appears; a Start-menu launch, `ghostlight open`, a watcher's Continue and the tray's Open
   always show the window.
5. Whether the 2.0.0 tag waits for the shell and the install journey. Default: it waits (read from
   the owner's order: seams, shell, journey, release bar).
6. Whether to fix the Policy page's missing own-control-page ceiling, and whether as a separate
   commit. Default: fix it, in its own commit.

## Owner answers

- Q1 (2026-10-15, binding): the plain ghost when ready plus a small dot in the band's palette for
  the other states (teal working, amber paused, red needs attention), no new artwork; with no
  browser connected it looks ready and the tooltip says so.
- Q2 (2026-10-15, binding): Quit sits at the end of Browser work; one press asks inline with no
  dialog ("Quit now? Agents lose the browser until Ghostlight starts again." Quit / Cancel).
- Q3 (2026-10-15, binding): no Theme and no Hold this screen in This screen.
- Q4 (2026-10-15, binding): when the service brings up the shell on its own (an agent started
  Ghostlight), only the icon appears; a Start-menu launch, `ghostlight open`, a watcher's Continue
  and the tray's Open always show the window.
- Q5: not answered. The work assumes the 2.0.0 tag waits.
- Q6 (coordinator, 2026-10-15): fix it as part of m2-01. It landed as its own commit, before the
  Settings commit.
- Cadence (2026-10-15, binding): the coordinator deploys each slice in place before the next
  starts.

## Owed live checks

After the coordinator deploys m2-03, with the owner (Tauri's tray and window are still present):

- m2-03, one window (Windows): start `bin/ghostlight-desktop` from Explorer with the service
  running: one window. Start it again: the first window comes forward and no second window opens;
  the second process is gone from Task Manager within a second.
- m2-03, person start (Windows): quit Ghostlight from the tray, then start the shell from Explorer:
  Starting, then the workbench, with the service back in Task Manager.
- m2-03, restart (Windows): with the shell open, end `ghostlight-service.exe` in Task Manager:
  Restarting, then the workbench again within a few seconds. The window keeps working (Pause,
  Settings) and the Tauri window may appear beside it until m2-04.
- m2-03, stopped (Windows): end the service, then rename `bin/ghostlight-service.exe` so it cannot
  start (rename it back afterwards): Restarting, then after about 30 s "Ghostlight stopped
  unexpectedly." with Start and Diagnostics. Diagnostics opens the diagnostics folder, or the
  Ghostlight folder when diagnostics were never on. With the file renamed back, Start shows
  Starting and then the workbench.
- m2-03, update (Windows): a development deploy with the shell window open: Updating, the window
  closes and reopens on the new build, and Task Manager shows no old `ghostlight-desktop.exe`
  left. A deploy with the shell window closed changes nothing visible (the shell has no tray yet,
  so the process is gone when the window closes).
- m2-03, session end (Windows): sign out and sign in, and restart Windows, with the shell open: no
  "this app is preventing shutdown", no stray `ghostlight-desktop.exe` afterwards.
- m2-03, High Contrast (Windows): with High Contrast on, each state screen (end the service to see
  Restarting and Stopped) is readable: the mark, the words and the buttons use the system colours,
  and a button shows a focus ring when tabbed to.
- m2-03, Linux: the shell shows one window only, the state screens draw, and SIGTERM and SIGHUP
  end the shell within two seconds with no stray process. SIGINT (Ctrl-C in a terminal) is no
  longer handled by the shell.

After the coordinator deploys m2-02, with the owner (the coordinator starts `bin/ghostlight-desktop`
by hand beside the Tauri window):

- m2-02, Continue (Windows): from a Chrome watch tab (the URL `ghostlight status` prints) on
  Settings > Diagnostics, Continue brings the shell's window forward on that section. Continue from
  the band and from the Integrations roster line brings the shell's window forward on the screen
  the tab shows. With the shell's window closed (its tray icon or not), Continue opens it again on
  the same section.
- m2-02, Locate (Windows): Locate in Integrations, in the shell's window, opens a file dialog owned
  by the shell's window (it sits over it and the window does not take clicks until it closes).
  Choosing a file locates the client; closing the dialog changes nothing.
- m2-02, links and folders (Windows): a link in About and Open log folder (Settings > Diagnostics,
  diagnostics on) open from the shell's window with no PowerShell or console window flashing.
- m2-02, no shell (Windows): with `ghostlight-desktop` not running, the same acts still work
  through the Tauri window (Continue from a watch tab, Locate, a link, Open log folder, Send test).
- m2-02, two shells: start a second `ghostlight-desktop` while the first runs. Continue and Locate
  go to the newer one. Close it, and they go to the older one.
- m2-02, Linux: the Linux build compiles with `libdbus-1-dev` present (notify-rust with only its
  `d` feature uses the `dbus` crate). Continue, a link, a folder and Locate work from the shell on
  GNOME and KDE. Locate opens the GTK file chooser over the shell's window (modal to it), the
  window and the tray stay responsive while it is open, choosing a file locates the client and
  closing the chooser changes nothing.
- m2-02, toast: waits for m2-04. An unpackaged program's toast shows only when a Start-menu
  shortcut carries the AUMID `Sylin.Ghostlight`, and the shortcut gets it with its new target.
  Until then, Send test in the shell's window shows no toast and prints one line on the shell's
  standard error, or shows nothing.

After the coordinator deploys m2-01, with the owner:

- m2-01, in the Tauri window (Windows): the Settings tab sits before About and opens a rail of
  five entries, each with its state ("Comfortable, motion follows the system", the Sharing, Browser
  work and Diagnostics words, the notification sentence). Each section works: Density and Motion
  stay after the window is reopened; Turn off and Turn on for Others can watch, Copy address, Pause,
  Resume, End session and Start session, Turn on and off for diagnostics, Open log folder and Send
  test all act. The band has no Controls button. Status shows the connection sentence at its top
  and nothing else changed there. The Policy page's "Always off limits" list ends with the line
  about Ghostlight's own control page.
- m2-01, Quit (Windows): Browser work ends with Quit Ghostlight. One press turns the button into
  the question with Quit and Cancel; Cancel puts the button back. Quit closes Ghostlight within
  about 1 s, the band says "Ghostlight is quitting." and not "Reconnecting" while it goes, and a
  Start-menu launch brings it back.
- m2-01, in a Chrome watch tab (the URL `ghostlight status` prints): Settings shows the rail and the
  values as text; This screen works; every other section shows "Change this in the Ghostlight
  window." with a Continue button that brings the window forward; there is no Quit; Pause works on
  the band. A typed `#settings/diagnostics` opens that section, and so does a typed `#policy`.
  An unknown hash such as `#nowhere` opens At a glance and the address bar then reads `#monitor`.
  Clicking tabs and rail entries leaves the back button where it was.
- m2-01, with "Others can watch" off, in a tab: This screen works and every other section shows
  the watching-off sentence, one line each; About still shows in full.
- m2-01, with Windows High Contrast on (Windows): the tabs, the selected tab, the rail entries,
  the selected rail entry and the state words stay visible, and the selected tab and entry use
  the system highlight.

## Blockers

- m1-12, carried over, answered by the owner on 2026-10-15: the service reads "Ghostlight
  service" in Task Manager. It lands with m2-05, which removes Tauri from the service and gives it
  a version resource of its own, like the edge and the window.
- Owed before the 2.0 release bar, not in this milestone: the per-binary process test of ADR-0199
  Decision 21 that sends `WM_QUERYENDSESSION` and `WM_ENDSESSION` to each binary.
- Owed beyond this milestone: the Linux shell honouring the GNOME or KDE session manager's end
  (Decision 14) beyond SIGTERM and SIGHUP.

## Notes

- m2-01: the snapshot part and the change keep their names, "controls" and
  `WorkbenchChange::ControlsChanged`. Nothing outside the page reads them and both sides are the same
  build, so a rename would change the wire for no reader. Doc comments now say that Settings is
  drawn from the part.
- m2-01: `quit_ghostlight` is one name on both transports. The Tauri command (`desktop/mod.rs`)
  and `POST /api/quit` both end in `Quit::begin(QuitReason::Quit, QUIT_GRACE)`: the command through
  `DesktopState.quit`, the route through a `WorkbenchQuitPort` the service attaches to the facade in
  `start_workbench`, so `workbench/` does not depend on `service/`. Both answer
  `{ "message": "Ghostlight is quitting." }`; the route answers 202. A facade with no port attached
  answers 422 `refused` with `QUIT_UNAVAILABLE`. `QUITTING` and `QUIT_UNAVAILABLE` are in
  `language/workbench.rs`.
- m2-01: the page learns it may quit from the `quit` act of `/api/context`, available in Control
  only (a watcher gets `needs_window`, Closed gets `watching_off`, like every other window-only
  route). The window can still quit while watching is off.
- m2-01: after a successful Quit the view keeps the band's word at the sentence the service gave,
  and a stream that is lost and found again does not turn it back into "Reconnecting".
- m2-01: addresses. `ADDRESSES`, `parseAddress` and `addressOf` are in `ui/lib/words.js`. The view's
  `navigate(view, section)` writes the canonical address with `history.replaceState`, so a bare
  `#settings` becomes `#settings/screen` (or the section shown last) and a tab or rail click adds no
  history entry. A page that loads with no hash stays bare. A hash that changes later is read by
  `app.js`: an unknown one opens At a glance and is replaced by `#monitor`; an emptied one opens At a
  glance. Opening `#policy` or `#integrations` does what clicking that tab does (the policy is read,
  the clients are looked for once), after the page has been told its context.
- m2-01: the spec lists "a hash on load opens that section" and "hashchange navigates" under
  `tests/workbench-settings.test.mjs`. Those two need `app.js`, so they run in
  `tests/workbench-surface.mjs` (one page per address, in Control, Watch and Closed). The settings
  test covers the closed address list, the parse, and `navigate`'s writes against the real markup
  and the real view.
- m2-01: the rail's state words come from the part (`CONTROLS_VIEWING_*`, `CONTROLS_RUNTIME_*`,
  `CONTROLS_NOTIFICATIONS_WORD`, the diagnostics word) and, for This screen, from
  `screenState(density, motion)` in `words.js`. The notification sentence is long for a rail, so an
  entry shows at most two lines and carries the whole text as its tooltip.
- m2-01: in Watch, Pause stays a button in Browser work while agents run, as the part says (the
  band's Pause is the one to use; the rule that the person is never offered a change they cannot
  make is unchanged). The Continue line shows in each section that has an act this screen cannot do.
- m2-01: a Closed page has no controls part, so the rail shows no state word for the four service
  sections and the sections show the watching-off sentence; This screen is drawn as always. The
  Sharing section's who-can-see sentence is the part's `you.sentence` in Control and Watch and the
  watching-off sentence when Closed.
- m2-01: the page's `CONTROLS` words became `SETTINGS`, with the rail's labels, the Quit labels and
  the question. The band's grid lost the column of the Controls button.
- m2-01: the UI-surface guards moved to `workbench/surface_tests.rs` (declared `#[cfg(test)] mod
  surface_tests;`). The Tauri tests stayed in `desktop/mod.rs`; `the_page_can_ask_what_it_may_do...`
  now reads `transport.js` directly and lists `quit_ghostlight`. One test was added: the
  stylesheets carry a forced-colors block that covers the band, the tabs and the rail. One string
  literal in `every_row_cell_has_a_grid_track_at_every_width` changed from a multi-line literal to
  `"\n    }"`, since moving the code changed its indentation.
- m2-01: ceilings. `OWN_CONTROL_PAGE_CEILING` (in `language/outcome.rs`) is now appended by
  `governance::effective::ceilings()`, so the person's Policy page, the watch view and
  `policy_explain` read one line from one constant. `work/policy.rs` no longer appends it, so
  `policy_explain` is byte-identical. A watcher's count line for never-touch sites still comes last.
- m2-01: `tests/workbench-surface.mjs` and `tests/workbench-preview-server.mjs` gained the quit act,
  the address bar, and the new ceiling line; the preview server's window fixture answers
  `quit_ghostlight`.
- m2-02: the address type is `PageAddress` (`workbench/address.rs`, re-exported from `workbench`),
  not `WorkbenchAddress` as the spec says, because `workbench::web::WorkbenchAddress` already names
  the server's loopback address and two types of one name in one module tree would read as one.
  A test reads `ui/lib/words.js` with `include_str!` and proves the two lists are equal.
- m2-02: the spec says the service ends the stream on an unknown kind. The tree skips it:
  `DesktopFrame::Unknown` (`#[serde(other)]`) is decoded and ignored, and only a frame of a known
  kind that does not decode ends the stream. The shell now skips both an unknown kind and a frame
  it cannot decode, and ends only on a goodbye, the end of the stream or a stream failure. A nested
  act kind the shell does not know decodes as `NativeAct::Unknown` and is answered `failed`, so the
  service never waits on a frame the shell dropped. The frame fields are `show {address}`,
  `notice {title, body}`, `act {id, act}` with `act` tagged by `kind` (`open_url {url}`,
  `open_folder {path}`, `pick_file {title}`), and `act_result {id, outcome}` with `outcome` tagged
  by `kind` (`done`, `picked {path}`, `cancelled`, `failed`). Extra goldens beyond the spec:
  `show-no-address.json` and one `act-result-*.json` per outcome. The relay is unchanged.
- m2-02: all three acts wait for `act_result`, not only `pick_file`. A link or a folder waits at
  most 10 s and then fails with `NATIVE_ACT_FAILED` ("This computer would not do that."), so the
  page can say that Open log folder did not open. A file dialog waits for the person with no limit.
  The stream ending, a goodbye (`Shells::say_goodbye`, called by the quit's farewell) and a quit
  that begins or a session end (`Shells::close`, called first in `Quit::begin` and
  `Quit::session_end`) each release every waiting act, which answers
  `WorkbenchPresentationError::Unavailable`; the facade maps that to its own
  `WorkbenchError::PresentationUnavailable`. A waiting act is not an invocation, so the quit's
  drain never counts it. After a quit begins no act starts.
- m2-02: the router is attached when the service host starts, not at the Tauri `Ready` event. The
  Tauri adapters are installed into it as the fallback at `Ready`
  (`ServiceHost::install_native_fallback`), and a request with no shell and no fallback is
  unavailable, as before. With a shell attached the shell wins, even when Tauri is up. m2-04 deletes
  `ShellRoute::fallback`, `install_fallback`, the host method and its one call in `desktop/mod.rs`.
  Tauri's `NativePresentation::reveal` takes the address and ignores it. The desktop stream is no
  longer enrolled in the farewell's `mcp` map; `Shells` says goodbye to it.
- m2-02: the shell checks the link scheme as well (`https://` only) before it asks the opener, as a
  second line behind the facade's own rule; an address that fails answers `failed`. The shell opens
  the window at a section by loading the workbench address with the hash after it when it has to
  make the window, and by setting `location.hash` when the window is open (Windows). On Linux an
  Open request rebuilds the window, and the requested address waits in `Shell.requested` for the
  rebuild. The address check (`a-z`, `/`, `-`, at most 48 characters, non-empty) runs when the
  request arrives and again where the address is used.
- m2-02: a notice is shown on a short-lived thread, so a slow notification service never holds the
  window. A failure is one line on the shell's standard error. Clicking a toast does nothing: an
  unpackaged program has no activator. On Windows the file dialog is `rfd`, parented to the
  shell window through `raw-window-handle`. On Linux it is a `gtk::FileChooserNative` made on the
  event-loop thread, modal to the shell window and answered from its response signal (see the
  next note).
- m2-02: a file dialog is answered by a callback, not a return value (`Computer::pick_file` takes
  a `Chosen`, `carry_out` takes an `Answer`), and the answer returns to the event loop as
  `ShellEvent::ActDone`, which sends the `act_result`. Deviation from the spec, which said `rfd`
  with `gtk3` on Linux: `rfd`'s gtk3 backend does not run GTK on the calling thread. It queues
  the dialog on the default GLib main context and waits on a condvar, and tao's Linux loop owns
  that context for its whole life, so Locate would freeze the window and the tray for good (and
  would drive GTK from two threads). `rfd` is now a Windows dependency only. On Linux the shell
  uses the `gtk` crate it already has, on the main thread, and nothing blocks the loop.
- m2-02: the Linux build of the shell now needs `libdbus-1-dev` (the `dbus` crate, through
  notify-rust's `d` feature). `.github/workflows/ci.yml` and `release.yml`
  install `libwebkit2gtk-4.1-dev` and friends, which normally pull `libdbus-1-dev`, but this was
  not run. It is an owed Linux check above; the workflows are not in this slice's files.
- m2-02: `Cargo.lock` gained `notify-rust`, `open` and `rfd` as dependencies of
  `ghostlight-desktop` and `dbus` as a dependency of `notify-rust`. No package was added. The
  desktop network guard now also bans `zbus`, `ashpd` and `async-std`, and passes on both targets.
- m2-02: the page. `transport.continueInWindow(address)` sends `{view, address}` over HTTP and
  `{address}` to the Tauri command, which ignores it. A Continue line in a Settings section sends
  `settings/<section>` (the kit gets `addressOf` through its words), the roster line sends
  `integrations`, and the band's line sends the address of the screen the page shows when it is
  pressed, since the band is on every screen. Apply on the Policy page, when this screen cannot apply, continues
  at the screen shown as well.
- m2-02: `src/work/tests/control.rs` needed the new `reveal` signature. Another lane's commit
  (`b7f9de10`) staged that file whole and carried those two lines in, so this slice's commit does
  not touch it.

- m2-03: the pure state machine is `crates/desktop/src/machine.rs` (not in the spec's file list).
  `Machine::on(event, now, outside)` answers with effects (`Open`, `Show`, `Exit`) and asks the
  world four things through `Outside` (may a process start, start the service, start the shell
  again, what image is in `bin`). The glue in `app.rs` runs the opening on a worker thread, owns
  the time through `ControlFlow::WaitUntil(machine.wake_at())`, and tags every stream with a
  generation so a late event of an older stream is ignored. The spec's "injected clock, service
  starter, spawner, image identity and session state" are the `now` argument and `Outside`.
- m2-03: "start_service(Recover) on the shared Backoff, three tries" is built as three tries of
  10 s each (`RECOVER_PATIENCE`), the first at the loss. Inside a try the opening is polled on the
  relay's backoff (0.5 s doubling). The shell cannot see that the service refused a `recover`
  start after a person's Quit (the refusal is the new service process exiting), so a refused
  start looks like no answer and ends as Stopped after the third try, or at once when the service
  answers the opening with a refusal.
- m2-03: a person's start that finds the service starting to quit waits without starting; the 30 s
  window (`STARTING_PATIENCE`) runs from the first attempt and again from the start itself. A
  start the machine cannot make (the spawn fails) is the Stopped screen at once. Nothing is
  started while the session is ending, and the wait runs out to Stopped.
- m2-03: the shell has no exit retention of its own to drop (ADR-0180 Decision 2 concerns Tauri's
  event loop), so on the final signal the listener's callback arms the relay's fallback exit
  (2 s, `stop_on_final`) and posts `SessionEnding`; the loop then sets `ControlFlow::Exit`, and
  `LoopDestroyed` only drops the window and the tray so Windows removes the icon. The shell's own
  fallback thread, signal handling and `SessionState` are gone; `session.rs` keeps `Session` (what
  the shell asks before it starts a process), `ThisSession` and `listen`. Linux SIGINT is no
  longer handled; the relay's listener covers SIGTERM and SIGHUP. `libc` left the shell's Linux
  dependencies with the glib signal sources.
- m2-03: "this shell is out of date" is the length and last-write time of the shell's own image,
  read once at start from `current_exe()`, against the file at `home::bin()` joined with
  `ghostlight-desktop(.exe)` read when the replace goodbye arrives. An unreadable file counts as the
  same, so the shell waits on Updating and never relaunches on a guess. The relaunch is
  `os::spawn_detached` of that file with `--updating`, working directory the Ghostlight home. The
  one-relaunch-per-episode rule is the `relaunched` flag plus the shell ending after the relaunch;
  a failed relaunch is the Stopped screen. A test in `install/deployment.rs` pins that the swap
  precedes the quit (`the_new_shell_is_in_bin_when_the_running_service_is_asked_to_quit`).
  This meets Decision 11's "relaunch a shell whose build is out of date" without a service-side
  hook: the departing shell relaunches itself when its window was open, and presence (m2-04)
  covers the closed case by bringing up a `--tray` shell.
- m2-03: one shell. `Shells` holds one place (`Free`, `Claimed`, `Attached`) instead of a list;
  `claim` is atomic and `Claim::attach` makes the shell routable after it has been handed the
  workbench, so a request never arrives before the first frame. A claimed place that is not yet
  attached cannot be shown, so a second shell that races a first still exits 0 and the first, which
  is opening its window anyway, is not asked. `DESKTOPS` stays 2. The m2-02 tests that opened two
  shells (`the_newest_of_two_shells_gets_the_request`, the capacity test) were replaced by
  `a_second_shell_is_told_it_is_already_open_and_the_first_is_shown`; the m2-02 owed check "two
  shells" is superseded by the one-window check above.
- m2-03: the state screen's buttons are links to `https://ghostlight.invalid/start` and
  `/diagnostics` (the reserved `.invalid` name never resolves). The webview's navigation handler
  (`states::navigation`) allows `about:blank` and `data:text/html...` (a document loaded from a
  string is one or the other, by platform), turns the two addresses into a press and refuses the
  navigation, and refuses everything else. The document sets a content policy of
  `default-src 'none'`, runs no script and has no IPC handler.
- m2-03: a webview is dropped before its replacement is built. wry's Windows webview removes the
  parent window's resize hook when it drops, so a webview built first would lose its hook to the
  drop of the old one.
- m2-03: the shell builds its tray when the first workbench frame carries a model, drops it when a
  later one carries none, and rewrites its tooltip and enabled items for the state screens
  (`TrayModel::while_without_service`). Until m2-04 the service sends no tray while Tauri's tray
  exists, so the live checks above run without a shell tray, and a shell without a tray ends when
  its window closes, once it has attached.
- m2-03: `--tray` and `--updating` have no live caller until m2-04 (the service) and the shell
  itself (`--updating`, on a replace goodbye). A `--tray` shell with no tray in its workbench frame
  ends after its first attach, since it would otherwise hold the desktop place invisibly.

## Rollback

Redeploying the m2-02 payload restores the old shell: no arguments, a stderr line and exit 1 with
no service, an exit at any goodbye, and a service that admits two desktop streams. The
`already_open` frame is only written by the m2-03 service; an older shell skips an unknown first
frame as a contract failure and exits.
