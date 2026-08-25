# Release-tooling repairs proven on Windows -- 2026-08-25

Dispositions for the three findings in
[frozen-source-cachyos-verification-2026-08-25.md](frozen-source-cachyos-verification-2026-08-25.md).
All three were release-runner defects, not product defects: the frozen product binaries passed
every check on both hosts before and after these repairs. Scripts sit outside the freeze's
product-path set (see scripts/assert-freeze.ps1), so no re-freeze was required.

## Repairs

1. scripts/release-preflight.ps1 no longer restores GHOSTLIGHT_BIN_DIR at stage-definition time.
   Every journey stage now pins the environment to the runner's own -TargetDirectory while it
   executes, and the caller's environment is restored once after all stages finish.
2. scripts/release-preflight.ps1 -IncludeDependencyGates now runs the authoritative split from
   RELEASE.md -- cargo deny check licenses bans sources plus cargo audit, whose configuration
   carries the 17 accepted GTK/Tauri-chain allowances -- instead of a broad deny invocation that
   failed on that accepted set.
3. scripts/demo-foundry.ps1 and scripts/demo-foundry.sh gained an "explain policy" beat:
   policy_explain must succeed with its authored summary sentence, so "whole catalog rehearsed"
   is true again at 24 tools.

## Proof on this host

- Full preflight against a FRESH custom target directory with dependency gates enabled:
  pwsh scripts/release-preflight.ps1 -TargetDirectory .target-preflight-check -IncludeDependencyGates
  -- 16 stages passed, 0 failed, 1 skipped (shell syntax, POSIX-only). The four journey stages ran
  against .target-preflight-check/debug binaries, which is exactly the scenario finding 1 showed
  silently verifying stale binaries before; the dependency stage passed with the split commands,
  which is exactly the shape finding 2 showed failing.
- Live foundry rerun against the deployed frozen graph including the new beat: recorded below.
- Live foundry rerun on this host against the deployed frozen graph: all 42 beats green,
  including the new line
  "explain policy   succeeded   Explained current authority across 4 capability areas over 0 layers."
