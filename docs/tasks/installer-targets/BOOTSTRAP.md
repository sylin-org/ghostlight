# BOOTSTRAP: additional installer targets (ADR-0071)

Adds MCP clients to the `ghostlight install` auto-registration set. ADR-0071 is normative; `PINS.md`
holds the resolved research and every code-level oracle. All five tasks are authored and READY:
T1 (Windsurf) is standalone; T2 is the merge-dialect + JSONC-safe foundation; T3-T5 (Zed, OpenCode,
Crush) each add one client and depend on T2. Two RESIDUAL pins (OpenCode's Windows path, Zed's
`source` field) are confirm-at-execution notes inside T3/T4, not blockers.

## Authority order (on conflict, higher wins; an unanticipated conflict = STOP)

1. The live tree (facts). Task files state tree facts AS OF AUTHORING (2026-07-13, dev @ the commit
   that added `docs/adr/0071-additional-installer-targets.md`). ALWAYS re-read the named files
   before editing -- the `install/` module was recently edited by a concurrent workstream.
2. `PINS.md` in this directory (exact code-level shapes and oracles; the frontier author resolved
   these -- transcribe, never derive).
3. `docs/adr/0071-additional-installer-targets.md` (semantics: paths, dialects, shapes, sequencing).
4. The task file being executed.

Do not re-litigate decided questions (ADR-0071 Decision + Provenance). Do not resolve ambiguity by
judgment: STOP per the failure protocol.

## Environment facts

- Windows 11; repo root `f:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp`; branch `dev`.
- Rust workspace; installer lives in `crates/core/src/install/`. Client registry:
  `install/clients.rs` (`ClientId` enum, `CLIENTS` array, `config_path`, `detect`, tests). JSON
  merge: `install/merge.rs` (`Dialect`, `ServerEntry`). T1 touches ONLY `clients.rs`.
- **Build/test in an isolated target dir** (live clients + the service lock `target/*.exe`; a plain
  build can relink-fail with os error 5 and leave a stale binary): prefix cargo with
  `CARGO_TARGET_DIR=target-check`.
- Gates (ALL must pass before the commit):
  1. `CARGO_TARGET_DIR=target-check cargo fmt --check`
  2. `CARGO_TARGET_DIR=target-check cargo clippy -p ghostlight-core --all-targets -- -D warnings`
  3. `CARGO_TARGET_DIR=target-check cargo test -p ghostlight-core --lib install::`
- ASCII only in code and docs: no em-dashes, arrows, or curly quotes. Code reads greenfield: cite an
  ADR only where the surrounding file already does so.
- SPDX header on any new file: `Apache-2.0 OR MIT`. T1 creates no new file.

## Task sequence (strict order; every prefix leaves a coherent, green tree)

| # | File | One-line goal | Depends on | On block |
|---|---|---|---|---|
| T1 | T1-windsurf.md | Windsurf target (reuses `mcpServers`, plain JSON) | -- | HALT |
| T2 | T2-merge-foundation.md | 3 merge dialects + JSONC-safe `Manual` fallback + tolerant detect | -- | HALT |
| T3 | T3-zed.md | Zed target (`context_servers`) | T2 | HALT |
| T4 | T4-opencode.md | OpenCode target (`mcp`, type local, command array) | T2 | HALT |
| T5 | T5-crush.md | Crush target (`mcp`, type stdio) | T2 | HALT |

Order: T1 and T2 are mutually independent (do either first). T3-T5 require T2. Every prefix of
`T1, T2, T3, T4, T5` leaves a coherent, green tree. T2 alone is shippable (new dialects compile,
unused until a client references them).

## Per-task procedure

1. Re-read every file the task's "Tree facts" names. If any named shape is gone or different, STOP
   (the module was refactored under you).
2. Make the edits exactly as pinned. Add the named test(s) with the pinned assertions verbatim --
   transcribe oracles, never derive them.
3. Run all three gates. All green.
4. One task = one commit: `feat(install): <summary> (ADR-0071)`. Update the LEDGER RESUME HERE +
   log entry (numbered deviations, if any).

## Completion criteria

- Per task: the pinned test(s) pass, all three gates green, one commit.
- Batch done: `client_by_id` resolves for `windsurf`, `zed`, `opencode`, `crush`; `ghostlight
  install --dry-run` plans the correct entry per client (Windsurf/Cursor-style `mcpServers`; Zed
  `context_servers`; OpenCode/Crush `mcp`); a JSONC config carrying comments plans as `manual`
  (printed steps), never `failed`.

## Failure protocol

If a task cannot complete as written: revert its edits (leave the tree green at the prior commit),
mark it BLOCKED in the LEDGER with the specific reason and the exact tree fact that did not hold,
and HALT. Do not improvise around a broken assumption. Do not skip ahead.

## NEVER touch (each NEVER names its one sanctioned exception, if any)

- The sacred MCP tool schemas / any tool surface. (No exception.)
- `install/merge.rs`. Sanctioned editor: **T2 only** (adds the three dialects + `to_value` arms).
  T1/T3/T4/T5 must not touch it -- they only reference dialects T2 created.
- Any client arm other than the one you are adding. (No exception.)
- `extension/`, `crates/core/src/governance/**`, docs/README/`llms-install.md` prose. (Doc/prose
  sync for the new client is a SEPARATE follow-up task, deliberately out of scope so the code lands
  green first. `doctor` lists the new client automatically because it iterates `CLIENTS`.)
