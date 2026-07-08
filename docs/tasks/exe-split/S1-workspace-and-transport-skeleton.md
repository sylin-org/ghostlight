# S1: Workspace + ghostlight-transport skeleton

Goal: turn the single-package repo into a workspace and add an EMPTY `ghostlight-transport`
member the root package depends on. No code moves yet. Smallest possible green first step.

## STOP preconditions

- `git status --porcelain` is not empty -> STOP.
- `git merge-base --is-ancestor fccca60 HEAD` fails -> STOP (wrong base).
- `git diff --stat fccca60..HEAD -- src crates tests Cargo.toml Cargo.lock .github packaging scripts extension site`
  prints ANY file -> STOP (something outside docs/ changed since the authored base).
- Root `Cargo.toml` already contains a `[workspace]` table -> STOP.

## Required changes

1. Root `Cargo.toml`: append the `[workspace]` table exactly as SPEC section 1 pins it
   (members incl. `"."` and all four crate paths -- the not-yet-existing members are created in
   this task for transport and in S4/S5/S6 for the rest, so ONLY list `"."` and
   `"crates/transport"` NOW; later tasks append their own member lines).
2. Create `crates/transport/Cargo.toml`: package `ghostlight-transport`, version `0.3.0`,
   edition 2021, `publish = false`, `license = "Apache-2.0 OR MIT"`,
   description "Ghostlight transport: the stable substrate shared by the role executables (ADR-0046)."
   Dependencies: NONE yet (S2/S3 add them as code arrives).
3. Create `crates/transport/src/lib.rs` containing only the SPDX header + the crate doc comment
   from SPEC section 2's lib.rs listing (no modules yet, no init_tracing yet).
4. Root `[dependencies]` gains `ghostlight-transport = { path = "crates/transport" }`.
   (It is momentarily unused; silence nothing -- an unused DEPENDENCY is not a warning.)

## Tests

None added. The oracle is: the existing suite is untouched and green.

## Verify (literal)

SPEC section 12, all four commands. Expected: identical results to the base commit (same test
counts, zero failures).

## Out of scope

Moving ANY code. Touching src/. Creating the other three crates.

## Commit

`chore(workspace): add the cargo workspace + empty ghostlight-transport member (S1)`
