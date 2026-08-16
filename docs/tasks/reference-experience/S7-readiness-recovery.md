# S7: readiness recovery

## Objective

When an admitted browser operation finds the local stack not ready, repair what can be repaired
safely and name precisely what cannot. The safety test is applied per platform and is expected to
produce different answers on Windows and Linux.

## Read first

- [BOOTSTRAP.md](BOOTSTRAP.md) and [PINS.md](PINS.md).
- The ADR S1 wrote, which named the browser-startup preference, its owner, and its default per
  platform.
- ADR-0082 (Linux user session discovery), ADR-0092 (install activates selected engine), ADR-0104
  (demand start and workbench activation), ADR-0113 (adapter liveness), ADR-0114 (plural browser
  adapters, including the existing resolution order), ADR-0115 (packaged native-host lifecycle),
  ADR-0123 (native browser packages only; Snap and Flatpak refused with a remedy).
- `crates/orchestrator/src/browser/`, `crates/orchestrator/src/work/`,
  `crates/orchestrator/src/install/browser_package.rs`.

## Verified facts as of authoring

Confirmed at `2f24943f`. Re-read before relying on any of them.

- The orchestrator spawns processes only in `install/handoff.rs` and `install/migration.rs`. Nothing
  in the runtime path launches a browser today.
- ADR-0114's resolution order already exists: explicit selection, then binding, then reported
  attention, then the sole connected browser, otherwise a refusal that names the candidates. Extend
  that chain; do not build a second one.
- Snap and Flatpak detection with a remedy sentence already exists in `install/browser_package.rs`.
  Reuse it. A sandboxed browser is a diagnosis, never a launch target.
- `docs/1.0/INTENT.md` promises the user's visible, existing, authenticated browser. Any launch uses
  the person's ordinary profile. Never a fresh profile, never a temporary profile, never automation
  flags.

## Required behavior

1. **Find the one seam that knows.** Identify the single place that learns readiness failed and can
   request recovery. Recovery logic must not appear at operation call sites.
2. **Implement the smallest closed sequence.** Single-flight launch where the decided preference
   enables it, bounded waiting for the inbound adapter, and safe repair of stale Ghostlight-owned
   registration where the lifecycle contract already authorizes it.
3. **Windows and Linux differ, deliberately.** Implement the per-platform posture S1 decided. On
   Linux, a launch requires a usable session environment; if that cannot be established through the
   ADR-0082 seam, the honest outcome is diagnosis rather than a launch attempt, and that is an
   acceptable completion of this stage.
4. **Choose a browser only from deterministic evidence.** Prefer an associated or uniquely obvious
   browser. Never guess across ambiguous installations or profiles. A sandboxed package is named,
   not launched.
5. **Stay authority-neutral and pre-effect.** Never install software, open a store page, overwrite
   foreign configuration, broaden policy, or replay an uncertain effect.
6. **Bound and name every wait.** On exhaustion, distinguish browser absence, launch failure,
   sandboxed package, extension absence, native-host failure, wrong profile, handshake timeout, and
   ambiguity, as precisely as the evidence permits.
7. **Concurrency is single-flight.** Simultaneous requests produce at most one launch or recovery
   attempt per owning scope, and cancellation or caller loss cannot leave work to continue later.

## Tests to add

Rust, by name:

- `recovery_is_requested_from_one_seam_only`
- `simultaneous_requests_produce_one_attempt`
- `manual_mode_never_launches_and_returns_one_useful_outcome`
- `a_sandboxed_browser_package_is_diagnosed_not_launched`
- `an_ambiguous_browser_set_refuses_and_names_candidates`
- `recovery_changes_no_authority_and_no_foreign_state`
- `cancellation_leaves_no_abandoned_operation`
- `each_closed_failure_reason_is_reachable_and_distinct`
- `a_launch_uses_the_ordinary_profile_with_no_automation_flags`

Process-level coverage in `tests/process-journey.mjs` for at least the no-browser-connected path and
the disabled-launch path.

## Verification

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/process-journey.mjs
    node tests/cli-journey.mjs

Then, on each platform available to you, run a real operation with no browser connected and record
the observed outcome, the platform, and the elapsed time in the ledger.

## Out of scope

Anything the extension renders. Workbench presentation, which is S6. Installing browsers. Snap or
Flatpak bridging. Any new preference beyond the one S1 named.

## STOP preconditions

- Recovery logic would have to be repeated at operation call sites.
- The design would need a generic recovery framework, a new daemon, or a second lifecycle authority.
- The available evidence cannot choose one browser conservatively.
- A timeout would leave an effect's truth unknown and then retry automatically.
- On Linux, a launch would proceed without a verified session environment.
- S1 did not record the startup preference, its owner, and its per-platform default.
