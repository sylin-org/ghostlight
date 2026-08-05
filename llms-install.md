# Installing and using Ghostlight (for AI agents)

Ghostlight lets you work in the user's signed-in Chromium browser while the user watches and keeps
control. Personal use is complete without policy. Optional governance may limit capabilities or
domains, and a denial is a boundary to explain rather than work around.

Install, test, and recover with the steps below. Pull the user in only for the visible actions a
CLI cannot complete: adding the store extension and restarting an MCP client that did not
hot-reload its tools.

## 1. Install and register

Run the idempotent installer:

```sh
npx -y ghostlight install
```

The launcher fetches the native MCP connector, persistent service, and browser connector. It
registers detected supported clients and the browser native host. Supported client ids are
`claude-code`, `claude-desktop`, `cursor`, `vscode`, `codex`, `windsurf`, `zed`, `opencode`, and
`crush`. Use `--client <id>` only when the user asked to limit the change. Show the user the plan
or result. Preserve any config the installer says it cannot merge safely.

For another compatible local stdio MCP client, run the installer for the browser side and add the
equivalent of this client-owned entry:

```json
{
  "mcpServers": {
    "ghostlight": {
      "command": "npx",
      "args": ["-y", "ghostlight"]
    }
  }
}
```

Requirements: Node.js for the `npx` launcher and Chrome, Edge, Brave, or Chromium 116+. The running
Ghostlight service is native Rust, not a Node service.

## 2. Complete the visible browser step

The CLI cannot silently install a Chromium extension. Ask the user to follow the walkthrough
opened by the installer and add
[Ghostlight in Browser from the Chrome Web Store](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl).
Do not offer a release archive, unpacked extension, or other end-user fallback.

## 3. Test the whole chain

If Ghostlight tools are absent, ask the user to restart or reconnect the current MCP client. Do
not launch a standalone connector as a workaround.

When the tools are present:

1. Call `tabs_context_mcp` with `createIfEmpty: true` to get a valid `tabId`.
2. Call `navigate` for that tab with `https://example.com/`.
3. Call `get_page_text` and report the page heading plus the exact tab used.
4. Do not click, type, submit, or change the page during this proof.

If that succeeds, the client, MCP connector, service, browser connector, extension, and browser
path all work. If it fails, run `npx -y ghostlight doctor` and follow its named finding.

## 4. Choose the next tool class

Keep tool choice compact; the live tool registry is authoritative.

- Inspect owned tabs with `tabs_context_mcp`; create or recover a workspace with
  `tabs_create_mcp`.
- Understand a page with `read_page`, `get_page_text`, or `find` before acting.
- Prefer `act_on` for one semantic target, `form_fill` for several labeled fields, and
  `form_input` for one field whose fresh ref is already known.
- Use `computer` for screenshots and low-level coordinate or keyboard work, not as the default for
  a semantic target or form.
- Use `browser_batch` when every step input is known before the call. Use `script` when later steps
  consume structured results from earlier ones.
- Use `wait_for` for a named page condition. Use page, console, and network reads to diagnose a
  failed workflow before guessing at another action.
- Call `explain` for the complete in-session action and capability directory.

## 5. Recover without guessing

- **No tools:** reconnect through the current MCP client. Cached tool names do not prove its stdio
  connector is still alive.
- **Browser disconnected:** run `npx -y ghostlight doctor`; ask the user to enable the store
  extension if the finding says it is disabled.
- **Stale tab or workspace:** call `tabs_context_mcp`. If no usable workspace remains, call
  `tabs_create_mcp` once. Do not substitute an arbitrary tab.
- **`Transport closed` during an effectful call:** stop, reconnect through the client, and inspect
  current tab/page state before deciding whether a retry is safe.
- **Denied:** call `explain`, report the missing capability or domain boundary, and stop or choose a
  genuinely lower-capability way to meet the user's goal. Do not evade policy with a lower-level
  tool.
- **Uncertain side effect:** inspect before retrying. A lost response does not mean the page action
  failed.

Ghostlight 0.8 implements exact local stdio MCP revisions `2025-11-25` and `2026-07-28`. Follow the
selected revision's workspace contract; do not invent a remote MCP endpoint. Current public
release and adapter state live in [`docs/public-status.json`](docs/public-status.json).
