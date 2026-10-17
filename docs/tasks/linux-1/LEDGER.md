# Linux batch 1: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `pass`, `fail` (with evidence),
`blocked` (with the reason), `skip` (with the reason).

## RESUME HERE

L01 passed. Next: L02, the full gate.

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

### L02 The full gate on Linux (todo)

From the repository root: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo test --workspace`, `npm test` in `extension/`, `node --test
"crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js"`, `node --test
tests/guards/`, `node --test "tests/workbench-*.test.mjs"`, `node tests/workbench-surface.mjs`,
`npm test --prefix packaging/npm`, `sh -n scripts/*.sh`, and `pwsh -NoProfile -File
scripts/check-repository-integrity.ps1`. These cover the Linux-only tests the ledgers owe: the
relay and edge tests (f07a to f12a in seams.md), the private-file modes (f09), the writer golden's
Linux file and Codex forwarding (c00 in design.md), the Linux icon resolver (e01) and the agent
table (c12). Record every failure exactly.

### L03 The journeys on Linux (todo)

Build into a fresh target directory and run the hardening suite's lanes
(`node tests/hardening-suite.mjs --lane=process` and `--lane=browser`), pointing
`GHOSTLIGHT_BIN_DIR` at the fresh build. Then `GHOSTLIGHT_TEST_LIVE_SYLIN=1 node
tests/frame-browser-journey.mjs` (the live mode owed by iframe-fixture-composed), and the Linux
installed drivers in `tests/linux/` as their README says. Record each journey's result.

### L04 Install the development channel (todo)

`pwsh scripts/dev-loop.ps1 -Plan`, then `pwsh scripts/dev-loop.ps1`. Then `ghostlight status`,
`ghostlight doctor`, `ghostlight --help`, `ghostlight call --help` (exit 0), and
`ghostlight install --dry-run --client cline` (c11). With the Linux completions installed,
`ghostlight install --client <Tab>` lists the agent and host ids.

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
