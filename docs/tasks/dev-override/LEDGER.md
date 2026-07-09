# dev-override batch -- LEDGER

Durable execution record. One task = one code commit + one ledger commit. Update after EVERY
task (or block); this file is the single source of truth for batch progress.

## RESUME HERE

**COMPLETE.** All five tasks landed (10 commits: 5 code + 5 ledger). Base was the bundle-
introducing docs commit `780adb3`; final code commit is `a406bf7` (T5). V-ALL is green at the
final tree, and `git diff --name-only 780adb3..HEAD` touches ZERO NEVER-list file (verified: 19
files, all within the owned sets; `src/main.rs` is only T4's one sanctioned help-comment line;
`tests/adapter_reconnect.rs` untouched and still passing). Nothing outside each task's named set.

Post-batch (user-supervised, NOT the executor): the user runs `ghostlight install --debug
--no-supervisor` from target/debug (rewrites the client entries as unpinned + the unified host,
which also fixes the stale cowork entry), reloads the unpacked extension, starts the dev service
(`ghostlight --debug --instance dev service --keep-warm`), then verifies cowork + Claude Code both
route to dev, that stopping dev falls back to the default, and that `ghostlight doctor` prints the
"Development override:" line.

## Task table

| Task | Status | Code commit | Notes |
|---|---|---|---|
| T1 agent override resolution | done | e80bec9 | V-ALL green; adapter_override + adapter_reconnect both pass |
| T2 browser adapter resolution | done | ababf4a | V-ALL green; transport tests 64 -> 66 (two pick_native_host tests) |
| T3 extension single host | done | 77ad837 | node --check x3 + grouping test pass; P3 post-grep zero; no dev-host in ext JS |
| T4 installer unified surface | done | c3e6418 | V-ALL green; install_instance 4/4 (dev-thin + qa-full); core 472 -> 473 |
| T5 doctor + docs + changelog | done | a406bf7 | V-ALL green; doctor override section; DEV-LOOP/README/CHANGELOG updated |
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

### T3 -- one extension host (ADR-0048 D5)

Code commit: `77ad837`. STOP preconditions all passed (every anchor present and matching;
`org.sylin.ghostlight.dev` appeared ONLY at service-worker.js's NATIVE_HOST_DEV line). Files
staged (exactly the three owned): extension/service-worker.js, extension/popup.js,
extension/options.js.

Verification (all green):
- node --check extension/service-worker.js / popup.js / options.js: all clean
- node --test tests/extension/grouping.test.js: 4 pass, 0 fail
- pinned P3 post-condition grep (nativeHost|boundInstance|NATIVE_HOST_DEV|NATIVE_HOST_DEFAULT|
  state.instance across the three files): ZERO matches
- cross-cutting pin: `org.sylin.ghostlight.dev` absent from extension/*.js after T3
- cargo fmt --check / clippy / build / test --workspace (0 failed) / linux check: all green
  (unchanged tree; the task is JS-only)

Deviations: none. (The extension JS files are CRLF in the working tree and Git normalizes them to
LF on commit -- a pre-existing repo characteristic, not a change this task introduced; the diff is
exactly the pinned label/host edits.)

### T4 -- the unified install surface (ADR-0048 D5/D6)

Code commit: `c3e6418`. STOP preconditions all passed (STORE_EXTENSION_ID/DEV_EXTENSION_ID absent
from all .rs; the only MissingExtensionId code callers were native_host.rs's two sites + the
error.rs variant; plan_install matched the pinned shape; DEV_INSTANCE present). Files staged
(exactly the five owned): crates/core/src/install/native_host.rs, crates/core/src/install/mod.rs,
crates/transport/src/error.rs, src/main.rs (the one sanctioned help-comment line),
tests/install_instance.rs.

The F2-blocker restructure was applied exactly as pinned: `plan_install` is now a 4-line resolver
wrapper; `plan_install_for` opens with `let scope` + a single `let mut actions`, then the
`if !dev_thin { ... }` block holds the launcher/manifest lets + the needs_copy block + the
windows/else browser block; the MCP-clients section and `Ok(actions)` stay OUTSIDE. There is
exactly one `let mut actions` in the fn.

Verification (all green, in the task's order):
- cargo fmt --check: clean
- cargo clippy --workspace --all-targets -- -D warnings: clean
- cargo build --workspace: ok
- cargo test -p ghostlight-core: 473 passed (472 -> 473: the new
  plan_install_for_the_dev_instance_is_client_entries_only; plus the updated
  host_manifest_json_has_type_stdio_and_exact_origin and the new
  resolve_without_an_id_allows_the_two_shipped_extensions)
- cargo test --test install_instance: 4 passed (the two new pinned subprocess tests
  dev_install_plan_is_thin_client_entries_only + a_named_non_dev_instance_still_plans_the_full_stack,
  plus the two unchanged ones)
- cargo test --workspace: every test binary reported 0 failed
- cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets: ok

Deviations: none. (`ghostlight install` was NOT run as verification, per the task; the unit +
dry-run subprocess tests are the gate. The install_instance.rs module-doc paragraph was manually
re-wrapped to fit the inserted pinned sentence -- rustfmt does not reflow `//!` comments.)

### T5 -- doctor's override line + docs + changelog (ADR-0048 D7)

Code commit: `a406bf7`. STOP preconditions all passed (doctor.rs had the in-scope `instance`
binding and the `state_line(&probe)` insertion anchor; T1's DEV_INSTANCE present; the CHANGELOG's
`## [Unreleased]` heading present). Files staged (exactly the four owned):
crates/core/src/hub/manage/doctor.rs, docs/DEV-LOOP.md, README.md, CHANGELOG.md. The doctor block
references `ipc`/`EndpointProbe` through the file's existing `use ghostlight_transport::ipc::{self,
EndpointProbe};` import (via `ipc::EndpointProbe::Absent`), matching the pin's guidance to reuse
that path.

Verification (all green):
- cargo fmt --check: clean
- cargo clippy --workspace --all-targets -- -D warnings: clean
- cargo build --workspace: ok
- cargo test --workspace --no-fail-fast: every test binary reported 0 failed
- cargo check --target x86_64-unknown-linux-gnu --workspace --all-targets: ok
- node --test tests/extension/grouping.test.js: 4 pass, 0 fail

Deviations: none. (The README "Useful flags" list did not mention `--extension-id` -- exactly as
the pin predicted -- so nothing was added there; only the three pinned README replacements were
made. README.md is CRLF in the working tree; Git normalizes to LF on commit, a pre-existing repo
characteristic.)

## Batch complete

All five tasks executed in order, each with a green V-ALL and two commits (code + ledger). Base
`780adb3` -> head (T5 code) `a406bf7`. Zero NEVER-list files touched. The dev-override design
(ADR-0048) is fully implemented: unpinned adapters resolve dev-first per connect episode, the
browser adapter probe-picks, the extension targets one host, the default install allows both
shipped extension ids and dev install is thin, and `ghostlight doctor` reports the live routing.
Handed back for the user-supervised post-batch install + verification (see RESUME HERE).
