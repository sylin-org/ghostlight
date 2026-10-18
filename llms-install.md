# Installing and using Ghostlight for an AI client

Ghostlight works in the user's visible, signed-in Chromium browser. Do not install an extension,
edit a harness configuration, or retry an uncertain browser effect without the user's knowledge.
Policy denial and the extension's preserve-tabs refusal are boundaries to explain, not evade.

This guide describes Ghostlight 2.0. The release published today is named in the README under
"Where it stands today".

## 1. Complete the user-visible installation

1. Ask the user to run `npx -y ghostlight install`, or the one-line installer for their system:

   ```powershell
   irm https://sylin.org/ghostlight/get.ps1 | iex
   ```

   ```sh
   curl -fsSL https://sylin.org/ghostlight/get.sh | sh
   ```

   Ghostlight changes only owned MCP client entries, creates a backup before client-config
   replacement, and preserves unrelated JSONC or TOML.
2. Tell the user what happens next. The install prints a short summary with one Next line and
   opens the Ghostlight window on its setup steps: This computer, Your browser, Your agent. The
   browser step lists the user's browsers; the user presses Add to open the Chrome Web Store
   listing for `Ghostlight in Browser` in that browser, or Open if the extension is already in a
   closed browser. Do not add the extension for the user. When the extension connects, the window
   moves to the agent step.
3. If the install says an agent needs a restart, ask the user to restart it. There is no other
   restart step. Then give the user this prompt to try: "Open sylin.org/ghostlight/demo/desk and
   ring the bell." The first request that reaches a browser and succeeds finishes setup.

If the user does not see the window (`--no-open`, a remote or automated install), they can open
Ghostlight from the Start menu or Applications, or run `ghostlight open`, to see the setup steps.
Use `ghostlight doctor` only if the connection needs recovery. It is not a required second
installation command. After an npm install, `npx -y ghostlight doctor` runs the same check. If the
install said its directory is not on the PATH, run the full path it named.

Ghostlight sets up every agent in its catalog. The
[installation guide](docs/guides/installation.md#agents-ghostlight-sets-up) lists each one with its
hosts and where its configuration lives. Agent setup supports Windows and Linux; macOS is not
supported yet. Another compatible local stdio MCP client can point
directly at the installed `ghostlight` executable with the argument `mcp` (the entry is
`ghostlight mcp`), with no role flag or model dialect. An entry that names
`ghostlight-mcp-connector` was written for an earlier build and no longer works: `ghostlight
doctor` names it and Fix rewrites it.

Do not launch `ghostlight mcp` as a standalone background service. Its stdio lifetime belongs to
the MCP client; it negotiates with the persistent service.

## 2. Test the whole chain

Use only the catalog returned by the client. For the first proof:

1. Call `browser_navigate` with `{"url":"https://example.com"}`.
2. Call `browser_read` with the returned `tab`.
3. Report the heading and the opaque tab handle.
4. Do not click, type, submit, upload, or run a script during this proof.

Success proves the MCP client, MCP relay, service, browser relay, native messaging,
extension, and visible browser path. On failure, ask the user to open **Status** and
**Integrations** in the workbench. Follow the named condition instead of inventing a second stack.

## 3. Choose the narrowest tool

- Use `browser_tabs` with `list`, `focus`, or `close` for controlled tab state.
- Use `browser_navigate` for a URL and `browser_history` for back, forward, or reload.
- Use `browser_window` for zoom and physical window size.
- Read with `browser_read`; inspect structure with `browser_inspect`; obtain a semantic
  target with `browser_find`.
- Prefer a semantic target for click, hover, scroll, fill, type, drag, and upload. Use screenshot
  coordinates only with the current `view` returned by `browser_screenshot`.
- Use `browser_flow` for a short, fully specified batch or when later steps need earlier results.
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
  prove its MCP relay is alive.
- **Browser unavailable:** ask the user to open Status, enable the matching extension, and keep
  Ghostlight running in the tray.
- **Ambiguous tab:** call `browser_tabs` with `{"action":"list"}`, then pass the exact `tab`.
- **Stale target or view:** inspect, find, or capture again. Handles are tied to current document
  and viewport state.
- **Transport loss during an effectful call:** inspect current browser state before deciding
  whether any new action is safe. Never infer failure from a lost response.
- **Blocked:** report the fixed reason and safe next steps. Do not reach for a lower-level tool to
  evade the same authority boundary.
- **Explicit human Pause/Stop:** respect the runtime hold in the Ghostlight window.
- **Credential guidance:** the session remains available. When the user explicitly authorized
  entry, set `user_authorized_credentials: true` on the fill or typing request. Earlier explicit
  authorization is sufficient; no resume step is needed.
- **Unknown or partial effect:** do not replay the call automatically.

Ghostlight uses local stdio MCP at the client edge and typed local IPC behind it. It exposes no
remote MCP endpoint. The npm launcher is a supported install and generic-stdio entry point; the
service and the edge remain native Rust processes.
