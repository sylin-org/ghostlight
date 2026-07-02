# Browser MCP

**Governed access to your own browser, for AI agents.**

Browser MCP is a single Rust binary plus a thin Chromium (Manifest V3) extension that gives an AI
coding agent controlled access to your real, authenticated browser session. It drives the browser
you are already logged in to, so the agent can observe and act on the web apps you already use,
through any MCP client (Claude Code, Cursor, and others).

## Status

The automation engine is complete, hardened, and verified against a real browser. What runs today:

- The full 13-tool surface, at byte-parity with the official Claude-in-Chrome tool schemas.
- Screenshots with coordinate mapping, an on-page agent cursor, accessibility-tree and text reads,
  form input (including shadow DOM), in-page JavaScript, console and network inspection, and tab
  management.
- Single portable binary with a built-in installer and a `doctor` diagnostic.
- One active protection: secret-field values (passwords, OTP, payment fields) are redacted from
  `read_page` output by default.

What is NOT built yet: the governance layer (capability manifests, identity-bound domain grants,
sacred never-touch lists, a take-the-wheel pause and panic kill switch, observe/shadow/enforce
modes, and structured audit). It is designed in detail but not implemented, so **today the engine
runs all-open**: an agent you connect has the full capability surface with no access restrictions
beyond secret redaction. Do not rely on this tool for access control yet.

Maturity: this is a developer setup. The extension is loaded unpacked, and Windows is the platform
it has been tested on. The macOS and Linux code paths exist but are not yet verified. There is no
published package or cross-platform release build yet.

## What makes it different

- **Bring your own agent.** It speaks the Model Context Protocol, so it works with any MCP client
  against the browser you already use. You are not locked into one vendor's app or cloud.
- **It is your session, not a clean-room browser.** The value is your own authenticated context:
  real cookies, real SSO, real tabs. Your work is never relocated to a cloud or a freshly launched
  browser to gain a technical property.
- **Single portable binary, zero runtime dependencies.** No Node.js, no `npx`, no separate servers
  to babysit. The class of install failures that affects Node-based browser MCPs does not exist.
- **Governance is a separable layer (coming).** The engine is unconstrained by design; the planned
  governance overlay can gate it or be absent entirely. All-open is a first-class supported mode,
  not a stripped-down build.

## Requirements

- A Chromium browser: Chrome, Edge, Brave, or Chromium, version 116 or newer.
- An MCP client: Claude Code, Claude Desktop, Cursor, or VS Code.
- A Rust toolchain (stable) to build the binary. Install from https://rustup.rs.

## Getting started

### 1. Build the binary

```sh
git clone <this-repo>
cd browser-mcp
cargo build --release
```

The binary is at `target/release/browser-mcp` (`browser-mcp.exe` on Windows). All commands below run
that binary.

### 2. Load the extension in Chrome

1. Open `chrome://extensions`.
2. Turn on **Developer mode** (top right).
3. Click **Load unpacked** and select the `extension/` directory of this repo.
4. Note the extension ID that Chrome assigns. The committed manifest key pins it to a stable value:
   `cjcmhepmagomefjggkcohdbfemacojoa`. Confirm the ID shown matches; you will pass it to the
   installer.

### 3. Register the native host and your MCP client

Run the installer from the binary you just built. It registers the native-messaging host with your
detected browsers and adds the MCP server entry to your detected MCP clients:

```sh
./target/release/browser-mcp install --extension-id cjcmhepmagomefjggkcohdbfemacojoa
```

Useful flags:

- `--dry-run` computes and prints the plan without writing anything. Run this first to see what will
  change.
- `--browser <id>` limits registration to one browser (`chrome`, `edge`, `brave`, `chromium`).
  Repeatable. `--all-browsers` registers every known browser, not just detected ones.
- `--client <id>` limits registration to one client (`claude-code`, `claude-desktop`, `cursor`,
  `vscode`). Repeatable. `--all-clients` adds to every known client.
- `--debug` registers the server to run with observability on (see Troubleshooting).
- `--system` registers machine-wide (HKLM on Windows) instead of per-user.

The installer is idempotent: re-running it will not create duplicate entries.

### 4. Restart the client and reload the extension

- Restart (or reload the MCP connection of) your MCP client so it picks up the new server entry.
- Reload the extension at `chrome://extensions` so it connects to the freshly registered host.

### 5. Verify

```sh
./target/release/browser-mcp doctor
```

A healthy result reports that the browser and client are registered, the IPC endpoint accepts
connections, and the extension is connected. If anything is off, `doctor` prints a specific,
actionable finding for each problem.

## Using it

Once connected, the browser tools appear in your MCP client and the agent can drive your browser.
Browser MCP works inside its own tab group (the "Browser MCP" group), so agent activity is visually
separated from your own tabs. A typical first request to the agent:

> Open a new browser tab, go to example.com, and tell me what the page says.

The agent will create a tab in the MCP group, navigate, read the page, and report back. It can then
click, type, fill forms, run JavaScript, take screenshots, and inspect console and network activity,
all in your real logged-in session.

### The tools

| Tool | What it does | Class |
|---|---|---|
| `navigate` | Go to a URL, or forward/back in history | observe |
| `computer` | Mouse, keyboard, and screenshots (13 actions) | observe or mutate per action |
| `read_page` | Accessibility-tree view of the page | observe |
| `get_page_text` | Visible text extraction | observe |
| `find` | Locate elements on the page | observe |
| `form_input` | Fill form fields, including shadow DOM | mutate |
| `javascript_tool` | Run JavaScript in the page context | mutate |
| `tabs_context_mcp` | List tabs in the MCP tab group | observe |
| `tabs_create_mcp` | Create a tab in the MCP tab group | mutate |
| `read_console_messages` | Recent console output | observe |
| `read_network_requests` | Recent network activity | observe |
| `resize_window` | Resize the browser window | manage |
| `update_plan` | Record the agent's working plan | manage |

The read/write class is an intrinsic property of each tool. It is informational today; the planned
governance layer uses it to allow observation while restricting mutation.

## CLI reference

The binary has no-subcommand and subcommand modes:

- No subcommand: the MCP server role. Your MCP client launches this over stdio; you do not run it by
  hand.
- `install` / `uninstall`: register or remove the native host and the MCP client entries (see flags
  above; both support `--dry-run`).
- `doctor [--verbose]`: one-shot, read-only diagnosis of the whole chain (registration, IPC
  endpoint, extension link) with a truthful exit code. It never changes anything.
- `status [--json]`: print a running server's live inner state. Requires a server started with
  `--debug` (or `BROWSER_MCP_DEBUG=1`).

## Troubleshooting

- **Start with `doctor`.** It pinpoints most problems: a browser or client that is not registered,
  no server running, a stale process holding the endpoint, or an extension that never connected.
- **Extension shows disconnected.** Reload it at `chrome://extensions`, make sure the browser is
  running, and confirm the extension ID matches what you passed to `install`.
- **Turn on observability.** Install with `--debug` (or set `BROWSER_MCP_DEBUG=1` in the server's
  environment), then run `browser-mcp status` to see live counters and per-session state.
- **Rebuilding the binary on Windows.** A running server locks `browser-mcp.exe`. Stop the MCP
  client (and reload/close the extension) before `cargo build`, then restart both.

## Architecture

```
MCP Client  --stdio-->  Rust Binary  --native messaging-->  Extension  --CDP-->  Browser
 (agent)                (the engine)   (4-byte framed)      (thin CDP           (your real
                                                             executor)           session)
```

Three processes, two protocol boundaries. The binary is both the MCP server (over stdio) and the
browser's native-messaging host; the extension is a deliberately thin CDP executor. All capability
lives in the binary, and the extension holds no policy. The planned governance layer attaches at a
single dispatch chokepoint inside the binary without touching any tool code.

## Roadmap

- **Engine (done).** The 13-tool automation surface, hardened and live-verified.
- **Governance (designed, not built).** Landing in three observable steps: (1) an audit flight
  recorder, (2) sacred never-touch domains plus a take-the-wheel pause and panic kill switch, (3)
  the full manifest engine (identity-bound grants, read/write enforcement, tool-advertisement
  filtering, observe/shadow/enforce modes) with layered configuration and org policy locks.
- **Packaging (partial).** Cross-platform release builds, CI, and macOS/Linux verification are still
  to do.

## Documentation

| Doc | What it is |
|---|---|
| [docs/SPEC.md](docs/SPEC.md) | The authoritative design specification. |
| [docs/adr/](docs/adr/) | Architecture Decision Records: the reasons behind the design and how it evolved. |
| [docs/design/](docs/design/) | Forward-looking design discussions (family and service architecture). |
| [docs/research/NORTH-STAR.md](docs/research/NORTH-STAR.md) | Governing design principles. |

## Positioning and prior art

This is a clean-room Rust rewrite informed by
[open-claude-in-chrome](https://github.com/noemica-io/open-claude-in-chrome), a Node.js
reimplementation of the Claude-in-Chrome extension. Prior art is studied as a concern surface (the
hazards and questions others hit), not as a feature catalog to copy. The tool schemas are preserved
verbatim so a trained agent behaves as expected; everything behind them is rebuilt.

## License

TBD (intended open-source).
