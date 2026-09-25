# Ghostlight 1.3.10

Ghostlight 1.3.10 fixes a Windows lifecycle defect that could leave the desktop authority or its
connectors running while Windows was shutting down or logging off.

### Fixed

- Every Windows Ghostlight process now owns a mandatory hidden top-level session listener, so
  `WM_QUERYENDSESSION` and `WM_ENDSESSION` reach the desktop authority and both connectors.
- The desktop authority arms its independent 1.5-second process-exit guard before asking Tauri to
  exit. A Tao event target already destroyed by shutdown can no longer prevent the fallback.
- Connector shutdown notification is non-blocking, and the console fallback preserves Windows'
  default process termination.
- The Windows native lifecycle journey now proves bounded successful end-session exit for the MCP
  connector, browser connector, and desktop authority.

Compatible with Chrome adapter 1.1.4 through 1.3.8 and Firefox adapter 1.3.9.
Chrome adapter 1.3.8 is currently published in the Chrome Web Store.
