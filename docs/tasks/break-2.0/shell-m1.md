# Break 2.0: shell milestone 1 ledger

Milestone 1 (M1) of the desktop shell track carries the 2026-10-15 amendment of
[ADR-0199](../../adr/0199-one-service-thin-shells.md): the Ghostlight web apps over loopback.
Slices m1-01 to m1-12 replace the private scheme and the page bridge with the existing workbench
served on a loopback port, with access decided in context, one live stream and a shared Controls
sheet. Tauri stays inside the service for the whole milestone and its window keeps working. The
milestone ends with the coordinator's live demo with the owner, in three parts: the workbench in
the Ghostlight window, the same URL watching in a Chrome tab, and watching turned off with the
About page still showing in full. The rest of the shell track is planned after that demo.

Gate, from the repo root on `dev`, in Git Bash, with
`CARGO_TARGET_DIR=E:/repo/github/sylin-org/ghostlight/.target-lane-shell-m1`:

- `cargo fmt --check -p ghostlight-service`, plus `-p ghostlight-desktop` when `crates/desktop`
  changed and `-p ghostlight` when `crates/edge` changed (fix only with `cargo fmt -p <package>`,
  never a bare `cargo fmt`).
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
- When anything under `crates/orchestrator/ui`, `tests/workbench-*.mjs` or
  `tests/workbench-preview-server.mjs` changed: `node --test tests/workbench-transport.test.mjs`
  (from m1-07), `node --test tests/workbench-controls.test.mjs` (from m1-08) and
  `node tests/workbench-surface.mjs`.
- `node --test "crates/desktop/guards/*.test.mjs"` when `crates/desktop`, any `Cargo.toml` or
  `Cargo.lock` changed.
- `node --test "tests/guards/*.test.mjs"` when `crates/edge`, any `Cargo.toml` or `Cargo.lock`
  changed.
- `node --check` on every changed `.js` and `.mjs` file.
- `pwsh -NoProfile -File scripts/check-repository-integrity.ps1`.

Never pass `--locked` to cargo build, clippy or test.

Rules:

1. The commit protocol is in [BOOTSTRAP.md](BOOTSTRAP.md).
2. One slice is one commit. Stage and commit only your own paths.
3. Never run a Ghostlight binary, a journey, an install, a carrier or `scripts/dev-loop.ps1`.
   The coordinator runs every live check; record each under Owed live checks.
4. Follow the tree when it differs from a spec, keep the spec's intent and record the difference
   under Notes. When the intent cannot be kept, record a blocker and stop.

## RESUME HERE

Next: m1-03 (the next slice key in the lane plan).

## Landed

| Slice | What | Commit |
|---|---|---|
| m1-01-adr-amendment | The owner-approved amendment of 2026-10-15 is appended to ADR-0199 (the Ghostlight web apps over loopback), with its README row, this ledger, and the pointers from the shell-spike ledger and the task index. Docs only. | see `git log` |
| m1-02-harness-evidence | Harness evidence names a foreign program and its argument count, never its arguments; a configuration that cannot be parsed carries no evidence (owner answers Q3 and Q8). | see `git log` |

## Route table

Filled by m1-04; every later slice that adds a route keeps it current.

## Owed live checks

None yet.

## Blockers

None.

## Notes

- Owner approval: the owner approved the amendment text on 2026-10-15, as written.
- Precondition 1: `seams.md` has Landed rows for f08a-edge, f08b-console-attach and
  w01-windows-user-path, and its RESUME HERE names none of them as next.
- Precondition 2: no tracked file under `crates/`, `Cargo.toml` or `Cargo.lock` was modified or
  staged; `install/user_path.rs` and `win-peer/src/settings_change.rs` are tracked.
- Precondition 3: the orchestrator package is `ghostlight-service`; `relay::home` has
  `SERVICE_EXECUTABLE_NAME` and `EDGE_EXECUTABLE_NAME`; `install/deployment.rs` has `service_swap`,
  `edge_swap` and `placed_in_bin`; `intake/listener.rs` `judge` refuses `Role::Desktop` and
  `service/mod.rs` refuses it again; `desktop/mod.rs` still has its `tauri::generate_handler!`
  list. No name differs from the spec.

- m1-02: the foreign entry's `program` is the file name of `command` when `command` names an existing file; otherwise the command is cut at its first whitespace before a file name is taken, and every later word counts as an argument. A configuration that cannot be parsed names the line of the parse error (taken from the parser, never any file text) in its detail and carries no evidence (owner answer Q3); `HarnessError::Malformed` carries only that line.
