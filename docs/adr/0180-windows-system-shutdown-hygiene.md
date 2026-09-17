# ADR-0180: Windows System Shutdown Hygiene

Date: 2026-09-17. Status: Accepted.

Builds on ADR-0102 and ADR-0119.

## Context

On Windows, Ghostlight prevented clean operating system shutdown and restart. Windows would display the system dialog "Ghostlight: This app is preventing shutdown" until the user explicitly clicked "Shut down anyway" or the system kill timeout expired.

Two interacting mechanisms caused this behavior:

1. **ADR-0119 Exit Retention During System-Initiated Window Closure:**
   ADR-0119 established that the Ghostlight desktop authority outlives any workbench window. When the user closes the workbench window, Ghostlight disposes the window while remaining resident in the notification area (system tray). This was implemented in `crates/orchestrator/src/desktop/mod.rs` via `should_prevent_desktop_exit(code: Option<i32>) -> bool { code.is_none() }`.
   During Windows shutdown or restart, the OS sends `WM_CLOSE` to all top-level windows, including Ghostlight's workbench. Tauri treated the resulting window closure as `RunEvent::ExitRequested { code: None, api }`. Because `code` was `None`, Ghostlight unconditionally invoked `api.prevent_exit()`, preventing the application from closing.

2. **Tao Message Loop Inaction on Session Termination:**
   Even when the workbench window had already been closed by the user (Ghostlight resident only in the system tray), Tao's underlying window implementation (`thread_event_target`) transitions its internal runner state to `Destroyed` upon receiving `WM_ENDSESSION`, but does not break out of its message loop or emit an exit request. Without active window closure or an explicit exit call, the process remained running. Windows waited for the duration of `WaitToKillAppTimeout` (default 5 seconds) before flagging Ghostlight as blocking shutdown.

3. **Console Connector Hygiene:**
   Standalone connector binaries (`ghostlight-mcp-connector`, `ghostlight-browser-connector`) running in console sessions also need to respond cleanly when receiving OS shutdown and logoff events (`CTRL_SHUTDOWN_EVENT`, `CTRL_LOGOFF_EVENT`).

## Decision

### 1. Win32 FFI Confinement in `crates/win-peer`

All Windows API interactions remain strictly confined to `crates/win-peer` in accordance with repository safety invariants (`unsafe_stays_confined_to_the_audited_ffi_crate`).

- Add `crates/win-peer/src/shutdown.rs` with `listen_for_system_shutdown()` and `listen_for_console_shutdown()`.
- `listen_for_system_shutdown()` spawns a dedicated background thread (`ghostlight-win-shutdown`) running a hidden top-level window (`WS_POPUP`, `WS_EX_TOOLWINDOW`, NULL parent). A top-level window is required because Windows broadcasts `WM_QUERYENDSESSION` and `WM_ENDSESSION` exclusively to top-level windows; message-only windows (`HWND_MESSAGE`) do not receive session broadcast notifications.
- When `WM_QUERYENDSESSION` is received, the window procedure returns `1` (`TRUE`) signaling readiness to terminate.
- When `WM_ENDSESSION` is received with `wParam != 0` (indicating the session is actually ending), the window procedure emits `ShutdownEvent::Terminating` over a crossbeam channel.
- `listen_for_console_shutdown()` registers a console control handler via `SetConsoleCtrlHandler` to detect `CTRL_SHUTDOWN_EVENT` and `CTRL_LOGOFF_EVENT` for console and connector processes.

### 2. Orchestrator Shutdown Awareness and Exit Disarming

In `crates/orchestrator/src/desktop/mod.rs`:

- Track system shutdown state via a module-level atomic flag (`SYSTEM_SHUTDOWN_IN_PROGRESS`).
- In `desktop::run()`, start `listen_for_system_shutdown()`. Upon receiving `ShutdownEvent::Terminating`:
  - Mark `SYSTEM_SHUTDOWN_IN_PROGRESS` as `true`.
  - Request application exit via `app_handle.exit(0)`.
  - Spawn a 1.5-second fallback thread calling `std::process::exit(0)`. This guarantees process termination even if Tao's event loop hangs or fails to process the exit event after `WM_ENDSESSION`, well within the 5-second Windows shutdown timeout.
- Update `should_prevent_desktop_exit(code)`: if `SYSTEM_SHUTDOWN_IN_PROGRESS` is `true`, return `false`. This allows the application event loop to terminate immediately without retaining the process.

### 3. Connector Console Shutdown Hygiene

In `crates/bridge/src/lifecycle.rs`:

- Register `listen_for_console_shutdown()` during connector startup on Windows.
- Map console shutdown events to `ConnectorExit::SystemShutdown`, ensuring connectors cleanly terminate when the user logs off or shuts down the machine.

## Consequences

- **Positive:** Windows shutdown, restart, and logoff proceed smoothly without Ghostlight displaying blocking dialogs or delaying system termination.
- **Positive:** Ordinary desktop usage is preserved without regression: closing the workbench window continues to dispose the window and keep Ghostlight resident in the system tray per ADR-0119.
- **Positive:** Unsafe Win32 FFI code remains 100 percent confined to `crates/win-peer`, preserving compile-time and test-suite safety guarantees.
- **Positive:** The 1.5-second fallback exit guard insulates Ghostlight from upstream Tao event loop termination issues during session destruction.
