# T07: dead-seam and stub deletions

## Goal

Implement ADR-0024 Decision 5: delete the four unwired port seams (`DomainPolicy`,
`ResourceResolver`, `ToolId`, `ResourcePattern`) and the `src/browser/tools/` doc-stub
subtree; rewrite the module docs that referenced them; optionally split the shrunken
`ports.rs` (accepted follow-up in ADR-0024's provenance). Pure deletion/tidy: zero
behavior change, zero new capability.

## Authority

ADR-0024 Decision 5 is normative. Where this prompt and the ADR disagree, THE ADR WINS.

## Depends on

t01-t06 landed (the deletions must not race earlier tasks' edits to the same files).
STOP if LEDGER RESUME HERE does not show t06 committed.

## Current behavior (verified 2026-07-03 against `b4b2faf`; re-verify -- t03/t06 edited
ports.rs and dispatch.rs since)

- `src/governance/ports.rs`: `DomainPolicy` (trait, no impl anywhere; its `requires`
  method was made shape-consistent in stage 3 but nothing consumes it),
  `ResourceResolver` (async-fn-in-trait, declared, never implemented or consumed),
  `ToolId` and `ResourcePattern` (unused placeholder newtypes). Verify each is
  reference-free beyond its own definition/docs/tests before deleting:
  `rg -n "DomainPolicy|ResourceResolver|ToolId|ResourcePattern" src/ tests/`.
- `src/browser/tools/`: 11 doc-comment-only stub modules (zero functions), whose
  `mod.rs` doc table restates all 13 tool names AND classifies them with the DELETED
  observe/mutate scheme (stale since stage-3 s06). `src/browser/mod.rs` declares
  `pub mod tools;` and its module doc names it.

## Required behavior

1. Delete the four items from `ports.rs`, including their doc references and any inline
   tests that exist only to exercise them. If a test uses one incidentally (as a stub
   type), retype it onto what production actually uses; never weaken an assertion.
2. Delete `src/browser/tools/` entirely; remove `pub mod tools;` from
   `src/browser/mod.rs`; rewrite the roster sentence in that module doc (the registry
   in [`directory`] is the per-tool authority; there are no per-tool code homes).
3. Sweep the module docs: `rg -n "DomainPolicy|ResourceResolver|observe/mutate|Observe tier|Mutate tier" src/`
   and rewrite every survivor to name the current authority (the registry, ADR-0024).
4. OPTIONAL (skip if the task is running heavy, and say so in the ledger): if
   `ports.rs` still exceeds ~600 lines, split it into a `ports/` directory
   (`ports/mod.rs` + two or three cohesive submodules, e.g. decision vs audit types)
   with `pub use` re-exports so EVERY existing `crate::governance::ports::X` path keeps
   compiling unchanged (zero call-site edits; the arch test still sees the same
   `src/governance/` prefix).

## Constraints

1. One commit: `chore(architecture): t07 dead-seam and stub deletions`.
2. Zero behavior change: the full suite passes with zero expectation edits; tools.json/
   fidelity untouched; all-open goldens untouched; `tests/architecture.rs` green.
3. Deletion completeness: the section-3 rg plus
   `rg -n "browser/tools|browser::tools" src/ tests/` return nothing afterward.
4. ASCII; no new dependencies.

## Tests (minimum)

No new tests (deletions). The suite must stay green with an UNCHANGED total count
except tests deleted WITH their subjects (list each deleted test by name in the ledger,
with the subject it died with).

## Verification

fmt/clippy/test green; the completeness rgs; ASCII scan; `git diff --stat` shows only
ports.rs (or ports/), dispatch.rs (doc refs), browser/mod.rs, the deleted subtree, and
LEDGER. Ledger entry lists every deleted item (the deletions ledger is a stage
deliverable, BOOTSTRAP Completion); RESUME HERE -> t08; commit. No browser checks.

## Out of scope

- Any addition of any kind; any behavior or message change; docs outside module docs
  (t08); re-introducing a resolver/policy trait (a future remote-PDP ADR owns that).
