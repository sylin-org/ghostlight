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
| Zed | `%APPDATA%\Zed\settings.json` on Windows; `~/.config/zed/settings.json` on Linux | Zed config dir or `zed` on PATH | `context_servers`, JSONC |
| OpenCode | global `~/.config/opencode/opencode.json` (XDG on all OSes -- PIN Windows) | `opencode` on PATH or `~/.config/opencode/` dir | `mcp` (type local, command array), JSONC |
| Crush | `$HOME/.config/crush/crush.json` (global; also project `.crush.json`/`crush.json`) | `crush` on PATH or `~/.config/crush/` dir | `mcp` (type stdio), format PIN |

Path nuances to PIN AT IMPLEMENTATION:
- **Zed casing is not uniform**: the directory is `Zed` on Windows but `zed` (lowercase) on
  Linux. The `config_path` arm must branch on OS, unlike VS Code's uniform `Code`.
- **OpenCode / Crush use `~/.config/` literally on every OS** (XDG-style), NOT the OS-native config
  base. Resolve these two from the home directory, not `ctx.config`. Re-verify OpenCode's Windows
  location specifically.

### D5. Sequencing and non-decisions

- **Ship Windsurf first** (D1) -- zero merge risk, large audience, reuses everything. Then land
  Zed + OpenCode + Crush together behind the D2 JSONC handling and D3 dialects.
- **Out of scope:** JetBrains IDEs (already reachable via the Claude Code plugin), Visual Studio,
  Antigravity (MCP unverified), Gemini
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

## Amendment (2026-08-15): the Zed `"source"` pin is resolved -- no field, no code change

Status: Accepted. Resolves D3's Zed pin. Every other decision in this ADR stands as written.

D3 shipped with the pin open and an example (`"source": "custom", "command": ..., "args": ...,
"env": {}`) that recorded the uncertainty rather than an answer. The shipped installer code
(`crates/orchestrator/src/install/mod.rs`, Zed's `expected_json_entry`) never actually added
`source`, registering the same `{command, args, env}` shape used for the other JSON-dialect
clients. That gap sat unresolved through 1.0: the code disagreed with the ADR's own example, and
the pin was never closed out in either direction.

Re-verified 2026-08-15 against Zed's actual current source, not a vendor docs page or a
third-party guide: `crates/settings_content/src/project.rs` in `zed-industries/zed` (main branch).
The stdio context-server variant is:

```rust
Stdio {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    remote: bool,
    #[serde(flatten)]
    command: ContextServerCommand,
},
```

with `ContextServerCommand` carrying `command` (as `path`), `args`, `env`, and `timeout`. There is
no `source` field anywhere in either type. Zed's own published example
(`zed.dev/docs/ai/mcp`) matches this exactly: `{"command": ..., "args": [...], "env": {}}`, no
`source`. Several third-party "how to configure MCP in Zed" guides found during the 2026-08-15
search claim `"source": "custom"` is required; none of them is Zed's own documentation or source,
and none could be corroborated against either. They are wrong, and are the likely origin of the
pin's uncertainty in the first place.

The shipped code was already correct. No code changes. This amendment exists to close the pin with
a sourced answer instead of leaving it re-litigable, and to flag the specific example at D3 as
superseded by this verification: read `{command, args, env}` there as the accurate shape, and the
`"source": "custom"` in that example as the exact uncertainty this amendment resolves, preserved
rather than edited out.

## Amendment (2026-08-15): VS Code's `--add-mcp` and our direct merge are the same mechanism

Status: Accepted. Confirms the pre-existing VS Code installer entry needs no change. Every other
decision in this ADR stands as written.

Line 30-31 mentions VS Code's `code --add-mcp` CLI in passing, as prior art for how an editor can
sidestep the plain-JSON-merge-versus-hand-edited-comments problem the new targets in this ADR had
to solve directly. That reference raised an unresolved question of its own, carried into this
pass's review rather than pinned in the original ADR text: Ghostlight's own VS Code installer
entry (`crates/orchestrator/src/install/mod.rs`, `definitions()` id `"vscode"`) does not shell out
to `code --add-mcp` at all -- it edits `%APPDATA%/Code/User/mcp.json` (or the XDG-style equivalent
off `roaming`) directly, via the same plain-JSON `edit_json` path used for every other
`ConfigDialect::Json` target, writing `{"type":"stdio","command":command,"args":[]}` under a
`"servers"` key (`JsonDialect::Servers`). The open question was whether that divergence from
`--add-mcp` reflects a stale or incorrect understanding of VS Code's actual mechanism.

Re-verified 2026-08-15 against VS Code's own current documentation (`code.visualstudio.com/docs/
copilot/customization/mcp-servers` and `code.visualstudio.com/docs/agents/reference/
mcp-configuration`), not a third-party guide. Three facts confirmed directly from that source:

- The user-level MCP file is `mcp.json` in the user profile, with a top-level `"servers"` object
  mapping server names to configurations -- exactly the key and shape Ghostlight already writes.
- A stdio server entry's documented fields are `type` (required, `"stdio"`), `command` (required),
  `args` (optional), and `env` (optional) -- a superset of, and consistent with, the
  `{type, command, args}` shape Ghostlight's `expected_json_entry` produces for this dialect.
- `code --add-mcp '{"name":...,"command":...,"args":[...]}'` is real and current. Per VS Code's own
  docs, it is a convenience CLI that "writes the server configuration to your user profile's
  `mcp.json` file" -- the same file, same key, same entry shape Ghostlight's installer already
  edits directly. It is not a separate registration path with different effects; it is an
  alternate way to produce the same edit.

Editing `mcp.json` directly was therefore always a deliberate, correct simplification, not a stale
assumption: it reaches the identical end state as invoking `--add-mcp` would, without spawning a
`code` process or depending on it being on `PATH`, which the installer cannot assume (VS Code is
one of several optional executables `HarnessContext` probes for, not a hard dependency). No code
change.

One question this verification could not close, and which this amendment does not attempt to
settle without evidence: whether `mcp.json` itself tolerates JSONC (comments, trailing commas) the
way `settings.json` does. VS Code's own reference pages do not state this either way. Ghostlight's
VS Code entry currently uses the plain-JSON `ConfigDialect::Json`, the same as Claude
Desktop/Cursor/Windsurf, not the JSONC-safe `toml_edit`-style or `jsonc_parser`-based merge this
ADR added for Zed/OpenCode/Crush. If `mcp.json` does turn out to support comments, a hand-edited
one could be affected the same way the newer targets' configs could have been before this ADR's
JSONC-safe merge work. PIN AT IMPLEMENTATION: confirm whether `mcp.json` is JSONC before assuming
either answer; this amendment only closes the `--add-mcp` question, not this one.

## Amendment (2026-08-15): the OpenCode v1/v2 split is real, and the shipped heuristic is correct

Status: Accepted. Confirms the pre-existing OpenCode installer entry needs no change. Every other
decision in this ADR stands as written.

The shipped `opencode_dialect()` (`crates/orchestrator/src/install/mod.rs`) picks between two
JSON shapes for the OpenCode entry -- `JsonDialect::OpenCodeV1` (flat `mcp.<name>`, entry
`{type, command, enabled}`) and `JsonDialect::OpenCodeV2` (nested `mcp.servers.<name>`, entry
`{type, command}`, no `enabled` field) -- using, among other signals, whether an `opencode2`
executable is on `PATH` while `opencode` is not. Read cold, `opencode2` looks like a guessed
future-binary name rather than something verifiable, and the V1/V2 split itself looked like it
could be speculative. This pass's review carried that as an open question rather than an
established fact: was there ever a real second schema and a real second binary, or was this
written ahead of any evidence for either?

Re-verified 2026-08-15 against OpenCode's own current documentation (`opencode.ai/docs/
mcp-servers` for the existing product, `opencode.ai/v2/docs` and `opencode.ai/v2/docs/
mcp-servers` and `opencode.ai/v2/docs/config` for the newer one), not a third-party guide. Four
facts confirmed directly from that source:

- OpenCode 2 is a real, currently shipping product that "installs and runs as `opencode2`" and
  explicitly "does not replace OpenCode 1's `opencode` binary, so you can keep both versions
  installed and run them side by side" -- the exact co-installed-binary situation the executable
  heuristic assumes.
- OpenCode 1's documented local-server example is `mcp.<name> = {"type":"local","command":[...],
  "enabled":true,...}`, flat under `mcp` -- exactly `JsonDialect::OpenCodeV1`'s shape, `enabled`
  included.
- OpenCode 2's documented example nests server entries under `mcp.servers.<name>` and its docs
  say in as many words that "V2 does not place server names directly under `mcp`" and that there
  is "no V2 `enabled` field" (use `disabled`, defaulting to enabled) -- exactly
  `JsonDialect::OpenCodeV2`'s shape, `enabled` correctly absent.
- OpenCode 2's global config is read from the same path as OpenCode 1: `~/.config/opencode/
  opencode.json(c)`. The two dialects target one shared file with two different internal shapes,
  which is exactly how this ADR's single `"opencode"` `HarnessDefinition` (one path, dialect
  chosen per-write) already treats it.

The shipped code was already correct on every axis checked: the split is real, both schemas are
exactly right, the detection heuristic is grounded in a real, still-current fact about how the two
binaries coexist, and the shared file path assumption holds. No code changes.
