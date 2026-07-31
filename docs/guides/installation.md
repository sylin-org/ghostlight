# Installing Ghostlight

Ghostlight is a native Rust service, a small relay, and a thin browser extension. Installation wires
three things together: your MCP client, the local service, and the extension. This guide covers both
install paths, what the installer actually writes, how to verify the chain, and how to undo it.

If you just want the fast path, the four stages in the
[README](../../README.md#try-it) are the whole story for most people. Come here
when you want a different path, a per-OS detail, or an explanation of what got registered.

```text
[1 Install service] -> [2 Add extension] -> [3 Restart MCP client] -> [4 Ask a first task]
      automatic           visible step             once                useful proof
```

Ghostlight has no hosted account to create or sign in to. The service, relay, and extension
connect locally as the current OS user. Website sessions remain in the Chromium profile you are
already using. Connect only MCP clients you trust: local browser access is powerful even when a
policy constrains it.

## Prerequisites

- A Chromium browser: Chrome, Edge, Brave, or Chromium, version 116 or newer. The 116 floor comes
  from the extension, which Chrome enforces when it loads; the binary itself checks no version.
- An MCP client (Codex, Claude Code, Claude Desktop, Cursor, VS Code, Windsurf, Zed, OpenCode,
  Crush, or another stdio MCP client).
- For the npm path, Node.js supplies the `npx` launcher; the running Ghostlight service is native
  Rust, not a Node service. For the source path, use a stable Rust toolchain (https://rustup.rs).

## Path A: the npm launcher

The launcher fetches the version-matched service and relay on first run and caches them. Nothing to
compile.

1. **Install and register Ghostlight** (idempotent, safe to re-run):

       npx -y ghostlight install

   The installer registers the browser side and every detected supported MCP client. Use
   `--client codex` to target Codex only, `--dry-run` to inspect the plan, or `--no-open` for a
   quiet installation. A first install opens the extension walkthrough.

2. **Add the extension.** Install
   [Ghostlight in Browser](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl)
   from the Chrome Web Store. Chrome shows Ghostlight's blue mascot when it is ready.

3. **Restart your MCP clients,** then try this read-only proof before asking it to act:

       In my current browser, summarize the active page and tell me which tab you used.
       Do not click or change anything.

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

The build produces two executables. `ghostlight` is the CLI and the persistent service.
`ghostlight-relay` is the thin pass-through your MCP client and Chrome actually launch; it depends
on almost nothing, so rebuilding the service never forces it to relink. To test the source tree
immediately, open `chrome://extensions`, enable Developer mode, choose `Load unpacked`, and select
the local `extension/` directory. Then register:

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

The client entry it writes points at `ghostlight-relay` with `--role agent`. You never launch the
binary by hand; the client and the browser do.

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
idempotent merge, so a foreign config is left alone), the per-instance relay copy, and the
supervisor. `--dry-run` shows the plan first.

## Troubleshooting

- **Start with `doctor`.** It pinpoints the common failures by name.
- **Store extension shows disconnected?** Confirm that it is enabled in the browser's extension
  manager, then restart the browser if needed.
- **Source-development extension shows disconnected?** Reload it at `chrome://extensions`. A
  service worker can be evicted; reloading re-establishes the link.
- **Developing on Windows?** Use the isolated engine swap in
  [DEV-LOOP.md](../DEV-LOOP.md). It builds away from locked release executables, swaps only the
  service holding the one endpoint, and lets the stable relays reconnect automatically.
- **Ran `ghostlight` and got an error exit?** That is expected. A bare `ghostlight` with no
  subcommand no longer serves anything; the MCP role lives in `ghostlight-relay`, which your client
  launches. Run a real subcommand (`install`, `doctor`, `status`), or let the client drive the
  relay.

## Environment variables

For most installs you set none of these. When you need them:

- `GHOSTLIGHT_DEBUG=1`: observability on (same as `--debug`).
- `GHOSTLIGHT_MANIFEST=file://...`: point the server at a policy manifest (see
  [governance-configuration.md](governance-configuration.md)).
- `GHOSTLIGHT_AUDIT_DIR`, `GHOSTLIGHT_LOG_DIR`: relocate the audit and log directories.

Named instances and endpoint overrides exist for the test harness only. They are deliberately not
a user or development workflow; [DEV-LOOP.md](../DEV-LOOP.md) explains the one-stack model.
