# S8: evaluation

## Objective

Show that the combined experience works on real desktops, on both supported platforms, including the
failure cases this epic was written to address. Automated correctness is necessary and not
sufficient.

## Read first

- [BOOTSTRAP.md](BOOTSTRAP.md), [PINS.md](PINS.md), and [LEDGER.md](LEDGER.md), including every
  numbered deviation recorded during S1 through S7.
- The ADR S1 wrote, which holds the acceptance thresholds this stage dispositions.
- ADR-0123, which already made the Ubuntu GNOME Wayland L1-L9 lifecycle the one release-blocking
  visible Linux lane, with CachyOS KDE as complementary evidence.
- ADR-0116 (Windows and Linux are both in scope, so both are evaluated).
- `docs/research/25-delightful-linux-experience-2026-08.md`, section "Smallest complete
  compatibility matrix", which explains why one primary lane beats many partial ones.

## What this stage is not

It is not a recruited user study. The cohort-based first-success process was rejected as a release
gate and is retained only as history in `docs/testing/greenfield-first-success.md`;
`docs/business/FOUNDER-TODO.md` states that a private greenfield cohort is not expected. User
evidence here comes from public channels, consented and content-free.

## Required evidence

1. **The owed Linux lane.** The Ubuntu GNOME Wayland L1-L9 lifecycle: clean install, extension
   confirmation, first proof, demand start, explicit open, close and reopen, browser restart, login
   and reboot, upgrade from public 0.8, failure recovery, and uninstall. Record it as a dated file
   under `docs/testing/`.
2. **A Windows lane.** The same journey shaped to Windows, including tray behavior, the notification
   area phrase from S3, and uninstall.
3. **The migration cases this epic added.** Each observed, not asserted:
   - an extension arriving by Chrome sync on a machine with no native host, reaching the S2 state
     and the route back;
   - the same case with the walkthrough host unreachable;
   - a harness running under WSL with the browser on Windows, reaching the S3 sentence;
   - the same person's two machines producing the same words for the same state.
4. **Adversarial automated journeys** for simultaneous requests, ambiguous browsers, sandboxed
   packages, stale owned registration, caller timeout, partial and uncertain effects, plural
   sessions, workbench closure, restart, and clean removal.
5. **Accessibility.** Keyboard-only use, screen-reader names, large text, high contrast, reduced
   motion, browser zoom, and fractional desktop scaling, on GNOME Wayland and KDE Wayland.
6. **Public first-use feedback.** Consented, content-free, collected through public channels. No
   recruitment, no private cohort, no browser content in any tracked artifact.
7. **A disposition for everything.** Every acceptance threshold from S1 and every numbered deviation
   in the ledger ends as met, not met with a named follow-up, or explicitly accepted by the owner.

## Fixing what it finds

Fix a finding at its owning seam. Do not add explanatory copy to cover a domain ambiguity. Rerun the
affected scenario and record before and after in the ledger.

## Verification

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/process-journey.mjs
    node tests/cli-journey.mjs
    node tests/workbench-surface.mjs

Live evidence is recorded as dated files under `docs/testing/`, linked from the ledger's evidence
matrix and from `docs/STATUS.md`.

## Out of scope

New behavior. New surfaces. Any change that has not already been decided by an ADR from this epic.
Any distribution format that does not exist today.

## STOP preconditions

- Completion is being inferred from automated tests alone.
- Evaluation would require a participant to understand MCP, native messaging, or process topology.
- A finding is being fixed in presentation when its cause is in lifecycle, execution, governance, or
  completion.
- Sensitive browser content would enter a tracked artifact.
- The Ubuntu GNOME Wayland lane cannot be run, in which case the epic stays open and says so rather
  than substituting a different distribution.
