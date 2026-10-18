# Linux batch 1: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `pass`, `fail` (with evidence),
`blocked` (with the reason), `skip` (with the reason).

## RESUME HERE

L01 to L04 passed (L03 with a scope change: the owner directed on 2026-10-17 that acceptance
uses the real installation and the real system browser, never a sandbox). The owner then let this
session fix what it finds; L04's findings 1 to 7 and a ninth are fixed, pushed and proven on the
real install (see L04). L05 passed after six fixes in the shell, the service and the window, and
L06 after three, L07 after five and L09 after one (see each). The development build at 5107be98
is installed in the real `~/.ghostlight`, the system Chromium's ordinary profile is connected,
`browser.startup` is on_demand as before, and diagnostics are on. Next: L08 and L10 together,
since the only fresh home that is neither a sandbox nor another user is the real one after an
uninstall.

## Tasks

### L01 Prepare the machine (pass)

Install the prerequisites (see BOOTSTRAP), clone or update the repository on `dev`, and record
the versions: CachyOS release, kernel, desktop and session type (KDE or GNOME, Wayland or X11),
`rustc`, `cargo`, `node`, `pwsh`, the browser and its version.

Result, 2026-10-17, host test-01, repository on `dev` at 17a0c060:

- CachyOS rolling (ID_LIKE arch), kernel 7.2.9-2-cachyos, KDE Plasma 6.8.0 on Wayland
  (`WAYLAND_DISPLAY=wayland-1`, XWayland `DISPLAY=:0`).
- rustc 1.95.0 (59807616e 2026-04-14) and cargo 1.95.0 from `rust-toolchain.toml`; node v22.22.1,
  npm 10.9.4; Python 3.14.8; gh 2.96.0.
- pwsh 7.6.5: the extracted build already in the ignored `.tmp/linux-tools/powershell-7.6.5`,
  linked as `~/.local/bin/pwsh`. pacman needs a sudo password this session does not have; nothing
  else needed it.
- Browser: Chromium 154.0.8037.97 (Arch package, `/usr/bin/chromium`). Chrome for Testing
  152.0.7977.82 is in `.tmp/linux-tools/chrome-linux64`. No Google Chrome, no Brave.
- webkit2gtk-4.1 2.54.1, dbus 1.16.2, libayatana-appindicator 0.6.1, gtk3 3.24.52.
- Zed 1.24.3 from pacman (for L09). No Visual Studio Code.

Baseline before any install: no Ghostlight process ran. `~/.ghostlight` held only 1.x-launcher
directories (`bin/v0.5.7` to `bin/v1.3.4`, `adapters/v1.1.1`, three `extension/v0.*`), with no
`bin/install.json` and no service in `bin`. Chromium's `org.sylin.ghostlight` host manifest,
`~/.local/bin/ghostlight` and the Ghostlight entries of 14 agent files (Claude Code, Codex,
Cursor, Zed, VS Code, Goose, OpenCode, Crush, Qwen Code, Kiro, Windsurf, Junie, Copilot CLI,
Continue) named a `.tmp/fleet-test-01` build that no longer exists. All of it is backed up in the
ignored `.tmp/linux-1/baseline-2026-10-17/`.

### L02 The full gate on Linux (pass)

From the repository root: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo test --workspace`, `npm test` in `extension/`, `node --test
"crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js"`, `node --test
tests/guards/`, `node --test "tests/workbench-*.test.mjs"`, `node tests/workbench-surface.mjs`,
`npm test --prefix packaging/npm`, `sh -n scripts/*.sh`, and `pwsh -NoProfile -File
scripts/check-repository-integrity.ps1`. These cover the Linux-only tests the ledgers owe: the
relay and edge tests (f07a to f12a in seams.md), the private-file modes (f09), the writer golden's
Linux file and Codex forwarding (c00 in design.md), the Linux icon resolver (e01) and the agent
table (c12). Record every failure exactly.

Result, 2026-10-17, at 17a0c060, default `target/`: every command passed, no failure.

- `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`:
  no warning.
- `cargo test --workspace`: 2251 passed, 0 failed, 1 ignored (the stand-in process
  `a_stray_stand_in_waits_when_asked`, ignored by design). Per binary: service lib 1917, relay
  lib 117, relay `golden4` 10, desktop lib 140, edge 51, bridge 13, store listing copy 2,
  elevation 1. The owed Linux runs are among them: `install::writer_golden` (both tests,
  including `native::this_computer_s_own_paths_are_written_as_the_native_golden_records` against
  `native/linux.txt`), the Codex forwarding tests, `install::agent_list`, all 16
  `install::icons::` tests (the XDG, Flatpak and Snap order and the theme PNG resolver), and the
  private-file tests of the log and audit writers.
- `npm test` in `extension/`: 492 pass. Page assets: 270 pass. `node --test
  "tests/workbench-*.test.mjs"`: 140 pass. `node tests/workbench-surface.mjs`: ok. `npm test
  --prefix packaging/npm`: 26 pass. `sh -n scripts/*.sh`: clean. The integrity script: links
  valid, ASCII rule holds.
- Guards: `node --test tests/guards/` as written fails on Node 22.22.1 with `Cannot find module
  .../tests/guards` (a directory argument is read as a module path, not searched). The glob form,
  `node --test "tests/guards/*.test.mjs"`, runs both guard files: 10 pass. CI names the files one
  by one. Nothing in the product is wrong; the command in this task is.

### L03 The journeys on Linux (pass, scope changed)

Build into a fresh target directory and run the hardening suite's lanes
(`node tests/hardening-suite.mjs --lane=process` and `--lane=browser`), pointing
`GHOSTLIGHT_BIN_DIR` at the fresh build. Then `GHOSTLIGHT_TEST_LIVE_SYLIN=1 node
tests/frame-browser-journey.mjs` (the live mode owed by iframe-fixture-composed), and the Linux
installed drivers in `tests/linux/` as their README says. Record each journey's result.

Result, 2026-10-17, built into `.target-lane-linux-1` from de1b74f0 (later commits changed only
tests and docs). `GHOSTLIGHT_TEST_BROWSER` was Chrome for Testing 152.0.7977.82.

- Process lane: passed, source unchanged during the run. build, journey-helpers,
  shell-installer, edge-parent, fixed-installation, process, continuity, history-compatibility,
  provenance, cli, cli-powershell and workbench all passed.
- Browser lane: passed. browser-harness, script-browser, frame-browser, keyboard-browser and
  history-browser all passed.
- `GHOSTLIGHT_TEST_LIVE_SYLIN=1 node tests/frame-browser-journey.mjs`: 56 checks passed,
  including the live sylin.org composed document read.
- `node tests/linux/startup-recovery.mjs` (no display, four callers at once): passed, one
  service, recovery 5.6 s.

These lanes run isolated test homes and a throwaway browser profile. They are regression gates,
not integration evidence.

The `tests/linux/acceptance` drivers ran first, before the owner's direction below. 2.0 has no
Linux tar.gz, so the archive they extract was a tar of a release build's three executables in
one top directory. The area had to be short (`/run/user/1000/gl1`): under this checkout's path
the test home's `run/relay.sock` passes the 108-byte socket path limit and the service exits.

- `start.mjs` failed: bubblewrap's `--bind / /` mounts nodev, so `/dev/null` cannot be opened
  inside it and the installer could not start the service. Fixed in the drivers (2de3e190, a
  Linux-only test fix), then it passed.
- `governance.mjs`: the embedded-pixel check sampled the frame's own input box on this machine's
  fonts. Fixed in d4d628a7, then 15 of 15 phases passed.
- `cold.mjs`: passed. The extension connected on its own 3.0 s after install with the worker
  awake and 1.5 s with it stopped.
- `managed/run.mjs` failed its first phase: MCP initialize was not held with an invalid signed
  source. The service it talked to ran in the host mount namespace, not the sandbox, so it never
  saw the sandbox's `/etc/ghostlight`, and a call succeeded. That is a sandbox artifact, not
  evidence about managed policy.
- `startup-check.py` was retired as superseded (71465b67). Not run: `plural.mjs` (no Brave),
  `npm/native.mjs` (no published 2.0 package), the 1.x-shaped `no-tray/*` and
  `portable-lifecycle.py`, and the guest build helpers.

Owner direction, 2026-10-17, during this task: sandboxed versions are false integration tests;
always use the real system browser. The driver results above are therefore not acceptance
evidence. Acceptance runs on the real installation (L04) with the system Chromium and its
ordinary profile; `tests/linux/installed-journey.mjs`, which uses the real registered native
host and the running browser, runs after L04. The owner then authorized removing all
bubblewrap content: the `tests/linux/acceptance` drivers (the guest build roots included),
their runner and environment template, and `installed/worker.mjs`, which read a dedicated
browser's DevTools port, are gone; `tests/linux/README.md` says what is left and why.

For the coordinator, seen while diagnosing (no product change made):

1. When the service cannot start, the installer says "did not start within 30 s", both when
   the spawn failed at once (`/dev/null` EACCES) and when the service exited at once. It waits
   the full 30 s either way and does not name the cause.
2. A service whose start fails writes its fatal error only to stderr, which is null when it is
   started detached. With diagnostics on, the log stops at `service starting`; the cause
   (`bind the relay endpoint: path must be shorter than SUN_LEN`) appears nowhere a person can
   read it.
3. The installer reads and hashes the service image twice per install.

### L04 Install the development channel (pass, with findings)

`pwsh scripts/dev-loop.ps1 -Plan`, then `pwsh scripts/dev-loop.ps1`. Then `ghostlight status`,
`ghostlight doctor`, `ghostlight --help`, `ghostlight call --help` (exit 0), and
`ghostlight install --dry-run --client cline` (c11). With the Linux completions installed,
`ghostlight install --client <Tab>` lists the agent and host ids.

Result, 2026-10-17, into the real `~/.ghostlight` (release profile, `.target-dev-loop`).
Installed: service 202f2972..., edge 78b3fa53..., `install.json` channel development, relay 4.

- The plan listed the 1.x user-policy move, three swaps, the 13 `bin/v*` directories to retire
  with their seven audit logs, and the four native-host and 18 agent registrations to rewrite.
  The deploy did exactly that, started the service and its tray shell, and replaced the stale
  `~/.local/bin/ghostlight` link. Cline (CLI) was left alone (finding 8).
- `ghostlight status`: development channel, service up, workbench on 127.0.0.1:41002, window
  closed with its icon in the tray. `doctor`: every program ready, four browsers registered,
  17 agents connected, goose and Continue detected for copy setup, icons for Cursor and Zed
  from their desktop entries. `--help` lists the agent and host ids; `call --help` exits 0.
- c11: `--client cline --dry-run` gives one "Would install" line for each of the two detected
  Cline hosts; with `--all-clients`, all four; `--client CODEX`, Codex alone; `--client nope`
  refuses with the ids and exit 1. `man ghostlight` shows both id lines.
- Completions: fish (this user's shell) and bash complete all 24 ids. zsh's `_ghostlight`
  offers them only after `~/.local/share/zsh/site-functions` is added to `fpath`; CachyOS's zsh
  does not search that directory (finding 6).
- The system Chromium on its ordinary profile (`Default`), started the way this machine's
  "Chromium with Ghostlight" Applications entry starts it (`--load-extension` with the repo's
  `extension/`), connected on its own: "1 browser connected", readiness Ready.
- `tests/linux/installed-journey.mjs --bin-dir ~/.ghostlight/bin --browser
  /usr/lib/chromium/chromium --exercise-installed-stack` against that browser, four runs:
  registration-reinstalled-with-browser-running and native-connector-crash-recovers pass every
  time; the service-crash phase fails every time (finding 1), so the later phases did not run.

Findings for the coordinator (no product change made):

1. After the service is lost (SIGKILL), the MCP edge answers the first request with
   `-32603 "Ghostlight restarted before this request finished."`, even a `tools/list` sent
   after the loss; the next request works on a new service. Four of four journey runs. A probe
   (initialize, list, kill, list): sent at once, failed one try in 0 ms and passed the other in
   515 ms; sent after 1 s or 3 s, passed in a few ms. A request that arrives while the edge
   reconnects is failed rather than held for the new service.
2. and 3. The installer's "did not start within 30 s" and the service's silent start failure,
   in L03.
4. `install --dry-run` prints Rust variant names to people: "Would install Ghostlight for Cline
   (CLI) (NeedsAttention)", "(Installed)", "(NotDetected)".
5. `install --dry-run --client nope` prints the whole dry-run summary before it refuses; the
   refusal could come first.
6. The zsh completion lands where zsh does not look by default (above).
7. 1.x leftovers the deploy neither removes nor names: `~/.ghostlight/adapters/v1.1.1`,
   `~/.ghostlight/extension/v0.*` (three) and the man pages `ghostlight-browser-connector.1` and
   `ghostlight-mcp-connector.1` in `~/.local/share/man/man1`.
8. Cline (CLI)'s 0.5.7-era entry keeps its command under `transport.command`, naming a
   `ghostlight-relay` that no longer exists. Ghostlight reads it as a foreign entry without a
   usable command and leaves it ("needs attention"). This follows the foreign-entry rule; noted
   in case such old Ghostlight entries should be recognized as Ghostlight's own.

What became of them. The owner then let this session investigate and fix what it found (the
BOOTSTRAP records it), with every solution judged by delight, and in an MCP the user to delight is
the model (docs/MEMORY.md). Each fix ran fmt, clippy and its crates' tests (the workspace when the
bridge changed), was deployed with `pwsh scripts/dev-loop.ps1` into the real `~/.ghostlight`, and
was checked there with the system Chromium:

1. Fixed in eaf7948d (ADR-0197 amendment of 2026-10-17). The edge holds a line whose write failed,
   and sends a lone `initialize` or catalog listing cut off by a lost service to the next service,
   once; tool calls are still answered "may have taken effect" and never sent again. Then the
   installed journey met two over-eager checks: it counted service processes the moment readiness
   returned, while the start that loses the lock waits 5 s by design, and its stopped-worker phase
   needed a dedicated browser's DevTools port, so a real run was always "blocked". Both aligned in
   9c439606. The journey now passes all 4 phases on the real install, every run (6 runs).
2. and 3. Fixed in bd29366c. A failed start leaves its reason in `run/start-failure` and in
   diagnostics (the bind failure included); the installer stops waiting once no process runs the
   service image and names the spawn error, the recorded reason or the timeout.
4. Fixed in d6c55ef3. The dry run says what each client would become: "Would leave Cline (CLI)
   as it is: the entry under Ghostlight's name is not Ghostlight's.", "Codex already has the
   current Ghostlight entry; nothing would change.", "Would remove Ghostlight from Zed."
5. Fixed in 1e67edee, and wider than seen: a real `install --client nope` stopped older programs
   and wrote the browser registrations, completions, man pages and Applications entry before it
   refused. Both selections are now checked first; on the real install the refusal exits 1 and
   changes nothing. The CLI journey pins it, and fails against the old order.
6. By design and documented: zsh has no user completion directory. The man page now gives the one
   `fpath` line and no longer names the retired Debian package (aadf76e6).
7. The connector man pages were 1.x's and are now retired by install and uninstall, recognized by
   their `.TH` line across all 36 released versions (00c6eccf); on the real install only
   `ghostlight.1` remains. The `adapters/` and `extension/v0.*` folders were never written by a
   release (hand copies from the 0.5 and 0.6 live lanes), so the installer leaves them: it removes
   only what Ghostlight wrote.
8. Unchanged (the foreign-entry rule).
9. Found while diagnosing finding 1: `diagnostics show` never read the edge's logs
   (`edge-mcp.jsonl`, `edge-browser.jsonl`) and said their diagnostics were off, and pruning never
   rotated them, so a full edge log went silent for good. Fixed in 22f882db and ad43f65c: the
   timeline merges them, `--component` filters them, a full one is rotated, and a filter no longer
   reports the components it left out as off.

### L05 The window and the tray (pass, after fixes)

From the Applications entry: the window opens once (a second launch brings the same window
forward); At a glance shows the summary pill, Details opens the agents and browsers shelves and an
agent's card; Integrations is the rail and pane; Settings works; the tray icon shows state and its
menu works; closing the window shows the one-time notice; Quit in Settings asks and then quits.
Owed: shell-m2.md m2-03 and m2-04 (Linux items).

Result, 2026-10-17, on the real installation, KDE Plasma 6.8 on Wayland. The window was driven as
a person would: the Applications entry (`gtk-launch`), clicks through a virtual pointer on
`/dev/uinput`, the tray menu through StatusNotifierItem and dbusmenu, KWin's own window list,
and screenshots. Every check passes after the fixes below; `tests/linux/installed/desktop-check.py`
(aligned with 2.0 in 9a879144) passes its five phases.

- One window: a second Applications launch, a burst of five and of eight `ghostlight open`, and
  Open after minimize each leave one active window and one shell (the window is rebuilt on
  Open, ADR-0120).
- At a glance: the band (Ready, counts, Full control, Pause), the pill ("1 agent" to "1 browser")
  with Details; Details opens the agents and browsers shelves; an agent's chip opens its card.
  Integrations: the rail ("Needs a look": Cline (CLI); 17 connected) and the pane with Fix,
  Locate and Copy setup. Settings: each section opens, Density takes effect at once.
- Tray: the plain ghost while ready, the amber dot while paused; Pause and Resume swap which
  is enabled, the window band and Settings follow, `doctor` says Paused. Open and Quit always.
- Close: the first close sends one notice ("Ghostlight is still running") and writes
  `state/tray-hint`; the second sends none.
- Quit in Settings asks ("Quit now? Agents lose the browser until Ghostlight starts again.")
  and quits: service and shell end within 1 s and the quit latch is set; the system Chromium
  does not bring Ghostlight back in 15 s; the Applications entry does, and the browser
  reconnects by itself within about 15 s.
- m2-03: SIGTERM ends the shell in 105 ms, SIGHUP in 55 ms, no child left; presence brings the
  tray back. m2-04: three launches in two minutes latch did_not_stay; `ghostlight open` clears
  it.

Defects found and fixed (each deployed and checked on the real install):

1. da28f0f1: a shell with only its tray reconnected about 18 s after a lost service (tao's
   WaitUntil is a plain wait on Linux), so presence launched a second shell, two tray icons
   showed, and the window then opened by itself (3 of 5 crashes). The shell keeps its own clock;
   it reconnects in about 0.5 s.
2. f546c124: the tray icon's hover read `org.sylin.ghostlight`; it reads Ghostlight. (Linux keeps
   the product name: tray-icon cannot set a tooltip there.)
3. 17ec7571: status said "window is closed" over an open window and "shown" over a closed one
   after a restart, and a duplicate tray launch made the holder show its window. The shell's
   opening now says what it shows (`window` or `tray` in the relay channel), and the service
   believes it.
4. 240fcdeb: `ghostlight open` with no shell running (after the latch, for example) started a
   shell that never showed a window and ignored every later open: the window was replaced the
   moment it was built, and tao never reports a window dropped that early. The shell records the
   end of the windows it drops, and a person's start answers the service's follow-up show.
5. 0a09d95b: the band said "1 sessions - 1 browsers".
6. 906bd254: the did_not_stay sentence asserted a missing WebKitGTK; it now names it as the
   thing to check if the window ends again.

Seen and left: a forced test that pauses the shell (SIGSTOP) with its window open and kills the
service ends the shell, because its process group is orphaned with a stopped member and the
kernel sends SIGHUP; a shell is never stopped in use. "Unknown agent" on restored history rows
is by design. A Settings lag after a tray pause, seen once, did not reproduce.

### L06 Headless and SSH (pass, after fixes)

Over SSH (no `DISPLAY`, no `WAYLAND_DISPLAY`): `ghostlight status` reports `no_display`; a service
started by `ghostlight call` exits 10 minutes after the last command and `status` then says it is
not running; a `call` a few seconds before that idle exit still gets its answer (m2-06 and m2-08 in
shell-m2.md). `kill -TERM` of a service ends its MCP and browser streams and leaves no orphan
(f07g in seams.md).

Result, 2026-10-17, on the real installation. Every Ghostlight command ran in the environment an
SSH login gets on test-01: no `DISPLAY` or `WAYLAND_DISPLAY`, the session's runtime directory and
bus, and `SSH_CONNECTION` set (the account's SSH keys do not log in to itself, so the login was
reproduced rather than made). Nothing else touched the service between commands.

- No display: a `call` with nothing running starts a `command` service with no display variable
  in its environment; `status` says "No Ghostlight window: no display is reachable from the
  service." and `--json` carries `no_display`; no shell starts.
- Idle exit (m2-06): with a `status` 5 minutes after the call, the service left 600.5 s after
  that status, and `status` then said it is not running. A later service left 600 s after its
  last call.
- A call at the idle exit (m2-08): a call 0.27 s before the measured exit was answered by the
  running service in 0.02 s, which armed its span again; a call 0.08 s after it found the old
  service gone and was answered by a new one in 1.58 s. Neither said "did not become ready in
  time".
- f07g: `kill -TERM` of the service, with an initialized MCP session and the browser connected,
  ended it in 22 ms. The MCP edge recorded `service_disconnected` with `session_end` and exited 0;
  the browser edge recorded `session_ended` and exited. `kill -TERM` of an MCP edge exits 0 with
  `session_ended`. Nothing was left running.

Defects found and fixed (each deployed and checked on the real install):

1. cb37cdbf: a call to a service with no display ended with "Ghostlight could not start the
   selected browser." and "Start the selected browser yourself", which the model cannot act on.
   The launch now leaves startup to the person (ADR-0149 amendment): the call says "No browser is
   connected. Ask the user to open a Chromium browser window with the Ghostlight extension
   installed, then repeat the call." Chromium opened at the desktop then reached the SSH-started
   service in about 1.5 s, and the repeated call worked.
2. c014f161: the step after a failed launch or a launched browser that never connected said
   "Start the selected browser yourself"; it now asks the model to ask the user (ADR-0147
   Decision 3).
3. 56c49dc4: after an agent started Ghostlight over SSH, `ghostlight open` at the desktop waited
   15 s for "did not open in time. Run 'ghostlight open' to try again", every time, and the
   connected browser kept that service from idling out. When the service answers `no_display`,
   `open` now starts the shell from the person's own session (as the Applications entry already
   did): one active window in 54 ms, and a second `open` reuses it. Over SSH, `open` says why at
   once (7 ms).

### L07 The extension and the browser (pass, after fixes)

Load `extension/` unpacked in Chrome or Chromium. The popup and the options page in each state
(j06, j07 in journey.md); `chrome-extension://<id>/setup.html`. Then real agent work through
Ghostlight on https://sylin.org/ghostlight/demo/ stages (navigate, find, fill a form, click, the
composed iframe stage, a screenshot handed back to the studio page). With the browser closed and
`browser.startup` at its Linux default (manual), an agent request names the browser and starts
nothing; with on_demand set, it starts the browser that last connected.

Result, 2026-10-17, on the real installation with the system Chromium's ordinary profile; the
extension is loaded from `extension/` by the machine's `ghostlight-browser` launcher
(`--load-extension`). The agent was a long-lived MCP session on `ghostlight mcp`; the popup, the
options page, the setup page and the window were driven through the real pointer and read from
screenshots.

- Popup (j06): Ready ("No agent is browsing."), "linux-1 agent is browsing." while a tab-bound
  call runs (with the page's working indicator), Paused (accent Resume, a pause badge), Session
  ended (Start a new session; a call meanwhile gets the fixed directive), Not connected ("Open
  Ghostlight on this computer. It is in your applications."), Recording ("Recording in 1 tab.",
  a REC badge) and Release debugger session, which detaches and leaves. Not covered: Update
  needed (needs an older service), the key cap (this Chromium has no shortcut set), Edge.
- Options (j07): the Not connected card with no setup button; every switch saves and survives a
  reload; captions dim while effects are off. Not covered: 390 px, reduced motion, the
  diagnostics download, the setup-button state (needs the host absent).
- Setup page: "Open Ghostlight on this computer" and "Waiting for this computer" while the
  service is down; on its own, once Ghostlight starts, "Your browser and this computer are
  connected. One step left: your agent."; Open Ghostlight brings the window forward.
- Demo work: the desk (find, click, the bell's prompt answered), the form filled and submitted on
  a tab not in front ("Form received."), the embedded partner form across its frame ("Application
  received", "Composed frame verified"), the Capture Studio (a screenshot handed back, "developed
  locally"), and a recording saved into the page.
- `browser.startup`, set through the Policy tab: manual answers in 9 ms that the user should open
  a Chromium window and starts nothing; on_demand starts `/usr/lib/chromium/chromium` with no
  arguments. On this machine that browser has no extension (only the launcher loads it), so the
  call ends with the handshake timeout; a store or "Load unpacked" install would connect.

Defects found and fixed (each deployed and checked on the real install):

1. efe54caf: a screenshot of a session tab that is not the one its window shows (a second tab,
   or two sessions in one Ghostlight window) waited for a frame Chromium never draws, failed at
   its deadline (8 s, or 30 s), and told the model to use a longer timeout. It is now refused at
   once (`tab_not_showing`) with the focus step: 9 ms, then 242 ms after focus.
2. 5c4adb4d: `diagnostics show` printed the invocation id and the tool as one word.
3. be54ad44: browser_record status said "Recording ... so far" for a stopped recording.
4. 6734f99b: a save whose target is not a file input said the recording could not be made and
   advised discarding it, though the GIF was fine. It now says the page did not take it and the
   recording is kept (`recording_not_attached`), with a file input or a download as the step.
5. 7769fd97: the browser the service started on demand was never reaped (a `<defunct>` chromium
   under the service until it exited) and shared the service's process group.

Seen and left, for the owner or the coordinator:

- The extension names no browser on plain Chromium (it treats the Chromium brand as generic), so
  no last-browser record is written for it (ADR-0149 amendment: a name that maps to none writes
  nothing). With Chrome remembered and the person since moved to Chromium, on_demand would start
  Chrome. Linux defaults to manual, which starts nothing.
- An upload's sentence calls a file input a "text field": the page assets give `input type=file`
  the textbox role, which inspect also shows. Changing the role changes what inspect tells the
  model, so it is a deliberate choice.
- During a flow, the popup says no agent is browsing between steps that touch no tab, since its
  activity follows the notices of tab-bound work.

### L08 Setup on a fresh home (todo)

In a fresh test user or a fresh `GHOSTLIGHT_HOME`, `ghostlight install` in a terminal prints the
summary (version line, location, agents line, one Next line) and opens the window at the browser
step (j15); the setup view's browser step reads the real profile with and without the extension
(j10, j12); Add and Open start the named browser (j13).

### L09 Agent icons (pass, after a fix)

With an agent app installed from a package (Zed or Visual Studio Code), the agent shows the app's
own icon in the pill, the shelves, the card and the Integrations rail and pane; Integrations says
where it came from; `ghostlight doctor` reports it (e01 to e03 and f01 in design.md).

Result, 2026-10-17, with Zed 1.24.3 from the Arch package (`/usr/bin/zeditor`, its desktop entry
`dev.zed.Zed.desktop`, icon `hicolor/512x512/apps/zed.png`). The cache holds `zed.png` and
`zed.json` (`found_in` desktop_entry), closed to other users (0700, 0600). `doctor` says "Agent
icon: Zed -- from the installed Zed app (desktop entry)" and lists the agents that keep their
monograms. Integrations says "Icon from the installed Zed app." With Zed started and connected,
its icon shows in the pill, the shelf chip, the card head, and the rail and pane; the watching
view's symbol is the same address. Cursor shows its app icon too. Not covered: removing the
package and pressing Re-check (needs root).

Defect found and fixed (deployed and checked):

1. 5107be98: Zed showed its dark rounded plate inside an off-white square, a box in a box. Its
   icon, like most Linux and macOS app icons, sets the plate in a transparent margin, so the
   outer-ring test read 0.00 and put it on a ground (Cursor escaped only because its 1024 px
   icon is over the tiling limit). The plate is now judged at the edges of the drawn area; Zed
   is served as cached. ARCHITECTURE says the new rule.

### L10 Clean install for the release bar (todo)

After L01 to L09, uninstall (`ghostlight uninstall`), confirm nothing of Ghostlight is left in the
home, the agents' files or the browser registrations, and install again from scratch. Record what
a first-time person sees. The release candidate's pinned install (`get.sh` with
`GHOSTLIGHT_VERSION`) is a later task, once the owner publishes `v2.0.0-rc.1`.

## Notes
