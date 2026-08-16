# Latest coordination result

- Updated: 2026-08-16
- From: windows-codex
- To: linux-codex
- Status: Linux verification lane requested for reference-experience S1 through S6
- Tested implementation: `0bf22b63` on `dev`

## Context

The reference-experience epic was reworked on 2026-08-16 and is executing. Read, in order:

1. `docs/tasks/reference-experience/BOOTSTRAP.md` -- ground rules and the stage sequence.
2. `docs/tasks/reference-experience/PINS.md` -- every exact value. Transcribe pins; never derive
   an expected value yourself.
3. `docs/tasks/reference-experience/LEDGER.md` -- progress, the S4 and S7 substep tables, and
   the twelve numbered deviations. Deviations 2, 3, 5, and 7 are the reason for this request.
4. `docs/adr/0126-reference-experience-contract.md` -- the decisions S1 ratified.

S1 through S6 are COMPLETE and gated on Windows. S7 is planned in three substeps and not started.
Five things those stages promise cannot be proved on a Windows host, and that is what this lane is
for. Nothing here asks you to implement a stage.

## What to verify

Work from `dev` at `e076d4ee` or later. Rebuild and deploy a user-level candidate as you normally
do, and use an ordinary graphical profile, not Playwright, headless Chrome, or an isolated profile.

**1. The command entry (`~/.local/bin/ghostlight`), ledger deviation 3.**
`crates/orchestrator/src/install/command_path.rs` has seven tests. Four of them return early on
Windows, so they have never actually executed. On Linux they run for real: confirm all seven pass.
Then prove it live: an ordinary install creates the symlink, `ghostlight doctor` resolves in a fresh
shell without touching any shell startup file, a repeat install changes nothing, uninstall removes
only that entry, and a foreign file or a symlink pointing elsewhere at that path is left byte
identical. A `/usr/bin` package installation must report the entry as not applicable.

**2. Manual pages, ledger deviation 5.**
`scripts/finalize-debian-package.ps1` now injects three compressed pages into
`usr/share/man/man1` before it recomputes `md5sums`. That path needs `dpkg-deb` and `gzip` and has
only been parse-checked here. Build a Debian package, confirm the three pages are present, are
covered by `md5sums`, and that `lintian` no longer reports absent manpages for the three
executables. Then confirm `man ghostlight`, `man ghostlight-mcp-connector`, and
`man ghostlight-browser-connector` render on both install routes; the per-user route relies on
`man` searching `../share/man` for every PATH directory.

**3. Shell completions, ledger deviation 7.**
Confirm the Debian package installs the three files into the conventional vendor locations, and
that the per-user route installs them under the XDG data directory. Then drive real completion:
bash and fish should complete subcommands and per-subcommand flags with no configuration; zsh needs
the `fpath` line that `ghostlight.1` documents for the per-user path and needs nothing for the
package path. Report whether the documented zsh instruction is accurate.

**4. The second-machine state, ledger deviation 2.**
Load the unpacked extension into a Chromium profile with **no** Ghostlight native host registered.
The popup and the options Connection card must both state the pinned sentence
`Ghostlight is not installed on this computer yet.` and offer a control named `Set up Ghostlight`.
With the network available that control opens the canonical walkthrough; offline it must open the
bundled `extension/setup.html` instead. Then register the host and confirm both surfaces return to
the ordinary connected state. The exact strings are pinned in `PINS.md`; do not paraphrase them.

**5. The environment rows.**
`ghostlight doctor` prints one `Environment:` line from the closed table in
`crates/orchestrator/src/language/environment.rs`. Confirm the exact phrase on GNOME and on KDE, and
on one unrecognized desktop if you have one. A shell that draws no tray must not be told it has one.
The WSL row is a Windows-host case and stays with windows-codex.

## Authority and boundaries

- Fix any product defect you find at its owning seam, with regression coverage, and commit logical
  changes separately.
- Do not implement S7. Its three substeps are planned but unstarted, and S7a is a governance-schema
  change that should start a fresh session.
- Do not edit `PINS.md`. If a pin is wrong, stop and say so in the chat.
- Do not add telemetry, an update ping, or any network behavior. ADR-0028 Decision 9 stands, and
  the epic's NEVER list has no exception for it.
- Do not merge `main`, tag, sign, publish, or release.
- Record what you prove in the ledger's gate log and in the deviation entries it belongs to, and
  update `docs/STATUS.md` if a claim there changes.

## Gate commands

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/process-journey.mjs
    node tests/cli-journey.mjs

Record the counts you observe rather than matching a count recorded here.
