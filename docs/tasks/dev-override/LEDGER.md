# dev-override batch -- LEDGER

Durable execution record. One task = one code commit + one ledger commit. Update after EVERY
task (or block); this file is the single source of truth for batch progress.

## RESUME HERE

Next task: **T3** (`T3-extension-single-host.md`). T1 landed at `e80bec9`, T2 at `ababf4a`.

## Task table

| Task | Status | Code commit | Notes |
|---|---|---|---|
| T1 agent override resolution | done | e80bec9 | V-ALL green; adapter_override + adapter_reconnect both pass |
| T2 browser adapter resolution | done | ababf4a | V-ALL green; transport tests 64 -> 66 (two pick_native_host tests) |
| T3 extension single host | pending | - | |
| T4 installer unified surface | pending | - | |
| T5 doctor + docs + changelog | pending | - | |

## Per-task log

(Append one entry per task: commit hash, verification results, and EVERY deviation from the task
file/PINS, numbered. A BLOCKED entry carries the failed precondition or error text verbatim and
your reasoning, then the batch HALTS per BOOTSTRAP.)

### T1 -- agent override resolution (ADR-0048 D1/D2/D3)

Code commit: `e80bec9`. STOP preconditions all passed (no pre-existing Selection/DEV_INSTANCE/
endpoint_candidates/candidates_from in transport; GHOSTLIGHT_ENDPOINTS absent from crates/src/
tests; tests/adapter_override.rs did not exist; relay_adapter + connect_and_handshake matched the
pinned shapes). Files staged (exactly the five owned): crates/transport/src/instance.rs,
crates/transport/src/ipc.rs, crates/adapter-agent/src/main.rs, tests/hub_identity.rs,
tests/adapter_override.rs (new).

Verification (all green, in the task's order):
- cargo fmt --check: clean
- cargo clippy --workspace --all-targets -- -D warnings: clean
- cargo build --workspace: ok
- cargo test -p ghostlight-transport: 64 passed (incl. the three new pure-fn tests
  selection_classify_maps_the_three_states, unpinned_candidates_are_dev_then_default,
  candidates_from_honors_the_precedence_order)
- cargo test --test adapter_override: 2 passed (prefers-first-and-fails-over incl. the debug-event
  "candidate 1/2" + "candidate 2/2" oracle; falls-back-when-first-absent)
- cargo test --test adapter_reconnect: 2 passed UNCHANGED (the single-candidate regression guard)
- cargo test --workspace: every test binary reported 0 failed
- cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets: ok

Deviations:
1. The `connect_and_handshake` doc sentence PINS specifies ("gains one sentence at the end") was
   rendered as a new trailing `///` paragraph (a blank `///` separator then the sentence), not
   appended inline to the prior paragraph. Semantics identical; reads cleaner. Same choice was
   made for the `relay_adapter` doc paragraph, which PINS explicitly framed as "a final
   paragraph".
2. rustfmt (run per the BOOTSTRAP-sanctioned normalization of new code) wrapped two items PINS
   printed on a single line -- the `relay_adapter` `connect_and_handshake(...)` call and the
   `relay_with_watchdog` signature -- to multi-line because they exceed the 100-column width. No
   semantic change; `cargo fmt --check` is green.

### T2 -- browser-adapter candidate resolution (ADR-0048 D4)

Code commit: `ababf4a`. STOP preconditions all passed (no pre-existing pick_native_host_endpoint;
relay_native_host's first body line was `let stream = connect(endpoint).await?;`; T1's
endpoint_candidates present). Files staged (exactly the two owned): crates/transport/src/ipc.rs,
crates/adapter-browser/src/main.rs.

Verification (all green, in the task's order):
- cargo fmt --check: clean
- cargo clippy --workspace --all-targets -- -D warnings: clean
- cargo build --workspace: ok
- cargo test -p ghostlight-transport: 66 passed (the two new pick_native_host_endpoint tests:
  prefers-the-first-present-candidate, falls-to-the-last-when-all-are-absent)
- cargo test --workspace: every test binary reported 0 failed
- cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets: ok

Deviations:
1. The `relay_native_host` doc paragraph PINS specifies ("APPEND this paragraph to its doc
   comment") was added as a new trailing `///` paragraph (a blank `///` separator then the three
   lines), matching the same rendering choice logged for T1. Semantics identical.
