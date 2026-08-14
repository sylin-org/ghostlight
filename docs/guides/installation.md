# Installing Ghostlight 1.0

Ghostlight installs as one local product: three sibling executables, a tray workbench, and a
matching Chromium extension. No account, no resident launcher service, no admin rights.

This guide covers the planned provenance-attested 1.0 package and the source-development path you
can follow today. The published 0.8 package and its Chrome adapter are documented in
[`../public-status.json`](../public-status.json); keep those and a 1.0 build apart, because they
are not interchangeable.

## Release installation journey

1. Run `npx -y ghostlight@1.0.0 install`. The checksum-bound launcher downloads one exact sibling
   set, registers the browser connector for the current user, and connects detected MCP clients.
   A native package, Scoop, WinGet, the one-line installer, or the Claude Desktop
   MCPB may provide the same binaries instead.
2. Install the matching `Ghostlight in Browser` 1.0 extension from its release listing.
3. Restart or reconnect the MCP harness, then run the bounded first proof from the README. Launch
   Ghostlight whenever you want the tray workbench.

That is the normal installation path. `npx -y ghostlight@1.0.0 doctor` is recovery when something
does not connect; it is not another required setup step.

Setup is now complete. Launching the registered MCP client or Chromium demand-starts Ghostlight if
it is absent. The complete desktop authority starts with its tray available and its workbench
minimized. Launch Ghostlight again whenever you want the workbench; the second launch focuses the
existing authority instead of creating another one.

Only checksum-bound, provenance-verified 1.0 packages and the matching 1.0 extension satisfy this
journey. Provenance verification, clean-machine install, upgrade, and uninstall are release gates,
not claims made by this source tree.

The npm process is a download and launch edge, not the product authority. Supported-client
registrations point directly at the cached native MCP connector. A bare `npx -y ghostlight@1.0.0`
remains the stdio command for another compatible MCP client.

## Workbench installation behavior

MCP integrations supports these explicit user-level configurations:

| Harness | Configuration family |
| --- | --- |
| Codex | `mcp_servers.ghostlight` in TOML |
| Claude Code, Claude Desktop, Cursor, Windsurf | `mcpServers.ghostlight` in JSON or JSONC |
| Visual Studio Code | `servers.ghostlight` in JSON or JSONC |
| Zed | `context_servers.ghostlight` in JSON or JSONC |
| OpenCode | detected v1 `mcp.ghostlight` or v2 `mcp.servers.ghostlight` dialect |
| Crush | `mcp.ghostlight` in JSON or JSONC |

**Check** is read-only. **Install** and **Uninstall** are explicit, serialized operations. They:

- point directly to the sibling `ghostlight-mcp-connector`;
- create the parent directory when preparing a not-yet-detected harness;
- preserve unrelated properties, JSONC comments, trailing commas, TOML comments, and formatting;
- create a `.ghostlight-backup` beside an existing file before replacement;
- are idempotent; and
- refuse malformed, unreadable, or foreign-owned `ghostlight` entries.

Uninstall removes only an entry whose command identifies Ghostlight's connector. It does not
uninstall the harness, delete unrelated empty sections, remove the browser extension, or stop the
orchestrator.

## Build from source

Use Rust 1.82 or newer:

```sh
git clone https://github.com/sylin-org/ghostlight
cd ghostlight
cargo build --workspace
```

Start the workbench:

```sh
target/debug/ghostlight
```

Or start only the persistent service:

```sh
target/debug/ghostlight --headless
```

For browser development, load `extension/` unpacked in Chromium 116 or newer. Its pinned key
preserves the established development identity. The platform native-messaging host must point at
the sibling source-built `ghostlight-browser-connector`; use the isolated procedure in
[`../DEV-LOOP.md`](../DEV-LOOP.md) and never replace an installed stack you do not own.

The repository process journey exercises the real three-executable topology without modifying a
user installation:

```sh
cargo build --workspace --target-dir .target-ghostlight-1.0
node tests/process-journey.mjs
```

## Verify

Open **Status**. A useful report distinguishes:

- orchestrator runtime state;
- connected browser adapter state;
- local and managed authority validity;
- audit/history readiness; and
- native notification availability through an explicit test.

The terminal equivalent is `ghostlight doctor`; `ghostlight doctor --fix` applies only
ownership-safe repairs. Use `ghostlight install --dry-run` before a scripted rollout.

Open **MCP integrations** and confirm your client reads Connected. Then ask it:

> Open https://example.com in a new Ghostlight tab, summarize the page, and tell me which tab you
> used. Do not click, type, submit, or change the page after it opens.

The browser should create or reuse one blue `Ghostlight - <client label>` group. A new first group
opens in a dedicated normal window rather than disrupting the user's active window.

## Recovery

- **Service is not running:** reconnect the MCP integration or enable the matching extension.
  Either connector demand-starts its exact sibling service and keeps retrying.
- **Workbench window is gone:** launch Ghostlight directly or open it from the tray. Closing the
  window destroys only that disposable surface; browser service and the tray remain available.
- **Workbench cannot initialize:** the orchestrator continues headlessly; reconnect after fixing
  the native WebView or tray environment.
- **Tools are absent:** re-check the registration in MCP integrations, then reconnect that MCP
  server in its own client.
- **Browser is disconnected:** keep Ghostlight running, enable the matching extension, and inspect
  Status again.
- **A client needs attention:** inspect its configuration. Ghostlight deliberately did not
  overwrite malformed or foreign data.
- **A tab close was blocked:** the tab is retained as visible evidence. Change both applicable
  orchestrator policy and the extension's local preserve-tabs setting only if the user wants
  model-driven close.
- **An in-flight call lost transport:** inspect before taking another effect. Relays reconnect, but
  Ghostlight never replays an unknown effect.

## Update and uninstall

A 1.0 updater replaces the three version-matched sibling executables and desktop assets as one
package. It does not silently edit harness registrations or extension settings.

Before removing Ghostlight, run `ghostlight uninstall` or use **MCP integrations** to Disconnect
each registration it owns.
Then remove the matching browser extension and use the operating system's package uninstall. A
release package must remove only its own native-messaging registration and files; clean-machine
verification of that behavior is required before publication.
