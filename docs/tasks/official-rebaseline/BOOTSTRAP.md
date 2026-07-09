# BOOTSTRAP -- official-rebaseline batch (ADR-0050)

You are a code-executing agent implementing ADR-0050 unattended. Assume ZERO conversational context.
Follow instructions literally; resolve nothing by judgment. When a task cannot complete, use the
Failure protocol below -- do NOT improvise a workaround.

## Authority order (highest wins)

1. `docs/adr/0050-official-rebaseline-and-file-tools.md` -- the NORMATIVE design. Semantics live
   there; the task prompts cite it. If a task prompt and ADR-0050 conflict, STOP and mark BLOCKED.
2. This BOOTSTRAP (ground rules, sequence, protocols).
3. The individual task prompt `T<n>-*.md` (the concrete steps + PINNED oracles for that task).
4. The live tree. It is the source of truth for CURRENT FACTS, but never overrides a pinned oracle:
   if the tree disagrees with a pinned expected string/count, STOP (a pin mismatch means either the
   tree drifted or the oracle is stale -- a human decides, you do not).

ORACLE RULE: every expected output, test assertion, count, and string in a task prompt was computed
by the author. You TRANSCRIBE oracles; you never DERIVE them. If a prompt says "assert the count is
18", write 18 -- do not recount and substitute your own number. If your implementation makes a pinned
test fail, the implementation is wrong (or the tree drifted -> STOP), not the oracle.

## Ground rules

- ASCII ONLY in code and docs (`.rs`, `.js`, `.json`, `.toml`, `.md`). Render em-dash as `--`. No
  arrows, curly quotes, or other non-ASCII. (The one place a non-ASCII-looking string is required is
  a description harvested from the official extension -- ADR-0050 already renders those to ASCII with
  `--`; use the ADR's rendering verbatim.)
- RE-READ THE TREE before editing. The "tree facts" below are as-of-authoring (2026-07-09) and may
  have drifted; every task prompt repeats "re-read" standing orders. Trust the live tree for current
  file contents; trust the pinned oracles for expected results.
- One task = one commit (some tasks may use two commits: the code change, then a
  `docs(rebaseline): ledger T<n>` commit -- the task prompt says which). Conventional commit
  messages, no attribution. Never `git push` unless the human explicitly asks.
- Never touch the NEVER list (below) except where a task prompt names the sanctioned exception.
- Never skip hooks, never `--no-verify`, never force-anything.

## Environment facts (as-of-authoring 2026-07-09; RE-READ to confirm)

- Repo: `f:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp`. Branch: `dev`. Workspace crates:
  root facade `ghostlight` + `ghostlight-transport` + `ghostlight-core` + `ghostlight-adapter-agent`
  + `ghostlight-adapter-browser`. The tool engine lives in `ghostlight-core` (`crates/core`).
- Platform: Windows 11, PowerShell primary (a Bash tool also exists). Use `--%` or care with paths.
- A `dev` Ghostlight SERVICE may be running and will LOCK `target/debug/ghostlight*.exe`, breaking a
  full `cargo build`. If a build fails to relink a locked exe: stop the service
  (`Get-Process ghostlight* | Stop-Process -Force`) OR use `cargo check`/`cargo test` (tests build
  their own harness bins). Do NOT run install/uninstall/schtasks/registry commands.
- Advertised tool count at batch START: **17** (13 trained + `wait_for` + `script` + `form_fill` +
  `explain`). Each additive-tool task raises it by one: T1 -> 18, T2 -> 19, T3 -> 20, T4 -> 21. T5
  adds no tools. Each task prompt pins its own count transition; transcribe it.
- Ordered advertised list ends with `explain` LAST; every new additive tool inserts BEFORE `explain`.
- Tool registry: `crates/core/src/browser/directory.rs` (`const REGISTRY`); fidelity is a set of
  HAND-MAINTAINED Rust asserts (NO golden fixture file, NO regeneration flag). The ADVERTISED-TOOL
  COUNT is hard-coded in SEVEN places that every additive-tool task must bump in lockstep (they all
  derive from `REGISTRY`): `tests/tool_schema_fidelity.rs` (two asserts), `tests/all_open_golden.rs`
  (`GOLDEN_TOOL_NAMES` length + a count assert), `tests/mcp_protocol.rs` (one assert),
  `tests/tool_enforcement.rs` (one assert, `all_open_invariant_no_manifest_means_no_denials`), and
  `crates/core/src/hub/outbound/mod.rs` (TWO asserts: `browser_capability_exposes_the_full_directory`
  and `registry_aggregates_the_browser_directory`) -- PLUS the `#[cfg(test)]` pin tables inside
  `directory.rs` (`total_variants`, `with_action_key`, the `EXPECTED` and `EXPECTED_TOOLS` tables).
  Each task prompt lists the exact asserts + values. Before committing a task, grep the whole repo for
  the OLD count as a bare number near "tools"/"directory"/"REGISTRY" to catch any the prompt missed.
- Extension: `extension/content.js` (`refToEl`/`deref(ref)`, DOM ops), `extension/service-worker.js`
  (`handlers` object). Extension unit tests: `node --test tests/extension/*.test.js`.

## Task sequence (linear; each prefix leaves a GREEN tree)

- T1 -- `file_upload` (ADR-0050 Decision 2). Smallest, standalone.
- T2 -- `browser_batch` overload over the shared `script` engine, `script` kept (Decision 3).
- T3 -- `upload_image` (Decision 4): screenshot cache + `imageId` + drag-drop. Highest risk.
- T4 -- `gif_creator` (Decision 5): phased; Phase 1 (record + download-export) is the landable floor.
- T5 -- re-baseline the 13 trained schemas vs v1.0.80 + retire `reference/` (Decision 1, 6).

Do them IN ORDER. Do not start T<n+1> until T<n> is committed with a green tree.

## Per-task procedure

1. Read the task prompt `T<n>-*.md` fully, then re-read ADR-0050's cited Decision.
2. Re-read the live files the prompt names (they may have drifted from the tree facts above).
3. Check the prompt's STOP preconditions. If any precondition is absent/false, STOP (Failure
   protocol) -- do not improvise.
4. Implement exactly what the prompt specifies (signatures, strings, schema literals verbatim).
5. Add the named tests with the PINNED assertions (transcribe oracles).
6. Run the verification block V-ALL (below). All must pass.
7. Commit (message given in the prompt). Update the LEDGER per-task log entry.

## V-ALL (verification; run from repo root; all must pass)

    cargo fmt --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo build --locked --workspace
    cargo test --locked --no-fail-fast --workspace
    node --test tests/extension/constants.test.js tests/extension/geometry.test.js tests/extension/grouping.test.js tests/extension/keys.test.js tests/extension/settle.test.js tests/extension/observation.test.js tests/extension/treediff.test.js tests/extension/fileset.test.js tests/extension/neuquant.test.js tests/extension/gifenc.test.js tests/extension/gifoverlay.test.js

(If a task adds an extension unit test file, the prompt adds it to the `node --test` line.)
Note the known-flaky macOS-only `peer_death` connection race and the quarantined `e2e-smoke` job are
CI concerns, not local gates; locally, all V-ALL must pass.

## Completion criteria (per task)

- The task's new/changed behavior works as the prompt specifies.
- Every pinned test passes; V-ALL is green.
- No NEVER-list file changed except the sanctioned append/bump the prompt names.
- One commit (or the two the prompt names). LEDGER updated.

## Failure protocol

If a task cannot complete (a STOP precondition fails, a pinned oracle disagrees with the tree, V-ALL
cannot go green without touching something out of scope, or the design is ambiguous):
1. `git restore .` / `git checkout -- .` to revert the working tree to the last green commit (do NOT
   leave a half-edit).
2. In the LEDGER, mark the task **BLOCKED** with: the exact precondition/oracle that failed, the file
   and line, and your reasoning (one paragraph).
3. HALT the batch (do not skip ahead to T<n+1>). A human resumes from the LEDGER's RESUME HERE.

## NEVER list (each NEVER names its sole sanctioned exception, if any)

- NEVER change any of the 13 TRAINED tools' names, parameter names, enum values, or description
  strings. EXCEPTION: T5 (Decision 6) may add ADDITIVE optional params / description-only edits; T3
  (Decision 4) may add an ADDITIVE OUTPUT field to `computer` -- never a rename/removal of a trained
  input field or enum.
- NEVER edit the `EXPECTED_TRAINED` block in `tests/tool_schema_fidelity.rs`. EXCEPTION: none.
- NEVER remove or weaken the ADR-0049 no-initialize-before-use behavior (reconnect replay needs it).
  EXCEPTION: none.
- NEVER rename or remove the `script` tool (ADR-0050 keeps it). EXCEPTION: none. `browser_batch` is
  ADDED alongside it.
- NEVER write non-ASCII into any code or doc file. EXCEPTION: none (use `--`).
- The sacred files `crates/core/src/browser/directory.rs`, `tests/tool_schema_fidelity.rs`,
  `tests/all_open_golden.rs` are edited ONLY to APPEND new additive rows/names and BUMP the
  hand-maintained counts/positions/pin tables the task prompt enumerates. `explain` stays LAST in
  every ordered list. EXCEPTION: the specific appends/bumps each task prompt pins.
