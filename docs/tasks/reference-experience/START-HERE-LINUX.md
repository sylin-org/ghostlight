# START HERE: the Linux lane for the reference-experience epic

You are `linux-codex`. You own everything in this file and nothing else. Assume you have no memory
of any prior session and that nobody will answer a question for you mid-task. Everything you need is
here or in the files this points to.

Work on branch `dev`. Do not create other branches.

## Before your first task

    git fetch origin dev
    git status --short

If `dev` is behind `origin/dev`, fast-forward it. If the working tree is dirty and the changes are
not yours, stop and say so in `coordination/CHAT.md` rather than stashing or discarding anything.
Never reset, clean, or check out over work you did not create.

## Read these first, in this order, before you touch anything

1. `AGENTS.md` -- how this repository works and what is forbidden in it.
2. `docs/tasks/reference-experience/BOOTSTRAP.md` -- the epic's ground rules.
3. `docs/tasks/reference-experience/PINS.md` -- every exact value. **Transcribe pins. Never invent
   an expected value, and never derive one from what the code currently does.**
4. `docs/tasks/reference-experience/LEDGER.md` -- what has happened, the S4 and S7 substep tables,
   and the twelve numbered deviations.
5. `docs/adr/0126-reference-experience-contract.md` -- the decisions this epic implements.

## Authority order

Higher wins. A conflict you did not expect is a STOP condition, not a judgment call.

1. The live tree and its tests, for what is true today.
2. `PINS.md`, for exact values.
3. `docs/1.0/INTENT.md`, `LANGUAGE.md`, `ARCHITECTURE.md`, `ACCEPTANCE.md`.
4. Accepted ADRs.
5. This file and the stage prompt it sends you to.

## Environment facts, true as of `0db4fcab` on 2026-08-16

Re-read the tree before relying on any of these. They are a starting point, not a promise.

- S1 through S6 are complete and were gated on a Windows host. S7 is planned and unstarted.
- Windows gate counts at that revision: 280 orchestrator library tests, 9 orchestrator binary, 32
  bridge, 6 MCP connector, 116 extension. **Record the counts you observe. Do not try to match
  these.** Your platform compiles different code.
- `crates/orchestrator/src/install/command_path.rs` has seven tests. Four of them return early on
  Windows and have therefore never actually run. On your host they run for real.
- `scripts/finalize-debian-package.ps1` injects manual pages and shell completions into the Debian
  payload before it recomputes `md5sums`. That code has only been parse-checked, never executed.
- The policy manifest validator is `validate_config` in
  `crates/orchestrator/src/governance/manifest.rs`. Every registered setting today is either a
  boolean or the `content.security.sacred_domains` array. There is no string-valued setting yet, and
  the only accessor is `boolean_setting` at `manifest.rs:203`.

## Gate commands

Literal. Run all of them before every commit.

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    node --check <each changed extension or ui JavaScript file>

When your change touches process startup, the CLI, or the workbench surface, also run:

    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/process-journey.mjs
    node tests/cli-journey.mjs
    node tests/workbench-surface.mjs

The journeys resolve executables from `.target-ghostlight-1.0/debug` unless `GHOSTLIGHT_BIN_DIR`
says otherwise. If you build somewhere else, pass it, or the journey passes against stale binaries
and proves nothing.

## Protect the machine you are on

This is somebody's working computer, and it very likely already has a working Ghostlight
installation from earlier lanes. Tasks V1 through V4 install, uninstall, and unregister things.
Read this before you run any of them.

1. **Record the starting state before you change anything.** Run `ghostlight doctor` and save the
   output. It tells you which browsers are registered, which MCP clients are set up, whether the
   command entry exists, and where the installed executable is. You are responsible for putting all
   of that back.
2. **Prefer a disposable profile, root, or container** for anything that would otherwise disturb the
   working installation. A separate Chromium profile is enough for V4; a throwaway `HOME` or
   `XDG_DATA_HOME` is enough for most of V1 through V3.
3. **Never leave the machine's real browser unregistered.** V4 needs a profile with no native host.
   Get that by using a browser profile that was never registered, not by unregistering the one that
   works. If you must unregister, re-register it in the same task and confirm with `doctor`.
4. **Restore and verify at the end of every task.** Run `ghostlight doctor` again and compare it to
   the output you saved. Any difference you did not intend is a defect in your procedure, not an
   acceptable side effect. Record the before and after in the ledger.
5. **If a machine-local note exists at `local/NOTES.md` or `local/MACHINE-STATE.md`**, it holds the
   truth about what is installed on that computer. `AGENTS.md` requires explicit owner authorization
   before reading anything under `local/`. If you were not given it, ask in
   `coordination/CHAT.md` and work from `doctor` output in the meantime. Never write machine state
   into a tracked file.

STOP if: you cannot restore the starting state, or you are unsure whether something you removed was
the owner's working setup. Say so in `coordination/CHAT.md` and stop rather than guessing.

## When a task finds a defect

Fix it at its owning seam, with a regression test, and commit that fix separately from the task that
found it. A verification task that finds a defect has succeeded, not failed.

Do not fix a defect by changing the test, the pin, or the documentation to match the behavior. If
the behavior and a pin disagree, the pin wins and the behavior is the defect, unless the pin is
wrong -- in which case STOP and say so in `coordination/CHAT.md` rather than editing it.

## Your tasks, in this order

Do them one at a time. One task, one commit, one green tree. Do not start the next one until the
current one is committed.

### V1. Prove the command entry

Owning code: `crates/orchestrator/src/install/command_path.rs`. Ledger deviation 3.

1. Run the seven tests in that module and confirm all seven pass, not four.
2. Install through the ordinary per-user route. Confirm `~/.local/bin/ghostlight` is created and
   that `ghostlight doctor` runs in a **new** shell.
3. Confirm no shell startup file changed. `git diff` will not tell you this; check the files
   themselves (`.bashrc`, `.zshrc`, `.profile`, and your shell's equivalent).
4. Install again. Confirm nothing changed.
5. Put a plain file at `~/.local/bin/ghostlight`, run install, and confirm the file is byte
   identical afterward. Then replace it with a symlink pointing at something that is not Ghostlight
   and confirm the same.
6. Uninstall. Confirm the entry is removed and nothing else in `~/.local/bin` was touched.
7. If a Debian package installation is available, confirm it reports the entry as not applicable
   because `/usr/bin/ghostlight` already exists.

STOP if: the entry is created anywhere other than `~/.local/bin`, any shell startup file changed, or
a foreign file was modified or removed.

### V2. Prove the manual pages

Ledger deviation 5.

1. Build a Debian package.
2. Confirm `usr/share/man/man1` contains the three `.gz` pages and that each appears in the
   package's `md5sums`.
3. Run `lintian` and confirm it no longer reports absent manpages for the three executables. Record
   any remaining findings.
4. Confirm `man ghostlight`, `man ghostlight-mcp-connector`, and `man ghostlight-browser-connector`
   render from the package.
5. On a per-user installation, confirm the same three render without any `MANPATH` configuration.

STOP if: the pages are missing from `md5sums`, or if `man` cannot find them on the per-user route
and the reason is not something `ghostlight.1` already documents.

### V3. Prove the shell completions

Ledger deviation 7.

1. Confirm the Debian package installs into `/usr/share/bash-completion/completions/ghostlight`,
   `/usr/share/zsh/vendor-completions/_ghostlight`, and
   `/usr/share/fish/vendor_completions.d/ghostlight.fish`.
2. Confirm the per-user route installs the same three under the XDG data directory.
3. In a real bash shell, type `ghostlight ` and press Tab. Confirm the eight subcommands appear.
   Then `ghostlight doctor --` and Tab, and confirm the flags appear.
4. Do the same in fish.
5. Do the same in zsh. The per-user path needs an `fpath` line, which `ghostlight.1` documents.
   **Report whether that documented instruction is accurate.** If it is wrong, fix the man page.

STOP if: a completion offers a subcommand the binary rejects. That means the guard in
`crates/orchestrator/src/main.rs` is not doing its job, and that is a defect to fix, not to work
around.

### V4. Prove the second-machine state

Ledger deviation 2. The exact sentences are in `PINS.md`. Transcribe them; do not paraphrase.

1. Load the unpacked extension into a Chromium profile that cannot reach a native host. Use a
   browser profile or a throwaway `HOME` that was never registered. Do **not** unregister the
   machine's working native host to create this condition; see "Protect the machine you are on".
2. Open the popup. It must state the pinned not-installed sentence and offer a control named
   `Set up Ghostlight`. It must **not** say `Waiting for the Ghostlight service...`.
3. Open the options page. The Connection card must say the same thing.
4. Click the control with the network available. Confirm it opens the canonical walkthrough.
5. Disconnect the network and click it again. Confirm it opens the bundled `extension/setup.html`
   instead, and that the page renders fully with no network.
6. Register the native host. Confirm both surfaces return to the ordinary connected state and the
   setup control disappears.

STOP if: the popup shows the not-installed sentence while a native host **is** registered. That is a
false claim and a worse defect than the one this fixed.

### V5. Prove the environment rows

Owning code: `crates/orchestrator/src/language/environment.rs`.

Run `ghostlight doctor` and read the single `Environment:` line, on GNOME and on KDE, and on one
unrecognized desktop if you have one. Confirm the phrase matches the row for that desktop and that a
shell without a tray is never told it has one.

The WSL row belongs to the Windows host and is not yours.

### S7a. Register the `browser.startup` setting

Read `docs/tasks/reference-experience/S7-readiness-recovery.md` and ADR-0126 Decision 7 first. This
substep is only the setting. Do not launch anything yet.

Pinned by ADR-0126 Decision 7:

- Key: `browser.startup`
- Closed values: the strings `on_demand` and `manual`. Any other value is refused.
- Default: `on_demand` on Windows, `manual` on Linux.
- It is an operational control, not a security boundary, exactly like `policy.user.enabled`.
- An organization ceiling applies to it the same way it applies to any other registered setting.

This is the first string-valued setting in the manifest, so you are adding a shape, not just a key.
Work through: `validate_config` in `manifest.rs`, a string accessor beside `boolean_setting`, the
effective-authority projection in `crates/orchestrator/src/governance/effective.rs`, the workbench policy editor, and the
policy grammar fixtures under `examples/` and `tests/policy-grammar.mjs`.

Tests to add, by name:

- `browser_startup_accepts_only_the_two_closed_values`
- `browser_startup_refuses_an_unknown_value`
- `browser_startup_refuses_a_non_string_value`
- `browser_startup_defaults_per_platform`
- `an_organization_ceiling_pins_browser_startup`

STOP if: adding a string setting would require changing the schema version, or if the workbench
editor cannot present a closed choice without a new settings framework.

### S7b. The decision layer

Still no launching. Implement the part that decides.

1. Find the one seam that learns an admitted operation has no usable browser. Recovery must be
   requested from there and nowhere else.
2. Choose a browser only from deterministic evidence, extending the existing ADR-0114 resolution
   order rather than building a second one.
3. Reuse the existing Snap and Flatpak diagnosis in
   `crates/orchestrator/src/install/browser_package.rs`. A sandboxed package is named, never chosen.
4. In `manual` mode, return one useful outcome that names the next action. Never launch.
5. Add a single-flight guard so simultaneous requests produce at most one attempt per owning scope.

Tests to add, by name:

- `recovery_is_requested_from_one_seam_only`
- `simultaneous_requests_produce_one_attempt`
- `manual_mode_never_launches_and_returns_one_useful_outcome`
- `a_sandboxed_browser_package_is_diagnosed_not_launched`
- `an_ambiguous_browser_set_refuses_and_names_candidates`
- `each_closed_failure_reason_is_reachable_and_distinct`

STOP if: recovery logic would have to appear at operation call sites, or the design needs a generic
recovery framework, a new daemon, or a second lifecycle authority.

### S7c. The launch

On Linux the default is `manual`, so this substep may correctly end as diagnosis only. That is a
completion, not a failure. Only implement a launch if you can resolve a usable session environment
through the ADR-0082 seam.

If you do implement it: the launched browser uses the person's ordinary profile. Never a fresh
profile, never a temporary profile, never automation flags. Every wait is bounded, and exhaustion
names one exact reason.

Test to add, by name: `a_launch_uses_the_ordinary_profile_with_no_automation_flags`.

STOP if: a launch would proceed without a verified session environment, or a timeout would leave an
effect's truth unknown and then retry it.

### S8. The evaluation

Read `docs/tasks/reference-experience/S8-evaluation.md`. It is the whole task; this file does not
restate it.

The release-blocking Linux lifecycle is the Ubuntu GNOME Wayland L1-L9 run that ADR-0123 already
made mandatory. It is not a user study, and it is not a cohort. If you cannot run it, the epic stays
open and you say so.

## Per-task procedure

1. Re-read the tree for the files the task names. The facts above are as-of, not current.
2. Check the task's STOP conditions. If one holds, stop and write it in the ledger.
3. Do the work at the owning seam.
4. Add the named tests with the pinned assertions.
5. Run every gate command.
6. Commit once, with a conventional-commit message that says what changed and why.
7. Update `docs/tasks/reference-experience/LEDGER.md`:
   - the `RESUME HERE` block, always;
   - the gate log, with the counts and evidence you actually observed;
   - for S7a, S7b, S7c, and S8: the stage or substep row and the evidence matrix;
   - for V1 through V5: the numbered deviation each one closes. V1 closes deviation 3, V2 closes 5,
     V3 closes 7, V4 closes 2. Mark it resolved with what you observed, or leave it open and say
     what is still missing. V5 has no deviation; record it in the gate log.
   - a new numbered deviation for anything you found, skipped, or could not prove.

## Never do these

Each entry names its one sanctioned exception, or says there is none.

- Never add telemetry, an update ping, an activation call, or any new outbound network request.
  ADR-0028 Decision 9 is permanent, and public trust and legal documents depend on it. **No
  exception.**
- Never edit an existing pin in `PINS.md`. If a pin is wrong, STOP and say so in
  `coordination/CHAT.md`. **No exception.**
- Never copy code from `reference/`. **No exception.**
- Never put policy, classification, or audit in the extension. **No exception.**
- Never weaken a claim in `docs/trust/` or `docs/legal/`. **No exception.**
- Never edit a shell startup file. **No exception.**
- Never change a `docs/1.0/` contract. **Exception: S7a**, and only if ADR-0126 Decision 7 made a
  statement there untrue.
- Never merge `main`, tag, sign, publish, or release. **No exception.**
- Never read or modify anything under `local/` or `/private/` without the owner saying so. **No
  exception.**

## Failure protocol

If a task cannot be completed: revert to the last green commit, mark the task `BLOCKED` in the
ledger with the reason and the evidence, and stop. Do not improvise around a broken assumption, do
not widen the task to make it fit, and do not skip ahead to the next one.

A blocked task with a clear reason is a good outcome. A task completed by guessing is not.

## When you are done, or blocked

Follow `coordination/INSTRUCTIONS.md`. Replace `coordination/RESULTS.md` with your result, append
one numbered reply to `coordination/CHAT.md`, commit the coordination files separately with
`chore(coordination): <description>`, and push `dev`.

Report what you proved, what you fixed, and what you could not do. Do not report a task as complete
because its tests pass; V1 through V5 and S8 are about what actually happens on a real desktop.

## You are finished when

Every task above is either committed with linked evidence in the ledger, or marked `BLOCKED` with a
reason. `docs/STATUS.md` describes only what the evidence proves. No accepted ADR contradicts what
shipped.
