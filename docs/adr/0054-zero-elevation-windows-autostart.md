# ADR-0054: Zero-elevation Windows auto-start -- HKCU Run key + detached self-heal spawn

Status: Accepted (2026-07-10; owner: "Let's do 0.5.1 then. This is exactly the kind of issues I was
hoping to catch"). Amends ADR-0030 Decision 8 (the always-ready-service amendment) on Windows only;
macOS launchd and Linux systemd --user paths are untouched. Fixes issue #17. Ships as v0.5.1.

## Context

ADR-0030 D8 promised a per-user, ZERO-ADMIN supervisor: the installer registers `ghostlight
service` to start at logon, and a thin adapter whose first dial fails asks that same supervisor to
start the service (self-heal) instead of spawning an in-job child (deleted for its stdio/lifetime
hazards). On Windows the chosen mechanism was a Task Scheduler logon task (`schtasks /create /sc
onlogon /rl limited`) plus `schtasks /run` for the on-demand start.

The premise fails on Windows: creating a LOGON-trigger task requires elevation. Probe-verified on
Windows 11 (unelevated): the identical `schtasks /create` succeeds with `/sc daily` and returns
`Access is denied` with `/sc onlogon`. The trigger type is the problem, not schtasks. Consequence,
observed live during the Cline marketplace validation (issue #17): every non-admin `ghostlight
install` warns and registers NOTHING, and the self-heal (`schtasks /run`) then addresses a task
that does not exist -- a fresh npm/winget/scoop Windows user's first tool call dies with
`SELF_HEAL_FAILURE_MESSAGE` until they run `ghostlight service` by hand. This is not a 0.5.0
regression; it shipped with H9.

Two facts shape the fix. First, Windows' genuinely zero-elevation per-user autostart is the
registry Run key (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) -- always writable by the
user, runs at logon. Second, the schtasks task never provided crash-restart anyway (a plain
onlogon task has no monitor); it provided exactly two things -- logon start and an on-demand start
handle -- so replacing it loses nothing.

## Decision

1. **Windows logon start is an HKCU Run key.** The installer writes value `Ghostlight Service`
   (named instances: `Ghostlight Service (<n>)` -- the SAME names
   `ghostlight_transport::supervisor::supervisor_task_name` already mints) under
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, data `"<exe>" service` (with
   `--instance <n>` for a named instance). Uninstall deletes the value. Both are best-effort,
   never fatal, matching the existing supervisor-step discipline.
2. **Windows self-heal is a detached service spawn.** When the adapter's first dial fails, instead
   of `schtasks /run` it spawns the SIBLING service executable (`ghostlight.exe` next to the
   relay; `--instance <n>` appended for a named instance) fully detached: `DETACHED_PROCESS |
   CREATE_NEW_PROCESS_GROUP` creation flags and null stdin/stdout/stderr. This is NOT the in-job
   child ADR-0030 D8 deleted: that hazard was inherited stdio (the MCP pipes) and lifetime
   coupling; a detached, null-stdio process shares neither. The installer uses the same helper for
   its start-once-after-install step. If the client's job object still reaps the service on client
   exit, the next call simply self-heals again -- the mechanism is idempotent by construction.
3. **Legacy migration.** Windows registration best-effort deletes the old scheduled task
   (`schtasks /delete /tn <name> /f`) so elevated installs from earlier versions converge on the
   Run key; uninstall keeps deleting both mechanisms.
4. **macOS and Linux are unchanged.** launchd LaunchAgents and systemd --user units are genuinely
   user-scoped, and both provide real crash-restart (`KeepAlive` / `Restart=on-failure`), which
   the Run key cannot -- so the richer mechanisms stay where they work.
5. **Pins move.** The PINNED Windows supervisor commands (H9 / PINS.md SS5.2: schtasks
   create/run/delete; the `supervisor_start_command` Windows arm) are superseded by this ADR. The
   new pins: the Run-key path + value name + data shape, the sibling-exe resolution
   (`ghostlight-relay*` -> sibling `ghostlight`), and the detached-spawn creation flags. macOS and
   Linux pins stand as written.

## Consequences

- A non-admin Windows user gets what ADR-0030 D8 promised: install once, tools work -- at next
  logon via the Run key, and immediately (plus after any service death) via the detached self-heal.
  No UAC prompt anywhere.
- Windows loses nothing it actually had (the task never crash-restarted) and drops a whole failure
  mode; self-heal no longer depends on install-time state at all on Windows.
- The detached spawn reintroduces a service-spawning code path in the adapter. The two hazards
  that killed the old one are structurally absent (null stdio, no job/process-group inheritance),
  and the singleton endpoint claim in the service makes concurrent heals harmless (losers exit).
- Windows installs now touch HKCU\...\Run, a location security tooling watches; the value is
  user-visible in Task Manager's Startup page, which is arguably MORE transparent than a
  scheduled task.

## Provenance

Found live during the Cline marketplace validation (2026-07-10): the installer's supervisor
[warn] lines in Cline's own transcript. Root cause bisected the same hour (daily-vs-onlogon probe).
Owner: catch-and-fix was the explicit purpose of the dogfooding pass; fix ordered as v0.5.1.
