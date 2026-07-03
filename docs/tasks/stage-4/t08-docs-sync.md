# T08: documentation sync

## Goal

Make the written record match the stage-4 architecture: hot-reload note and SPEC-updates
items in the stage-2 shared-format doc, one missed stage-3 banner, a consolidated
live-check pointer in BROWSER-TESTS, and the RUN SUMMARY. Docs-only: no code, no tests,
no SPEC.md edits. Last stage-4 task. Commit as `docs(architecture): t08 documentation
sync`.

## Authority

ADR-0023/0024/0025 are normative; the shared-format doc stays authoritative for whatever
they do not supersede. Banners and notes are pure insertions; never rewrite history.

## Depends on

t01-t07 landed. Check: `rg -n "pub mod pipeline" src/transport/mcp/mod.rs` matches, and
`rg -n "parse_org_config" src/` is empty. If either fails, STOP.

## Current behavior (verified 2026-07-03; re-read before editing)

- `docs/tasks/stage-2/00-shared-format.md`: section `### 1.3. User-supplied manifest`
  ends with the startup selection rule (manifests are startup-fixed; nothing mentions
  reload). Line ~282 (section 4 preamble, manifest field list) still reads
  "`schema`: integer, required. Stage 2 defines schema `2`..." -- an ADR-0022
  supersession the s08 banner pass missed (its banners covered 4.3/6.1/8 only). The
  `## 10. SPEC updates needed` list currently ends at item 17.
- `docs/tasks/stage-2/BROWSER-TESTS.md`: carries the stage-3 backlog (s-live-1..4) plus
  stage-4 per-task entries appended by t01/t05/t06 (verify their ids: t01-1, t05-1,
  t06-1, t06-2).
- `docs/adr/README.md`: as of authoring its table ends at the 0022 row -- the rows for
  0023/0024/0025 are ABSENT and appending them is this task's mainline work (Required
  behavior 0), not an edge case. (If a prior commit already added them, section 0 is a
  verified no-op.)

## Required behavior

### 0. ADR index rows (`docs/adr/README.md`; BOOTSTRAP's never-touch list names this
task as the owner of these rows)

Append exactly these three rows after the 0021/0022 rows, verbatim:

    | [0023](0023-one-loader-for-the-policy-file.md) | One loader for the policy file | Accepted |
    | [0024](0024-tool-registry-and-generic-ingest-pipeline.md) | Tool registry and the generic ingest pipeline | Accepted |
    | [0025](0025-manifest-hot-reload.md) | Manifest hot-reload | Accepted |

If any row already exists (a prior commit added it), skip that row; never duplicate.

### 1. Shared-format insertions (pure insertions, blank line before and after)

- Immediately after the section `### 1.3. User-supplied manifest`'s selection-rule text
  (before the next heading), insert:
  `NOTE (ADR-0025, docs/adr/0025-manifest-hot-reload.md): as of stage 4 the active manifest hot-reloads. The org policy path and a file:// user source are watched; the selection rule above is re-evaluated on every change (including file creation and deletion); an invalid edit keeps the last-good manifest (fail closed). The startup-fixed description below this point is retained as history for the stage-2 implementation record.`
- Immediately before the line defining `schema` as "Stage 2 defines schema `2`", insert:
  `SUPERSEDED by ADR-0022 (docs/adr/0022-intent-calibrated-capabilities.md): the schema version is 3; schema 2 never shipped and is rejected. Additionally, per ADR-0023 (docs/adr/0023-one-loader-for-the-policy-file.md), duplicate config keys in one manifest are a validation error. The text below is retained as history.`

### 2. Extend `## 10. SPEC updates needed` with items 18-20, verbatim

18. **One loader for the policy file (ADR-0023; SPEC 4.4).** The policy file has exactly
    one parser and one schema authority (the manifest parser, schema 3); org config
    layers derive from the parsed manifest's `config` entries; duplicate config keys are
    a validation error; every load path performs one parse per invocation or change.
19. **Tool registry and generic ingest pipeline (ADR-0024; SPEC 3, 5).** One per-tool
    descriptor table (capability variants, resource shape, handler kind, hooks) drives
    validity, classification, enforcement input, advertisement, explain, and result
    post-processing; governance owns audit-record selection through a per-call scope;
    the sacred check and grant path share one tab-URL resolution per call. The 13
    trained tool schemas plus `explain` remain byte-identical (ADR-0022 Decision 7).
20. **Manifest hot-reload (ADR-0025; SPEC 4.4, 2).** The org policy path and a file://
    user manifest source are watched; grants/mode/hash swap atomically per call
    snapshot; an advertised-set change emits `notifications/tools/list_changed`; policy
    transitions record `manifest_reload` / `user_manifest_ignored` session events;
    invalid edits keep the last-good manifest.

### 3. BROWSER-TESTS consolidated pointer

Append after the last existing entry:

    ## t-live-1: stage-4 regression pass (pipeline rewrite)
    Changed: stage 4 rewrote the dispatch pipeline (registry-driven, ADR-0024) with
    behavior pinned byte-identical by the test wall; only a live pass proves the wall had
    no holes.
    Steps: re-run the stage-3 backlog s-live-1 through s-live-4 unchanged against the
    stage-4 tree, plus t01-1, t05-1, t06-1, and t06-2.
    Expect: every expectation in those entries holds unchanged; any divergence is a
    stage-4 regression (file it against the pipeline rewrite, not the entry).

### 4. RUN SUMMARY

Per BOOTSTRAP Completion: tasks completed, commit range, every conservative choice,
the DELETIONS LEDGER (aggregate every task's "Deletions performed" list -- this stage's
deliverable is what got REMOVED), BROWSER-TESTS state, and the plain statement that
hot-reload and the loading fix are shipped-but-unverified-end-to-end until a human runs
the live backlog. No push, no merge.

## Constraints

1. Files touched: exactly `docs/adr/README.md` (section 0 rows only),
   `docs/tasks/stage-2/00-shared-format.md`, `docs/tasks/stage-2/BROWSER-TESTS.md`, and
   `docs/tasks/stage-4/LEDGER.md`. Nothing under `src/`, `tests/`, `examples/`,
   `extension/`; no `docs/SPEC.md`, no ADR body edits, no CLAUDE.md (its stale structure
   tree stays stale; out of scope).
2. Insertions only in the shared-format doc. ASCII in every added line.

## Tests (minimum, all rg from repo root)

- `rg -c "SUPERSEDED by ADR-0022" docs/tasks/stage-2/00-shared-format.md` prints `4`
  (the three s08 banners plus this task's schema banner).
- `rg -c "ADR-0025" docs/tasks/stage-2/00-shared-format.md` prints at least `1`.
- `rg -n "^20\." docs/tasks/stage-2/00-shared-format.md` prints one line (item 20).
- `rg -c "^## t-live-1" docs/tasks/stage-2/BROWSER-TESTS.md` prints `1`.
- `rg -c "0023-|0024-|0025-" docs/adr/README.md` prints `3`.

## Verification

`cargo fmt --check` / `clippy` / `cargo test` identical to the t07 run (nothing compiled
changed). All rg assertions above. ASCII scan of added lines
(`git diff -U0 | grep "^+" | rg -n "[^\x00-\x7F]"` empty). Ledger entry + RUN SUMMARY,
RESUME HERE marked complete, commit.

## Out of scope

- SPEC.md itself; ADR text; CLAUDE.md; README marketing copy; research docs; renaming
  anything; any code or test change.
