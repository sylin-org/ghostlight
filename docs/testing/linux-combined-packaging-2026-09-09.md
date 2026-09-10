# Combined Linux package validation, 2026-09-09

Exact candidate: `0129ff15bf0c6348a78e8c0d2e4e1d2716695ae8`, tree
`a8f014c5cd53c0acaab5f24980f24cb72f6e3a7f`, on
`codex/fleet-test-01-final-packaging` in the ordinary repository.

This final focused package round includes the bounded startup/readiness, selected
connector copy-retry and explicit Flatpak activation changes. The ac1becab archive
record remains historical. Its artifacts, failed attempts and installed paths are
preserved. Prepared Ubuntu 22.04/Debian 12/Ubuntu 24.04 roots are reused with fresh
candidate-scoped user state; the new source/build/archive area is
`.tmp/linux-local/combined-0129ff15/`. No worktree or fleet folder was created.

Checkpoint: all 1,045 staged Git blobs match the candidate. Formatting, Clippy,
564 Rust tests, 222 extension tests, the fresh real process journey and the exact
selected-image copy-retry regression pass. Ubuntu baseline packaging is in flight.
All three installed siblings need replacement; the dev loop is building and
swapping them at the existing live path. Consumer cold/upgrade/recovery, explicit
native non-mutation of Flatpak setup, and normal browser smoke remain pending.
No current-candidate package acceptance or release readiness is claimed yet.
