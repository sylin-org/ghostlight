# H7: Audit health

## Accepted behavior

The owner accepted Keep working by default, an optional Require audit policy, persistent health
indication, truthful result/storage separation, bounded automatic recovery, and explicit history
gaps without replay. [ADR-0159](../../adr/0159-audit-health-and-recovery.md) records the contract.

## One implementation task

Replace ignored audit failures and the projecting decorator with one shared recorder. Preserve
content-minimized receipts and action truth; add monotonic policy, final dispatch checks, human
and model projections, bounded storage recovery, gap markers, and tolerant history reconstruction.
Do not add another connector protocol or persistence queue. One logical commit carries this work.

## Exploration

- `work/receipt.rs`: direct, child, and preparation receipt completion.
- `governance/mod.rs`: existing safe record, reason codes, snapshots, and old JSONL sink.
- `service/mod.rs`: destination construction, browser-event receipts, and service lifetime.
- `workbench/mod.rs` and `history.rs`: old projecting decorator and bounded grouped restoration.
- `governance/documents.rs`: existing monotonic choice and effective-source pattern.
- `language/audit.rs`, `outcome.rs`, and `composition.rs`: existing safe language and progress.

The utilities catalog named by the explore skill is absent. Existing record, policy, completion,
and UI types were reused. New audit-health types live in the owning orchestrator modules.
The owner's Proceed authorized implementation after ideation; no further approval was needed.

## Verification

Implemented and verified locally on Windows, 2026-09-07. The H7 commit is
`feat(audit): expose storage health and enforce audit requirements`; Git owns its hash.

- `cargo fmt --check` passed.
- `cargo clippy --workspace --all-targets --target-dir .target-ghostlight-1.0 -- -D warnings` passed.
- `cargo test --workspace --target-dir .target-ghostlight-1.0 --quiet` passed: 488 tests, including
  410 orchestrator library tests. Ten new tests cover failing writes, cold failure, concurrent
  completions, bounded recovery, torn/oversized lines, actual file replacement, strict composition
  stopping, preserved effects, final-boundary admission, and parent/child storage separation.
- `npm test` in `extension/` passed: 192 tests. Changed JavaScript and MJS syntax, ASCII, and diff
  whitespace checks passed.
- A fresh workspace build into `.target-ghostlight-1.0` passed. `tests/process-journey.mjs` used
  those binaries and passed its existing H1-H6 checks plus H7's real destination failure, default
  continuation, strict refusal under observe mode, native human controls, five-second recovery,
  marker/no-backfill checks, and cold startup with unavailable history. The browser adapter is
  synthetic; the orchestrator and both connectors are real processes.
- `tests/workbench-surface.mjs` passed 58 checks, including authored audit choice, organization
  floor, persistent failure/gap notice, and quiet healthy state. `tests/policy-grammar.mjs` passed.
- `tests/workbench-history-browser.mjs` passed H4/H5 and new H7 checks in isolated Chromium using
  the actual bundled UI and synthetic projections: independent child storage, persistent health,
  recovery gaps, preserved expansion, and 720-pixel layout. The narrow screenshot was inspected.
  Local artifacts are `.tmp/h4-audit-unavailable.png` and `.tmp/h4-audit-recovered.png`.
- `tests/frame-browser-journey.mjs` passed all 29 Sylin/MV3 checks against the fresh H7 binaries
  with Chrome for Testing 152.0.7977.82. It uses the live Sylin iframe page plus copies on distinct
  local hosts. As in H6, only native-port discovery is replaced with a test loopback transport;
  installed native-host registration is not established by this lane.

The low-level writer keeps its append-only access requirement. Separators after startup or a
failed attempt isolate torn bytes without needing read access to the destination. The history
reader accepts blank separators and bounds retained line bytes at 1 MiB. Recovery retains one
content-free incident counter, not failed receipts. Raw storage error text is never projected.

## Remaining limits

Startup may preserve only the readable portion of old history. Repairing writes does not reload
an earlier unreadable portion or upgrade unconfirmed live receipts; restarting can reload the
file. Markers preserve known gaps only when they can themselves be synchronized. A crash during
an outage may lose the volatile gap count, and filesystem synchronization is not a guarantee
against every storage or power failure. Strict policy governs later dispatch; it cannot retract
an already-admitted browser effect.

No live deployment, installed native-host browser lane, Linux runtime check, push, or publication
is implied by the source tests. H6 deployment and its installed transport proof remain separate.
