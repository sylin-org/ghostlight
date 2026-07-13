# LEDGER: additional installer targets (ADR-0071)

Durable progress. One task = one commit. Update RESUME HERE and add a log entry after each task.

## RESUME HERE

- Next task: **T4 (OpenCode)**. T2 is complete, so every remaining task has its merge substrate. All
  remaining tasks are authored and ready; oracles are in `PINS.md`.
- Two RESIDUAL confirms live inside the tasks (not blockers): OpenCode's Windows config path (T4),
  and whether Zed needs `"source": "custom"` (T3). Confirm at execution; the pinned defaults follow
  current vendor docs.

## Task sequence

`T1, T2, T3, T4, T5` -- every prefix leaves a green tree. T1/T2 independent; T3-T5 depend on T2.

## Task log

| Task | Commit | Status | Notes |
|------|--------|--------|-------|
| T1 Windsurf | d4ad8ab | DONE | clients.rs; reuses `Dialect::McpServers` |
| T2 merge foundation | e219d60 | DONE | merge.rs 3 dialects + mod.rs JSONC->Manual + clients.rs tolerant detect |
| T3 Zed | (this commit) | DONE | `context_servers`; per-OS dir casing; no `source` per current official docs |
| T4 OpenCode | (pending) | READY (needs T2) | `mcp` type:local command-array; RESIDUAL: Windows path |
| T5 Crush | (pending) | READY (needs T2) | `mcp` type:stdio |

## Deviations

1. T3 runtime confirmation: Zed is not installed on the execution machine. Current official Zed
   documentation was re-checked on 2026-07-13 and shows local entries with `command`, `args`, and
   `env`, without `source`. The pinned no-`source` shape is unchanged.

## Research resolution (was: open pins)

Resolved 2026-07-13 (see `PINS.md` for the pinned shapes):
1. **Zed** -- entry shape == `mcpServers` (NO `source` field, command string), under key
   `context_servers`. Settings.json is JSONC; dir casing is per-OS (`Zed` mac/win, `zed` linux).
   RESIDUAL: re-confirm the no-`source` fact against a running Zed (T3).
2. **OpenCode** -- key `mcp`, entry `{type:"local", command:[cmd,...args], enabled:true}`, env under
   `environment`; JSONC. RESIDUAL: Windows config path (T4).
3. **Crush** -- key `mcp`, entry `{type:"stdio", command, args, env}`; JSON-vs-JSONC is moot because
   T2's JSONC-safe fallback handles both.
4. **merge.rs** -- the JSONC-safe path is NOT new machinery: it is routing a JSON `MergeError::Parse`
   to `Op::Manual` (already exists) + a substring detection fallback (VS Code already does this).
   Three dialects added to `to_value`/`top_key`. Full spec + oracles in `PINS.md` P2-P4.
