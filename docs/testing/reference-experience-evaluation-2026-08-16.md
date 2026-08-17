# Reference experience evaluation -- 2026-08-16

Status: BLOCKED at the required real-desktop boundary

## Scope observed

The available host is CachyOS Linux on KDE Wayland. `gnome-shell` is not installed, and this host
cannot execute a Windows installed-product lifecycle. The S8 prompt explicitly forbids substituting
this desktop for the release-blocking Ubuntu GNOME Wayland lane.

This run therefore inventories evidence and runs the final automated gate. It does not claim S8 or
the reference-experience epic complete.

A subsequent Windows source-host check replaced the stale live development authority and exercised
the corrected recovery inventory. It still did not run an installed candidate, so it adds evidence
without changing this evaluation's `BLOCKED` disposition.

## Current evidence

- Linux V1-V5 in the reference-experience ledger prove the per-user command entry, Debian and
  per-user manual pages, real bash/zsh/fish completion, the live online and offline second-machine
  handoff in Chromium, and the KDE plus unknown environment rows.
- S7's live no-browser call on KDE Wayland returned `failed`, `effect: none`, reason
  `browser_startup_manual`, and one useful next step in 53 ms.
- The final automated gate passed formatting, warnings-denied workspace Clippy, 307 orchestrator
  library tests, 10 orchestrator binary tests, 33 bridge tests, 6 MCP connector tests, 116 extension
  tests, an isolated workspace build, and the process, CLI, and workbench-surface journeys.
- The process coverage includes reconnect, ordinary work, recording, close, and a pinned-disconnect
  pre-effect refusal. The Rust coverage includes simultaneous recovery, ambiguous installations, sandboxed
  packages, owned stale-registration planning, caller cancellation, bounded handshake timeout,
  partial and uncertain effects, plural sessions, restart, and ownership-safe removal.
- On the Windows source host, the exact-path release swap converged existing connectors on one new
  authority. A real Chrome adapter completed open, read, and screenshot. Its close was blocked by
  the person's `preserve-tabs` interlock, so the script did not pass as a whole; the test tab was
  closed directly without changing the setting. An isolated no-adapter release call returned
  `browser_recovery_ambiguous` with exactly Google Chrome and Microsoft Edge and no effect. All four
  native-host registrations were missing and unchanged, so physical on-demand launch remains
  unproved.

## Acceptance disposition

| ADR-0126 measure | Disposition on 2026-08-16 | Evidence or missing proof |
| --- | --- | --- |
| First use | NOT MET as an S8 gate | Development-host journeys exist, but neither required clean installed desktop completed first proof against the candidate and matching store extension |
| Second machine | PARTIAL | Live KDE Chromium proved both extension surfaces plus online and offline routes; the same-person cross-platform wording comparison is not observed |
| Recovery | PARTIAL | Closed automated outcomes, caller-local single-flight cancellation/deadlines, a real Windows source-host Chrome/Edge ambiguity, and Linux manual-mode diagnosis pass; physical installed-Windows on-demand startup and the full desktop failure matrix are not observed |
| Control | PARTIAL | Pause and stop are truth-preserving in automated journeys; keyboard and visible behavior on both required installed desktops are not observed |
| Parity | NOT MET, owner decision required | The Rust-owned vocabulary guard passes, but deviation 12 records that `doctor` has no live aggregate-readiness line; closing the literal measure requires either an approved amendment or a service-query design |
| Accessibility | NOT MET | Keyboard-only, screen reader, large text, high contrast, reduced motion, browser zoom, and fractional scaling were not run on GNOME Wayland and KDE Wayland |
| Evidence | NOT MET | Ubuntu GNOME Wayland L1-L9, the clean Windows lane, the remaining migration observations, and public first-use feedback are missing |

## Blocking evidence

The canonical Ubuntu record remains [linux-live-lifecycle.md](linux-live-lifecycle.md), where L1
through L9 are all `NOT RUN`. The existing Windows source record is
[windows-current-source-pass-2026-08-15.md](windows-current-source-pass-2026-08-15.md); it explicitly
does not cover install, public-0.8 upgrade, login/reboot, tray, notifications, visible browser work,
or uninstall.

No public first-use feedback was collected in this run. Automated results are not substituted for
human-visible evidence, and no private browser content or machine-local notes were read.

## Exact unblock sequence

1. On current Ubuntu Desktop LTS, GNOME, and Wayland, run L1-L9 from
   [linux-live-lifecycle.md](linux-live-lifecycle.md) against one checksum-bound Debian candidate and
   its matching store extension. Include the S8 accessibility matrix.
2. On Windows, run the same installed-product journey shaped to notification-area, tray,
   login/reboot, public-0.8 upgrade, S7 on-demand browser startup, and ownership-safe uninstall.
3. Observe the WSL rendered sentence and compare the same state on the same person's two machines.
4. Collect consented, content-free first-use feedback through existing public channels.
5. Disposition every remaining acceptance row and ledger deviation. The owner must resolve the
   literal `doctor` parity measure before S8 can pass.
