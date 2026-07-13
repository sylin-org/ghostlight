# LEDGER: additional installer targets (ADR-0071)

Durable progress. One task = one commit. Update RESUME HERE and add a log entry after each task.

## RESUME HERE

- Next task: **T1 (Windsurf)** or **T2 (merge foundation)** -- mutually independent, do either first.
  T3-T5 require T2. All five tasks are authored and ready; oracles are in `PINS.md`.
- Two RESIDUAL confirms live inside the tasks (not blockers): OpenCode's Windows config path (T4),
  and whether Zed needs `"source": "custom"` (T3). Confirm at execution; the pinned defaults follow
  current vendor docs.

## Task sequence

`T1, T2, T3, T4, T5` -- every prefix leaves a green tree. T1/T2 independent; T3-T5 depend on T2.

## Task log

| Task | Commit | Status | Notes |
|------|--------|--------|-------|
| T1 Windsurf | (pending) | READY | clients.rs only; reuses `Dialect::McpServers` |
| T2 merge foundation | (pending) | READY | merge.rs 3 dialects + mod.rs JSONC->Manual + clients.rs tolerant detect |
| T3 Zed | (pending) | READY (needs T2) | `context_servers`; per-OS dir casing; RESIDUAL: source field |
| T4 OpenCode | (pending) | READY (needs T2) | `mcp` type:local command-array; RESIDUAL: Windows path |
| T5 Crush | (pending) | READY (needs T2) | `mcp` type:stdio |

## Deviations

(record any numbered deviation from a task file here, with the reason, as it happens)

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
