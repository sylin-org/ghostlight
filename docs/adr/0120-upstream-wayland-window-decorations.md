# ADR-0120: Upstream Wayland window decorations

- Status: Accepted
- Date: 2026-08-13
- Amends: ADR-0119 Decision 2 for Linux

## Context

Ghostlight leaves minimize, maximize, close, resize, and move behavior to the native windowing
stack. Under KDE Wayland, the restored workbench displayed all three titlebar controls but none
accepted pointer input. Double-clicking the titlebar to maximize the window changed the input
stacking and made the controls work.

This is Tao issue 1218, also reported through Tauri issue 13440 with the same maximize/restore
sequence. Tao 0.35.3 installs a custom GTK `EventBox` above its Wayland header bar. That input
window consumes the pointer events intended for the header controls. The merged upstream fix
removes the custom header and returns decorated windows to GTK's native decoration handling.

Tao published the fix in 0.36.0. The current published Tauri runtime still requires Tao 0.35,
but the exact upstream merge commit retains package version 0.35.3 and is compatible with that
runtime constraint.

## Decision

Ghostlight uses Tao's upstream Wayland decoration fix as its windowing implementation. The
workspace patches the crates.io Tao package to the exact merged upstream commit
`07f3742b1833b64be27b1ef991e38d557d4276c9`.

This is a dependency pin, not a Ghostlight window-control implementation. Ghostlight does not
create a custom titlebar, alter GTK widgets after Tauri initializes them, force XWayland, or
special-case one compositor. Windows remains on Tao's normal Windows implementation.

The Wayland xdg-shell protocol lets a client request minimization but deliberately provides no
request to unset it and no way for the client to know whether the surface is minimized. A taskbar
can restore the surface through compositor authority, but a later Ghostlight Open cannot portably
distinguish or recover that state through its own window handle. On Linux, Open therefore discards
any existing workbench and builds a fresh disposable view through the same serialized lifecycle
seam used after close or renderer loss. An ordinary taskbar restore remains compositor-owned and
does not rebuild anything.

Open requests coalesce while replacement is pending. Tauri must first report the old window as
destroyed; only then does the main loop construct the replacement under the same label. This keeps
tray callbacks and service activation on one lifecycle path and avoids a second `main` window
during toolkit event delivery. Renderer loss and an ordinary close do not set replacement pending,
so neither causes an automatic rebuild.

Remove the patch and its `cargo-deny` source allowance when the selected Tauri runtime accepts
Tao 0.36 or newer. The lockfile must continue to identify the exact source revision until then.

## Consequences

- Linux decorated windows use GTK's native titlebar behavior on Wayland.
- Native controls remain owned by the compositor and toolkit, as ADR-0119 requires.
- Linux Open reconstructs the disposable workbench because xdg-shell can neither report nor unset
  client-requested minimization. Windows continues to focus or restore its existing workbench.
- Opening an already visible Linux workbench refreshes that presentation from the facade snapshot.
- Ghostlight carries no forked Tao source and no duplicate window-control behavior.
- Dependency resolution needs network access when this exact Git revision is not already cached.
- The temporary Git source is explicit in both Cargo metadata and the supply-chain allowlist.

## Prior art

- [Tao Wayland decoration fix](https://github.com/tauri-apps/tao/pull/1218)
- [Tauri native titlebar control failure](https://github.com/tauri-apps/tauri/issues/13440)
- [Wayland xdg-shell toplevel protocol](https://wayland.app/protocols/xdg-shell)
