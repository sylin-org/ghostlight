# ADR-0071: Additional installer targets (Windsurf, Zed, OpenCode, Crush)

Date: 2026-07-13
Status: Accepted
Builds on: ADR-0006 (MCP-client-agnostic server), the installer's client registry
(`crates/core/src/install/clients.rs`, `merge.rs`), and ADR-0067 (Codex as a first-class target,
which added the TOML file-merge path). Coordinates with the in-flight `install/` work: this ADR
adds targets to the SAME module, so it lands only after that work commits, or is handed to the same
executor.

## Context

Ghostlight is MCP-client-agnostic (ADR-0006): any client that speaks stdio MCP already works today
by pasting the stdio entry `{ "command": "ghostlight-relay", "args": ["--role","agent"] }` by hand.
"Installer support" is a convenience layer only -- `ghostlight install` detects the client and
merges our server entry into its config idempotently, never clobbering the file. Five targets exist
today: Claude Code, Claude Desktop, Cursor, VS Code, Codex (`install/clients.rs` `CLIENTS`).

The 2026-07 agentic-coding client-compatibility survey
(`lbotinelly/state-of-agentic-coding`, editions/2026-07) lists more MCP-capable clients with real
user bases. This ADR adds four as installer targets. Adding a target is additive convenience, not
new capability, and touches only the installer.

Two facts, verified against vendor docs on 2026-07-13, shape the design:

- **Config-surface diversity.** Each client uses a different top-level key and entry shape, and two
  of the four combine `command` + `args` differently or require extra fields (`type`, `enabled`).
- **JSONC vs plain JSON.** The installer's JSON merge (`merge.rs`) is a pure `serde_json`
  pretty-print: it reformats the whole file and would STRIP comments. That is safe for a
  machine-managed plain-JSON config but destructive for a JSONC config a human comments. VS Code
  sidesteps this with its `code --add-mcp` CLI; Codex uses a comment-preserving `toml_edit` merge
  (ADR-0067). Windsurf's config is plain JSON; Zed's and OpenCode's are JSONC; Crush's is
  uncertain (its docs show a `$schema` field and JSONC-style examples). Never clobbering a user's
  file is a core Ghostlight promise, so JSONC handling is the central decision here.

## Decision

### D1. Windsurf -- ship first, reuses the existing dialect

Windsurf (now Devin Desktop / Cascade, Cognition) uses the EXISTING `Dialect::McpServers` verbatim:
top key `mcpServers`, entry `{ command, args, env }`, and the file is PLAIN JSON. So it is a new
`ClientId` + config path + detection signal and nothing else -- no merge changes.

- Config path (all OSes, home-relative): `~/.codeium/windsurf/mcp_config.json`.
- Detection: `~/.codeium/windsurf/` exists, or `windsurf` on PATH.
- Registration: `AddVia::JsonFileMerge(Dialect::McpServers)`, the current safe merge.

### D2. JSONC handling -- never destroy comments (the pivotal rule)

For any target whose config is JSONC (Zed, OpenCode, and Crush if confirmed), the pure-JSON merge
must NOT run blindly. The rule: **detect our entry's presence tolerantly, but never rewrite a file
that carries comments.**

- Parse for our entry using a JSONC-tolerant read (strip comments before `serde_json` for the
  detection/no-op check only). This lets `doctor` and the install no-op check work on JSONC.
- Write path: if the on-disk file has NO comments (a fresh or machine-managed file), the existing
  value-level merge is safe -- use it. If the file DOES carry comments, DO NOT reformat it; instead
  print the exact manual entry to add (the same "print steps instead of failing" posture the
  extension-store step uses when a credential is missing). This preserves the never-clobber promise
  with zero new dependencies.
- A later, optional upgrade (its own change, not required here): a comment-preserving JSONC edit
  (surgical insert/update of only our server object). Deferred until demand justifies the dependency.

This keeps Windsurf fully automatic and makes Zed/OpenCode/Crush automatic-when-safe,
manual-instructions-when-a-comment-would-be-lost -- honest either way.

### D3. New dialects for the non-`mcpServers` shapes

Add these to the installer's dialect surface (`merge.rs` `Dialect` + `ServerEntry::to_value`).
Exact shapes, verified 2026-07-13 (our entry is `command = <ghostlight-relay path>`,
`args = ["--role","agent"]`, `name = "ghostlight"`):

- **Zed -- `context_servers`** (JSONC). Entry is `{ command: <string>, args: [...], env: {} }`, the
  same field shape as `mcpServers` but under a different top key. PIN AT IMPLEMENTATION: whether the
  current Zed schema also requires `"source": "custom"` on a custom (non-extension) server -- recent
  Zed versions have used it; the 2026-07 docs example omitted it. Re-verify against the running
  Zed's settings schema before shipping.

  ```json
  "context_servers": { "ghostlight": { "source": "custom", "command": "<relay>", "args": ["--role","agent"], "env": {} } }
  ```

- **OpenCode -- `mcp`** (JSONC). Entry COMBINES command + args into ONE array, and requires
  `type: "local"` and `enabled: true`; the env field is named `environment`.

  ```json
  "mcp": { "ghostlight": { "type": "local", "command": ["<relay>","--role","agent"], "enabled": true } }
  ```

- **Crush -- `mcp`** (format PIN). Entry is `{ type: "stdio", command: <string>, args: [...], env: {} }`.

  ```json
  "mcp": { "ghostlight": { "type": "stdio", "command": "<relay>", "args": ["--role","agent"] } }
  ```

`ServerEntry::to_value` grows a per-dialect arm (command-string vs command-array; extra
`type`/`enabled`/`source` fields), mirroring how it already special-cases VS Code's `type: "stdio"`.

### D4. Config paths and detection (verified 2026-07-13)

| Client | Config path | Detection | Dialect / format |
| --- | --- | --- | --- |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` (all OSes) | `~/.codeium/windsurf/` dir or `windsurf` on PATH | `mcpServers`, plain JSON (existing) |
| Zed | macOS `~/Library/Application Support/Zed/settings.json`; Linux `~/.config/zed/settings.json`; Windows `%APPDATA%\Zed\settings.json` | config dir exists or `zed` on PATH | `context_servers`, JSONC |
| OpenCode | global `~/.config/opencode/opencode.json` (XDG on all OSes -- PIN Windows) | `opencode` on PATH or `~/.config/opencode/` dir | `mcp` (type local, command array), JSONC |
| Crush | `$HOME/.config/crush/crush.json` (global; also project `.crush.json`/`crush.json`) | `crush` on PATH or `~/.config/crush/` dir | `mcp` (type stdio), format PIN |

Path nuances to PIN AT IMPLEMENTATION:
- **Zed casing is not uniform**: the directory is `Zed` on macOS/Windows but `zed` (lowercase) on
  Linux. The `config_path` arm must branch on OS, unlike VS Code's uniform `Code`.
- **OpenCode / Crush use `~/.config/` literally on every OS** (XDG-style), NOT the OS-native config
  base (`%APPDATA%` / `~/Library/Application Support`). Use a home-relative `.config/...` path for
  these two, not `ctx.config`. Re-verify OpenCode's Windows location specifically.

### D5. Sequencing and non-decisions

- **Ship Windsurf first** (D1) -- zero merge risk, large audience, reuses everything. Then land
  Zed + OpenCode + Crush together behind the D2 JSONC handling and D3 dialects.
- Out of scope: Xcode 26.3 (macOS-only, brand new), native JetBrains AI Assistant (MCP unverified;
  already reachable via the Claude Code plugin), Visual Studio, Antigravity (MCP unverified), Gemini
  CLI (retired), Aider (no MCP). Revisit any of these on a named trigger, not now.
- No change to the sacred tool surface, the relay entry we register (`--role agent`), or any client
  already supported.

## Consequences

- `ghostlight install` auto-registers into four more clients; users of those editors skip the
  manual paste. The never-clobber guarantee holds -- JSONC files with comments get printed steps,
  never a reformat.
- The installer gains a JSONC-tolerant detection read and three dialect arms; `doctor` reports the
  new targets accurately (as it does for Codex's TOML today).
- Windsurf can ship immediately; the JSONC trio depends on D2/D3, which is where the real work is.
- The client set stays a small, explicit registry -- no plugin system, no dynamic discovery.

## Provenance

- Owner request, 2026-07-13: review the `lbotinelly/state-of-agentic-coding` 2026-07
  `client-compatibility.csv` for MCP clients Ghostlight could add support for, then "draft an ADR
  ... followed by a deep research step to make sure we capture all the necessary data to implement
  it."
- Research verified against vendor docs on 2026-07-13: Windsurf/Cascade
  (`docs.devin.ai/desktop/cascade/mcp` -- `~/.codeium/windsurf/mcp_config.json`, `mcpServers`, plain
  JSON), Zed (`zed.dev/docs` -- `context_servers`, JSONC, per-OS settings.json paths), OpenCode
  (`opencode.ai/docs/mcp-servers` -- `mcp` key, `type: "local"`, command-array, `environment`,
  JSONC), Crush (`github.com/charmbracelet/crush` README -- `mcp` key, `type: "stdio"`,
  `$HOME/.config/crush/crush.json`). Ghostlight's merge surface read from
  `crates/core/src/install/merge.rs` (`Dialect::{McpServers,Servers}`, pure-JSON pretty-print) and
  `clients.rs` at authoring time. Items marked PIN AT IMPLEMENTATION were not fully resolvable from
  docs and MUST be re-verified against the running client before shipping.
