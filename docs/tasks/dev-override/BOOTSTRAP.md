# dev-override batch -- BOOTSTRAP

You are the EXECUTOR of a prepared, red-teamed task batch implementing ADR-0048 (the development
override: a live dev instance shadows the default for unpinned clients) in the Ghostlight repo.
You follow instructions literally and resolve nothing by judgment. When something is
underdetermined, you BLOCK (failure protocol below); you never improvise around a broken
assumption.

## Authority order

When documents disagree, the higher one wins; report the disagreement in the ledger either way:

1. The task file you are executing (`T<n>-*.md`).
2. `PINS.md` (this directory) -- pinned oracles: exact strings, formats, signatures, test names.
3. `docs/adr/0048-development-override.md` -- the normative design. Semantics live THERE; task
   files cite it and never restate it.
4. The live tree.

Line numbers in task files are as-of-authoring hints. ALWAYS re-locate by the quoted anchor text,
never by line number alone. If an anchor cannot be found, STOP (protocol).

## Environment facts (as of authoring, 2026-07-09)

- Repo: `f:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp`, branch `dev`, clean tree. Your
  base is the docs-only commit that ADDED this bundle (find it:
  `git log --oneline -1 -- docs/tasks/dev-override/BOOTSTRAP.md`); every SOURCE anchor in the
  task files was verified at its parent, `3928a74` (the bundle commit touches only docs/).
  Windows 11; PowerShell primary; bash available.
- Workspace (ADR-0046): root facade crate `ghostlight` (src/lib.rs re-exports; src/main.rs CLI)
  + `crates/transport` (`ghostlight-transport`) + `crates/core` (`ghostlight-core`)
  + `crates/adapter-agent` + `crates/adapter-browser`. Root `Cargo.toml` depends on BOTH core and
  transport, so files under `tests/` may use `ghostlight::...` re-export paths (see
  `tests/hub_identity.rs` for the `ghostlight::native::ipc::` style).
- The two adapters depend on `ghostlight-transport` ONLY. Never add a `ghostlight-core`
  dependency to either adapter crate. (Load-bearing: ADR-0046.)
- Integration tests that spawn the adapter/service DELIVERABLE binaries by path require a fresh
  `cargo build --workspace` first: `cargo test` rebuilds test harnesses, NOT the sibling
  deliverable bins (lesson pinned from the tab-identity batch, its ledger T3 deviation 1). The
  verification blocks below bake this in -- run the commands in the order given.
- Extension JS is plain MV3 (no build step). `node` is available. Extension unit tests run with
  `node --test tests/extension/grouping.test.js`.
- ASCII ONLY in every file you write (code AND docs): no emoji literals, no em-dashes (use `--`),
  no arrows or curly quotes.
- Do not push. Do not run `ghostlight install`/`uninstall` (not even dry-run), and do not run any
  `schtasks` or `systemctl` command. Do not start `ghostlight service` outside the pinned test
  commands (the integration tests spawn their own on test-unique endpoints).

## Task sequence (strict order; every prefix leaves a coherent, green tree)

| Task | File | One line |
|---|---|---|
| T1 | `T1-agent-override-resolution.md` | Selection tri-state + candidate endpoints + the agent adapter resolves dev-first (ADR-0048 D1/D2/D3). |
| T2 | `T2-browser-adapter-resolution.md` | The browser adapter probes candidates and picks the first live one (D4). |
| T3 | `T3-extension-single-host.md` | The extension always targets the one `org.sylin.ghostlight` host; instance badges dropped (D5). |
| T4 | `T4-installer-unified-surface.md` | Unified host manifest (both shipped extension ids), optional `--extension-id`, dev install thinned (D5/D6). |
| T5 | `T5-doctor-docs-changelog.md` | Doctor's override line + DEV-LOOP/README/CHANGELOG (D7). |

## Per-task procedure

1. Read the task file fully. Re-read every tree location it names (the anchors), and check every
   STOP precondition. Any failed precondition -> failure protocol.
2. Implement exactly the pinned changes. No drive-by refactors, no extra cleanups, no renames
   beyond those pinned.
3. Add the pinned tests BY NAME with the pinned assertions (PINS transcribes the oracles; you
   transcribe, never re-derive).
4. Run the task's verification block (every command, in order; all green).
5. Commit TWICE: first the code commit with the task's pinned message (stage ONLY the files the
   task names), then update `LEDGER.md` (status, commit hash, deviations) and commit it as
   `docs(dev-override): ledger T<n>`.
6. Move to the next task.

## Verification (V-ALL; run per task unless the task narrows it)

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace --no-fail-fast
cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets
node --test tests/extension/grouping.test.js
```

(`cargo fmt` without `--check` first is fine to normalize your own new code; `--check` is the
gate. `cargo build --workspace` MUST precede `cargo test` -- see Environment facts.)

Plus, when the task touched extension JS:

```
node --check extension/service-worker.js
node --check extension/popup.js
node --check extension/options.js
```

## Failure protocol

If a STOP precondition fails, a pinned anchor is missing, verification cannot go green, or the
task is underdetermined: (1) commit a BLOCKED entry to `LEDGER.md` FIRST (what blocked, exact
error text, your reasoning); (2) `git checkout -- . && git clean -fd` to drop the task's partial
work (the ledger commit survives); (3) HALT the batch. Do not skip ahead.

## NEVER touch (no exceptions unless a task names its single sanctioned one)

- `crates/core/src/browser/directory.rs` -- the sacred tool schemas. NO task edits it.
- `tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`.
- `extension/manifest.json` (carries the pinned dev key).
- `extension/popup.html`, `extension/options.html`, `extension/content.js`,
  `extension/agent-visual-indicator.js`, `extension/lib/**` (T3 edits ONLY the three JS files it
  names: service-worker.js, popup.js, options.js).
- `crates/core/src/governance/**`, `crates/core/src/hub/session.rs`,
  `crates/core/src/mcp/**` -- no task in this batch touches session identity or the pipeline.
- `src/main.rs` EXCEPT T4's single pinned help-comment line.
- `docs/tasks/**` other than `docs/tasks/dev-override/` (history is never edited).
- `packaging/**`, `.github/**`, `site/**`, `scripts/**`.
- `tests/adapter_reconnect.rs` (its env-pinned behavior is a regression guard for this batch --
  it must pass UNCHANGED).

## Completion criteria

All five tasks committed (10 commits: 5 code + 5 ledger), LEDGER RESUME HERE = COMPLETE, V-ALL
green at the final tree, zero NEVER-touch files modified (verify:
`git diff --name-only <base>..HEAD` contains none of them), and no commit touches files outside
its task's named set.
