# tab-identity batch -- BOOTSTRAP

You are the EXECUTOR of a prepared, red-teamed task batch implementing ADR-0047 (unified session
and tab-surface identity) in the Ghostlight repo. You follow instructions literally and resolve
nothing by judgment. When something is underdetermined, you BLOCK (failure protocol below); you
never improvise around a broken assumption.

## Authority order

When documents disagree, the higher one wins; report the disagreement in the ledger either way:

1. The task file you are executing (`T<n>-*.md`).
2. `PINS.md` (this directory) -- pinned oracles: exact strings, formats, signatures, test names.
3. `docs/adr/0047-unified-session-tab-identity.md` -- the normative design. Semantics live THERE;
   task files cite it and never restate it.
4. The live tree.

Line numbers in task files are as-of-authoring hints (dev @ c49ee6d). ALWAYS re-locate by the
quoted anchor text, never by line number alone. If an anchor cannot be found, STOP (protocol).

## Environment facts (as of authoring, 2026-07-08)

- Repo: `f:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp`, branch `dev`, clean tree.
  Your base is the docs-only commit that ADDED this bundle (find it:
  `git log --oneline -1 -- docs/tasks/tab-identity/BOOTSTRAP.md`); every SOURCE anchor in the
  task files was verified at its parent, `c49ee6d` (the bundle commit touches only docs/).
  Windows 11; PowerShell primary; bash available.
- Workspace (ADR-0046): root facade crate `ghostlight` (src/lib.rs re-exports; src/main.rs CLI)
  + `crates/transport` (`ghostlight-transport`) + `crates/core` (`ghostlight-core`)
  + `crates/adapter-agent` + `crates/adapter-browser`. Root `Cargo.toml` depends on BOTH
  `ghostlight-core` and `ghostlight-transport`, so files in `tests/` may use
  `ghostlight_transport::...` paths directly.
- The two adapters depend on `ghostlight-transport` ONLY. Never add a `ghostlight-core`
  dependency to either adapter crate. (Load-bearing: ADR-0046.)
- Extension JS is plain MV3 (no build step). `node` is available. Extension unit tests run with
  `node --test tests/extension/grouping.test.js`.
- ASCII ONLY in every file you write (code AND docs): no emoji literals (write `\u{1F47B}`
  escapes), no em-dashes (use `--`), no arrows or curly quotes.
- Do not push. Do not run any install/uninstall (not even dry-run is needed), `schtasks`, or
  `systemctl` command. Do not start `ghostlight service` outside the pinned test commands (the
  integration tests spawn their own).

## Task sequence (strict order; every prefix leaves a coherent, green tree)

| Task | File | One line |
|---|---|---|
| T1 | `T1-managed-surface-predicate.md` | Extension tool gate accepts every Ghostlight-managed group (ADR-0047 D1). |
| T2 | `T2-transport-down-classifier.md` | Service-side read errors reconnect instead of exiting (D6). |
| T3 | `T3-stable-session-guid.md` | One SessionGuid per adapter process, re-presented on reconnect (D2). |
| T4 | `T4-session-scoped-tab-operations.md` | `guid` on the tool envelope; tabs birth into the session's group (D3). |
| T5 | `T5-client-name-titles.md` | Client-name group titles + recovery-steering tab errors (D4). |
| T6 | `T6-ownership-liveness-gc.md` | Dead-owner tab adoption + sessionGroups pruning + CHANGELOG (D5). |

## Per-task procedure

1. Read the task file fully. Re-read every tree location it names (the anchors), and check every
   STOP precondition. Any failed precondition -> failure protocol.
2. Implement exactly the pinned changes. No drive-by refactors, no extra cleanups, no renames
   beyond those pinned.
3. Add the pinned tests BY NAME with the pinned assertions (PINS transcribes the oracles; you
   transcribe, never re-derive).
4. Run the task's verification block (every command; all green).
5. Commit TWICE: first the code commit with the task's pinned message (stage ONLY the files the
   task names), then update `LEDGER.md` (status, commit hash, deviations) and commit it as
   `docs(tab-identity): ledger T<n>`.
6. Move to the next task.

## Verification (V-ALL; run per task unless the task narrows it)

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets
node --test tests/extension/grouping.test.js
```

Plus, when the task touched extension JS:

```
node --check extension/service-worker.js
node --check extension/lib/grouping.js
```

## Failure protocol

If a STOP precondition fails, a pinned anchor is missing, verification cannot go green, or the
task is underdetermined: (1) commit a BLOCKED entry to `LEDGER.md` FIRST (what blocked, exact
error text, your reasoning); (2) `git checkout -- . && git clean -fd` to drop the task's partial
work (the ledger commit survives); (3) HALT the batch. Do not skip ahead.

## NEVER touch (no exceptions unless a task names its single sanctioned one)

- `extension/manifest.json` (carries the pinned dev key).
- `crates/core/src/browser/directory.rs` -- the sacred tool schemas and descriptors. NO task in
  this batch edits it.
- `tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`.
- `docs/tasks/hub/**` and every other `docs/tasks/**` directory except `docs/tasks/tab-identity/`
  (history is never edited; ADR-0047 carries the supersessions).
- `src/main.rs`, `crates/adapter-agent/**`, `crates/adapter-browser/**`, `packaging/**`,
  `.github/**`, `site/**`, `scripts/**`.
- `extension/popup.*`, `extension/options.*`, `extension/content.js`,
  `extension/agent-visual-indicator.js`, `extension/lib/` files other than `grouping.js`.
- Any org-policy path or supervisor registration logic.
- Existing test FILES not named by a task (adding new test files/functions named by a task is
  the sanctioned path).

## Completion criteria

All six tasks committed (12 commits: 6 code + 6 ledger), LEDGER RESUME HERE = COMPLETE, V-ALL
green at the final tree, zero NEVER-touch files modified (verify:
`git diff --name-only <base>..HEAD` contains none of them), and no commit touches files outside
its task's named set.
