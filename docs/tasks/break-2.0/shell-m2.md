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

m2-01-settings-tab has landed (see Landed). Settings is a tab before About with a rail of five
sections, the Controls button and sheet are gone, every tab has an address, Status carries the
connection sentence, the page can quit in Control, and the Policy page's ceilings end with the
line about Ghostlight's own control page. Next: the coordinator deploys it and runs the m2-01 live
checks with the owner, then m2-02-shell-carries-the-desktop starts. Its first job is to add the
section address to the body of `POST /api/window`, so a watcher's Continue opens the window at the
same section. The Rust mirror of the closed address list (`ADDRESSES` in `ui/lib/words.js`)
arrives in m2-02 with its first Rust consumer.

## Landed

| Slice | What | Commit |
|---|---|---|
| m2-01-settings-tab | Settings, a tab with a left rail before About, replaces the Controls button and sheet. The rail has This screen, Sharing, Browser work, Notifications and Diagnostics, each with its state word, and `kit/controls.js` became `kit/settings.js`. Every tab has an address from the closed list `ADDRESSES` (`#settings/sharing`, and so on), written in place with `history.replaceState`. In Control the page can quit Ghostlight: `WebAct::Quit`, `POST /api/quit` (202), a quit port the service attaches to the facade, and `quit_ghostlight` on both transports. Watch shows the Continue line in each section and Closed shows the watching-off line. The connection sentence moved to Status. The UI-surface guard tests moved out of `desktop/mod.rs` into `workbench/surface_tests.rs`. The Policy page's ceilings end with the line "Ghostlight's own control page on this computer is never automated.", authored once. Forced-colors rules cover the band, the tabs, the rail and the state words. | see `git log` |

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
