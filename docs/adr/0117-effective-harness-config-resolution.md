# ADR-0117: Effective harness configuration resolution

- Status: Accepted
- Date: 2026-08-13
- Amends: ADR-0015, ADR-0067, and ADR-0071
- Builds on: ADR-0116

## Context

Harness configuration is not always rooted below the operating-system home directory. Codex, for
example, uses `CODEX_HOME` when it is set. Ghostlight previously inspected only
`~/.codex/config.toml`, so install and doctor could report success while the active Codex process
continued to use an older registration elsewhere. Pre-1.0 `ghostlight-relay --role agent`
registrations also appeared foreign after the connector executable was renamed.

## Decision

The existing fixed harness registry remains the installer architecture. It gains no plugin layer,
trait hierarchy, or generic discovery framework.

- Linux resolves home from `HOME`, configuration from `XDG_CONFIG_HOME` with `~/.config` as the
  fallback, and Codex from `CODEX_HOME` with `~/.codex` as the fallback.
- Windows resolves home from `USERPROFILE`, roaming configuration from `APPDATA` with the standard
  profile fallback, and applies the same `CODEX_HOME` precedence.
- Each harness definition continues to choose its explicit path and dialect from those roots.
- The old `ghostlight-relay` command is owned only when its arguments are exactly
  `--role agent` and it belongs to the same versioned Ghostlight install root as the current MCP
  connector. That exact legacy entry is updatable. Other relay commands remain foreign.
- Automatic setup reports every malformed or foreign entry it skips. It does not claim that no
  configuration needs attention when one was found.

## Consequences

- Install, uninstall, doctor, and the workbench inspect the configuration used by the active
  Codex environment.
- Recognized pre-1.0 registrations migrate without weakening foreign-entry protection.
- Linux behavior is executable on the current development host. Windows resolution is covered at
  the pure path seam and still requires the normal native Windows release validation.
