# Bootstrap: unattended execution of the stage-4 registry/pipeline prompts

You are an autonomous implementation agent working unattended in this repository. Your job is to
implement ADR-0023 (`docs/adr/0023-one-loader-for-the-policy-file.md`), ADR-0024
(`docs/adr/0024-tool-registry-and-generic-ingest-pipeline.md`), and ADR-0025
(`docs/adr/0025-manifest-hot-reload.md`) by executing the task prompts in
`docs/tasks/stage-4/` one at a time, in the exact sequence below, fully implementing each
prompt including its tests, while keeping durable written records so a context wipe never
loses work.

Read this whole file before doing anything. Then read ALL THREE ADRs IN FULL. They are the
normative sources for every semantic in this stage: ADR-0023 owns policy-file loading,
ADR-0024 owns the tool registry, the ingest pipeline, and the governance authorize/audit
API, ADR-0025 owns manifest hot-reload. Task prompts cite the ADRs instead of restating
them; where a task prompt and an ADR disagree, THE ADR WINS.

## Ground rules

1. Your context may be compacted or reset AT ANY TIME. `docs/tasks/stage-4/LEDGER.md` and
   `docs/tasks/stage-2/BROWSER-TESTS.md` are your memory. At the start of every task, after
   any interruption, and whenever you are unsure of your state: read LEDGER.md (RESUME HERE
   first), then the ADR(s) the current task cites, then the task prompt, then continue.
   Never rely on remembering earlier work; re-read files.
2. There is NO human available. Never ask questions; never wait for input. Make the
   conservative choice, record it in the ledger as a numbered deviation, and continue.
3. There is NO live browser available. Every verification that needs a real browser is
   DEFERRED: append it to `docs/tasks/stage-2/BROWSER-TESTS.md` (one shared file across
   stages; format documented there) instead of attempting it.
4. AUTHORITY ORDER: ADR-0023/0024/0025 (each owns its own scope; they do not overlap) >
   the stage-4 task prompts > ADR-0022 (still fully in force for the capability model,
   schema 3, and the action-directory semantics the registry absorbs) >
   `docs/tasks/stage-2/00-shared-format.md` (still authoritative for whatever no ADR
   supersedes: audit record fields, denial-id mechanics, section 7.2 message voice) >
   `docs/SPEC.md`. The ADRs own SEMANTICS; the prompts own the concrete Rust identifiers,
   signatures, and pinned strings (an ADR type sketch or prose naming -- e.g. the ADR's
   "AuditScope" concept rendered as the prompt's `CallAudit` identifier -- is schematic,
   not a pin; a prompt rendering it precisely is NOT a disagreement and needs no
   deviation). This BOOTSTRAP ranks below the ADRs and prompts for scope-specific
   statements; it wins only on process rules. If two sources genuinely conflict and none
   is higher, choose what best serves: all-open stays byte-identical, the `governance/`
   core stays free of `browser`/`transport` edges, fail-closed defaults,
   behavior-preservation over structure-preservation, and fewer moving parts. Record the
   choice in the ledger as a numbered deviation.
5. Work on branch `stage-4`. Create it from `stage-3` if it does not exist
   (`git checkout -b stage-4 stage-3`). Never push. Never merge. Never commit to `main`,
   `stage-2`, or `stage-3`.
6. One task = one commit. The commit includes the code, its tests, and the
   LEDGER/BROWSER-TESTS updates for that task. Message format:
   `refactor(architecture): <task-id> <short title>` (use `feat`/`fix`/`docs`/`chore` if
   more accurate; t01 is a `fix`, t06 is a `feat`).
7. THE SACRED SURFACE: `src/transport/mcp/schemas/tools.json` and
   `tests/tool_schema_fidelity.rs` must not change AT ALL in this stage. Stage 4 has NO
   sanctioned exception. If any task needs to touch either file, that task is wrong;
   revert and rethink.
8. BEHAVIOR PRESERVATION of user-visible surfaces is the stage-wide invariant, with
   exactly the sanctioned deltas each owning prompt names explicitly and no others:
   - t01: a policy file at the org path actually loads (today ANY org-path policy file is
     a fatal startup error; that is the bug being fixed), with its pinned corollaries:
     duplicate config keys become a validation error (ADR-0023 Decision 3, all origins);
     `config list` surfaces a broken policy file as a hard error instead of silently
     dropping the shadow line; an org file with invalid GRANTS now fails a config reload
     keep-last-good; `config list`/presets now see a user manifest's config entries.
   - t03: a governed directory miss (known tool, unknown sub-action) is DENIED with rule
     `unknown_action` instead of dispatching ungoverned -- the deliberate fix of the
     `b4b2faf` fail-open regression, restoring ADR-0022's absent-means-DENY (ADR-0024
     Decision 3); all-open misses still dispatch.
   - t05: internal frame traffic only (one `tab_url_request`, no synthesized
     `tabs_context_mcp` probe); inline seen-vector test edits, ledgered.
   - t06: manifest hot-reload, the two ADR-0025 session events, and the `list_changed`
     notification.
   Everything else -- tool results, denial texts and ids, audit record bytes, explain
   output, advertised sets, all-open behavior -- must be byte-identical before and after
   every task. `tests/all_open_golden.rs` and `tests/mcp_protocol.rs` guard this;
   compile-necessary retyping in guard tests is allowed (t03 deletes the `no_requires`
   helper; t04 retargets the three `is_known_tool` uses in `tests/all_open_golden.rs`
   onto the registry lookup -- both preserve every expectation and are ledgered);
   expectation edits are not (no task in this stage is sanctioned to change a guard
   expectation).
9. The `governance/` core stays domain-agnostic: no `crate::browser`, `crate::transport`,
   `crate::mcp`, `crate::native`, or `url` references anywhere under `src/governance/`
   (`tests/architecture.rs` enforces this; it must pass after every task). Browser facts
   cross the boundary as injected values (fn pointers or governance-defined types), never
   as imports, exactly as the stage-2/3 code already does.
10. ASCII only in everything you write (code, tests, docs, JSON, ledger entries): no
    em-dashes, no arrows, no curly quotes.
11. Never leave the tree dirty between tasks. Commit it or revert it. Keep it green:
    `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
12. No new dependencies of any kind, in any task, including dev-dependencies. ADR-0025's
    swap mechanism uses the primitives the tree already has (the ConfigStore idiom).
13. Delete what you replace, in the same task that replaces it. Stage 4 retires
    `parse_org_config`, `load_and_resolve`, `is_known_tool`'s per-call fixture re-parse,
    the five inline tool-name branches at the chokepoint, `resolve_governing_resource`'s
    name match, the five public `record_*` methods (`record_call`, `record_deny`,
    `record_navigate_landing_deny`, `record_shadow_deny`, `record_held`;
    `record_session_killed` STAYS -- t06's session events depend on it), the duplicated
    label formatters, `resolve_tab_host`, the four dead port seams, and the
    `src/browser/tools/` stub subtree. Each prompt says exactly which task deletes what;
    superseded code and its dead tests never survive a task boundary as a parallel path.
14. Byte-pinned oracles move by TRANSCRIPTION, never re-derivation. When a pinned
    expectation (explain text, denial message, advertised-set list, audit key order)
    must move files, copy the pinned bytes; if a test starts failing on a pinned literal,
    the code is wrong, not the pin.

## Environment facts

- Windows 11 dev machine. Shell: prefer bash-compatible commands; PowerShell also works.
- Build/test from the repo root: `cargo test` runs everything. Also
  `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` (fix with
  `cargo fmt` before committing).
- If `target/debug/browser-mcp.exe` is locked by a running session, rename it aside and
  rebuild: `mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`.
- The stage-3 tree this stage builds on is COMPLETE: all 8 stage-3 tasks plus a gap-fix
  commit landed (see `docs/tasks/stage-3/LEDGER.md` RUN SUMMARY and its follow-up
  section). The baseline at branch point (`stage-3` head) is 461 tests passing, clippy
  and fmt clean.
- The three ADRs and everything under `docs/tasks/stage-4/` are COMMITTED at the branch
  point. If `git status` shows any of them untracked or modified at run start, STOP:
  the handoff is broken; do not commit, revert, or clean them yourself. They are never
  revert-fodder under any rule in this file.
- Line numbers in prompts drift; trust names and prose, and re-read files before editing.
- The stage-3 survey facts each prompt's "Current behavior" section cites were verified
  2026-07-03 against `b4b2faf`. They are as-of-authoring: re-verify against the live tree
  before acting on any of them.

## Task sequence

Execute in exactly this order. Later tasks assume earlier ones landed. Every prefix of
the sequence leaves a coherent, green tree.

1. `t01` one loader for the policy file (ADR-0023; fixes the org-policy startup outage;
   independently valuable, lands even if the run stops here)
2. `t02` the tool registry (ADR-0024 Decision 1; generalizes the action directory in
   place; purely additive, nothing consumes the new fields yet)
3. `t03` governance authorize + CallAudit (ADR-0024 Decision 3; the audit-ownership
   inversion; call sites adapted in place, pipeline still name-branched)
4. `t04` the generic ingest pipeline (ADR-0024 Decision 2; the chokepoint rewrite,
   consuming the registry and the new governance API together; the largest task)
5. `t05` one tab-URL resolution per call (ADR-0024 Decision 4)
6. `t06` manifest hot-reload (ADR-0025; swappable governance snapshot, re-advertisement,
   the two session events)
7. `t07` dead-seam and stub deletions (ADR-0024 Decision 5)
8. `t08` documentation sync (supersession notes, SPEC updates list, BROWSER-TESTS
   entries)

Each prompt is self-contained (Goal, Authority, Depends on, Current behavior, Required
behavior, Constraints, Tests, Verification, Out of scope). Respect every Out of scope
section literally.

## Never touch

Unless a prompt names the file in its Required behavior (single ownership: at most one
task owns any given change):

- `src/transport/mcp/schemas/tools.json` and `tests/tool_schema_fidelity.rs`: NEVER, no
  exception task exists in this stage.
- `extension/` (all of it): NEVER, no exception task. Stage 4 is binary-only.
- `examples/*.json`, `src/governance/templates.rs` templates, `tests/fixtures/simulate/*`,
  `tests/fixtures/explain/*`: NEVER; no stage-4 task changes what any manifest or golden
  SAYS. (t01 changes how manifests are LOADED, not their content.)
- `docs/adr/*` (ADRs are immutable once accepted) EXCEPT `docs/adr/README.md`'s index
  table rows, which t08 owns; `docs/SPEC.md` (amended only via the shared-format
  updates-needed list, which t08 owns); `docs/tasks/stage-2/*` and
  `docs/tasks/stage-3/*` except the shared BROWSER-TESTS.md appends and the s08-style
  supersession banner edits t08 owns.
- `Cargo.toml` / `Cargo.lock`: NEVER (rule 12).
- Guard-test EXPECTATIONS in `tests/all_open_golden.rs` and `tests/mcp_protocol.rs`:
  never edited; compile-necessary helper/signature retyping only, documented in the
  ledger when it happens.

## Per-task procedure

1. Read LEDGER.md RESUME HERE. Confirm which task is next and that the tree is clean.
2. Read the ADR(s) the prompt cites (at minimum the Decisions it names) and the task
   prompt, in full.
3. Re-read the actual target files in the current tree before editing. If a STOP
   precondition in the prompt fails, STOP: revert any partial work, record the blocker in
   the ledger (numbered, with what you observed), and halt the run.
4. Implement the Required behavior exactly. Where the prompt pins a signature, string,
   table, or algorithm, transcribe it verbatim. Add the tests the prompt names, with the
   prompt's pinned assertions.
5. For any verification needing a real browser, append a BROWSER-TESTS.md entry instead
   of running it, and note the count in the ledger.
6. Verify: `cargo fmt` then `cargo clippy --all-targets -- -D warnings` clean;
   `cargo test` green including the new tests; `tests/architecture.rs`,
   `tests/all_open_golden.rs`, `tests/mcp_protocol.rs`, and
   `tests/tool_schema_fidelity.rs` all passing with tools.json and the fidelity test
   byte-unchanged. ASCII-scan touched files: `rg -n "[^\x00-\x7F]" <files>` prints
   nothing.
7. Update LEDGER.md: append a task-log entry (files touched, summary, numbered deviations
   with reasoning, verification results with exact test counts, browser checks queued)
   and rewrite RESUME HERE to point at the next task.
8. Commit everything for this task in one commit. Leave the tree clean.
9. Go to the next task.

## Failure protocol

When a task cannot complete: revert the working tree to the last commit -- run
`git status` first, then `git restore .` for modified tracked files PLUS delete any
files this task itself created (they show as untracked; NEVER touch anything under
`docs/tasks/stage-4/` or `docs/adr/`, which predate the run). Stash nothing: partial
work is abandoned; the prompt is the source of truth for a retry. Then append a
task-log entry marked BLOCKED with numbered reasoning (what failed, what you observed,
which STOP precondition or verification step tripped), rewrite RESUME HERE to say the
run is halted at that task, and COMMIT that ledger edit alone as
`docs(architecture): <task-id> BLOCKED` so the halt state itself leaves a clean tree.
Then STOP the run. Do not skip ahead: every later task assumes the earlier ones landed.

## Completion

When task 8 (`t08`) is committed and green, write a RUN SUMMARY entry in LEDGER.md:
tasks completed, the commit range, every conservative choice made, the deletions
actually performed (the ledger of removed parts is a stage deliverable), and the state
of BROWSER-TESTS.md. State plainly that manifest hot-reload and the org-policy loading
fix must be verified by a human against a live browser (the stage-3 s-live backlog plus
the new t06 entries) before stage 4 is considered verified end to end. Do not push or
merge; a human decides when `stage-4` merges.
