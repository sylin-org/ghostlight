# ADR-0067: Codex as a first-class installer target

Date: 2026-07-12
Status: Accepted
Amends: ADR-0015 (self-registering installer) for Codex's TOML configuration dialect.

## Context

Ghostlight works with any MCP client, but `ghostlight install` originally knew only four client
configuration dialects: Claude Code, Claude Desktop, Cursor, and VS Code. Codex uses a shared
`~/.codex/config.toml` across its CLI, desktop app, and IDE extension. The generic JSON
`mcpServers` guidance is not valid Codex configuration, and the installer therefore left Codex
unregistered even after correctly installing the browser-side native host and service.

Running `codex mcp add` externally would make the installer depend on a separate executable being
on PATH and would make symmetric uninstall depend on that executable too. A handwritten TOML
serializer would risk reformatting or clobbering the same shared file's model settings, project
trust state, comments, and sibling MCP servers.

## Decision

1. Codex is a first-class client id: `codex`. It is detected by a `codex` executable on PATH or a
   `~/.codex` directory, and its user-scope configuration file is `~/.codex/config.toml` on every
   platform.
2. The installer writes the active instance's ordinary relay entry under
   `[mcp_servers.<instance-name>]`: an absolute `ghostlight-relay` path, `--role agent`, and the
   named-instance argument when relevant. It never registers `ghostlight` itself as an MCP stdio
   server.
3. Codex TOML changes use a lossless editor. Install updates only Ghostlight's owned table;
   uninstall removes only that table. Both operations re-read at apply time, write atomically,
   back up before a real change, and leave a malformed or unreadable file untouched.
4. `doctor` parses the configured client dialect to report registration accurately. It does not
   infer Codex registration from JSON-shaped text.
5. Documentation presents `ghostlight install --client codex` as the complete registration path.
   The browser extension remains a separate, user-visible installation step because the CLI cannot
   load an extension into Chromium.

## Consequences

- A Codex CLI, desktop app, or IDE-extension user can run one Ghostlight installer command and get
  the same native-host, service, and MCP registration experience as the other supported clients.
- Codex configuration remains global by default, matching Codex's own shared-host model; a user
  may still use a project-scoped Codex config deliberately outside Ghostlight's installer.
- The core installer adds one pure-Rust TOML editing dependency and maintains a fifth client
  dialect. Tests pin lossless preservation, idempotence, uninstall symmetry, and doctor reporting.
