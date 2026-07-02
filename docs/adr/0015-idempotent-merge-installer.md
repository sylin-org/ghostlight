# 0015. Self-registering installer via idempotent value-level JSON merge

- Status: Accepted
- Date: 2026-07

## Context

Installation friction is a first-class hazard harvested from prior art (npx and
Windows install pain). Getting the tool running requires two registrations that
users otherwise do by hand: a Chromium native-messaging host manifest (so the
browser can launch the binary) and an MCP server entry in each coding client
(Claude Code, Claude Desktop, Cursor, VS Code). These client configs are shared,
live files (`~/.claude.json` may be actively written by a running Claude Code),
so a naive rewrite risks clobbering sibling servers, unrelated keys, or a
concurrent write. Shelling out to each client's CLI is brittle and not uniformly
available.

## Decision

The binary self-registers via `install`, reverses via `uninstall`, and reports
detection plus registration state via `doctor` (commit 2ae81de; `src/install/`).

Native-messaging host registration is per-browser (Chrome/Edge/Brave/Chromium,
multi-signal detection): on Windows, one shared manifest file plus a per-browser
registry key (HKCU for --user, both WOW6432 HKLM views for --system); on
macOS/Linux, a per-browser file drop. Removal is ownership-checked: only a
manifest whose name is ours is deleted; a foreign manifest at the same path is
reported as a manual skip, never removed.

MCP client registration uses an idempotent, value-level JSON merge (not a
client CLI). The merge re-reads the config at apply time so a concurrent write
is not lost, preserves sibling servers and key order (`serde_json`
`preserve_order`), and errors rather than clobbers when the root or the servers
key is not an object (`src/install/merge.rs`). It is dialect-aware
(`mcpServers` vs `servers`). VS Code's JSONC config is the sole exception,
driven through `code --add-mcp` (or an exact manual command when the CLI is
absent). Writes are atomic with backup-before-rewrite; --dry-run writes nothing;
per-target failures are independent, so one malformed config blocks only its own
target and manual-only runs exit 0.

## Consequences

- Positive: one binary is also its own installer (no separate package, no
  Node.js, deterministic and safe to re-run).
- Positive: concurrent-write safety and preserve-order merging protect a live
  `~/.claude.json`; ownership-checked removal never deletes a stranger's file.
- Negative: each client's config dialect must be tracked in-tree, and VS Code
  still depends on its CLI for correct JSONC editing.
- Follow-up: `doctor` surfaces detection and registration drift so a broken
  install is diagnosable without manual file inspection.
