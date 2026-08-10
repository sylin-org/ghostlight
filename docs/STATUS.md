# STATUS -- Ghostlight 1.0 source candidate

Last updated: 2026-08-10.

This is the mutable implementation snapshot. Git history, the ADR index, dated research, and the
preserved `docs/0.8/` material carry history; this file does not rewrite it.

## Implemented

- One Rust 2021 workspace builds four roles: the shared typed bridge, `ghostlight` orchestrator,
  generic MCP connector, and opaque browser connector.
- The orchestrator owns the 24-tool model-facing catalog, workspace aggregate, one executor and
  completion path, immutable authority snapshots, runtime controls, payload-free audit, browser
  port, and content-free presentation decisions.
- The stable browser fringe includes a policy-free Manifest V3 extension, durable native relay,
  operation-disposition recovery, one browser-wide exact-title group per client label, dedicated
  Ghostlight window placement, and the established visual language and product identity.
- Model-driven tab close is admitted by service authority and then checked by the extension's
  default-on preserve-tabs interlock. A refusal stays visible and returns a blocked no-effect
  result.
- The `ghostlight` executable now hosts a Tauri 2 workbench inside the modular monolith. It has a
  tray lifecycle, at-a-glance home, plural sessions/operations/browser instances, payload-free
  history, checkup, runtime configuration, supported-harness installations, bounded global search,
  and content-free native notifications.
- Supported harness registrations are Codex, Claude Code, Claude Desktop, Cursor, Visual Studio
  Code, Windsurf, Zed, OpenCode, and Crush. Check is read-only. Install/uninstall are explicit,
  serialized, ownership-checked, backed up, and preserve unrelated JSONC/TOML comments and
  configuration.
- `ghostlight --headless` retains the service-only execution path. Recoverable desktop startup and
  event-loop failures leave that service running.

## Verified in this workspace

- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace`: 64 Rust tests across orchestrator, bridge, and MCP edge.
- `npm test --prefix extension`: 39 extension tests.
- `node tests/process-journey.mjs`: stable MCP and browser relays reconnect through a service
  restart without replaying an interrupted effect, then complete open/read/close.
- The workbench renders against a plural-state visual fixture, uses the byte-identical original
  Ghostlight artwork, and exposes keyboard-reachable rail destinations and controls.
- The complete desktop-workbench change has an empty diff under `crates/mcp-connector`,
  `crates/browser-connector`, `crates/bridge`, and `extension`.

## Release gates still requiring an owner or release environment

- Produce and sign platform bundles, install the native-messaging registration, and verify upgrade
  and uninstall from a clean machine on Windows, macOS, and Linux.
- Complete interactive native-window, tray, and notification smoke tests on each platform. The
  automated environment verifies native build and failure containment but does not expose its GUI
  desktop to the test runner.
- Run the accepted browser-job matrix against visible supported Chromium browsers, including
  screenshots, file upload, form input, dialogs, governed denial, reconnect, and local close
  interlock journeys.
- Reconcile release metadata, public status, store submission, compatibility, distribution, and
  the final public documentation only when the 1.0 artifacts exist.

## Canonical 1.0 sources

- Product intent: [`1.0/INTENT.md`](1.0/INTENT.md)
- Model-facing language: [`1.0/LANGUAGE.md`](1.0/LANGUAGE.md)
- Architecture: [`1.0/ARCHITECTURE.md`](1.0/ARCHITECTURE.md)
- Acceptance: [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md)
- Desktop decision: [`adr/0102-integrated-desktop-workbench.md`](adr/0102-integrated-desktop-workbench.md)
