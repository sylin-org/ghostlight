# Latest result

## [0023] BLOCKED: frozen Linux product lane passes; three release runners need owner disposition

Freeze binding passed at docs-only head `75a1540c` against frozen revision `e7d8986b`. The exact
source passed formatting, warnings-denied workspace/all-target Clippy, 400 Rust tests, 137
extension tests, 10 npm tests, 4 MCPB tests, fresh isolated build, process/CLI/PowerShell/policy/
workbench journeys, shell and JavaScript syntax, dependency policy plus the 17 accepted audit
warnings, repository integrity, public truth, and complete 0.8 recovery.

The optimized revision-qualified user candidate was deployed without removing its predecessor.
Ordinary visible Chromium connected through the exact candidate, all 41 Foundry beats passed, and
the exact 24-tool catalog plus a separate `policy_explain` call passed. The frozen extension was
unchanged and not reloaded.

Three frozen release-tooling findings keep the lane BLOCKED pending owner disposition:

1. `release-preflight.ps1 -TargetDirectory <custom>` restores `GHOSTLIGHT_BIN_DIR` before queued
   journeys run, so those journeys silently use the default target. Direct exact-bin runs pass.
2. `-IncludeDependencyGates` runs broad `cargo deny check`, contradicting the authoritative split;
   the documented deny command and `cargo audit` both pass.
3. Both Foundry runners omit the new 24th tool, `policy_explain`, while claiming the whole catalog;
   the missing call passes separately.

Evidence is commit `75a1540c` and
`docs/testing/frozen-source-cachyos-verification-2026-08-25.md`. No product fix, extension change,
main merge, tag, upload, submission, publication, or release occurred.
