# S9: --no-supervisor + the dev-loop doc

Goal: an explicit install flag that skips OS-supervisor registration (dev instances run the
service in a terminal; an auto-started dev service would hold the exe lock during rebuilds), plus
the canonical dev workflow document.

## STOP preconditions

- S8 not logged complete -> STOP.

## Required changes

1. Root `src/main.rs` `InstallArgs`: add `no_supervisor: bool` with the SPEC section 10 doc
   string; map into `InstallOptions.no_supervisor` (add the field in
   `crates/core/src/install/mod.rs`).
2. `run_install`: when `no_supervisor` is set, print the existing section header
   (`\nSupervisor (auto-start):`) followed by exactly `  (skipped: --no-supervisor)` and skip
   `supervisor::apply_steps`. Uninstall unchanged.
3. NEW `docs/DEV-LOOP.md` per SPEC section 10's content list (plain human prose, ASCII, no
   em-dashes; under ~60 lines; the five topics pinned there, with the exact commands).

## Tests (pinned)

- `tests/install_instance.rs` NEW test `no_supervisor_flag_plans_no_supervisor_steps` per SPEC
  section 10 (dry-run with the flag; stdout contains `(skipped: --no-supervisor)`; stdout does
  NOT contain any of `schtasks`, `launchctl`, `systemctl`).
- Existing install tests stay green unmodified.

## Verify (literal)

SPEC section 12.

## Out of scope

Making non-default instances IMPLY --no-supervisor (identity stays behavior-free, ADR-0044).
Packaging (S10).

## Commit

`feat(install): --no-supervisor flag + the DEV-LOOP workflow doc (S9)`
