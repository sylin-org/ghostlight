# LEDGER -- release streamlining

One task = one logical commit set. This ledger is the authority on progress.

## RESUME HERE

Nothing. All five tasks are complete; the batch is closed.

## Tasks

### P1 -- scripts/release-preflight.ps1

Status: COMPLETE (windows-codex, 2026-08-24). Validated by a full green run at HEAD
(14 passed / 0 failed / 3 honest skips) with the dated evidence record written to
docs/testing/. Deviations recorded: the dependency-gates stage skips by default because
local cargo-deny reports the 17 known GTK/Tauri advisory allowances that this process
deliberately accepts and rechecks against the frozen graph -- run with
-IncludeDependencyGates to see them fail loudly here too; CI-only rows (repository truth,
documentation links, ASCII, 0.8 recovery detail) are MANUAL rows in the generated skeleton.

### P2 -- machine-readable freeze declaration

Status: COMPLETE (windows-codex, 2026-08-24). scripts/declare-freeze.ps1 writes
docs/release/freeze.json bound to a full sha (-Force to redeclare); assert-freeze.ps1
verifies HEAD-or-given revision equals it AND refuses on a dirty tree. The preflight runs
assert-freeze as an automatic stage once the declaration exists; verify-custody binds the
candidate manifest to it before anything else.

### P3 -- candidate custody verifier

Status: COMPLETE (windows-codex, 2026-08-24). scripts/verify-custody.ps1 <dir>: freeze-to-
manifest binding, deep check-release-candidate invocation, SHA256SUMS recomputation against
bytes on disk (exactly 17 lines), optional GitHub provenance attestation via gh
(-IncludeProvenance), and the printed custody instruction (copy locally, re-verify from the
copy).

### P4 -- store-submission ordering rule

Status: COMPLETE (windows-codex, 2026-08-24). RELEASE-CHECKLIST G3 now carries the hard
ordering rule: G2 custody precedes any store submission or resubmission, because staged
reviews go stale the moment the package changes.

### P5 -- journey artifact hygiene

Status: COMPLETE (windows-codex, 2026-08-24). Swept 146 straggler runtime/lock/audit files
from tests/ and added tests/.gitignore pinning `.ghostlight-*` so future leftovers stay
invisible to status.

## Evidence

- (appended per task)
