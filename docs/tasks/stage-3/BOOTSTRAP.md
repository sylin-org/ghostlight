# Bootstrap: unattended execution of the stage-3 capability-model prompts

You are an autonomous implementation agent working unattended in this repository. Your job is to
implement ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) by executing the task
prompts in `docs/tasks/stage-3/` one at a time, in the exact sequence below, fully implementing
each prompt including its tests, while keeping durable written records so a context wipe never
loses work.

Read this whole file before doing anything. Then read ADR-0022 IN FULL. The ADR is the single
normative source for every semantic in this stage: the capability taxonomy, the action directory
table, host polarity, the evaluation algorithm, denial rules, schema 3, and the sanctioned tool
addition. Task prompts cite the ADR instead of restating it; where a task prompt and the ADR
disagree, THE ADR WINS.

## Ground rules

1. Your context may be compacted or reset AT ANY TIME. `docs/tasks/stage-3/LEDGER.md` and
   `docs/tasks/stage-2/BROWSER-TESTS.md` are your memory. At the start of every task, after any
   interruption, and whenever you are unsure of your state: read LEDGER.md (RESUME HERE first),
   then ADR-0022, then the task prompt you are on, then continue. Never rely on remembering
   earlier work; re-read files.
2. There is NO human available. Never ask questions; never wait for input. Make the conservative
   choice, record it in the ledger, and continue.
3. There is NO live browser available. Every verification that needs a real browser is DEFERRED:
   append it to `docs/tasks/stage-2/BROWSER-TESTS.md` (one shared file across stages; format
   documented there) instead of attempting it.
4. AUTHORITY ORDER: ADR-0022 > the stage-3 task prompts > `docs/tasks/stage-2/00-shared-format.md`
   (still authoritative for everything ADR-0022 does not supersede: config file formats, audit
   record fields other than `rw`, denial id mechanics, section 7.2 message voice) > `docs/SPEC.md`.
   ADR-0022 explicitly supersedes shared-format sections 4.3 (grant fields), 6.1 `rw`, and 8
   (classification). If two sources genuinely conflict and none is higher, choose what best
   serves: all-open stays byte-identical, the `governance/` core stays free of
   `browser`/`transport` edges, fail-closed defaults, and governance as delight. Record the
   choice in the ledger as a numbered deviation.
5. Work on branch `stage-3`. Create it from `stage-2` if it does not exist
   (`git checkout -b stage-3 stage-2`). Never push. Never merge. Never commit to `main` or
   `stage-2`.
6. One task = one commit. The commit includes the code, its tests, and the LEDGER/BROWSER-TESTS
   updates for that task. Message format: `feat(governance): <task-id> <short title>` (use
   `fix`/`refactor`/`docs`/`chore` if more accurate).
7. THE SACRED SURFACE, amended by ADR-0022 Decision 7: the 13 existing tool entries in
   `src/transport/mcp/schemas/tools.json` (names, parameters, descriptions, enum values, field
   order) must stay byte-identical FOREVER. Task s07, and ONLY task s07, adds exactly one new
   tool (`explain`) to that file and amends `tests/tool_schema_fidelity.rs` to pin the new
   13-plus-1 invariant. No other task touches tools.json or the fidelity test for any reason.
   If any task other than s07 breaks the fidelity test, that change is wrong; revert and rethink.
8. All-open stays first-class. With no manifest and default config, every tool result is exactly
   what the stage-2 tree produces. `tests/all_open_golden.rs` and `tests/mcp_protocol.rs` guard
   this. Only s07 may change what `tools/list` returns (the one sanctioned addition); s07's
   prompt states exactly which test expectations change and how; every change to a guard test's
   expectation is documented in the ledger as a deliberate, sanctioned edit.
9. The `governance/` core stays domain-agnostic: no `crate::browser`, `crate::transport`,
   `crate::mcp`, `crate::native`, or `url` references anywhere under `src/governance/`
   (`tests/architecture.rs` enforces this; it must pass after every task). Browser-domain data
   (the action directory, host matching) lives in `src/browser/`; the core consumes it through
   injected function pointers, exactly as the stage-2 code already does everywhere.
10. ASCII only in everything you write (code, tests, docs, JSON, ledger entries): no em-dashes,
    no arrows, no curly quotes.
11. Never leave the tree dirty between tasks. Commit it or revert it. Keep it green:
    `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
12. No new dependencies of any kind, in any task, including dev-dependencies. Everything stage 3
    needs is already in the tree.
13. Delete what you replace, in the same task that replaces it. Stage 3 retires the read/write
    classification (`RwClass`, `browser/classify.rs`), the `access`/`tools`/`exclude_tools`
    grant fields, and the audit `rw` field. Each prompt says exactly which task deletes what;
    superseded code and its dead tests never survive a task boundary as a parallel path. Lean
    internals are a project value; two sources of truth are a defect.

## Environment facts

- Windows 11 dev machine. Shell: prefer bash-compatible commands; PowerShell also works.
- Build/test from the repo root: `cargo test` runs everything. Also
  `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` (fix with `cargo fmt`
  before committing).
- If `target/debug/browser-mcp.exe` is locked by a running session, rename it aside and rebuild:
  `mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`.
- The stage-2 tree this stage builds on is COMPLETE (all 23 stage-2 tasks landed; see
  `docs/tasks/stage-2/LEDGER.md` RUN SUMMARY). Line numbers in prompts drift; trust names and
  prose, and re-read files before editing.
- The `Grant` type is compile-coupled to enforcement, dispatch, advertisement, explain,
  simulate, and templates: changing its shape breaks all of them at once. That is why s05 is
  deliberately one large atomic task; do not try to split it, and do not leave any consumer
  half-adapted.
- ASCII scan for files you created or edited (run before each commit):
  `rg -n "[^\x00-\x7F]" <files>` must produce no output.

## Task sequence

Execute in exactly this order. Later tasks assume earlier ones landed.

1. `s01` navigate is read (standalone reclassification on the CURRENT stage-2 model; correct
   under both the old model's own rationale and ADR-0022; keeps the tree coherent even if this
   run stops here)
2. `s02` capability vocabulary in the governance core (`Capability`, sets, subset containment)
3. `s03` the action directory in the browser plugin (the ADR Decision 2 table + invariants)
4. `s04` host polarity evaluation in the browser plugin (ADR Decision 4 semantics)
5. `s05` the schema-3 switch: manifest grant shape, enforcement, dispatch, advertisement,
   explain, simulate, examples, templates, and every affected test, in one atomic task
   (ADR Decisions 3, 4, 5, 6, and 8)
6. `s06` audit `capability` field; delete `classify.rs` and `RwClass` (ADR Decision 8)
7. `s07` the `explain` directory tool: the ONLY sanctioned tools.json change (ADR Decision 7)
8. `s08` documentation sync: shared-format supersession notes, CLAUDE.md, SPEC updates list,
   BROWSER-TESTS live-check entries for capability enforcement

Each prompt is self-contained (Goal, Authority, Depends on, Current behavior, Required behavior,
Constraints, Tests, Verification, Out of scope). Respect every Out of scope section literally.

## Per-task procedure

1. Read LEDGER.md RESUME HERE. Confirm which task is next and that the tree is clean.
2. Read ADR-0022 (at minimum the Decisions the prompt cites) and the task prompt.
3. Re-read the actual target files in the current tree before editing.
4. Implement the Required behavior exactly. Where the prompt cites the ADR for a table, string,
   or algorithm, transcribe from the ADR verbatim. Add the tests the prompt names.
5. For any verification needing a real browser, append a BROWSER-TESTS.md entry instead of
   running it, and note the count in the ledger.
6. Verify: `cargo fmt` then `cargo clippy --all-targets -- -D warnings` clean; `cargo test`
   green including the new tests, `tests/architecture.rs`, `tests/all_open_golden.rs`, and
   (outside s07) `tests/tool_schema_fidelity.rs` unchanged. ASCII-scan touched files.
7. Update LEDGER.md: append a task-log entry (files touched, summary, numbered deviations with
   reasoning, verification results with exact test counts, browser checks queued) and rewrite
   RESUME HERE to point at the next task.
8. Commit everything for this task in one commit. Leave the tree clean.
9. Go to the next task.

## Completion

When task 8 (`s08`) is committed and green, write a RUN SUMMARY entry in LEDGER.md: tasks
completed, the commit range, every conservative choice made, and the state of BROWSER-TESTS.md.
State plainly that the new capability enforcement must be verified by a human against a live
browser before stage 3 is considered verified end to end, and that public copy must describe it
as shipped-but-unverified-end-to-end until then. Do not push or merge; a human decides when
`stage-3` merges.
