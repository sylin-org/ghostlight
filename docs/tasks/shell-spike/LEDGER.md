# Shell spike ledger

The shell spike builds `ghostlight-desktop`, the third executable of ADR-0199 Decisions 1 and 7,
as a standalone crate in `crates/desktop`. It proves the window, the tray and the notifications
can live outside the service, render only pages served from memory through a private scheme, and
link no network client. It runs in parallel with the seams lane, depends on no other workspace
crate and is not wired to the service. After the seams lane lands, the shell lane moves this
ledger into `docs/tasks/break-2.0/` and the network guard into `tests/guards/`. ADR-0199 is
cited here by its draft path, `docs/adr/0199-one-service-thin-shells.md`, in prose only: the seams
lane files it, and it is linked once that commit exists.

## Toolkit decision

The shell is built on `wry` + `tao` + `tray-icon`. Not Tauri: it brings `tokio`, `reqwest` and
`http`, plus its runtime, configuration, capability codegen, `tauri-build` and plugins, so a
larger binary and more upgrade surface for a process that must decide nothing. Not raw platform
bindings (WebView2 COM calls, Win32 windows and `Shell_NotifyIcon`, hand-driven `webkit2gtk`):
hundreds of lines of unsafe code, which the workspace forbids outside its audited FFI crate.
`wry`, `tao` and `tray-icon` come from the same upstream as Tauri and are already locked at these
exact versions (`tao` patched to the ADR-0120 Wayland fix), so the crate adds no package to
`Cargo.lock` except itself.

The one name on the network-crate ban list in the shell's tree is `http`, the request and response
types `wry`'s custom-scheme and IPC callbacks take. It does no I/O and opens no socket.
`crates/desktop/guards/network.test.mjs` names it as the only exception and fails when it leaves
the tree.

## Gate

After every slice, from the repository root in Git Bash, with
`CARGO_TARGET_DIR=E:/repo/github/sylin-org/ghostlight/.target-lane-shell-spike`:

- `cargo clippy -p ghostlight-desktop --all-targets -- -D warnings` and
  `cargo test -p ghostlight-desktop`
- `cargo fmt --check` (fix this lane's files with `cargo fmt -p ghostlight-desktop` or `rustfmt`,
  never a bare `cargo fmt`)
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`
- `node --test "crates/desktop/guards/*.test.mjs"` (the glob form: Node 24 on Windows does not
  accept a directory argument)
- `node --test "crates/desktop/src/web/tests/*.test.js"` from shell-spike-3 on
- `node --check` on every new or changed `.js` and `.mjs`
- `pwsh -NoProfile -File scripts/check-repository-integrity.ps1`
- `node --test "tests/guards/*.test.mjs"` in shell-spike-1 only, which changes `Cargo.toml`

Never pass `--locked` to cargo build, clippy or test, because the seams lane may be mid-way
through a manifest change. The network guard passes `--locked` to `cargo tree` only.

## RESUME HERE

Next: shell-spike-2-scheme-and-policy.

`crates/desktop/Cargo.toml` declares every dependency the lane uses and may not change after
shell-spike-1. A slice that needs another crate records a blocker instead.

## Landed

| Slice | What | Commit |
|---|---|---|
| shell-spike-1 | The `ghostlight-desktop` crate in the workspace (library only until shell-spike-7), its full dependency list, and `bundle.rs`: an in-memory content-addressed set of web files. Paths are plain relative ASCII paths of at most 128 bytes; file types come from a closed table (html, css, js, png, svg, json) and anything else is refused; at most 256 files and 8 MiB; the id is the SHA-256 over `<path>\0<file sha256>\n` lines in path order. `Bundle::service` refuses the reserved `_shell` first segment and `Bundle::shell` requires it. The network guard checks the shell's tree on Windows and Linux. | see `git log` |

## Owed live checks

- shell-spike-1: on a Linux test machine, `cargo clippy -p ghostlight-desktop --all-targets -- -D
  warnings` and `cargo test -p ghostlight-desktop`. The coordinator's host builds only
  `x86_64-pc-windows-msvc`, and `gtk` and `webkit2gtk` need the Linux system libraries.

## Blockers

None.

## Notes

- The lane gate writes `node --test crates/desktop/guards/`. On Node 24 on Windows a directory
  argument fails with `MODULE_NOT_FOUND` (`node --test tests/guards/` fails the same way), so the
  gate here uses the glob form.
- `tray-icon` is declared with `default-features = false, features = ["gtk"]`: `libxdo` stays off,
  as in Tauri's build, because CI installs no `libxdo`.
- `embed-resource` is declared now for the Windows `build.rs` that shell-spike-7 adds; Cargo finds
  `build.rs` and `src/main.rs` without a manifest change.
