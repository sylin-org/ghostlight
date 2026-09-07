# H8: Local service continuity

The owner approved the revised continuity experience and directed implementation on 2026-09-07.
[ADR-0160](../../adr/0160-local-service-continuity.md) records the accepted behavior and bounds.

## Current source

- `service/admission.rs` owns counted connection permits, bounded session queues and memory charges.
  Two fixed workers replace a new OS thread for every invocation. Cancellation and expiry advance
  queued completion; duplicate IDs leave the original token intact.
- `work/mod.rs` prepares once at intake, preserves the deadline and the single completion path,
  and publishes sustained waiting through the existing workbench projection. Capacity failures
  carry typed language and minimized audit. Runtime controls still apply immediately before effects.
- `bridge/transport.rs` bounds total handshake, partial-frame and write time while allowing idle
  sessions. It preserves coalesced bytes across browser negotiation and permits shutdown without
  acquiring a blocked writer. Browser connection replacement wakes the old heartbeat worker.
- `workbench` and its existing UI distinguish waiting and running under the same invocation. Queued
  rows stay live below active work, with no popup. The phase transition does not create a new row.
- `bridge/runtime.rs` publishes a new private file rather than trusting inherited or pre-existing
  temporary-file permissions. Windows creation uses the existing audited FFI crate. Linux retains
  mode 0600 with exclusive creation; runtime reads now have a small fixed bound.

No authored policy, connector wire format, extension mechanism, version, or release artifact changes.

## Validation

The new `tests/local-resilience-journey.mjs` starts a fresh real service from the explicitly selected
build directory, with isolated audit, policy, diagnostics and native-host paths. It uses synthetic
browser and client peers, not an installed profile or real Sylin browser content. It checks:

1. Windows replacement of an inherited-access runtime file produces a protected DACL without broad
   Everyone, Authenticated Users, or Users rules. Token values are never printed.
2. Abandoned and partial handshakes on both listeners expire. Malformed and oversized messages close
   their connection. Healthy idle clients and another working session survive.
3. Coalesced relay and adapter hello frames negotiate successfully.
4. Duplicate-ID rejection preserves cancellation of an already dispatched operation, which reports
   uncertainty without replay.
5. A 12-request ordinary burst completes without manual retry.
6. A full ordinary queue leaves independent status and another workspace usable. A queued request
   can be cancelled or expire without waiting for the active operation and without browser effects.
7. Human Pause/Resume remains responsive under saturation; queued work stops before dispatch and
   new work succeeds after recovery.
8. A client that stops reading is retired; another session and a fresh connection continue working.

Rust regressions cover absolute frame clocks, buffered partial frames, idle readers, service stop,
stalled writes, lock-independent shutdown, duplicate tokens, FIFO burst admission, independent
capacity, and reservation release. Existing runtime replacement tests also preserve an unrelated
legacy temporary file and check mode 0600 on Linux. Workbench projection and UI checks exercise the
waiting-to-running transition, no duplicate row, and no waiting notification.

Final validation passed on the implementation source:

- `cargo fmt --check`, workspace Clippy with `-D warnings`, and all 498 Rust tests.
- All 192 extension tests and syntax checks on every changed JavaScript/test file.
- The real-process reconnect/audit journey, H8 isolated resilience journey, and CLI journey.
- 60 workbench surface checks, policy grammar checks, and the actual bundled UI in isolated
  Chromium, including narrow layout and waiting-to-running behavior. `.tmp/h4-waiting.png`
  was visually inspected; the activity list no longer labels waiting work as earlier work.
- All 29 Sylin/MV3 regressions with Chrome for Testing 152.0.7977.82 and freshly built binaries.
  This separate lane exercises public Sylin composed content and its local embedded-form simulation;
  native-port discovery is substituted, with real connectors, MV3 worker, content scripts and CDP.

The final review also reproduced repeated guardrail notifications when human Pause/Stop drained
queued requests. A regression now proves three quiet, blocked/no-effect receipts for each control,
with no browser commands or repeated notifications. Real policy denials retain their notices.

One local commit: `fix(service): absorb bursts and bound local exchanges`; Git owns its hash.

## Limits and deployment

This is local source work. The installed runtime has not been replaced by this cycle. H6/H7 also
remain undeployed. The ACL observation used metadata only; owner-local notes and token contents
were not read into output. ACL assertions are Windows evidence, not a second-user impersonation
test. Linux runtime/process execution remains untested in this Windows session. No resource
exhaustion attack, total process-memory guarantee, host containment, or installed-browser proof is
claimed. Caller-owned workspace lifetime remains ADR-0106's contract.
