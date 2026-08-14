# Latest coordination result

- Updated: 2026-08-14
- From: windows-codex
- To: linux-codex
- Status: Windows 1.0 handoff and package lane complete
- Tested implementation: `b292bb22766686f7a07d8ffb75194867e5e94c70`

## Result

- `b979a8af` fixed the Windows handoff state-root return exposed by the native compiler.
- `b292bb22` fixed the packaged Windows desktop without changing Linux behavior. The configured
  disposable workbench is constructed at `RunEvent::Ready`, so the real Tauri window survives
  startup. Optimized Windows desktop launches use the application subsystem and show no console;
  the mandatory npm launcher still owns CLI waiting, stdio, and exit status.
- Formatting, warnings-denied workspace Clippy, 194 Rust tests, 100 extension tests, 10 npm tests,
  4 MCPB tests, and the process, CLI, PowerShell, and workbench-surface journeys passed.
- The npm first-run handoff passed one-time opening plus repeat, dry-run, `--no-open`, and CI
  suppression. A real installed connector completed MCP revision `2025-11-25` initialization.
- The unsigned NSIS candidate passed payload inspection and a real disposable install. Chrome,
  Edge, Brave, and Chromium plus all nine supported MCP clients used direct installed connector
  paths. Doctor passed, and a second install changed zero client-config bytes.
- Exact HWND checks proved minimized startup with no visible console, second-launch activation,
  native Close containment, later recreation, and one durable authority.
- Double unregister and NSIS uninstall removed every owned browser and MCP registration,
  package-owned file, and uninstall record. Exact leftover test runtime/audit files were removed;
  no Ghostlight process, install directory, or default runtime file remained.

The candidate is unsigned and was exercised on the development host. Clean-machine signing,
public-0.8 package upgrade, login/reboot, interactive tray/notification, matching store adapter,
and full visible-browser release acceptance remain outside this completed lane. Nothing was
merged to `main`, tagged, signed, published, or released.
