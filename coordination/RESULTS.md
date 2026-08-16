# Latest coordination result

- Updated: 2026-08-16
- From: windows-codex
- To: linux-codex
- Status: the remainder of the reference-experience epic is delegated to the Linux lane
- Tested implementation: `1d5629e6` on `dev`

## Context

**Your entry point is `docs/tasks/reference-experience/START-HERE-LINUX.md`.** Open it first and
follow it top to bottom. It is written to be read cold: what to read and in what order, the
authority order, environment facts labelled as-of, nine tasks with their owning code and STOP
conditions, the literal gate commands, a NEVER list where every entry names its one sanctioned
exception, and the failure protocol.

This file exists only to hand you that pointer and to record the boundary. Everything operational
is in the entry point, and the ledger is the authority on progress.

## Scope

S1 through S6 are complete and were gated on a Windows host at `1d5629e6`. The Windows session is
not implementing further stages. You own:

- V1 through V5, verifying work that landed but could only be compiled on Windows. These map to
  ledger deviations 3, 5, 7, and 2, plus the environment rows.
- S7a, S7b, and S7c, the readiness work, separated because their verification stories differ. S7a
  is a governance-schema change and should start a fresh session.
- S8, the evaluation, whose release-blocking Linux lifecycle is the Ubuntu GNOME Wayland L1-L9 run
  that ADR-0123 already made mandatory.

The WSL environment row stays with the Windows host.

## Boundaries

- Never add telemetry, an update ping, or any outbound network request. ADR-0028 Decision 9 is
  permanent and public trust and legal documents depend on it.
- Never edit an existing pin in `PINS.md`. If a pin is wrong, STOP and say so in `CHAT.md`.
- Never merge `main`, tag, sign, publish, or release.
- A blocked task with a clear reason is a good outcome. A task completed by guessing is not.
