# ADR-0119: Durable desktop authority and disposable workbench

- Status: Accepted
- Date: 2026-08-13
- Supersedes: ADR-0102 Decision 1 close-to-hide behavior
- Amends: ADR-0112 Decision 3
- Builds on: ADR-0118

## Context

Ghostlight is primarily a local background authority. Its tray is the human access point to a
secondary workbench window, and explicit Quit is the action that ends browser service.

The original desktop adapter treated the configured WebView as permanent. It intercepted the
native close control and hid that same window, while tray activation assumed the window would
always exist. This couples the authority lifetime to a presentation object and cannot recover when
WebKit's renderer terminates. It also makes the native close control appear broken when a delayed
hide races the compositor.

Established desktop implementations separate these lifetimes. Electron's tray example keeps the
application alive after all windows close and implements Open as focus-if-present or create-if-
absent. Tauri exposes application exit retention independently from window events. WebKitGTK
publishes `web-process-terminated` specifically so the native owner can contain an abnormal
renderer exit.

WebKitGTK also has unresolved DMA-BUF failures with proprietary NVIDIA drivers on Linux. Its
upstream issue tracker documents `WEBKIT_DISABLE_DMABUF_RENDERER=1` as a compatibility path. That
policy must be selected before WebKit starts; it is not renderer-lifecycle recovery.

## Decision

### 1. The authority outlives any workbench window

The desktop authority and tray remain alive when the last workbench window closes or its renderer
fails. An implicit last-window exit request is retained by the application event loop. Explicit
Quit uses the programmatic exit path and remains terminal.

### 2. Close destroys, minimize minimizes, and Open creates or focuses

Ghostlight does not intercept the native close or minimize controls. Close destroys the disposable
workbench window. Minimize leaves that window under normal window-manager control.

Tray activation and authenticated second-instance activation serialize one operation: focus the
existing workbench, or rebuild it from the canonical Tauri window configuration when absent. The
replacement receives a fresh `WorkbenchFacade` snapshot and owns no service state.

Linux and Windows activation request native deiconify, show, and focus operations. Normal minimize
and taskbar restore remain window-manager-owned.

### 3. Renderer loss discards only the failed view

On Linux, the adapter observes WebKitGTK's `web-process-terminated` signal. It schedules destruction
of that exact failed window after the signal callback returns. It does not automatically rebuild,
which avoids a renderer crash loop. The next explicit Open creates a fresh WebView.

Windows keeps the same authority/window lifecycle. Its WebView runtime does not use the Linux
WebKitGTK signal or compatibility policy.

### 4. Linux renderer compatibility is startup policy

Before GTK or WebKit initialization, Linux disables the WebKitGTK DMA-BUF renderer when the
proprietary NVIDIA kernel driver is present. An explicitly supplied
`WEBKIT_DISABLE_DMABUF_RENDERER` value wins. Other Linux graphics stacks and Windows retain their
native renderer defaults.

## Consequences

- Native close and minimize controls keep their platform meanings.
- A closed or failed workbench releases its WebView resources while browser service continues.
- Open is idempotent across present, absent, minimized, hidden, and renderer-failed states.
- Renderer recovery has no retry thread, timer, alternate authority, or automatic crash loop.
- Linux NVIDIA compatibility may trade WebView rendering performance for stability only on the
  affected driver family, and users retain an explicit override.

## Prior art

- [Electron tray lifecycle](https://www.electronjs.org/docs/latest/tutorial/tray)
- [Tauri application event loop](https://docs.rs/tauri/latest/tauri/struct.App.html)
- [WebKitGTK renderer termination signal](https://webkitgtk.org/reference/webkit2gtk/2.40.0/signal.WebView.web-process-terminated.html)
- [WebKitGTK NVIDIA DMA-BUF failure](https://bugs.webkit.org/show_bug.cgi?id=280210)
- [Windows native window controls](https://learn.microsoft.com/en-us/windows/win32/uxguide/win-window-mgt)
