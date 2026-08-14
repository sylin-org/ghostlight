# H9: Installer auto-start (register + start the per-user OS supervisor)

> Batch: Ghostlight Hub. Normative: docs/adr/0030-ghostlight-hub-orchestrator.md (Decision 8 as
> amended 2026-07-04: "AUTO-START (the installed default)"; Migration line H9). Oracle identifiers:
> docs/tasks/hub/PINS.md SS5.2 (`SUPERVISOR_TASK_NAME` / `SUPERVISOR_UNIT`). One
> task = one commit. Facts below are as-of-authoring 2026-07-04 -- RE-READ the named files first.

## Goal

Make the always-ready service TRUE for the installed product: register a per-user, zero-admin OS
supervisor that runs `ghostlight service` at login and restarts it on crash, and START it once at
install time so the first session is already up. Unregister + stop it on uninstall. The supervisor
identifiers MUST equal the ones H6's adapter self-heal targets (PINS.md SS5.2), so `ghostlight`
adapters can start the service by the same name.

This is the LAST task (after H6-H8). It is largely command/file construction wired into the existing
`install` module. Real OS registration is verified by MANUAL SMOKE (a cargo test cannot register a
real Task Scheduler task or systemd unit); the cargo gates verify the pure builders.

## Authority

1. docs/adr/0030-ghostlight-hub-orchestrator.md (amended Decision 8; Migration H9) -- NORMATIVE.
2. docs/tasks/hub/PINS.md SS5.2 -- the pinned supervisor identifiers (transcribe; do not rename).
3. BOOTSTRAP.md ground rules.
4. This task file.

## Current-tree facts (as-of-authoring; RE-READ before relying)

- `src/install/` hosts the installer (`run_install`/`run_uninstall`, `InstallOptions`/
  `UninstallOptions`, `Selection`); `src/main.rs` routes `Install`/`Uninstall`. RE-READ the module to
  find where the native-messaging host + MCP-client registration happen and ADD the supervisor
  register/unregister ALONGSIDE them (same idempotent, re-read-at-apply style the installer already
  uses; see the memory of the idempotent value-level JSON merge for MCP clients -- do NOT regress it).
- The installed binary's absolute path is what the supervisor must launch with the `service`
  subcommand: `"<exe-path>" service`. RE-READ how the installer already resolves its own exe path for
  the native-messaging host manifest and REUSE that resolution.
- PINS.md SS5.2 identifiers (transcribe): Windows task `Ghostlight Service`; Linux systemd --user
  unit `ghostlight.service`.

## Required behavior (Decision 8 amendment; transcribe the oracles below)

1. Register on install (per-user, zero-admin, LeastPrivilege), then START it once. Idempotent
   (re-install must not duplicate; re-read/replace).
2. Unregister + stop on uninstall. Idempotent (absent supervisor -> no-op, not an error).
3. NEVER elevate / run as SYSTEM (Decision 8). Both supervisors are per-user.
4. The register/unregister/start/stop actions are best-effort ADDITIONS to the existing install flow:
   a failure to register the supervisor must WARN, not abort the whole install (the adapter self-heal
   + manual `ghostlight service` remain fallbacks). Log clearly.

PINNED oracles (transcribe verbatim; `<exe>` = the resolved installed binary path):

- Windows (Task Scheduler, LeastPrivilege logon task):
  - register: `schtasks /create /tn "Ghostlight Service" /tr "\"<exe>\" service" /sc onlogon /rl limited /f`
  - start now: `schtasks /run /tn "Ghostlight Service"`
  - unregister: `schtasks /delete /tn "Ghostlight Service" /f`
- Linux (systemd --user). Unit at `~/.config/systemd/user/ghostlight.service`:
  ```
  [Unit]
  Description=Ghostlight Hub service
  [Service]
  ExecStart=<exe> service
  Restart=on-failure
  [Install]
  WantedBy=default.target
  ```
  - enable + start: `systemctl --user daemon-reload` then `systemctl --user enable --now ghostlight.service`
  - remove: `systemctl --user disable --now ghostlight.service` then remove the unit file

Keep the supervisor code in `src/install/` (or a new `src/install/supervisor.rs`), NEVER in
`src/governance/**` (a7). Reuse H6's `src/hub/supervisor.rs` identifier constants (import them) so
there is ONE source of truth for the names.

## Tests (BY NAME; assertions pinned)

- Add `tests/install_supervisor.rs` (or inline `#[cfg(test)]`), PURE builders only:
  - `windows_task_register_command_is_pinned` (`#[cfg(windows)]`): the register argv contains
    `/tn`, `Ghostlight Service`, `service`, `/rl`, `limited`, `/sc`, `onlogon`.
  - `linux_unit_names_the_service_subcommand` (`#[cfg(target_os = "linux")]`): the rendered unit
    contains `ExecStart=` ... `service` and `Restart=on-failure`.
- These transcribe the pinned oracles above; they NEVER run `schtasks` or `systemctl`.
- Keep green: all sacred tests (`tool_schema_fidelity`, `all_open_golden`, `architecture` a7).

Manual smoke (documented in the LEDGER entry, NOT a cargo gate): on each platform, `install`, confirm
`ghostlight service` is running (Task Scheduler or `systemctl --user status`), open an editor and
confirm it connects with no manual start, then `uninstall` and confirm the supervisor is
gone.

## Verification (literal commands)

```
cargo build --all-targets
cargo test --test install_supervisor
cargo test --test all_open_golden --test tool_schema_fidelity --test architecture
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## STOP preconditions

- If `src/hub/supervisor.rs` (H6) does not define `SUPERVISOR_TASK_NAME`/`SUPERVISOR_UNIT`, STOP:
  H6 has not landed; do not re-pin the identifiers here.
- If the installer has no existing exe-path resolution to reuse for the `service` launch string, STOP
  and reconcile (do not hardcode a path).
- If any register/start action would require elevation / admin / SYSTEM, STOP (Decision 8: per-user,
  zero-admin only).
- If landing this would require moving a NEVER-touch fence, STOP.

## NEVER touch (this task)

- `src/transport/mcp/tools.rs` (`TOOLS_JSON`) + `tests/tool_schema_fidelity.rs`. Byte-frozen.
- `tests/all_open_golden.rs` client-visible assertions; `tests/architecture.rs` a7. No governance
  addition here (supervisor code is install-side, never `src/governance/**`).
- `src/transport/native/host.rs` framing; the MCP JSON-RPC wire.
- Do NOT change the existing native-messaging-host / MCP-client registration behavior (idempotent
  value-level JSON merge). ADD the supervisor alongside it; regress nothing.
