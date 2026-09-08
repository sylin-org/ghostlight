# Historical reader compatibility, 2026-09-08

The owner clarified a product-wide resilience rule: be lenient toward harmless variation and
keep unaffected work available. MEMORY owns that standing directive. ADR-0163 records its
application here; this run does not establish resilience for every product boundary.

## Change

Historical receipt and recovery-gap readers accept unknown additive fields. Typed serialization
omits them, so unknown payloads are not copied into new audit or projections. Known required
meanings still validate. Existing H7 behavior preserves unsupported lines on disk, counts omitted
records, keeps readable neighbors, and starts the service without making old history a new write
failure. Active policy, live protocols, and operation inputs are unchanged.

## Actual executable evidence

Windows ran `tests/history-compatibility.mjs` using the fresh `.target-dev-loop/debug` build and
the public 1.3.4 Windows executable, verified against that release's SHA256SUMS:

- Public 1.3.4 SHA-256: `5ed7c8080b71983d97f4fc33105f4c912c2fca6cc435884f7cc76bd065186bb7`.
- Current 1.3.5 SHA-256: `755503712f8fab01ff8850a2a343e91864e3ee0becf9e67eaa641d91369ad4d3`.
- Report: `.tmp/history-compatibility/run-fsRPfR/results.json`.

The real predecessor starts its desktop authority and writes a receipt through its CLI intake.
The candidate loads that exact file, completes new policy inspection, and saves its own receipt.
A second candidate start does the same. All earlier bytes remain identical after each phase.
There is no browser fixture or replacement service in this test; no browser acceptance is claimed.

Separate format tests introduce an unknown optional field and an unsupported required reason.
The optional field remains readable; only the unsupported record is counted as unreadable.
Two more real authority starts and calls succeed and save, preserving the entire original prefix.
These mutations prove the reader rule, not compatibility with an actual future executable.
Rust tests also assert that unknown payloads cannot survive typed reserialization and that known
neighbors and extended gap markers remain readable.

Earlier failed test attempts remain under `run-JF5ulP` and `run-2vrzSO`. They supplied an absent
explicit policy file and an absent explicit runtime-control file respectively. The final driver
creates a valid isolated test policy and active control file; no user settings were changed.

Formatting, warnings-denied workspace Clippy, workspace Rust tests, and all 207 extension tests
passed. The driver is committed and included in the full/process hardening lanes. Its default
run explicitly records that no predecessor was exercised unless one is supplied.
The existing process journey also passed against those exact fresh binaries, including cold
history failure, strict audit admission, automatic write recovery, retained effects, and no replay.
Log: `.tmp/history-compatibility-process.log`.

No deployed binaries, extension bytes, publication metadata, or published 1.3.4 reader changed.
The known 1.3.4 failure reading 1.3.5 history still needs a compatible delivery/storage strategy.
