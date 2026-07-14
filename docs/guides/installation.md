# Installing Ghostlight

Ghostlight is a native Rust service, a small relay, and a thin browser extension. Installation wires
three things together: your MCP client, the local service, and the extension. This guide covers both
install paths, what the installer actually writes, how to verify the chain, and how to undo it.

If you just want the fast path, the three steps in the
[README](../../README.md#try-it) are the whole story for most people. Come here
when you want a different path, a per-OS detail, or an explanation of what got registered.

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

2. **Add the extension.** Until the Chrome Web Store listing is public, download
   `ghostlight-extension-v*.zip` from the
   [latest release](https://github.com/sylin-org/ghostlight/releases/latest), unzip it, and load it
   unpacked at `chrome://extensions` (Developer mode, then Load unpacked). The walkthrough always
   presents the current path.

3. **Restart your MCP clients,** then try a browser request. Verification is optional:

       npx -y ghostlight doctor

For an MCP client the installer does not recognize, add this stdio entry manually, then run the
same install command for the browser side:

    { "command": "npx", "args": ["-y", "ghostlight"] }

## Path B: build from source

The path when you want to read what you are running.

    git clone https://github.com/sylin-org/ghostlight
    cd ghostlight
    cargo build --release

The build produces two executables. `ghostlight` is the CLI and the persistent service.
`ghostlight-relay` is the thin pass-through your MCP client and Chrome actually launch; it depends
on almost nothing, so rebuilding the service never forces it to relink. Load the extension as in
Path A step 2 (from the local `extension/` directory), then register:

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
  and prints the exact entry as a manual step. Guidance is reported separately from failure.
- **Allow-lists the extension** by id. The Web Store and unpacked-dev ids are always allowed;
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
anything else (Cline and the rest), add the stdio server entry from the Path A example by hand and
it behaves the same. The installer's job is convenience, not gatekeeping.

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
- **Extension shows disconnected?** Reload it at `chrome://extensions`. A service worker can be
  evicted; reloading re-establishes the link.
- **Rebuilding on Windows?** Stop the MCP client first. A running client holds the relay executable
  open, and the build cannot overwrite a locked file. This is the most common "my build failed for
  no reason" on Windows, and it has a one-line cause.
- **Ran `ghostlight` and got an error exit?** That is expected. A bare `ghostlight` with no
  subcommand no longer serves anything; the MCP role lives in `ghostlight-relay`, which your client
  launches. Run a real subcommand (`install`, `doctor`, `status`), or let the client drive the
  relay.

## Environment variables

For most installs you set none of these. When you need them:

- `GHOSTLIGHT_DEBUG=1`: observability on (same as `--debug`).
- `GHOSTLIGHT_MANIFEST=file://...`: point the server at a policy manifest (see
  [governance-configuration.md](governance-configuration.md)).
- `GHOSTLIGHT_INSTANCE=<name>`: select a named, isolated instance (advanced; lets two independent
  setups coexist on one machine).
- `GHOSTLIGHT_AUDIT_DIR`, `GHOSTLIGHT_LOG_DIR`: relocate the audit and log directories.
- `GHOSTLIGHT_ENDPOINT` / `GHOSTLIGHT_ENDPOINTS`: pin the IPC endpoint name(s).
