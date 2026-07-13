# Ghostlight for the solo developer

A few minutes from install to an agent driving your real browser, plus optional personal safety
rails. Everything on this page is free, forever, with no key and no account.

## What you get

Your AI agent (Claude Code, Claude Desktop, Cursor, VS Code, or any MCP client) gets a
real browser: your cookies, your logins, your tabs. Twenty-one tools -- navigate, click,
type, screenshot, read the page, find elements, fill forms (by ref or by label), run
JavaScript, inspect console and network traffic, wait for dynamic pages to settle,
compose multi-step scripts, upload files and screenshots to page inputs, batch actions,
record a GIF of a session, and manage tabs -- at byte-parity with the schemas the model
was trained on, plus the additive tools. The agent works inside its own tab group
(labeled with a ghost) so its activity is visually separate from yours.

By default Ghostlight is all-open: no policy and no restrictions. The local flight recorder is on
so you can understand what happened; it does not enforce anything. Governance is an overlay you can
opt into later, one setting at a time.

## Setup

Prerequisites: a Chromium browser (Chrome, Edge, Brave, or Chromium 116+), an MCP client, and Node
for the `npx` launcher. Ghostlight itself runs as native Rust executables and does not keep a Node
service running.

1. Install the local service, native host, and detected MCP clients:

       npx -y ghostlight install

   The command is idempotent and opens the current browser-extension walkthrough. Use `--dry-run`
   if you want to see every planned change first, or `--no-open` for a quiet installation.

2. Install the extension using that walkthrough. Until the Chrome Web Store listing is public,
   download the extension archive from the latest GitHub release and load it unpacked at
   `chrome://extensions`.

3. Restart your MCP client. If you want to verify the whole chain:

       npx -y ghostlight doctor

   A healthy report says the browser and client are registered, the IPC endpoint
   accepts, and the extension is connected. Anything wrong prints as a specific finding.

4. First prompt to your agent:

   > Open a new browser tab, go to example.com, and tell me what the page says.

## Optional personal safety rails

These are for you, not for an employer, and they are always free.

**Sacred domains** -- sites the agent must never touch, enforced on every tool call
regardless of anything else:

    ./target/release/ghostlight config set content.security.sacred_domains '["*.mybank.com","brokerage.example"]'

**The pause and the kill switch.** The extension popup gives you take-the-wheel: pause
the agent mid-run, take over the browser, resume when ready. The panic kill switch
severs the session outright.

**Secret redaction.** Password, OTP, and payment field values are replaced with
`[value redacted]` in page reads when `content.security.secrets.redact` is on (it is on
under the default preset).

**The audit trail, for yourself.** The default flight recorder writes one JSON line per tool call
to your local data directory (`audit.jsonl`). `ghostlight config get audit.file.path` shows where;
set `audit.enabled` to `false` if you prefer not to retain it.

**A personal policy**, if you want the agent limited to certain sites. Start from an
example and preview what it means in plain sentences:

    npx -y ghostlight policy init --template developer-unrestricted --out my-policy.json
    npx -y ghostlight policy explain my-policy.json

Then point the server at it by setting `GHOSTLIGHT_MANIFEST=file:///path/to/my-policy.json`
in the MCP server's environment (or see `examples/research-read-only.json` for a
read-only starting point). No manifest means all-open; removing the variable removes all
policy.

## Where the free line is

Everything above, all of it, free forever, including for your side business. The paid
line is organizations of more than five people running centrally-managed governance in
production: see [PRICING.md](../../PRICING.md). If that is not you, you never need to
think about it.

## When something breaks

Run `npx -y ghostlight doctor` first; it pinpoints the common failures. If you are building from
source on Windows, use the isolated dev loop documented in [DEV-LOOP.md](../DEV-LOOP.md); a live installed
service can otherwise hold release executables open. The installation guide has the rest.
