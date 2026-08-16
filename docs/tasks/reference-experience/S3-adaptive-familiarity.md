# S3: adaptive familiarity

## Objective

Give the product one owner for what it says about the machine it is running on, so install, `doctor`,
and the workbench describe the same reality in the vocabulary of the desktop the person is actually
using. Include the case the product currently ignores entirely: running under WSL while the browser
runs on Windows.

## Read first

- [BOOTSTRAP.md](BOOTSTRAP.md) and [PINS.md](PINS.md).
- ADR-0082 (Linux user session discovery), ADR-0112 (one minimized desktop startup), ADR-0116
  (Windows and Linux platform scope), ADR-0118 (recoverable Linux workbench startup), ADR-0123 (lean
  Linux install and visible activation, which made the Applications entry universal and forbade
  tray-only reachability).
- `docs/research/25-delightful-linux-experience-2026-08.md`, especially its note that a status icon
  must never be the only route to critical functionality.
- `crates/orchestrator/src/install/desktop_entry.rs`, `crates/orchestrator/src/install/mod.rs`,
  `crates/orchestrator/src/main.rs` (where `doctor` lives), `crates/orchestrator/src/language/`.

## Verified facts as of authoring

Confirmed at `2f24943f`. Re-read before relying on any of them.

- No WSL detection or messaging exists anywhere in the product. `is-wsl` appears only as a
  transitive lockfile entry.
- The Linux desktop entry is written under `data_home/applications` with id `org.sylin.ghostlight`.
- Snap and Flatpak browser packages are already detected with a remedy sentence in
  `crates/orchestrator/src/install/browser_package.rs`.
- No autostart entry is written on any platform, which is correct and is nowhere stated to the user.

## Required behavior

1. **One module owns the environment vocabulary.** Add a module under
   `crates/orchestrator/src/language/` that resolves a closed environment value and returns the
   phrases that depend on it. Follow the repository rule that a named vocabulary lives in a
   dedicated domain module rather than as literals at call sites. Detection rules for WSL and the
   Linux desktop shell are pinned in `PINS.md`.
2. **The environment is a table, not a branch.** One row per platform and shell, including an honest
   unknown row. Adding macOS later must be a row plus evidence, never a restructure.
3. **Every phrase has one definition.** Where Ghostlight lives after install is stated once per row:
   a tray and Applications menu where the shell has a tray, the Applications menu where it does not,
   the notification area on Windows. No call site composes its own sentence.
4. **The background posture is stated.** A successful install ends with the pinned background-posture
   sentence from `PINS.md`, so a person arriving from a platform where such tools run at login knows
   that nothing does here.
5. **WSL is named honestly.** When the pinned WSL rule matches, install and `doctor` state that the
   browser and the product are on different sides of that boundary and name the concrete next step.
   Do not attempt to bridge WSL to a Windows browser in this stage, and do not imply that it works.
6. **`doctor` and install agree.** Both consume the same module. A phrase that exists in one and not
   the other is a defect.

## Tests to add

Rust unit tests inline in the new module, by name:

- `resolves_wsl_from_distro_environment`
- `resolves_wsl_from_kernel_release_marker`
- `resolves_each_recognized_linux_shell`
- `unrecognized_desktop_falls_back_to_the_unknown_row`
- `every_environment_row_has_a_location_phrase`
- `background_posture_sentence_matches_the_pinned_value`

The fifth is an exhaustiveness guard: it must fail if a row is added without its phrase. Verify that
by breaking it once, confirming the failure, and restoring it.

## Verification

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/process-journey.mjs

Then run the built `ghostlight doctor` on this host and confirm by eye that the environment phrase
matches the actual desktop. Record the host, the shell, and the output line in the ledger.

## Out of scope

Launching a browser, which is S7. Any change to what the workbench renders, which is S6. Man pages,
completions, and PATH, which are S4. Any new preference. Any change to the extension.

## STOP preconditions

- The environment cannot be resolved without a new dependency or a privileged call.
- A phrase would need to differ between `doctor` and install to stay truthful.
- Detecting the desktop shell requires anything beyond the environment variables pinned in
  `PINS.md`.
- Naming the WSL case would require claiming behavior the product does not have.
