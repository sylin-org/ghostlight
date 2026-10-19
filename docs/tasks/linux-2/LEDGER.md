# Linux batch 2: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `done` (with commits and evidence),
`blocked` (with the reason).

## RESUME HERE

Opened 2026-10-19 by windows-claude. Part 1 started the same day. The owner then narrowed this
machine's part to CachyOS (Notes, "Scope, 2026-10-19"): L2-01 and L2-03 are out of this batch,
and the owner validates the other Linux environments. Part 2 starts when windows-claude's message
says the head is ready (arch-1 integrated, deployed on Windows and pushed).

## Files this batch owns

`.github/workflows/release.yml`, `scripts/get.sh`, `scripts/get.ps1`,
`scripts/check-release-candidate.ps1`, `packaging/npm/**`, `tests/installer-shell.mjs`,
`tests/linux/**`, `tests/golden/cargo-tree/**`, and the release and installation docs
(`docs/RELEASE.md`, `docs/guides/installation.md`). Anything else: claim it in the chat first.

## Part 1: make Linux ready (start now)

| Task | What | Status |
| --- | --- | --- |
| L2-01 | Alpine (musl) gets a working Ghostlight from the release | blocked: owner, CachyOS only |
| L2-02 | The install scripts install from a local release bundle | todo |
| L2-03 | Bluefin readiness: Flatpak browsers, SELinux, read-only /usr | blocked: owner, CachyOS only |
| L2-04 | What Claude Code does with the plugin and the installer's entry together | todo |

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

## Part 2: the full test (after windows-claude's go)

| Task | What | Status |
| --- | --- | --- |
| L2-10 | The full gate and every journey on the go head | todo |
| L2-11 | The release workflow's bundle, built and verified | todo |
| L2-12 | A clean install from the bundle, and real use | todo |
| L2-13 | The same with a Flathub browser | blocked: owner, CachyOS only |
| L2-14 | Alpine from the bundle | blocked: owner, CachyOS only |
| L2-15 | The owner's steps for Bluefin and Alpine | todo |

**L2-11.** Dispatch the release workflow on the go head, download the bundle, and run
`scripts/check-release-candidate.ps1` (with `pwsh`) on it.

**L2-12.** Remove this machine's development install, then install from the bundle with the
real `get.sh` and L2-02's local source. Then: the extension from the bundle (unpacked, the store
has no 2.0 yet), `ghostlight install` for the agents found, a real browser task on the sylin.org
demo stages from an agent, the window and the tray, idle exit, a restart of the browser mid-session
(ADR-0203: the agent's handle reads "no longer open" when the page changed), and uninstall.

**L2-15.** A short page under `docs/testing/` with the owner's exact steps for Bluefin and Alpine:
where to get the bundle, the install command, what to try, and what each result means.

## Notes

**Scope, 2026-10-19.** The owner, while Part 1 started: "you shouldn't try to install
dependencies, I think - just make sure Ghostlight is working for this OS specifically. I'll deploy
ghostlight on different Linux environments, and validate against that." Asked whether the release
workflow should still build musl images on CI for Alpine, the owner chose "CachyOS only". So
nothing is installed on this machine (no podman, Flatpak or Alpine root), and L2-01, L2-03, L2-13
and L2-14 are out of this batch. Known and unchanged: the release's Linux images are
`x86_64-unknown-linux-gnu` only, and they cannot start on Alpine, whose musl system has no
`/lib64/ld-linux-x86-64.so.2` (test-03, September). L2-15's page still gives the owner's steps
for the other environments.
