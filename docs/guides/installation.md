# Installing Ghostlight

The goal is simple: install one local service, add the store extension, restart the MCP client,
and get one useful browser result. A healthy setup ends with `ghostlight doctor` reporting the
client, service, browser connection, and extension ready.

If you just want the fast path, the four stages in the
[README](../../README.md#try-one-useful-task) are the whole story for most people. Come here
when you want a different path, a per-OS detail, or an explanation of what got registered.

```text
[1 Install service] -> [2 Add extension] -> [3 Restart MCP client] -> [4 Ask a first task]
      automatic           visible step             once                useful proof
```

Ghostlight has no hosted account to create or sign in to. The MCP connector, service, browser
connector, and extension connect locally as the current OS user. Website sessions remain in the
Chromium profile you are already using. Connect only MCP clients you trust: local browser access
is powerful even when optional policy constrains it.

## Prerequisites

- A Chromium browser: Chrome, Edge, Brave, or Chromium, version 116 or newer. The 116 floor comes
  from the extension, which Chrome enforces when it loads; the binary itself checks no version.
- An MCP client (Codex, Claude Code, Claude Desktop, Cursor, VS Code, Windsurf, Zed, OpenCode,
  Crush, or another stdio MCP client).
- For the npm path, Node.js supplies the `npx` launcher; the running Ghostlight service is native
  Rust, not a Node service. For the source path, use a stable Rust toolchain (https://rustup.rs).

## Path A: the npm launcher

The launcher fetches and caches the version-matched MCP edge, service, and browser relay on first
run. Nothing to compile.

1. **Install and register Ghostlight** (idempotent, safe to re-run):

       npx -y ghostlight install

   The installer registers the browser side and every detected supported MCP client. Use
   `--client codex` to target Codex only, `--dry-run` to inspect the plan, or `--no-open` for a
   quiet installation. A first install opens the extension walkthrough.

2. **Add the extension.** Install
   [Ghostlight in Browser](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl)
   from the Chrome Web Store. Chrome shows Ghostlight's blue mascot when it is ready.

3. **Restart your MCP clients,** then try this bounded first proof:

       Open https://example.com/ in a new Ghostlight tab, summarize the page, and tell me
       which tab you used. Do not click, type, submit, or change the page.

   Verification is optional:

       npx -y ghostlight doctor

For an MCP client the installer does not recognize, add this stdio entry, then run the same install
command for the browser side:

    { "command": "npx", "args": ["-y", "ghostlight"] }

## Path B: build from source

The path when you want to read what you are running.

    git clone https://github.com/sylin-org/ghostlight
    cd ghostlight
    cargo build --release

The build produces three product executables. `ghostlight-mcp-connector` owns MCP stdio and the
exact `2025-11-25` and `2026-07-28` wire state machines. `ghostlight` is the CLI and persistent,
protocol-neutral service. `ghostlight-browser-connector` is the browser-only native host Chromium
launches.
To test the source tree immediately, open `chrome://extensions`, enable Developer mode, choose
`Load unpacked`, and select the local `extension/` directory. Then register:

    ./target/release/ghostlight install --extension-id cjcmhepmagomefjggkcohdbfemacojoa

Verify with `./target/release/ghostlight doctor`.

## What `install` actually does

It is worth knowing what gets written, because the answer is "less, and more carefully, than you
might expect." For each browser and client it targets, `install`:

- **Registers the native-messaging host** so the browser can launch Ghostlight. On Windows that is
  a registry entry (per-user under HKCU, or system-wide under HKLM with `--system`) plus a host
  manifest file; on macOS and Linux it is a host manifest file in each browser's host directory.
- **Adds the MCP server to your client's config** with an idempotent, value-level merge. This is
  the part to trust: it re-reads the file at write time and changes only the one entry it owns, so
  it never clobbers a hand-edited config and never duplicates itself if you run it twice.
  If comments make a JSONC file unsafe to merge automatically, the installer leaves it untouched
  and prints the exact entry to add. Guidance is reported separately from failure.
- **Allow-lists the extension** by id. The Web Store and source-development ids are always allowed;
  `--extension-id` adds another.
- **Registers an auto-start supervisor** so the service is there when a client asks for it. Skip it
  with `--no-supervisor`.
- **Offers the browser extension once** after a first install. The stable walkthrough
  URL contains no machine identifier or installation data. Use `--no-open` to suppress it.

The client entry it writes points directly at the sibling `ghostlight-mcp-connector` executable
with no role flag. The native-host manifest independently points Chromium at
`ghostlight-browser-connector`.

### Which clients and browsers it knows

`install` auto-detects and registers nine clients (`claude-code`, `claude-desktop`, `cursor`,
`vscode`, `codex`, `windsurf`, `zed`, `opencode`, `crush`) and four browsers (`chrome`, `edge`,
`brave`, `chromium`). That list is smaller than the set of clients Ghostlight *works* with, and the
gap is worth understanding. Any stdio MCP client can use Ghostlight; the installer only knows how
to write config for these nine because each location and dialect is handled specifically. For
anything else (Cline and the rest), add the stdio server entry from the Path A example and it
behaves the same. The installer's job is convenience, not gatekeeping.

### Useful flags

- `--dry-run` computes and prints the plan without writing anything. A good habit before the first
  real run.
- `--browser <id>` / `--client <id>` limit the scope (repeatable); `--all-browsers` /
  `--all-clients` widen it to every known target, detected or not.
- `--system` registers machine-wide (HKLM) instead of per-user.
- `--debug` registers the server to run with observability on.
- `--extension-id <id>` allows an additional extension id.
- `--no-open` prints the extension walkthrough URL without launching the default browser.

## Verify with `doctor`

`ghostlight doctor` is read-only and diagnoses the whole chain: browser registered, client
registered, IPC endpoint accepting, extension connected. A healthy run exits 0. Anything wrong
prints as a specific, actionable finding rather than a generic failure. `--verbose` adds detail,
and `--fix` is the one mode that changes anything, reaping orphaned sessions and clearing stale
state.

## Uninstall

    ghostlight uninstall

This reverses what `install` wrote: the native-host registration, the client entries (again by
idempotent merge, so a foreign config is left alone), managed executable files, and the supervisor.
`--dry-run` shows the plan first.

## Troubleshooting

- **Not sure which link failed?** Run `npx -y ghostlight doctor`. Start with its named finding
  instead of reinstalling everything.
- **No Ghostlight tools after install?** Restart or reconnect from the current MCP client. Do not
  launch `ghostlight-mcp-connector` in a separate terminal; the client owns that stdio connection.
- **Store extension shows disconnected?** Confirm that it is enabled in the browser's extension
  manager. Run `doctor` again, and restart the browser only if the finding still asks for it.
- **Source-development extension shows disconnected?** Reload it at `chrome://extensions`. A
  service worker can be evicted; reloading re-establishes the link.
- **A tab or workspace is stale?** Ask the agent to call `tabs_context_mcp`. If no usable workspace
  remains, it should call `tabs_create_mcp` once. Other tools do not silently switch workspaces.
- **The MCP client reports `Transport closed`?** Stop and reconnect through that client. Inspect
  tab and page state before retrying an effectful call whose result may be unknown.
- **A governed call is denied?** Ask the agent to call `explain`. Treat the denial as a boundary,
  not a reason to try a lower-level tool.
- **Developing on Windows?** Use the isolated engine swap in
  [DEV-LOOP.md](../DEV-LOOP.md). It builds away from locked release executables, swaps only the
  persistent service, and lets existing MCP connectors and browser connectors reconnect.
- **Ran `ghostlight` and got an error exit?** That is expected. A bare `ghostlight` with no
  subcommand does not serve MCP; your client launches `ghostlight-mcp-connector`. Run a real
  subcommand (`install`, `doctor`, `status`), or let the client drive the MCP edge.

## Environment variables

For most installs you set none of these. When you need them:

- `GHOSTLIGHT_DEBUG=1`: observability on (same as `--debug`).
- `GHOSTLIGHT_MANIFEST=file://...`: point the server at a policy manifest (see
  [governance-configuration.md](governance-configuration.md)).
- `GHOSTLIGHT_AUDIT_DIR`, `GHOSTLIGHT_LOG_DIR`: relocate the audit and log directories.

Named instances and endpoint overrides exist for the test harness only. They are deliberately not
a user or development workflow; [DEV-LOOP.md](../DEV-LOOP.md) explains the one-stack model.
