# S4: terminal citizenship

## Objective

Make the command line a first-class way to use and understand Ghostlight, and make `doctor` the one
place that explains state. A person who never opens the workbench should be able to install, verify,
diagnose, and script the product without reading source.

## Read first

- [BOOTSTRAP.md](BOOTSTRAP.md) and [PINS.md](PINS.md).
- ADR-0105 (scripted intake channels), ADR-0106 (caller-owned sessions), ADR-0123 (lean Linux
  install and visible activation), ADR-0124 (user-writable system package runtime).
- `docs/research/25-delightful-linux-experience-2026-08.md`, which requires that the installer never
  edit shell startup files and that the install outcome name the command path.
- `crates/orchestrator/src/main.rs`, `crates/orchestrator/src/cli/`,
  `crates/orchestrator/src/install/`, `packaging/`.

## Verified facts as of authoring

Confirmed at `2f24943f`. Re-read before relying on any of them.

- No man pages and no shell completions are built or installed anywhere.
- The npm package declares a `ghostlight` bin, so a global npm install puts the command on PATH.
  The documented primary journey, `npx -y ghostlight install`, does not.
- The Debian package provides `/usr/bin/ghostlight`; the per-user route installs under
  `~/.ghostlight/bin/v<version>/` with no PATH integration.
- Argument parsing is hand-rolled in `crates/orchestrator/src/main.rs`. There is no `clap`, so
  completions and man pages are authored, not generated.
- `status --json` and `call --json` exist. Other commands may not.
- `lintian` already reports the absent manpages for the three sibling executables.

## Required behavior

1. **The command is reachable.** Implement the PATH decision S1 recorded in `PINS.md`. If it owns
   `~/.local/bin/ghostlight`, that entry is created only when ownership is safe, is removed on
   uninstall, is byte-identical on repeat install, and never overwrites a foreign file. Under no
   circumstance edit a shell startup file. Either way, a successful install prints the exact
   absolute command path.
2. **Man pages exist for the three siblings.** Authored `man1` sources, installed by the Debian
   package into the standard location and available to the per-user route. Content is factual: what
   the command does, its options, its exit codes, and its files.
3. **Completions exist for bash, zsh, and fish.** Authored, installed to the conventional locations
   for each install route, and covering the current subcommands and their flags. A completion that
   offers a subcommand the binary does not accept is a defect; add a test that compares the
   completion's subcommand list against the parser's.
4. **`--json` is uniform.** Audit every subcommand. Any command that reports state supports `--json`
   with a stable shape through the existing command seam. Do not add a new output framework.
5. **`NO_COLOR` is honored** wherever the product emits styled terminal output.
6. **`doctor` is the one explanation surface.** Every state the product can be in has a `doctor`
   line, in the same words the workbench uses, sourced from the S3 module and the orchestrator's
   outcome language. Add a guard test that fails when a renderable state has no `doctor` line.

## Tests to add

Rust, by name, beside the code they guard:

- `install_reports_the_absolute_command_path`
- `path_entry_is_ownership_checked_and_idempotent` (only if S1 chose to own one)
- `uninstall_removes_only_the_owned_path_entry` (same condition)
- `completion_subcommands_match_the_parser`
- `every_reportable_state_has_a_doctor_line`
- `no_color_suppresses_styling`

Break the fourth and fifth once each, confirm they fail, and restore them.

## Verification

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/process-journey.mjs
    node tests/cli-journey.mjs

On Linux, additionally confirm by hand that a fresh shell resolves `ghostlight`, that
`man ghostlight` renders, and that completion works in at least bash. Record the shell and results
in the ledger.

## Out of scope

Any workbench change, which is S6. Any recovery behavior, which is S7. New subcommands beyond what
uniform `--json` requires. Any packaging format that does not exist today. Any network behavior.

## STOP preconditions

- S1 did not record the PATH decision in `PINS.md`.
- Making the command reachable would require editing a shell startup file or writing outside an
  owned location.
- A `--json` shape cannot be added through the existing command seam without a new output framework.
- A reportable state has no words in the orchestrator's language to reuse, meaning the sentence would
  have to be authored here.
