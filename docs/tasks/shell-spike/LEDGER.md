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

Superseded in part by the ADR-0199 amendment of 2026-10-15: the private scheme, the in-memory bundle and the page bridge give way to the Ghostlight web apps over loopback, and the shell continues in [shell-m1.md](../break-2.0/shell-m1.md). Items 2 and 3 of the shell-spike-7 live checklist (the bridge echo and the content-policy probes) no longer apply.

All seven slices have landed. Next: the coordinator runs the live checks under Owed live checks
and records the results per platform. The shell lane then wires the service and deletes
`crates/desktop/src/spike.rs` and `crates/desktop/spike/`.

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
| shell-spike-6 | `session.rs`: the end of the session (ADR-0199 Decisions 13 and 14). `SessionState` (clones share two flags) maps `SessionSignal` (`Query`, `Cancelled`, `Final`) to `SessionAct`: a query marks the session ending, a cancel lifts it unless the final signal came, the first final signal gives `ArmFallbackThenExit` and the state stays ending, a later one does nothing. `may_start_process` is false while ending; every later path that starts a process asks it first. `handle` arms, then exits. `arm_fallback_exit(FALLBACK_EXIT)` spawns the `ghostlight-desktop-exit` thread (1500 ms, `exit(0)`, at once if the thread cannot spawn). Sources: on Linux `install_unix_signals` maps SIGTERM, SIGHUP and SIGINT to `Final` on the GTK main context; on Windows the event loop maps its destroyed event to `Final` (`tao` answers the query with true). The query-time ending state on Windows arrives with relay's shared listener when the shell lane wires the shell, which replaces both sources. | see `git log` |
| shell-spike-7 | `main.rs`, `spike.rs`, `spike/` and `build.rs`: the standalone spike window that composes every module, not wired to the service. `main` is a GUI-subsystem program on Windows in debug builds too (PE subsystem 2 checked in both profiles), takes no arguments (`accepts_arguments`; anything else exits 2) and returns `spike::run()`. `run` applies the Linux startup helpers (exit 3 with no display), builds the event loop with a user-event proxy, forwards tray events to it, and on `Init` builds the tray from `tray_model()` (Open, Close, Quit; a failure goes to stderr), installs the Linux signals and sends an Open to the lifecycle. The scheme serves `spike_bundles()`: a placeholder `_shell/index.html` and the spike page (`spike/index.html`, `spike.css`, `spike.js`, `mascot.png` from the product icon). `reply_to_page` drops a foreign page and any non-object body and echoes the rest with `"from": "shell"`. Close and Destroyed feed the lifecycle, Linux renderer loss feeds `RendererLost`, and `LoopDestroyed` feeds the final session signal after dropping the window and the tray. `build.rs` (Windows hosts targeting Windows) embeds the product icon and a version block with the description Ghostlight. | see `git log` |

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

- shell-spike-6: on a Linux test machine, `cargo clippy -p ghostlight-desktop --all-targets -- -D
  warnings` and `cargo test -p ghostlight-desktop`. `install_unix_signals` is Linux-only and has
  not been compiled on the coordinator's host.
- shell-spike-6: live, once the event loop exists (shell-spike-7): that on Windows a logoff or
  shutdown with the window open ends the process in under 2 s with no "this app is preventing"
  screen; that on Linux SIGTERM, SIGHUP and SIGINT each end the process at once through the
  fallback path and in under 2 s.
- shell-spike-7: on a Linux test machine, `cargo clippy -p ghostlight-desktop --all-targets -- -D
  warnings` and `cargo test -p ghostlight-desktop`. `spike.rs` compiles Linux-only code
  (`RendererLost`, the signal install) that has not been built on the coordinator's host.
- shell-spike-7: the coordinator's live checklist. Run `ghostlight-desktop` from
  `.target-lane-shell-spike/debug` on Windows 10, Windows 11, and GNOME and KDE on Wayland (X11
  too where available), and record the result per platform:
  1. The tray icon and one window with the spike page appear within 2 s, dark from the first
     frame, with no console.
  2. Post a message gets an echo marked `"from":"shell"`.
  3. Probes fetch, img and script say "blocked by the content policy", window.open says "new
     window denied", location.assign says "navigation denied".
  4. Closing the window keeps the tray. Tray Open (on Windows also a left click on the icon)
     creates exactly one window, also after five quick Opens. On Windows count top-level windows
     of class "Ghostlight Window" owned by the process, hidden ones included; on Linux count
     toplevels.
  5. Minimize, then Open: Windows restores the same window, Linux replaces it with exactly one
     fresh window.
  6. Linux: killing the WebKitWebProcess discards only the window, and Open makes a fresh one.
  7. Signing out with the spike running shows no "This app is preventing shutdown", and the
     process is gone within 2 s.
  8. Task Manager shows the description Ghostlight with the ghost icon.
  9. No connection to 192.0.2.1 is ever attempted (Resource Monitor, or `ss -tn` on Linux).
  10. After tray Quit on Windows, no icon is left in the notification area (the tray and window
      are dropped before the process exits, so Explorer removes the icon at once).
- shell-spike-7: if the window stays blank because a webview navigates to `about:blank` first,
  the follow-up for the shell lane is to allow exactly `about:blank` in
  `scheme::allows_navigation`, with the reason in a comment.

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
- Release size of `ghostlight-desktop.exe` (`cargo build --release -p ghostlight-desktop`, built
  never run): 1,457,152 bytes (1.39 MiB) on `x86_64-pc-windows-msvc`. Record the Linux size when
  the Linux machine builds it.
- The spike has no `about:blank` allowance in `scheme::allows_navigation`; the live checks say
  whether a platform needs one.
