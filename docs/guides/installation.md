# Installing Ghostlight 1.0

Ghostlight installs as one local product: three sibling executables, a tray workbench, and a
matching Chromium extension. No account, no launcher runtime, no admin rights.

This guide covers the planned signed 1.0 package and the source-development path you can follow
today. The published 0.8 package and its Chrome adapter are documented in
[`../public-status.json`](../public-status.json); keep those and a 1.0 build apart, because they
are not interchangeable.

## Release installation journey

1. Install the signed Ghostlight package for the operating system. Packaging places
   `ghostlight`, `ghostlight-mcp-connector`, and `ghostlight-browser-connector` side by side and
   registers the browser connector as the `org.sylin.ghostlight` native host for the current user.
2. Install the matching `Ghostlight in Browser` 1.0 extension from its release listing.
3. Launch Ghostlight, open **MCP integrations**, find your client, and choose **Register directly**.
4. Restart or reconnect the MCP harness, then run the bounded first proof from the README.

Setup is now complete. Launching the registered MCP client or Chromium demand-starts Ghostlight if
it is absent. The complete desktop authority starts with its tray available and its workbench
minimized. Launch Ghostlight again whenever you want the workbench; the second launch focuses the
existing authority instead of creating another one.

Only signed 1.0 packages and the matching 1.0 extension satisfy this journey. Artifact signing,
clean-machine install, upgrade, and uninstall are release gates, not claims made by this source
tree.

## Agent Plugin connection path

The repository root is an Agent Plugins v1 package. Its `plugin.json` and `mcp.json` form a thin,
MCP-only connection declaration. A compatible client starts the bare command
`ghostlight-mcp-connector`; the package does not contain or download a second Ghostlight runtime.

This is an alternative to **Register directly**, not another prerequisite after it. When using
the Agent Plugin route:

1. Install the signed Ghostlight operating-system package and matching store extension first.
2. Install or enable the Ghostlight Agent Plugin in the client.
3. Manage that connection, including disable, update, and removal, in the client that owns it.
4. Run the same bounded first proof.

Do not also register Ghostlight directly unless a second connector session is intentional. The
Workbench does not inspect or mutate client plugin stores, so it does not report a client-owned
plugin as a direct registration. If a client writes the portable bare connector command into its
ordinary configuration, the Workbench shows **Managed in client** and leaves it untouched.

The source tree proves the portable declaration and an isolated installed-command topology. It
does not yet prove that a signed 1.0 installer makes the bare connector name discoverable to every
supported GUI and terminal client. No client, platform, marketplace, or one-click installation
claim should be made until the signed real-client matrix passes.

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

**Re-check** is read-only. **Register directly** and **Remove registration** are explicit,
serialized operations. They:

- point directly to the sibling `ghostlight-mcp-connector`;
- create the parent directory when preparing a not-yet-detected harness;
- preserve unrelated properties, JSONC comments, trailing commas, TOML comments, and formatting;
- create a `.ghostlight-backup` beside an existing file before replacement;
- are idempotent; and
- refuse malformed, unreadable, or foreign-owned `ghostlight` entries.

Remove registration removes only a direct entry with an absolute path identifying Ghostlight's
connector. A bare connector command is client-managed and cannot be removed here. The action
does not uninstall the harness, remove a client-owned Agent Plugin, or delete unrelated empty
sections. It also does not remove the browser extension or stop the orchestrator.

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
node tests/agent-plugin-contract.mjs
node tests/agent-plugin-journey.mjs
node tests/process-journey.mjs
```

## Verify

Open **Status**. A useful report distinguishes:

- orchestrator runtime state;
- connected browser adapter state;
- local and managed authority validity;
- audit/history readiness; and
- native notification availability through an explicit test.

For a Workbench-owned connection, open **MCP integrations** and confirm your client reads **Direct
registration**. For an Agent Plugin connection, confirm that the plugin is enabled in its client.
Then ask it:

> Open https://example.com in a new Ghostlight tab, summarize the page, and tell me which tab you
> used. Do not click, type, submit, or change the page after it opens.

The browser should create or reuse one blue `Ghostlight - <client label>` group. A new first group
opens in a dedicated normal window rather than disrupting the user's active window.

## Recovery

- **Service is not running:** reconnect the MCP integration or enable the matching extension.
  Either connector demand-starts its exact sibling service and keeps retrying.
- **Workbench window is gone:** launch Ghostlight directly or open it from the tray. Closing the
  window only hides it.
- **Workbench cannot initialize:** the orchestrator continues headlessly; reconnect after fixing
  the native WebView or tray environment.
- **Tools are absent through direct registration:** re-check the entry in MCP integrations, then
  reconnect that MCP server in its own client.
- **Tools are absent through an Agent Plugin:** verify that the signed Ghostlight package is
  installed, the client can resolve `ghostlight-mcp-connector`, and the plugin is enabled. Manage
  the plugin in the client, not in the Workbench.
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

Before removing Ghostlight, use **MCP integrations** to remove each direct registration it owns,
and use each client to remove or disable its Agent Plugin connection. Then remove the matching
browser extension and use the operating system's package uninstall. Removing an Agent Plugin does
not remove the independently installed Ghostlight product. A release package must remove only its
own native-messaging registration and files; clean-machine verification of that behavior is
required before publication.
