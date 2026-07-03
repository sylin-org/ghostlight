# Bootstrap: unattended execution of the stage-2 governance prompts

You are an autonomous implementation agent working unattended in this repository. Your job is to
build the stage-2 governance layer by executing the task prompts in `docs/tasks/stage-2/` one at a
time, in the exact sequence below, fully implementing each prompt including its tests, while keeping
durable written records so a context wipe never loses work.

Read this whole file before doing anything. Then read `PLAN.md` and `RECONCILIATION.md`.

## Ground rules

1. Your context may be compacted or reset AT ANY TIME. `docs/tasks/stage-2/LEDGER.md` and
   `docs/tasks/stage-2/BROWSER-TESTS.md` are your memory. At the start of every task, after any
   interruption, and whenever you are unsure of your state: read LEDGER.md (RESUME HERE first), then
   `PLAN.md` and `RECONCILIATION.md`, then the task prompt you are on, then continue. Never rely on
   remembering earlier work; re-read files.
2. There is NO human available. Never ask questions; never wait for input. Make the conservative
   choice, record it in the ledger, and continue.
3. There is NO live browser available. You cannot reload the extension or click anything in Chrome.
   Every verification that needs a real browser is DEFERRED: write it into BROWSER-TESTS.md (format in
   that file) instead of attempting it.
4. AUTHORITY ORDER. `PLAN.md` defines the order and the cross-cutting workstreams. `RECONCILIATION.md`
   is AUTHORITATIVE over any conflicting detail in a `g`-doc (placement, hot-reload, ports, org
   policy). The new `a`-prompts (`a1`, `a2`, `a3`, `a5`, `a7`) already encode the current vision. Where
   nothing overrides it, the `g`-doc stands. If two sources genuinely conflict and none is higher, make
   the choice that best serves: all-open stays byte-identical, the domain-agnostic `governance/` core
   stays free of `browser`/`transport` edges, and governance behaves as delight. Record the choice.
5. Work on branch `stage-2`. Never push. Never merge. Never commit to `main`.
6. One task = one commit. The commit includes the code, its tests, and the LEDGER/BROWSER-TESTS updates
   for that task. Message format: `feat(governance): <task-id> <short title>` (use `fix`/`refactor`/
   `chore` if more accurate). Example: `feat(governance): a2 governance ports and seam contract`.
7. Never modify the tool schemas (`src/mcp/schemas/tools.json`), tool names, parameters, or
   descriptions. `tests/tool_schema_fidelity.rs` must pass after every task. If a change breaks it, the
   change is wrong; revert and rethink (ADR-0007, the sacred surface).
8. All-open stays first-class and byte-identical. With no manifest and default config, every tool
   result is exactly what stage 1 produced (STEP-0 short-circuit). The all-open golden test (added in
   Phase A) and the untouched `tests/mcp_protocol.rs` are the guard.
9. ASCII only in everything you write (code, tests, docs, ledger entries): no em-dashes, no arrows, no
   curly quotes. Use Rust `\u{..}` escapes if a test needs a non-ASCII input.
10. Never leave the tree dirty between tasks. Commit it or revert it. Keep it green: `cargo test`,
    `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.

## Environment facts

- Windows 11 dev machine. Shell: prefer bash-compatible commands; PowerShell also works.
- Build/test from the repo root: `cargo test` runs everything. Also `cargo clippy --all-targets --
  -D warnings` and `cargo fmt --check` (fix with `cargo fmt` before committing).
- If `target/debug/browser-mcp.exe` is locked by a running session, rename it aside and rebuild:
  `mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1` (or stop the MCP client first).
- The extension is vanilla JS with no test harness. For every touched JS file run a syntax check:
  `node --check extension/service-worker.js` (and each other touched `.js`). Do NOT add a JS test
  framework.
- New Rust dependencies are allowed ONLY where a task prompt explicitly calls for one (for example
  `sha2` for the manifest hash in g09, `chrono`/`uuid` for audit in g06, `url` for the matcher in g07,
  and a file-watch crate in a5 if that prompt selects one). Add nothing else. The `url` crate may only
  be used inside `browser/`, never `governance/` (the a7 arch-test enforces this).
- ASCII scan for files you created or edited (run before each commit):
  `python -c "import sys;[print(f,[c for c in open(f,encoding='utf-8').read() if ord(c)>127][:5]) for f in sys.argv[1:]]" <files>`
  Any output other than empty lists means fix the file.

## Task sequence

Execute in exactly this order. It is a linearization of the `PLAN.md` dependency graph (Phase A
foundations, then ADR-0018 steps B/C/D). Prompt files are `docs/tasks/stage-2/<id>-<slug>.md`.

Phase A (foundations; all-open stays byte-identical throughout):
1. `a1` module reorg (governance/ browser/ transport/; adds the all-open golden test)
2. `a2` governance ports (the seam contract)
3. `a3` governance facade (dispatch chokepoint; all_open() zero-cost Allow)
4. `a7` arch-test (fail-closed; guards governance/ before the config code lands)
5. `g01` typed key registry (owned Config; placement per RECONCILIATION section 1-2)
6. `g02` layered resolution (returns a re-resolvable owned snapshot)
7. `a5` hot-reload substrate (atomic swap + file-watch + validate-then-swap; org fail-closed)
8. `g03` config CLI (config set triggers an immediate re-resolve)
9. `g04` schema generation

Phase B (audit flight recorder; pure observation):
10. `g05` r/w classification
11. `g09` manifest identity
12. `g06` audit recorder + sinks

Phase C (sacred + pause + kill; first enforcement, audited):
13. `g07` domain matcher (url crate lives here, in browser/)
14. `g08` sacred domains
15. `g10` take-the-wheel pause
16. `g11` panic kill switch

Phase D (manifest engine + trust UX):
17. `g12` manifest engine
18. `g13` grant enforcement
19. `g14` advertisement filtering (emits tools/list_changed on manifest hot-reload)
20. `g15` shadow mode
21. `g16` policy explain
22. `g17` policy simulate
23. `g18` presets and templates

Each prompt is self-contained (Goal, Depends on, Project context, Current behavior, Required behavior,
Constraints, Verification, Out of scope). Line numbers inside prompts drift as earlier tasks land;
trust function names and prose over line numbers, and re-read the target file before editing. Respect
every Out of scope section literally. Apply RECONCILIATION.md over every `g`-doc.

## Per-task procedure

1. Read LEDGER.md RESUME HERE. Confirm which task is next and that the tree is clean.
2. Read PLAN.md (the relevant phase), RECONCILIATION.md (the relevant sections), and the task prompt.
3. Re-read the actual target files in the current tree (paths/line numbers have drifted).
4. Implement the Required behavior exactly, applying RECONCILIATION.md where it overrides the g-doc.
   Add the unit tests the prompt names. Keep the domain-agnostic `governance/` core free of
   `browser`/`transport`/`mcp`/`native`/`url` edges.
5. For any verification the prompt describes that needs a real browser, do NOT run it: append a
   BROWSER-TESTS.md entry (format in that file) and note the count in the ledger.
6. Verify: `cargo fmt` then `cargo clippy --all-targets -- -D warnings` clean; `cargo test` green
   including the new unit tests, the all-open golden test, `tests/tool_schema_fidelity.rs` unchanged,
   and `tests/mcp_protocol.rs` unchanged. Run `node --check` on any touched JS.
7. Update LEDGER.md: append a task-log entry (per its shape) and rewrite RESUME HERE to point at the
   next task.
8. Commit everything for this task in one commit (code + tests + ledger + browser-tests) with the
   message format above. Leave the tree clean.
9. Go to the next task.

## Completion

When task 23 (`g18`) is committed and green, write a RUN SUMMARY entry in LEDGER.md: the tasks
completed, the total commit range, any conservative choices made, and the state of BROWSER-TESTS.md.
State plainly that BROWSER-TESTS.md must then be run by a human against a live browser (as release-1
was), and that until step D is verified live, public copy must say governance is shipped-but-
unverified-end-to-end. Do not push or merge; a human decides when `stage-2` merges to `main`.
