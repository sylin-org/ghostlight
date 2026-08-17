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

| ADR-0126 measure | Current disposition | Evidence or missing proof |
| --- | --- | --- |
| First use | NOT MET as an S8 gate | Development-host journeys exist, but neither required clean installed desktop completed first proof against the candidate and matching store extension |
| Second machine | PARTIAL | The ADR-0126 measure itself is met: live KDE Chromium proved both extension surfaces plus the online and offline routes. The separate same-person cross-platform wording comparison is not observed, but it needs no third environment. The Ubuntu GNOME and clean Windows lanes are that person's two machines, and `language/environment.rs` is one closed table with a guard test proving every row carries every phrase, consumed by both install and `doctor`. Each lane recording its rendered environment sentence makes the comparison a desk check rather than a separate run |
| Recovery | PARTIAL | Closed automated outcomes, caller-local single-flight cancellation/deadlines, a real Windows source-host Chrome/Edge ambiguity, and Linux manual-mode diagnosis pass; physical installed-Windows on-demand startup and the full desktop failure matrix are not observed |
| Control | PARTIAL | Pause and stop are truth-preserving in automated journeys; keyboard and visible behavior on both required installed desktops are not observed |
| Parity | PARTIAL: implementation met 2026-08-17 | The owner approved the live query. Text and JSON `doctor` now read the exact orchestrator-owned readiness projection through an authenticated no-session, no-audit service opening. Six-state and real-process guards pass; installed Windows and Ubuntu observation remains S8 evidence. See [doctor-readiness-parity-2026-08-17.md](doctor-readiness-parity-2026-08-17.md) |
| Accessibility | NOT MET, and half of it is runnable today | Keyboard-only, screen reader, large text, high contrast, reduced motion, browser zoom, and fractional scaling were run on neither GNOME Wayland nor KDE Wayland. Only the GNOME half waits on the Ubuntu machine; the KDE half runs on the existing CachyOS host and should not be queued behind it |
| Evidence | NOT MET | Ubuntu GNOME Wayland L1-L9, the clean Windows lane, and the remaining migration observations are missing. Public first-use feedback was removed from this row on 2026-08-17: G0 of the release checklist decided it is not part of 1.0 in any form, per ADR-0126's seven measures and Decision 11 |

## Blocking evidence

The canonical Ubuntu record remains [linux-live-lifecycle.md](linux-live-lifecycle.md), where L1
through L9 are all `NOT RUN`. The existing Windows source record is
[windows-current-source-pass-2026-08-15.md](windows-current-source-pass-2026-08-15.md); it explicitly
does not cover install, public-0.8 upgrade, login/reboot, tray, notifications, visible browser work,
or uninstall.

Automated results are not substituted for human-visible evidence, and no private browser content or
machine-local notes were read.

## What is decidable without a new machine

Sorted on 2026-08-17 so the release checklist's G0 could close its S8 half. These need no
installed-product observation:

- Public first-use feedback is out of 1.0 entirely. It was never one of ADR-0126's seven measures.
- The parity decision and its implementation are closed. Only its observation is S8 evidence.
- The cross-platform wording comparison needs no third environment. The two required lanes are the
  two machines, and one guard-tested closed table supplies the words to both install and `doctor`.
- The KDE half of the accessibility matrix runs on the existing CachyOS host today.
- The WSL sentence needs a WSL harness with the browser on Windows. It does not need the clean
  installed-Windows machine and should not wait for it.

Everything else below is genuinely blocked on a real desktop.

## Exact unblock sequence

1. Run the KDE half of the accessibility matrix on the existing host now. It blocks on nothing.
2. On current Ubuntu Desktop LTS, GNOME, and Wayland, run L1-L9 from
   [linux-live-lifecycle.md](linux-live-lifecycle.md) against one checksum-bound Debian candidate and
   its matching store extension. Include the GNOME half of the accessibility matrix, and record the
   rendered environment sentence.
3. On Windows, run the same installed-product journey shaped to notification-area, tray,
   login/reboot, public-0.8 upgrade, S7 on-demand browser startup, and ownership-safe uninstall.
   Record the rendered environment sentence here too.
4. Observe the WSL rendered sentence, then compare the three recorded sentences as a desk check.
5. Disposition every remaining acceptance row and ledger deviation. Observe the `doctor` line beside
   the workbench state in both installed desktop lanes.
