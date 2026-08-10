# Installing and using Ghostlight 1.0 for an AI client

Ghostlight works in the user's visible, signed-in Chromium browser. Do not install an extension,
edit a harness configuration, or retry an uncertain browser effect without the user's knowledge.
Policy denial and the extension's preserve-tabs refusal are boundaries to explain, not evade.

This file describes the planned 1.0 package. The public 0.8 package and adapter are not compatible
substitutes for a 1.0 source build.

## 1. Complete the user-visible installation

1. Ask the user to install and launch the matching Ghostlight 1.0 platform package.
2. Ask them to open Ghostlight from the tray and choose **Installations**.
3. Use **Check** for the current harness, then let the user choose **Install**. Ghostlight changes
   only its owned entry, creates a backup, and preserves unrelated JSONC or TOML content.
4. Ask the user to install the matching `Ghostlight in Browser` 1.0 store adapter.
5. Reconnect or restart the MCP client if it does not refresh its tool catalog.

Supported workbench registrations are Codex, Claude Code, Claude Desktop, Cursor, Visual Studio
Code, Windsurf, Zed, OpenCode, and Crush. Another compatible local stdio MCP client can point
directly at the packaged sibling `ghostlight-mcp-connector` executable with no role flag or model
dialect.

Do not launch the connector as a standalone background service. Its stdio lifetime belongs to the
MCP client; it negotiates with the persistent orchestrator.

## 2. Test the whole chain

Use only the catalog returned by the client. For the first proof:

1. Call `browser_open_page` with `{"url":"https://example.com"}`.
2. Call `browser_read_page` with the returned `tab`.
3. Report the heading and the opaque tab handle.
4. Do not click, type, submit, upload, or run a script during this proof.

Success proves the MCP client, stdio connector, orchestrator, browser connector, native messaging,
extension, and visible browser path. On failure, ask the user to open **Checkup** and
**Installations** in the workbench. Follow the named condition instead of inventing a second stack.

## 3. Choose the narrowest tool

- Use `browser_list_tabs` to see controlled tabs and `browser_activate_tab` to bring one into view.
- Use `browser_open_page`, `browser_navigate_page`, `browser_navigate_history`, and
  `browser_reload_page` for distinct navigation intents.
- Read with `browser_read_page`; inspect structure with `browser_inspect_page`; obtain a semantic
  target with `browser_find`.
- Prefer a semantic target for click, hover, scroll, fill, type, drag, and upload. Use screenshot
  coordinates only with the current `view` returned by `browser_take_screenshot`.
- Use `browser_run_sequence` only for a short, fully specified sequence whose later inputs do not
  depend on earlier page results.
- Use `browser_run_script` only when explicit execute authority and page JavaScript are genuinely
  required.
- Use `browser_wait` for an explicit observable condition and `browser_handle_dialog` for the
  currently visible browser dialog.

The complete schemas, defaults, capabilities, and terminal result envelope are in
[`docs/1.0/LANGUAGE.md`](docs/1.0/LANGUAGE.md).

## 4. Recover without guessing

- **No tools:** reconnect the existing MCP server through the client. A cached catalog does not
  prove its connector is alive.
- **Browser unavailable:** ask the user to open Checkup, enable the matching extension, and keep
  Ghostlight running in the tray.
- **Ambiguous tab:** call `browser_list_tabs`, then pass the exact `tab`.
- **Stale target or view:** inspect, find, or capture again. Handles are tied to current document
  and viewport state.
- **Transport loss during an effectful call:** inspect current browser state before deciding
  whether any new action is safe. Never infer failure from a lost response.
- **Blocked:** report the fixed reason and safe next steps. Do not reach for a lower-level tool to
  evade the same authority boundary.
- **Attention required:** stop browser effects and wait for the user.
- **Unknown or partial effect:** do not replay the call automatically.

Ghostlight 1.0 uses local stdio MCP at the client edge and typed local IPC behind it. It exposes no
remote MCP endpoint and requires no Node.js launcher.
