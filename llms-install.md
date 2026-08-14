# Installing and using Ghostlight 1.0 for an AI client

Ghostlight works in the user's visible, signed-in Chromium browser. Do not install an extension,
edit a harness configuration, or retry an uncertain browser effect without the user's knowledge.
Policy denial and the extension's preserve-tabs refusal are boundaries to explain, not evade.

This file describes the planned 1.0 package. The public 0.8 package and adapter are not compatible
substitutes for a 1.0 source build.

## 1. Complete the user-visible installation

1. Ask the user to run `npx -y ghostlight@1.0.0 install`, or install the matching signed native
   package. Never substitute public 0.8 binaries for a 1.0 adapter.
2. Ask the user to install the matching `Ghostlight in Browser` 1.0 store adapter.
3. Reconnect or restart the MCP client if it does not refresh its tool catalog. Ghostlight changes
   only owned entries, creates a backup before client-config replacement, and preserves unrelated
   JSONC or TOML.

Use `npx -y ghostlight@1.0.0 doctor` only if the connection needs recovery. It is not a required
second installation command.

Supported workbench registrations are Codex, Claude Code, Claude Desktop, Cursor, Visual Studio
Code, Windsurf, Zed, OpenCode, and Crush. Another compatible local stdio MCP client can point
directly at the packaged sibling `ghostlight-mcp-connector` executable with no role flag or model
dialect.

Do not launch the connector as a standalone background service. Its stdio lifetime belongs to the
MCP client; it negotiates with the persistent orchestrator.

## 2. Test the whole chain

Use only the catalog returned by the client. For the first proof:

1. Call `browser_navigate` with `{"url":"https://example.com"}`.
2. Call `browser_read` with the returned `tab`.
3. Report the heading and the opaque tab handle.
4. Do not click, type, submit, upload, or run a script during this proof.

Success proves the MCP client, stdio connector, orchestrator, browser connector, native messaging,
extension, and visible browser path. On failure, ask the user to open **Status** and
**MCP integrations** in the workbench. Follow the named condition instead of inventing a second stack.

## 3. Choose the narrowest tool

- Use `browser_tabs` with `list`, `focus`, or `close` for controlled tab state.
- Use `browser_navigate` for a URL and `browser_history` for back, forward, or reload.
- Use `browser_window` for zoom and physical window size.
- Read with `browser_read`; inspect structure with `browser_inspect`; obtain a semantic
  target with `browser_find`.
- Prefer a semantic target for click, hover, scroll, fill, type, drag, and upload. Use screenshot
  coordinates only with the current `view` returned by `browser_screenshot`.
- Use `browser_sequence` only for a short, fully specified sequence whose later inputs do not
  depend on earlier page results.
- Use `browser_execute` only when explicit execute authority and page JavaScript are genuinely
  required.
- Use `browser_wait` for an explicit observable condition and `browser_dialog` for the
  currently visible browser dialog.
- Use `browser_record` for a bounded memory-only recording and `browser_diagnose` for opt-in,
  problem-focused console or network evidence.

The complete schemas, defaults, capabilities, and terminal result envelope are in
[`docs/1.0/LANGUAGE.md`](docs/1.0/LANGUAGE.md).

## 4. Recover without guessing

- **No tools:** reconnect the existing MCP server through the client. A cached catalog does not
  prove its connector is alive.
- **Browser unavailable:** ask the user to open Status, enable the matching extension, and keep
  Ghostlight running in the tray.
- **Ambiguous tab:** call `browser_tabs` with `{"action":"list"}`, then pass the exact `tab`.
- **Stale target or view:** inspect, find, or capture again. Handles are tied to current document
  and viewport state.
- **Transport loss during an effectful call:** inspect current browser state before deciding
  whether any new action is safe. Never infer failure from a lost response.
- **Blocked:** report the fixed reason and safe next steps. Do not reach for a lower-level tool to
  evade the same authority boundary.
- **Attention required:** stop browser effects and wait for the user.
- **Unknown or partial effect:** do not replay the call automatically.

Ghostlight 1.0 uses local stdio MCP at the client edge and typed local IPC behind it. It exposes no
remote MCP endpoint. The npm launcher is a supported install and generic-stdio entry point; the
orchestrator and both connectors remain native Rust processes.
