# Linux batch 1: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `pass`, `fail` (with evidence),
`blocked` (with the reason), `skip` (with the reason).

## RESUME HERE

L01 to L04 passed (L03 with a scope change: the owner directed on 2026-10-17 that acceptance
uses the real installation and the real system browser, never a sandbox; L04 with findings for
the coordinator). The development build is installed in the real `~/.ghostlight` and the
system Chromium's ordinary profile is connected. Next: L05, the window and the tray.

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

### L05 The window and the tray (todo)

From the Applications entry: the window opens once (a second launch brings the same window
forward); At a glance shows the summary pill, Details opens the agents and browsers shelves and an
agent's card; Integrations is the rail and pane; Settings works; the tray icon shows state and its
menu works; closing the window shows the one-time notice; Quit in Settings asks and then quits.
Owed: shell-m2.md m2-03 and m2-04 (Linux items).

### L06 Headless and SSH (todo)

Over SSH (no `DISPLAY`, no `WAYLAND_DISPLAY`): `ghostlight status` reports `no_display`; a service
started by `ghostlight call` exits 10 minutes after the last command and `status` then says it is
not running; a `call` a few seconds before that idle exit still gets its answer (m2-06 and m2-08 in
shell-m2.md). `kill -TERM` of a service ends its MCP and browser streams and leaves no orphan
(f07g in seams.md).

### L07 The extension and the browser (todo)

Load `extension/` unpacked in Chrome or Chromium. The popup and the options page in each state
(j06, j07 in journey.md); `chrome-extension://<id>/setup.html`. Then real agent work through
Ghostlight on https://sylin.org/ghostlight/demo/ stages (navigate, find, fill a form, click, the
composed iframe stage, a screenshot handed back to the studio page). With the browser closed and
`browser.startup` at its Linux default (manual), an agent request names the browser and starts
nothing; with on_demand set, it starts the browser that last connected.

### L08 Setup on a fresh home (todo)

In a fresh test user or a fresh `GHOSTLIGHT_HOME`, `ghostlight install` in a terminal prints the
summary (version line, location, agents line, one Next line) and opens the window at the browser
step (j15); the setup view's browser step reads the real profile with and without the extension
(j10, j12); Add and Open start the named browser (j13).

### L09 Agent icons (todo)

With an agent app installed from a package (Zed or Visual Studio Code), the agent shows the app's
own icon in the pill, the shelves, the card and the Integrations rail and pane; Integrations says
where it came from; `ghostlight doctor` reports it (e01 to e03 and f01 in design.md).

### L10 Clean install for the release bar (todo)

After L01 to L09, uninstall (`ghostlight uninstall`), confirm nothing of Ghostlight is left in the
home, the agents' files or the browser registrations, and install again from scratch. Record what
a first-time person sees. The release candidate's pinned install (`get.sh` with
`GHOSTLIGHT_VERSION`) is a later task, once the owner publishes `v2.0.0-rc.1`.

## Notes
