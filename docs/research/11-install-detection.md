# Install-Flow Auto-Detection

**Date:** 2026-07-01 · **Track:** Delight (developer) / Fork 4 · **Source:** research agent (verbatim report)

> What a `browser-mcp install` command must discover and wire on Windows / macOS / Linux to
> register (A) a native-messaging host for Chromium browsers and (B) an MCP stdio server in AI
> coding clients. This is the actionable spec for Fork 4's self-registering installer. Sources
> cited inline.

---

## A. Chromium browsers: detection + native-messaging host registration

### A.0 The invariants (read first)

**Native-messaging manifest JSON schema** (identical across all Chromium browsers),
[Chrome docs](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging):
```json
{
  "name": "org.sylin.browser_mcp",
  "description": "Browser MCP native host",
  "path": "/absolute/path/to/browser-mcp",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://<EXTENSION_ID>/"]
}
```
- `name`: lowercase alphanumeric + `_` + `.`; no leading/trailing dot, no `..`.
- `path`: must be **absolute** on macOS/Linux; on Windows may be relative to the manifest dir.
- `type`: always `stdio`.
- `allowed_origins`: **no wildcards**, must enumerate exact `chrome-extension://<id>/` origins.
  This forces the installer to know the extension ID (see A.6).

**Two registration models, split by OS:**
- **macOS / Linux** -> drop a `<host_name>.json` file into a **per-browser directory**.
- **Windows** -> the manifest can live anywhere; a **registry key** whose `(Default)` value is the
  absolute path to the `.json` is what Chrome reads. Registry is authoritative; file location is
  arbitrary. Confirmed by [claude-code #24367](https://github.com/anthropics/claude-code/issues/24367),
  [codex #24040](https://github.com/openai/codex/issues/24040).

> A third-party repo describes Windows as `%LOCALAPPDATA%\<Browser>\User Data\NativeMessagingHosts\`.
> That is NOT how stock Chromium reads hosts on Windows. Chromium on Windows uses the registry
> lookup only.

**Windows registry lookup:** 32-bit view first, then 64-bit; hosts `HKLM` then `HKCU` (Edge:
`HKCU` first). Practically, `HKCU` needs no admin. Prefer it.

**Claude in Chrome host names** (both exist): `com.anthropic.claude_browser_extension` (Desktop),
`com.anthropic.claude_code_browser_extension` (Code CLI). **We pick our own** (`org.sylin.browser_mcp`)
to dodge the Desktop/Code collision (see B.7).

### A.1 Windows: detection + registry paths

**Detect installed** (check any): App Paths `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{chrome,msedge,brave,vivaldi,opera}.exe`;
Uninstall keys `HK{LM,CU}\SOFTWARE\[WOW6432Node\]Microsoft\Windows\CurrentVersion\Uninstall\*` (match `DisplayName`);
`HKLM\SOFTWARE\Clients\StartMenuInternet\*`; fallback = User Data dir under `%LOCALAPPDATA%`/`%APPDATA%`.

**Native-host registry key** (write `(Default)` = absolute manifest path), append `\<host_name>`:

| Browser | Registry key |
|---|---|
| Chrome | `HK{CU,LM}\SOFTWARE\Google\Chrome\NativeMessagingHosts\` |
| Edge | `HK{CU,LM}\SOFTWARE\Microsoft\Edge\NativeMessagingHosts\` |
| Brave | `HK{CU,LM}\SOFTWARE\BraveSoftware\Brave-Browser\NativeMessagingHosts\` |
| Vivaldi | `HK{CU,LM}\SOFTWARE\Vivaldi\NativeMessagingHosts\` (also falls back to Chrome's key) |
| Opera | `HK{CU,LM}\SOFTWARE\Google\Chrome\NativeMessagingHosts\` (Opera reads Chrome's namespace on Windows) |
| Chromium | `HK{CU,LM}\SOFTWARE\Chromium\NativeMessagingHosts\` |
| Arc | Chromium-based; verify its own key at runtime, else Chrome namespace |

Rust write (no admin for HKCU): create `HKCU\SOFTWARE\<vendor>\NativeMessagingHosts\<host_name>`, set
default value to the manifest path. Sources:
[#24367](https://github.com/anthropics/claude-code/issues/24367),
[keepassxc-browser #48](https://github.com/keepassxreboot/keepassxc-browser/issues/48) (Vivaldi->Chrome fallback),
[Edge native messaging](https://learn.microsoft.com/en-us/microsoft-edge/extensions/developer-guide/native-messaging).

### A.2 macOS: detection + host directories

**Detect** via `.app` bundle (+ `CFBundleIdentifier`):

| Browser | App path | Bundle ID |
|---|---|---|
| Chrome | `/Applications/Google Chrome.app` | `com.google.Chrome` |
| Edge | `/Applications/Microsoft Edge.app` | `com.microsoft.edgemac` |
| Brave | `/Applications/Brave Browser.app` | `com.brave.Browser` |
| Vivaldi | `/Applications/Vivaldi.app` | `com.vivaldi.Vivaldi` |
| Opera | `/Applications/Opera.app` | `com.operasoftware.Opera` |
| Arc | `/Applications/Arc.app` | `company.thebrowser.Browser` |
| Chromium | `/Applications/Chromium.app` | `org.chromium.Chromium` |

**Native-host dir** (drop `<host_name>.json`; user-level, no admin) under `~/Library/Application Support/`:
Chrome `Google/Chrome/NativeMessagingHosts/`; Edge `Microsoft Edge/NativeMessagingHosts/`; Brave
`BraveSoftware/Brave-Browser/NativeMessagingHosts/`; Vivaldi `Vivaldi/NativeMessagingHosts/`; Opera
`com.operasoftware.Opera/NativeMessagingHosts/`; Arc `Arc/User Data/NativeMessagingHosts/` (verify);
Chromium `Chromium/NativeMessagingHosts/`. System-wide (needs admin): `/Library/Google/Chrome/NativeMessagingHosts/`, etc.
Also check `~/Applications/`. Sources: [Chrome docs](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging),
[claude-code #20887](https://github.com/anthropics/claude-code/issues/20887).

### A.3 Linux: detection + host directories

**Detect** via binary on PATH (`google-chrome[-stable]`, `microsoft-edge[-stable]`, `brave-browser`,
`vivaldi[-stable]`, `opera`, `chromium[-browser]`), `.desktop` files, and config-dir presence.

**Native-host dir** (note casing: user dirs = CamelCase `NativeMessagingHosts`; `/etc` system dirs =
kebab `native-messaging-hosts`):

| Browser | User dir | System dir |
|---|---|---|
| Chrome | `~/.config/google-chrome/NativeMessagingHosts/` | `/etc/opt/chrome/native-messaging-hosts/` |
| Edge | `~/.config/microsoft-edge/NativeMessagingHosts/` | `/etc/opt/edge/native-messaging-hosts/` |
| Brave | `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/` | `/etc/opt/chrome/native-messaging-hosts/` |
| Vivaldi | `~/.config/vivaldi/NativeMessagingHosts/` | `/etc/vivaldi/native-messaging-hosts/` |
| Opera | `~/.config/opera/NativeMessagingHosts/` (also reads Chrome's + `/etc/chromium/`) | `/etc/chromium/native-messaging-hosts/` |
| Chromium | `~/.config/chromium/NativeMessagingHosts/` | `/etc/chromium/native-messaging-hosts/` |

Sources: [Chrome docs](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging),
[claude-code #14391](https://github.com/anthropics/claude-code/issues/14391) (exact Linux dirs +
installer-only-writes-Chrome bug), [vdhcoapp PR #110](https://github.com/aclap-dev/vdhcoapp/pull/110/files).

### A.4 User-data / profile directories

For (a) confirming real use, (b) enumerating profiles, (c) dev-time scraping of a loaded unpacked
extension's ID from `<UserData>/<Profile>/Preferences` (`extensions.settings`) or `Secure Preferences`.
Chrome roots: Windows `%LOCALAPPDATA%\Google\Chrome\User Data\`; macOS `~/Library/Application Support/Google/Chrome/`;
Linux `~/.config/google-chrome/`. Real-profile markers: `Local State`, `Default/Preferences`.

### A.5 Per-browser gotchas

- **`allowed_origins` has no wildcards**: pin exact extension ID (drives the `--extension-id` fallback).
- **Linux dir casing:** `NativeMessagingHosts` (user) vs `native-messaging-hosts` (`/etc`). Wrong = silent "host not found."
- **Opera/Vivaldi piggyback on Chrome's namespace** (Windows registry; sometimes dirs). Write their own path too.
- **Windows off-store force-install** requires AD/Entra/CBCM enrollment; unmanaged consumer Windows cannot silently force-install an off-store extension. ([Chrome Enterprise](https://chromeenterprise.google/policies/extension-install-forcelist/))
- **Windows registry `(Default)` value is what's read**, not a conventional file path.

### A.6 The `key` field -> deterministic extension ID (solves `allowed_origins`)

Chrome derives the extension ID from the manifest `key`: base64-decode `key` -> SHA-256 -> first 32
hex chars -> map `0-9a-f` to `a-p`. Ship a fixed `key` and the ID is **deterministic and identical**
across every install/browser, so the installer hardcodes `allowed_origins` at build time and never
prompts. Without `key`, an unpacked dev extension gets a path-derived, per-machine ID (forcing the
`--extension-id` fallback). Sources:
[Chrome `key` docs](https://developer.chrome.com/docs/extensions/reference/manifest/key),
[Plasmo consistent-ID guide](https://www.plasmo.com/blog/posts/how-to-create-a-consistent-id-for-your-chrome-extension).

**Recommendation:** generate a keypair, embed the public key as `key`, compute the ID at build time,
bake `allowed_origins` as a constant. Fully non-interactive install for the common case.

---

## B. MCP clients: detection + add-stdio-server mechanism

### B.0 The stdio server JSON shape (three dialects)

Most use `mcpServers` with `{command, args, env}`. Exceptions: **VS Code** uses `servers` (+ `type`),
**Zed** uses `context_servers` (+ `source: "custom"`). Canonical:
`{ "command": "browser-mcp", "args": ["--manifest", "..."], "env": {} }`.

### B.1 Claude Code (CLI)

- **Detect:** `claude` on PATH; `~/.claude.json` exists.
- **Config/scopes:** `local` (default) = `~/.claude.json` under `projects.<path>.mcpServers`;
  `user` = `~/.claude.json` global; `project` = `.mcp.json` at repo root.
- **CLI (preferred):** `claude mcp add --scope user browser-mcp -- browser-mcp --manifest file://...`
  (flags before name; everything after `--` passed untouched). Also `claude mcp add-json`.
- Precedence: local > project > user > plugin > connector.

### B.2 Claude Desktop

- **Config file:** macOS `~/Library/Application Support/Claude/claude_desktop_config.json`;
  Windows `%APPDATA%\Claude\claude_desktop_config.json`; Linux `~/.config/Claude/claude_desktop_config.json`.
- **Schema:** `{ "mcpServers": { "<name>": {command, args, env} } }`. No CLI: merge JSON, user restarts.

### B.3 Cursor

- **Config:** global `~/.cursor/mcp.json`; project `.cursor/mcp.json`. Schema: `mcpServers`.
- **One-click:** `cursor://anysphere.cursor-deeplink/mcp/install?name=...&config=<base64>`; else write JSON.

### B.4 Windsurf

- **Config:** `~/.codeium/windsurf/mcp_config.json`. Schema: `mcpServers`. No CLI: write JSON.

### B.5 VS Code (Copilot agent) + Continue / Cline

- **Config:** workspace `.vscode/mcp.json`; user `mcp.json` (Win `%APPDATA%\Code\User\mcp.json`;
  macOS `~/Library/Application Support/Code/User/mcp.json`; Linux `~/.config/Code/User/mcp.json`).
  **Top-level key is `servers`** (+ optional `inputs`), entries carry `"type": "stdio"`.
- **CLI (preferred):** `code --add-mcp '{"name":"browser-mcp","command":"browser-mcp","args":[...]}'`;
  deeplink `vscode:mcp/install?<url-encoded-json>`.
- **Continue:** `~/.continue/config.yaml` (`mcpServers`). **Cline:** `cline_mcp_settings.json` in the
  extension's global-storage (`mcpServers`).

### B.6 Zed

- **Config:** `~/.config/zed/settings.json` (Win `%APPDATA%\Zed\settings.json`); project `.zed/settings.json`.
- **Key is `context_servers`**, each needs `"source": "custom"`.

### B.7 MCP-client gotchas

- **npx-on-Windows stdio bug:** stdio servers via `npx`/`npx.cmd` frequently fail; the `cmd /c`
  workaround is itself mangled by `claude mcp add` on Windows (`/c` -> `C:/`). **A native binary with
  `command` = absolute exe path sidesteps this entirely.** ([claude-code #20061](https://github.com/anthropics/claude-code/issues/20061), [playwright-mcp #1540](https://github.com/microsoft/playwright-mcp/issues/1540))
- **Per-project vs global:** default Claude Code scope is `local`; pass `--scope user` for a browser tool.
- **Host-name collision (Desktop + Code):** with both installed, `connectNative` is pinned to Desktop's
  host, so only one works. **We use a unique host name + unique extension ID** to never collide.
  ([#20887](https://github.com/anthropics/claude-code/issues/20887), [#21426](https://github.com/anthropics/claude-code/issues/21426))

---

## C. Synthesis for the installer

### C.1 Detection matrix (browsers): see A.1-A.3 tables (installed? check -> native-host write target).

### C.2 Detection matrix (MCP clients)

| Client | installed? | config path | add mechanism | key |
|---|---|---|---|---|
| Claude Code | `claude` on PATH / `~/.claude.json` | `~/.claude.json` / `.mcp.json` | CLI `claude mcp add --scope user` | `mcpServers` |
| Claude Desktop | config file exists | per-OS `claude_desktop_config.json` | edit JSON + restart | `mcpServers` |
| Cursor | `~/.cursor/` | `~/.cursor/mcp.json` / `.cursor/mcp.json` | JSON / `cursor://...` deeplink | `mcpServers` |
| Windsurf | `~/.codeium/windsurf/` | `mcp_config.json` | edit JSON | `mcpServers` |
| VS Code | `code` on PATH / User dir | `.vscode/mcp.json` / user `mcp.json` | CLI `code --add-mcp` | `servers` |
| Zed | `~/.config/zed/` | `settings.json` / `.zed/settings.json` | edit JSON | `context_servers` (`source:"custom"`) |
| Continue | `~/.continue/` | `config.yaml` | edit config | `mcpServers` |
| Cline | VS Code ext storage | `cline_mcp_settings.json` | edit JSON | `mcpServers` |

### C.3 Minimum-viable `browser-mcp install` UX

**Auto-detect (no prompt):** OS+arch; own path (`std::env::current_exe`); installed browsers (C.1);
installed clients (C.2); extension ID pre-pinned via manifest `key` (A.6).

**One-click actions (idempotent; back up before editing existing JSON):** per browser, write
`<host>.json` (mac/linux) or registry key (win) with our unique host name + pinned `allowed_origins`;
per client, prefer CLI (`claude mcp add`, `code --add-mcp`) else merge JSON under the correct top-level
key, `command` = absolute exe path (no npx). Print a summary; offer `--dry-run`.

**Must ask / decide:** scope (user vs project; default user); which browsers/clients (default = all
detected); HKCU vs HKLM / user-dir vs system-dir (default user-level; `--system` for enterprise).

**Fallbacks:** `--extension-id <id>` (only for keyless dev unpacked loads); `--browser`/`--manifest-dir`
(portable/custom profiles, snapshot/dev channels); `--client`/explicit path; Windows enterprise deploy
needs AD/Entra/CBCM + `ExtensionInstallForcelist` (installer can only emit the policy snippet); manual-
instructions escape hatch printing exact path/key + content on any write failure.

### C.4 Key implementation constraints (do-not-forget list)

1. **Pin the extension ID at build time** via manifest `key` -> hardcode `allowed_origins` (no wildcards).
2. **Use a unique host name** (`org.sylin.browser_mcp`) to dodge the Anthropic Desktop/Code collision.
3. **Windows = registry**, not a file path; **macOS/Linux = drop file** in the per-browser dir.
4. **Linux casing:** `NativeMessagingHosts` (user) vs `native-messaging-hosts` (`/etc`).
5. **`command` = absolute native-binary path**; never npx -> sidesteps the Windows cmd/c stdio bug.
6. Prefer client CLIs (`claude mcp add`, `code --add-mcp`); JSON-merge (never overwrite) otherwise.

---

## Sources

Chrome native messaging: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging ·
`key` manifest: https://developer.chrome.com/docs/extensions/reference/manifest/key ·
Edge: https://learn.microsoft.com/en-us/microsoft-edge/extensions/developer-guide/native-messaging ·
Chrome Enterprise force-install: https://chromeenterprise.google/policies/extension-install-forcelist/ ·
Deterministic ID: https://www.plasmo.com/blog/posts/how-to-create-a-consistent-id-for-your-chrome-extension ·
claude-code issues #14391 / #24367 / #20887 / #21426 / #21582 / #20341 / #20061 ·
codex #24040 · playwright-mcp #1540 ·
Community installer: https://github.com/stolot0mt0m/claude-chromium-native-messaging ·
MCP clients: Claude Code https://code.claude.com/docs/en/mcp · Claude Desktop https://modelcontextprotocol.io/docs/develop/connect-local-servers ·
VS Code https://code.visualstudio.com/docs/agents/reference/mcp-configuration · Zed https://zed.dev/docs/ai/mcp ·
Bundle IDs: https://macbundleid.lemonproductions.ca/ · Arc: https://resources.arc.net/hc/en-us/articles/22353769256471

---

**Feeds:** Fork 4 (self-registering installer) implementation. **Key takeaways:** bake a fixed
extension `key` (deterministic ID -> compile-time `allowed_origins`); Windows = registry (HKCU, no
admin), macOS/Linux = per-browser dir file drop (mind Linux casing); ship a native binary with an
absolute `command` path in every client config (avoids npx-on-Windows failures); use a unique host
name; prefer client CLIs, fall back to careful JSON merges across three key dialects.
