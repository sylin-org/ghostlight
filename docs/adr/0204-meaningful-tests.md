# ADR-0204: Meaningful tests

Date: 2026-10-21. Status: Accepted by owner direction. On 2026-10-21 the owner judged the project
bloated with poorly written, low-value tests, called 2,850 Rust tests unconscionable and set 500 as
the most, and named the mandate: "I want the mandate to be 'meaningful tests', not 'massive number
of tests'." The same day the owner chose a JavaScript ceiling as well, the cut before 2.0.1, lanes
on the test machines, and a guard.

## Context

On 2026-10-21 the workspace held 2,851 Rust tests (2,431 of them in the service), 553 extension
tests, 275 page-asset tests and 286 golden and fixture files. The extension's test code was longer
than the extension. The same day showed what that bought:

- Three defects passed the whole suite. A script's exception reached the agent as "never
  confirmed", and tests asserted that sentence (L-D6). The service and the extension counted
  characters in different units, and each side tested only its own limit (P-D4). Half an emoji at
  the end of a cut text made a frame the service could not read, and the service closed every
  session's browser connection; one live call found it (P-D5).
- One title prefix changed about 20 test sites that restated it.
- Two tests failed under load because they synchronized with a fixed sleep and an 80 ms real-time
  deadline.

Many tests restate implementation detail: literal text, shapes, counts, private helpers. Few test
the boundaries where the failures were.

## Decision

1. **The mandate is meaningful tests.** A test is written, kept or deleted for what it protects,
   never to add to a count. A test is meaningful when it does one of four things:
   - it proves a user promise: an ACCEPTANCE item or a contract in `docs/1.0/`;
   - it guards a contract between two components: the wire, a golden both sides read, the
     installer's writes, the model surface;
   - it reproduces a defect that was fixed, named in a ledger, an ADR or a commit;
   - it covers `unsafe` or FFI code (`crates/win-peer`, the `os` module of `crates/relay`).

   A test that restates a literal the code defines, tests a private helper a meaningful test
   already covers, duplicates another, pins a count, or can fail only when someone changes the code
   on purpose is not meaningful.
2. **A meaningful test is well written.** It says what it protects, in its name or its doc comment.
   It waits on a condition, never on a fixed sleep, and a real-time deadline in it only bounds a
   failure. A table-driven test is one test when its cases are one behavior. Unrelated assertions
   are not merged into one test to save a count.
3. **A ceiling keeps the mandate honest.** The workspace holds at most 500 Rust tests: `#[test]`
   functions under `crates/`, inline modules and `tests/` directories alike. The extension's and
   the page assets' node tests get a ceiling of their own, which the coordinator proposes after the
   Rust cut and the owner sets in an amendment. Journeys and guards under `tests/` are outside both
   ceilings and follow Decisions 1 and 2. A new test replaces a weaker one, or the owner raises the
   ceiling.
4. **A guard enforces the ceiling.** `tests/guards/test-budget.test.mjs` fails when the workspace
   holds more than 500 Rust tests. It lands in the cut's last slice, once the count is under the
   ceiling.
5. **The cut runs before 2.0.1** as batch [tests-1](../tasks/tests-1/BOOTSTRAP.md), in lanes on the
   test machines. arch-1's open refactor slices wait for it, and its open defect slices go first.

## Consequences

- About 2,350 Rust tests go, and most of the test code with them. CI and local runs get faster, and
  a change touches only the tests that protect what it changes.
- Coverage of internals drops on purpose. The cut adds the boundary tests it finds missing, within
  the ceiling.
- No agent adds a test by default: each new one says what it protects and what it replaces.
