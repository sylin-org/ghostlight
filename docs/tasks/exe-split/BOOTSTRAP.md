# exe-split BOOTSTRAP: ground rules for the executor

You are executing the exe-split batch: splitting the single multi-role `ghostlight` binary into
three role-specific executables per ADR-0046. Work strictly by these rules.

## Authority order (highest wins)

1. `docs/tasks/exe-split/SPEC.md` (the normative pins)
2. `docs/adr/0046-role-specific-executables.md` (the decision), ADR-0044, ADR-0045
3. The task files `S1..S10` in this directory (procedure)
4. The live tree (for FACTS; if a fact contradicts SPEC/task, STOP and record it)

Never re-litigate anything in SPEC's Provenance section. Resolve nothing by judgment: if a task
underdetermines a choice that matters, STOP and record BLOCKED.

## Environment facts

- Windows 11 machine; repo at `f:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp`.
- Branch: `dev`. Base commit for this batch: `fccca60` (tree green; commits after it touching
  only `docs/` are expected -- the batch files themselves).
- Shell: bash tool available (use POSIX sh syntax) and PowerShell. Prefer bash for cargo/git.
- `x86_64-unknown-linux-gnu` target is installed for cross-checks. macOS is CI-only: never
  hand-verify mac code beyond mechanical rewrites.
- ASCII ONLY in every file you write or edit (code AND docs): no em-dashes, no unicode arrows,
  no curly quotes.
- `Cargo.lock` is committed; commit its changes together with each task.

## Linear task sequence

S1 -> S2 -> S3 -> S4 -> S5 -> S6 -> S7 -> S8 -> S9 -> S10. No reordering, no skipping (a skipped
task = BLOCKED + halt). Every prefix of the sequence leaves a coherent, green, shippable tree.

## Per-task procedure

1. Read the task file fully, then re-read every tree location it names (files move between tasks;
   trust the live tree for facts).
2. Check the task's STOP preconditions. If any fails: do not start; record BLOCKED in the LEDGER
   with the exact failing precondition and halt the batch.
3. Implement exactly the Required changes. Mechanical moves are mechanical: never "improve" moved
   code, never reformat beyond `cargo fmt`, never rename beyond the pinned renames.
4. Add the pinned tests BY NAME with the pinned assertions. Do not invent extra tests; do not
   weaken pinned assertions.
5. Run the verification commands from SPEC section 12 (plus any task-specific ones). ALL must
   pass. If a pinned EXISTING test fails, your change is wrong -- fix the change, never the test
   (exception: a task explicitly pins a test edit).
6. `git add` ONLY the files the task owns (plus Cargo.lock). Commit with the task's pinned commit
   message. One task = exactly one CODE commit.
7. Update `LEDGER.md` (move RESUME HERE to the next task; append the task's log entry: the code
   commit hash, verification results, deviations as a numbered list -- every deviation, however
   small), then commit the ledger by itself:
   `git add docs/tasks/exe-split/LEDGER.md && git commit -m "docs(exe-split): ledger S<n>"`.
   So each task lands as exactly two commits: its code commit, then its ledger commit.

## Failure protocol

If a task cannot complete (a STOP precondition fails, verification cannot be made green within the
task's scope, or the task would require touching a NEVER item):

1. FIRST record BLOCKED in the LEDGER under that task (what failed, the exact error text, your
   best one-paragraph diagnosis, numbered deviations for anything already done differently) and
   commit ONLY the ledger:
   `git add docs/tasks/exe-split/LEDGER.md && git commit -m "docs(exe-split): BLOCKED at S<n>"`.
2. THEN revert the in-progress work: `git checkout -- . && git clean -fd` (this removes untracked
   files -- your own in-progress files included; that is intended; the ledger survives in its
   commit).
3. HALT the batch. Do not attempt later tasks.

## NEVER touch (each names its only sanctioned exception, if any)

- `extension/` -- no exception in this batch.
- `crates/core/src/mcp/tools.rs` content (the sacred tool surface), wherever it lives at the
  moment: MOVE-ONLY (S4 moves the file verbatim); never edit its contents.
- `tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs` -- never edited; they must pass
  unchanged at every task boundary.
- `org_policy_path()` (machine-wide, never instance-suffixed, ADR-0044 D3) -- move it with its
  file, never change its body.
- Governance SEMANTICS (grants, capabilities, enforcement, audit record shapes) -- moves only.
- LICENSE files and SPDX headers -- headers move with their files, never change; exception: NEW
  files get the header their crate's license dictates (SPEC section 1/2/3).
- `.github/workflows/release.yml` -- exception: S10 only.
- `.github/workflows/ci.yml` -- exception: S5 only (the test job's two cargo lines AND the
  e2e job's `cargo build --locked` line, all gaining `--workspace`).
- `.github/workflows/pages.yml`, `site/` -- no exception.
- `docs/adr/**` -- exception: S8 appends the pinned amendment note to ADR-0045 only.
- `docs/tasks/**` other than this batch's own LEDGER -- no exception.
- Behavior of the DEFAULT instance identity (byte-identical, ADR-0044 D2) -- guarded by existing
  tests; if one fails, your change is wrong.
- Do not run `ghostlight install` (non-dry-run), `schtasks`, `launchctl`, or `systemctl` against
  the real machine. Dry-run only. No exception.
- Do not push. Commits stay local; the owner pushes after review. No exception.

## Completion criteria (the batch is DONE when)

- All ten tasks have their code + ledger commits, logged in the LEDGER, RESUME HERE says COMPLETE.
- SPEC section 12 verification is green at the final commit.
- `install_instance`, `adapter_reconnect`, `bare_invocation`, `mcp_protocol`,
  `tool_schema_fidelity` test files all pass in `cargo test --workspace`.
- No file outside the tasks' ownership lists changed (verify with `git diff --stat fccca60..HEAD`
  and compare against the union of task ownerships; unexpected files = a deviation to log).
