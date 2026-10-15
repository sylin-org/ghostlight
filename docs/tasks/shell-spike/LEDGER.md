# Shell spike ledger

The shell spike builds `ghostlight-desktop`, the third executable of ADR-0199 Decisions 1 and 7,
as a standalone crate in `crates/desktop`. It proves the window, the tray and the notifications
can live outside the service, render only pages served from memory through a private scheme, and
link no network client. It runs in parallel with the seams lane, depends on no other workspace
crate and is not wired to the service. After the seams lane lands, the shell lane moves this
ledger into `docs/tasks/break-2.0/` and the network guard into `tests/guards/`. The decision is
[ADR-0199](../../adr/0199-one-service-thin-shells.md).

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

Next: shell-spike-6.

`crates/desktop/Cargo.toml` declares every dependency the lane uses and may not change after
shell-spike-1. A slice that needs another crate records a blocker instead.

## Landed

| Slice | What | Commit |
|---|---|---|
| shell-spike-1 | The `ghostlight-desktop` crate in the workspace (library only until shell-spike-7), its full dependency list, and `bundle.rs`: an in-memory content-addressed set of web files. Paths are plain relative ASCII paths of at most 128 bytes; file types come from a closed table (html, css, js, png, svg, json) and anything else is refused; at most 256 files and 8 MiB; the id is the SHA-256 over `<path>\0<file sha256>\n` lines in path order. `Bundle::service` refuses the reserved `_shell` first segment and `Bundle::shell` requires it. The network guard checks the shell's tree on Windows and Linux. | see `git log` |
| shell-spike-2 | `scheme.rs` and `BundleSet`. The private `ghostlight` scheme (`https://ghostlight.localhost` on Windows, `ghostlight://localhost` elsewhere, both forms parsed everywhere by `request_path`), the fixed `CONTENT_POLICY` (`default-src 'none'`), `respond` (GET and HEAD only, 404, 405, security headers, the policy repeated as a meta element in every HTML page, 500 for HTML with no head), `allows_navigation`, and `harden`, the one place the webview settings are applied. `BundleSet` serves `_shell/` paths from the shell bundle and every other path from the service bundle. | see `git log` |
| shell-spike-3 | `bridge.rs` and `web/bridge.js`: the one post-and-receive JSON bridge. `bridge.js` is injected into the main frame and defines the frozen, non-writable, non-configurable `window.ghostlightShell` (`post`, `listen`) and the hidden `__ghostlightShellDeliver`. `decode_page_message` takes only a JSON object of at most 1 MiB, `delivery_script` encodes the message as a JSON string literal inside the call, and `page_origin` sorts a URL into shell page, service page or foreign (foreign messages are dropped). | see `git log` |
| shell-spike-4 | `tray.rs`: the tray built from a model. `TrayModel` (`tooltip`, `items`) and `TrayItem` (`id`, `label`, `enabled`) with `validate` (1 to 16 items, unique ids of `[a-z0-9_]{1,32}`, labels of 1 to 64 printable characters, a tooltip of at most 64), `icon_image` and `decode_rgba` (PNG to 8-bit RGBA), `Tray::build` and `Tray::show`, and `install_events` mapping a left-button-up click to `TrayEvent::Activate` and a menu choice to `TrayEvent::Item(id)`. The shell owns no item; Linux reports no clicks, so Open must be a menu item there. | see `git log` |
| shell-spike-5 | `window.rs`: the window and its webview, moved unchanged from the Tauri glue. `WindowLifecycle` is the pure rule set (`Absent`, `Open`, `Replacing`; `OpenRequested`, `CloseRequested`, `Destroyed`, `RendererLost` give `Create`, `Focus` or `Destroy`): Windows focuses an open window, Linux destroys it and builds a fresh one after the destroyed event and coalesces Opens meanwhile (ADR-0120), a lost renderer discards the window and nothing rebuilds it (ADR-0119). `renderer_policy` and `apply_renderer_policy`, `prepare_identity` and `init_display` are the Linux startup helpers, no-ops on Windows. `ShellWindow::create` builds the tao window (title, 1180 by 760, minimum 880 by 620, centered by the pure `centered_origin`, product icons, Windows class `Ghostlight Window`) and the hardened wry webview with the private scheme, the IPC handler and the injected bridge; `focus`, `navigate`, `deliver`, `id` and, on Linux, `on_renderer_lost`. | see `git log` |

## Owed live checks

- shell-spike-1: on a Linux test machine, `cargo clippy -p ghostlight-desktop --all-targets -- -D
  warnings` and `cargo test -p ghostlight-desktop`. The coordinator's host builds only
  `x86_64-pc-windows-msvc`, and `gtk` and `webkit2gtk` need the Linux system libraries.

- shell-spike-2: on a Linux test machine, `cargo clippy -p ghostlight-desktop --all-targets -- -D
  warnings` and `cargo test -p ghostlight-desktop`.
- shell-spike-2: `scheme::harden` and the scheme handler are glue and are checked live in
  shell-spike-3 and later: that WebView2 serves `https://ghostlight.localhost/` from memory with
  no network, that a `<script src=https://example.com/x.js>` and a `fetch` are blocked by the
  policy, that `window.open`, a download and a link to another origin do nothing, and that
  devtools are absent in a release build.
- shell-spike-3: on a Linux test machine, `cargo clippy -p ghostlight-desktop --all-targets -- -D
  warnings` and `cargo test -p ghostlight-desktop`.
- shell-spike-3: live, with the bridge injected by `with_initialization_script_for_main_only`: that
  `window.ghostlightShell` exists before page scripts run on WebView2, that a posted object reaches
  the IPC handler with the page URL, that `evaluate_script` of a `delivery_script` reaches a
  listener, and that assigning `window.ghostlightShell` from a page does nothing.

- shell-spike-4: on a Linux test machine, `cargo clippy -p ghostlight-desktop --all-targets -- -D
  warnings` and `cargo test -p ghostlight-desktop`.
- shell-spike-4: live, once the event loop exists (shell-spike-5 or later): that `Tray::build` shows the
  product icon and tooltip, that a right click lists the model's items in order with disabled ones
  greyed, that a left click on Windows reports `Activate` and does not open the menu, that choosing
  an item reports `Item(id)`, that `Tray::show` replaces the menu and tooltip, and that a Linux
  indicator shows the menu but reports no click.

- shell-spike-5: on a Linux test machine, `cargo clippy -p ghostlight-desktop --all-targets -- -D
  warnings` and `cargo test -p ghostlight-desktop`. The Linux-only code in `window.rs`
  (`apply_renderer_policy`, `prepare_identity`, `init_display`, the GTK container path of the
  webview build and `on_renderer_lost`) has not been compiled on the coordinator's host.
- shell-spike-5: live, once the event loop exists (shell-spike-6 or later): that the window opens
  titled Ghostlight at 1180 by 760 centered on its monitor and cannot be resized below 880 by
  620; that on Windows its class is `Ghostlight Window` and the taskbar shows the product icon;
  that on Windows a second Open restores a minimized window and focuses it; that on Linux a
  second Open destroys the window and builds a fresh one, and five quick Opens build one; that
  closing the window leaves the process running; that on Linux killing the WebKit web process
  discards only that window and the next Open builds a new one; that on Linux with the NVIDIA
  driver loaded `WEBKIT_DISABLE_DMABUF_RENDERER` is set to 1 unless the user set it; that the
  Wayland app id and X11 class match the installed entry; and that a session with no display
  makes `init_display` return `NoDisplay` instead of panicking.
- shell-spike-5: live on Linux under X11, once the event loop exists: that an Open presents and
  raises the new window both for the first create and for a replacement, and that the window is
  not left behind other windows.

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
- `ShellWindow::create` builds the window hidden, centers it with `centered_origin` (pure, in
  physical pixels on the window's monitor) and then shows it, which replaces the `center: true` of
  the Tauri configuration without a flash at a default position. Wayland ignores the position.
  The caller calls `focus` after `Create`. On Linux `focus` presents the window from a GTK idle
  callback, because `tao` drops a focus request while its queued show request has not yet run, and
  a window freshly created hidden is in exactly that state.
- If a `Create` act fails, the caller feeds `LifecycleEvent::Destroyed` so the lifecycle returns to
  `Absent`.
