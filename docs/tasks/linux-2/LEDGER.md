# Linux batch 2: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `done` (with commits and evidence),
`blocked` (with the reason).

## RESUME HERE

Opened 2026-10-19 by windows-claude. The owner narrowed this machine's part to CachyOS (Notes,
"Scope, 2026-10-19"): L2-01, L2-03, L2-13 and L2-14 are out of this batch, and the owner validates
the other Linux environments with L2-15's page. Every other task is done on CachyOS, Part 2 on the
go head 62bfa224 and its bundle from run 39297671342. After windows-claude's fixes, test-01 runs
the bundle of release run 39306759480 on 68419db8 (release channel 2.0.0, installed with `get.sh
--restore` and `GHOSTLIGHT_FROM`; every job of that run passed, `install-from-bundle` included) with
the owner's policy and diagnostics on; the home from before L2-12 was removed at
the owner's request on 2026-10-19. Open for windows-claude: the `AdvertisedCatalog` defect (Notes, "F1
on the go head") and the observations under L2-12.

## Files this batch owns

`.github/workflows/release.yml`, `scripts/get.sh`, `scripts/get.ps1`,
`scripts/check-release-candidate.ps1`, `packaging/npm/**`, `tests/installer-shell.mjs`,
`tests/linux/**`, `tests/golden/cargo-tree/**`, and the release and installation docs
(`docs/RELEASE.md`, `docs/guides/installation.md`). Anything else: claim it in the chat first.

## Part 1: make Linux ready (start now)

| Task | What | Status |
| --- | --- | --- |
| L2-01 | Alpine (musl) gets a working Ghostlight from the release | blocked: owner, CachyOS only |
| L2-02 | The install scripts install from a local release bundle | done |
| L2-03 | Bluefin readiness: Flatpak browsers, SELinux, read-only /usr | blocked: owner, CachyOS only |
| L2-04 | What Claude Code does with the plugin and the installer's entry together | done |

**L2-01.** The release builds `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu` only. On
Alpine the glibc images cannot start: their interpreter `/lib64/ld-linux-x86-64.so.2` is absent
(test-03, September), and the error said nothing useful. Make Alpine work from the release:
`ghostlight` and `ghostlight-service` built for `x86_64-unknown-linux-musl` (static, now that the
service no longer links GTK, ADR-0199), and `ghostlight-desktop` built in an Alpine container
against Alpine's WebKitGTK. The release workflow, its manifest and checksums, the SBOMs and
`scripts/check-release-candidate.ps1` carry the new images. `get.sh` and the npm launcher choose
the musl images on a musl system, and anything that still cannot run says so plainly. Prove it in
an Alpine VM or container on this machine: install from the bundle, the service starts, the edge
connects, and a real browser task runs if Alpine's Chromium can run there. Record what only the
owner's Alpine machine can show.

**L2-02.** `get.sh` and `get.ps1` install only from a GitHub release. With no rc.1 tag, the owner's
Bluefin and Alpine tests need the real install script to run against the release workflow's
bundle. Add a local source (for example `GHOSTLIGHT_FROM=<unpacked bundle directory>`) that
verifies `SHA256SUMS` exactly as the GitHub path does and refuses anything unverified. Document it
in each script's head and in the installation guide's testing section. Tests for both scripts;
the PowerShell side is checked by CI's Windows jobs.

Result, 2026-10-19, done. `GHOSTLIGHT_FROM=<folder>` in `get.sh` and `get.ps1` names the release
workflow's unpacked `release-candidate` artifact (SHA256SUMS beside `assets`, with
`release-candidate.json` naming the version) or a folder of release downloads (SHA256SUMS beside
the images). Each image is copied into the script's temporary folder and the copy is checked
against SHA256SUMS by the same rules as a download: one well-formed line per image, and the hash.
Anything unbound, missing or different is refused before anything runs, and the scripts ask the
network for nothing (`gh attestation verify` still runs when the GitHub CLI is present). It
cannot be combined with `GHOSTLIGHT_VERSION`; `--restore` and `GHOSTLIGHT_NO_REGISTER` work with
it. Documented in both script heads, the installation guide ("Test a build before its release")
and RELEASE.md's cutover steps 1 to 3, which no longer publish `v2.0.0-rc.1`. Tests:
`tests/installer-shell.mjs` passes here (both layouts, restore, no-register, corrupt, missing,
unbound, no SHA256SUMS, version combination, absent folder, and no request in any of them).
`tests/installer-powershell.ps1` gains a local bundle install with no web request, a corrupt image
refused with the installation unchanged, and the version combination refused; it runs on CI's
Windows job. Here, `get.ps1` in PowerShell 7.6.5 was run against stub images: the install and the
seven refusals behave as in `get.sh`. Found on the way, for Part 2: the bundle's extension ZIP is
the store upload, without the development key, so an unpacked copy of it gets an ID the browser
registration refuses. Testers load `extension/` from the bundle's revision instead (guide).

**L2-03.** Bluefin is Fedora Atomic: read-only `/usr`, SELinux enforcing, browsers from Flathub,
Homebrew for command-line tools, GNOME. Check Ghostlight against each part you can reach here: a
Flathub Chrome or Chromium on this machine (ADR-0165's host activation), an agent installed from
Homebrew if one applies, and the tray under GNOME if a GNOME session is available. Fix what
fails. List what only Bluefin can show (SELinux above all) for the owner's run.

**L2-04.** The owner decided the Claude Code plugin must "work regardless": plugin only, installer
only, or both, the person ends up with one working Ghostlight and one set of tools. Claude Code's
docs say a plugin's MCP server that points at the same command as a configured server is dropped
as a duplicate, but not whether `args` count, and nothing about two servers with different
commands. In an isolated Claude Code home (`CLAUDE_CONFIG_DIR` pointing at a scratch folder, so
this session is not touched), install the plugin from this repository's marketplace and
Ghostlight's own entry, and record with `claude --debug` and a process listing: whether one or
both servers start, the parent process of each, and whether a `notifications/tools/list_changed`
from the second server is honored. Report the facts in the chat; windows-claude builds the
installer and service side of "works regardless" from them.

Result, 2026-10-19, done (reported in the chat). Claude Code 2.1.303 on test-01, in an isolated
`CLAUDE_CONFIG_DIR` under the session's scratch folder. The plugin came from this repository's
marketplace (`ghostlight@sylin-org-plugins` 1.0.1, server `npx -y ghostlight`); the installer's
entry was written as `ghostlight install` writes it (`/home/test/.ghostlight/bin/ghostlight`,
args `["mcp"]`, env `{}`). npm still serves 1.3.14, so `npx` was a PATH stand-in that logged each
start and ran this repository's 2.0 launcher, which runs the installed edge. No login was used: a
local stand-in for the Messages API on 127.0.0.1 answered each turn and recorded the tools each
request offered the model.

1. **Both start.** `plugin:ghostlight:ghostlight` and `ghostlight` both connect, 1 ms apart, in
   print mode, in the interactive TUI and in `claude mcp list`. Each request offers the model 46
   Ghostlight tools: 23 `mcp__ghostlight__*` and 23 `mcp__plugin_ghostlight_ghostlight__*`.
2. **The duplicate rule.** Claude Code drops the plugin's server ("Suppressing plugin MCP server
   ... duplicates manually-configured ...") only when a configured server has the same command and
   the same args. Args count: `npx -y ghostlight mcp`, or `npx` with no args, beside the plugin's
   `npx -y ghostlight` starts both. The name does not count (an entry named `foo` suppressed it),
   and neither does `env`. `${HOME}` in a plugin's command is expanded before the comparison: a
   scratch copy of the plugin with `${HOME}/.ghostlight/bin/ghostlight` and `["mcp"]` was
   suppressed by the installer's entry. The configured server always wins.
3. **Parents.** The installer's edge is a child of the Claude Code process. The plugin's edge is
   its great-grandchild: Claude Code, `npm exec ghostlight` (npx), the npm launcher (node), then
   `ghostlight mcp`. The real npx chain was read from npx running this repository's package with
   an isolated npm cache and `--offline`.
4. **list_changed.** Honored as a re-list: "Received tools/list_changed notification, refreshing
   tools", and a `tools/list` within 4 ms, in print mode and the TUI (the notification was
   injected by a logging proxy in front of the plugin's server). Additions reach the model: a server
   whose first `tools/list` had no tools and whose re-list had 23 was offered with 0 tools on the
   first turn and 23 on the next. Removals do not: after a re-list answered with no tools, or one,
   the next turn still offered all 23. So a second server keeps itself out only by answering its
   first `tools/list` with no tools, and can add its tools later.

Side finding, not in this batch's files: the installer writes Claude Code's entry to
`~/.claude.json` (`install/mod.rs`, the Claude Code path), but a Claude Code with
`CLAUDE_CONFIG_DIR` set reads `$CLAUDE_CONFIG_DIR/.claude.json`, so that person's Claude Code
never sees the entry.

## Part 2: the full test (after windows-claude's go)

| Task | What | Status |
| --- | --- | --- |
| L2-10 | The full gate and every journey on the go head | done |
| L2-11 | The release workflow's bundle, built and verified | done |
| L2-12 | A clean install from the bundle, and real use | done |
| L2-13 | The same with a Flathub browser | blocked: owner, CachyOS only |
| L2-14 | Alpine from the bundle | blocked: owner, CachyOS only |
| L2-15 | The owner's steps for Bluefin and Alpine | done |
| L2-16 | CI installs the candidate from its bundle and uses it, as a person on Linux does | done |

**L2-10.** Result, 2026-10-19, done, on ebfc4b4e (the go head 62bfa224 plus docs and
coordination) on test-01. `tests/hardening-suite.mjs --lane=all` on a hidden KWin display in its
own D-Bus session, with the system Chromium 154.0.8037.97 as the test browser: format, clippy,
the Rust tests, the extension, both launchers, the WinGet archive, the policy grammar and every
process journey passed, and so did the browser journeys frame, keyboard and history. One,
script-browser, timed out on the extension's hello. Cause, in this harness only: on KDE, Chromium
looks for its password store through D-Bus, and in an isolated D-Bus session that store does not
answer, so the extension worker's first request to the journey's loopback bridge is held. A probe
showed the worker had opened its port and its `/open` request never settled. The journey passed
3 of 3 on the real session bus and 3 of 3 on the isolated bus with `--password-store=basic`, and
CI (Chrome for Testing, no desktop) never meets it; no product or test change is needed. The
Linux checks of `tests/linux` passed on the go head deployed in place: `startup-recovery.mjs` (four
callers, one service, recovered in 5.6 s), `installed-journey.mjs` (all four cases, with the
owner's Chromium connected) and `installed/desktop-check.py` (all five window phases under KDE
Plasma on Wayland). After the deploy, the owner's Chromium reconnected to the new service by
itself.

**L2-11.** Dispatch the release workflow on the go head, download the bundle, and run
`scripts/check-release-candidate.ps1` (with `pwsh`) on it.

Result, 2026-10-19, done. Run 39297671342 of `release.yml` on 149c04d6 (the go head plus tests,
docs and coordination; CI 39296552280 green on it, 11 of 11) passed every job, the new
`install-from-bundle` on ubuntu-22.04 and ubuntu-24.04 included: each printed "PASS release
install" after "Ghostlight is uninstalled.". The `release-candidate` artifact, downloaded to
`.tmp/release-candidate-39297671342`, passes `scripts/check-release-candidate.ps1`: "Release
candidate verified: version 2.0.0, source 149c04d67b27f68f211137f6b6976d788dcca4ed, 16 artifacts."
`sha256sum -c` binds all 16 and `gh attestation verify` accepts the Linux service image. This is
the bundle the owner's page for Bluefin and Alpine names.

**L2-12.** Remove this machine's development install, then install from the bundle with the
real `get.sh` and L2-02's local source. Then: the extension from the bundle (unpacked, the store
has no 2.0 yet), `ghostlight install` for the agents found, a real browser task on the sylin.org
demo stages from an agent, the window and the tray, idle exit, a restart of the browser mid-session
(ADR-0203: the agent's handle reads "no longer open" when the page changed), and uninstall.

Result, 2026-10-19, done, on test-01 with the L2-11 bundle:

- Uninstall of the development install: `ghostlight uninstall` removed the four browser
  registrations, the command, the Applications entry and the entry in all 18 agents, and stopped
  the service; the agents' running MCP edges are theirs and stayed. The home was moved to
  `~/.ghostlight.l212-before`.
- Install: the real `get.sh` from the bundle's revision on GitHub with `GHOSTLIGHT_FROM` printed
  "Checksums and build provenance verified."; `install.json` reads the release channel and the
  bundle's service hash; the owner's Chromium connected by itself and doctor read Ready.
- The extension: the owner reloaded `extension/` at the bundle's revision (one packaged file,
  `lib/buffers.js`, had changed since the morning's load).
- Real use: a fresh Claude Code process (`claude -p --model haiku`, 12 turns, $0.0078) through the
  installer's entry rang the demo desk's bell, answered its prompt and typed in its field; the page
  read back both.
- The window and the tray: `installed/desktop-check.py` passed all five phases; the tray icon is
  registered with KDE's StatusNotifierWatcher.
- F1 on the bundle install: 23 Ghostlight tools at every step, never 46; with no service replace
  in between, the holder did get `list_changed` on the policy change and on its restore; `/mcp`
  disabling the holder brought the plugin's 23.
- Idle exit: measured only on the bundle's images in a scratch home with nothing connected, where
  the service exited by itself 632 s after the install. On test-01 itself the owner's agents and
  browser always hold a stream, so idle exit is not validated on this machine.
- A browser restart mid-session, on the owner's Chromium and the installed bundle: an MCP client on
  the installed edge opened the demo desk and the Ghostlight page in new tabs; Chromium was quit
  with SIGTERM and started again with `--restore-last-session`, and its extension reconnected
  within 1 s. Both of the agent's handles then read "That tab handle names no open tab in this
  session." (`tab_unavailable`), ADR-0203's "no longer open", because Chromium restores tabs under
  new ids; the session listed no tabs, and the agent reopened the desk and read it.
- Uninstall again was clean, and the reinstall from the bundle reconnected the owner's Chromium
  by itself.

Observations, for windows-claude:

1. Explained, not a defect: after the extension's reload the owner's Chromium was not connected
   for a few minutes because it had exited at 14:07:32 (user journal); the first agent call
   (`browser_navigate`) at 14:11:08 started Chromium as on_demand says, its relay started in 0.7 s
   and the page opened in 1.2 s.
2. `browser_tabs` with `list` succeeds with "Listed 0 tabs in this session." when no browser is
   connected and the session has no tab, while its catalog description says list "refuses when
   none is connected".

**L2-15.** A short page under `docs/testing/` with the owner's exact steps for Bluefin and Alpine:
where to get the bundle, the install command, what to try, and what each result means.

Result, 2026-10-19, done:
[docs/testing/linux-other-environments-2026-10-19.md](../../testing/linux-other-environments-2026-10-19.md).
It takes the bundle from the L2-11 run below and the extension from the bundle's revision. For
Bluefin it states that a Flathub browser cannot start Ghostlight by design (ADR-0197 Decision 22)
and how to go further with a layered native Chromium, and it lists the SELinux, tray and uninstall
checks. For Alpine it states the expected failure and why.

**L2-16.** Assigned by windows-claude in the chat ([0060]), approved by the owner in the CI review
of 2026-10-19: CI does not prove the path real people take. A job in `release.yml`, after the
candidate bundle is assembled, on ubuntu-22.04 and ubuntu-24.04 under xvfb as the runner user, that
installs from the bundle with `GHOSTLIGHT_FROM` and the real `get.sh`, starts Ghostlight, reads
doctor as Ready, runs one real browser task against Chrome for Testing with the extension loaded
unpacked from the bundle's revision, and uninstalls.

Result, 2026-10-19, done. `install-from-bundle` in `release.yml` runs `tests/linux/release-install.mjs`.
The browser runs in Chrome's own user data folder, where Chrome looks for native hosts, so its
extension connects through the registration the install wrote, not a test bridge. The task runs
from an MCP client on the installed edge that agents' entries name, against a page the script
serves on 127.0.0.1. Uninstall must remove Chrome's registration and the `ghostlight` command and
leave no service running. The script refuses to run outside CI. Rehearsed on test-01 before the
first release run, in a scratch home with the suite's build, a stand-in `google-chrome` on PATH so
the install registers Chrome as it does on a runner, and the system Chromium: it passed end to end.
The rehearsal found three things the script now handles: a registration and a process's executable
compare as resolved paths, and the browser keeps its passwords in its profile
(`--password-store=basic`), because a browser that looks for a keyring can hold its first requests
while it waits. The first release run on the go head is L2-11's.

## Notes

**F1 on the go head, 2026-10-19 (for L2-12).** On the go head deployed in place, in the isolated
Claude Code home of L2-04 with the plugin (now `npx -y ghostlight@2`, run by the stand-in that
starts this repository's 2.0 launcher) and the installer's entry, and the local Messages API
stand-in: `/mcp` listed `ghostlight` with 23 tools and `plugin:ghostlight:ghostlight` with "no
tools". One turn per step offered the model 23 Ghostlight tools, never 46: with both entries
connected, after the service was replaced (`scripts/dev-loop.ps1`), after a policy change, and after
the policy was restored byte for byte. After `/mcp` disabled `ghostlight`, the plugin's server sent
`tools/list_changed` and the next turn offered its 23. Found on the way, for windows-claude: after a
service replace, a session whose client has not listed tools again is never sent
`tools/list_changed`. `AdvertisedCatalog` starts empty in the new service, and nothing is compared
until the client asks for a catalog, which a client that already listed with the previous service
does not do. Measured with a plain MCP client on the installed edge: dropping `execute` from the
policy sent `list_changed` within 8 s to a fresh session and to a resumed one that had listed again,
and sent nothing to a resumed one that had not, although its catalog is then 22 tools. So the
policy step of F1 never reached Claude Code, and a tool that a policy change or a newer service
adds does not reach a running agent until it restarts.
Fixed by windows-claude in 0285ce2d and measured again on 2026-10-19 on test-01's real
installation with 816154bf deployed (the owner directs tests on this machine, not in a scratch home):
the plain MCP client, after a replace and no re-list, got `list_changed` 1.3 s after the replace
returned; a policy that drops `execute` sent another and a re-list read 22 tools; the restore sent a
third and a re-list read 23. In Claude Code, F1 with a replace now logs `list_changed` from the
holder after the replace, on the policy change and on its restore, the standby stays silent until it
is promoted, and the model is offered 23 Ghostlight tools at every step.

**CachyOS check, 2026-10-19.** The development build at e9646471 was deployed on test-01 (it had
run 7abbdc39); `ghostlight doctor` reads Ready, with Chromium registered and every agent
connected. A first real task (`browser_navigate` to the demo desk) failed: "The Ghostlight adapter
did not connect in time after the browser started." No browser relay had run since 2026-10-18
21:22. Cause, on this machine: Chromium started with the session at 08:29:09, and the drive that
holds the repository (`/run/media/test/WORKBENCH`, /dev/sdb2) mounted at 08:30:27, so the unpacked
2.0 extension's folder was missing at browser start and Chromium dropped the extension from the
profile. The profile still holds the 1.x store extension, disabled. Owed: the owner loads
`extension/` unpacked again (the browser's own data is the owner's). An unpacked extension stays
only if its folder is there when the browser starts. Seen on the way, for windows-claude: the
extension probe counts the disabled 1.x store copy as holding the extension, so the refusal said to
open a window of the browser, which cannot help; and the morning service's diagnostics file ends in
a cut-off record at its exit.
Then the owner loaded `extension/` unpacked again, the browser relay started and connected, and
the first task passed through this session's Ghostlight tools: `browser_navigate` opened the demo
desk, `browser_find` found "Ring the bell", `browser_click` opened its prompt, `browser_dialog`
answered it, and `browser_read` showed the page's readout of the answer. `ghostlight status` reads
"2 agents and 1 browser connected". Closing the tab was kept by preserve-tabs, as the owner set it.

**Scope, 2026-10-19.** The owner, while Part 1 started: "you shouldn't try to install
dependencies, I think - just make sure Ghostlight is working for this OS specifically. I'll deploy
ghostlight on different Linux environments, and validate against that." Asked whether the release
workflow should still build musl images on CI for Alpine, the owner chose "CachyOS only". So
nothing is installed on this machine (no podman, Flatpak or Alpine root), and L2-01, L2-03, L2-13
and L2-14 are out of this batch. Known and unchanged: the release's Linux images are
`x86_64-unknown-linux-gnu` only, and they cannot start on Alpine, whose musl system has no
`/lib64/ld-linux-x86-64.so.2` (test-03, September). L2-15's page still gives the owner's steps
for the other environments.
