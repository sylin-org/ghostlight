# Installing Ghostlight (for AI agents)

Ghostlight is an MCP server that gives you governed access to the user's own, already-logged-in
Chromium browser. You run the commands, then test; pull the user in only for what a CLI can't do
-- restart the client to load the server, and load the browser extension if the test shows it
missing. Everything here is idempotent and safe to re-run.

## 1. Install and register Ghostlight

Run the installer. The `npx` launcher fetches the native service and relay on first run, so there is
nothing to compile. It registers every detected supported MCP client and the browser's native host:

```sh
npx -y ghostlight install
```

Supported client ids are `claude-code`, `claude-desktop`, `cursor`, `vscode`, `codex`, `windsurf`,
`zed`, `opencode`, and `crush`. Use `--client <id>` if the user asked to target only one. The command
is idempotent and a first install opens the extension walkthrough. Show the user the output. If a
commented JSONC config needs a manual step, preserve the file and use the exact entry the installer
prints; a manual step is guidance, not an installation failure.

For a client the installer does not recognize, add this entry manually after running the command:

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

Requirements on the machine: Node.js (for `npx`) and a Chromium browser (Chrome, Edge, Brave, or
Chromium, version 116+). The binary itself has zero runtime dependencies.

## 2. Complete the visible browser step

The CLI cannot silently install a Chromium extension. Ask the user to follow the walkthrough opened
by step 1. Until the Chrome Web Store listing is public, that means downloading
`ghostlight-extension-v*.zip` from https://github.com/sylin-org/ghostlight/releases/latest,
unzipping it, and loading it at `chrome://extensions` (Developer mode -> Load unpacked).

## 3. Test the whole chain

No ghostlight tools yet? Have the user restart the client (skip if it hot-reloaded them). Then get a
`tabId` (`tabs_context_mcp`, `createIfEmpty: true`) and `navigate` to https://sylin.org/ghostlight/.

- Loads and readable -> the whole chain works. Go to step 4; the extension is already there, so do
  not ask the user to install it.
- Errors or not connected -> the browser extension is the likely cause (the one piece the CLI can't
  install):
  1. Run `npx -y ghostlight doctor` to confirm which link is broken.
  2. Have the user complete the extension walkthrough from step 2.
  3. Retest (reload the extension at `chrome://extensions` first if the browser was already open).

## 4. First use

You already have a `tabId` from the test above (reuse it, or call `tabs_context_mcp` again). You
work inside a dedicated, clearly labeled tab group, visually separate from the user's own tabs.
Call `explain` at any time to see every available action and the capability it requires under the
session's policy (with no policy configured, everything is allowed).
