# LEDGER -- handle continuity

One task = one logical commit set. This ledger is the authority on progress.

## RESUME HERE

T1 (idle wake-and-retry at the port seam) is next. Nothing has started; the batch was
authored 2026-08-24 at dev `64c2ba3b` or later.

## Tasks

### T1 -- idle wake-and-retry

Status: NOT STARTED.

Seam: the browser-port chokepoint where a pre-dispatch empty-or-unavailable roster is
detected today (the spot that answers "No browser is connected"). One bounded recovery
attempt inside the invocation budget, then the existing honest refusal if it genuinely
fails. Regression test: fake relay absent on first probe, present on second; caller sees
one succeeded call, not an error.

Deviations: none yet.

### T2 -- tab-handle continuation

Status: NOT STARTED.

Depends on T1 only in spirit; can run in parallel. Key seams: binding resolution (where
TabUnavailable-class refusals originate), the OpenPage path for governed recreation,
same-handle rebind on recovery, per-tool semantics per BOOTSTRAP D1. New regression tests:
navigate-to-dead-tab recreates and rebinds with repeat_safe false; close-of-dead-tab
succeeds as already-gone; focus-of-dead-tab recreates and brings it forward.

Deviations: none yet.

### T3 -- language and guidance

Status: NOT STARTED.

LANGUAGE.md gains the two-tier handle distinction (identity slots vs perception tokens).
The scripting guide's handle guidance flips from stash-and-hope to selectors plus durable
tab handles.

Deviations: none yet.

## Evidence

- (appended per task)
