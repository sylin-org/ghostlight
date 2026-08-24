# Pre-freeze debt batch -- ground rules

Created 2026-08-24. The owner promoted four `STATUS.md` "Owed" items into the window between
the store submission and the G0 source freeze. Google's review latency is the work window;
none of it may touch the Chrome extension, because the pending STAGED_PUBLISH review covers
bytes built from `extension/` at `70869631`.

## Invariants

1. `extension/` must not change between `b9a017a1` and the freeze. Verify at freeze time:
   `git diff --stat b9a017a1..HEAD -- extension/` prints nothing.
2. One task = one commit. Every commit passes the full AGENTS.md gate stack: fmt, clippy
   `-D warnings`, `cargo test --workspace`, `npm test` from `extension/` (regression guard,
   expected untouched), `node --check` on changed JavaScript, plus the journeys the change
   touches (process-journey for connector seams).
3. Authority order: AGENTS.md, the `docs/1.0/` contracts, ADRs, then this batch's LEDGER.
   A task that cannot complete reverts its working tree and records BLOCKED in the LEDGER
   with reasoning; it does not half-land.
4. Never touch: `reference/`, `/private/`, `saps/`, `local/`, anything outward-facing.
   Never weaken `docs/trust/` claims. No phone-home. ASCII only, docs included.
5. Published-count claims move in lockstep with code. When the catalog grows, the same
   commit updates every active-truth document that states the count (LANGUAGE, ARCHITECTURE,
   ACCEPTANCE, STATUS, README) and every pinned test.

## Task order

- T1: `crates/mcp-connector` adopts the shared service handshake from `crates/bridge`.
  The duplicate hello/catalog negotiation dies; the connector's event pump, cancellation
  forwarding, and concurrent-invoke correlation stay local because they are edge concerns.
- T2: unsettled readiness rows get a color treatment beside the existing running/blocked ones.
- T3: model-facing policy explain joins the catalog per a new ADR (ADR-0122 Decision 9
  deferred it here). Catalog grows 23 -> 24; EMPTY requirement class keeps it always available.
- T4: ADR-0105 stages 2 and 3 through one new audited FFI crate (the only sound shape for a
  scoped relaxation of the workspace `unsafe_code` forbid). Stage 3 ships default-off.

Freeze follows the last landed commit; `b9a017a1` remains the extension-byte floor.
