# ADR-0115: Packaged native-host lifecycle without a resident supervisor

- Status: Accepted
- Date: 2026-08-13
- Amends: ADR-0015, ADR-0046, ADR-0063, ADR-0065, ADR-0092, and ADR-0104
- Builds on: ADR-0091 and ADR-0096

## Context

The 1.0 clean-room source retained the three executable shores and their shared demand-start seam,
but it did not retain the packaged native-messaging registration that lets Chromium launch the
browser connector. It also dropped the package build and upgrade proofs. Source and extension tests
could therefore pass while a fresh or upgraded end-user installation could not connect.

The 0.8 line contains useful evidence for this boundary: four Chromium registration layouts,
two fixed extension identities, ownership-checked removal, three sibling executables, Linux
user-session failures, and upgrade drift where a stale path kept serving. Its resident service
supervisors are no longer current architecture. ADR-0104 replaced them with one connector-owned
demand-start seam and one service lifetime lease.

## Decision

### 1. One small native-host lifecycle service

The orchestrator owns a typed native-host lifecycle module. It computes and applies only the fixed
per-user registrations for Google Chrome, Microsoft Edge, Brave, and Chromium. The manifest points
at the sibling `ghostlight-browser-connector` and allows exactly the public store identity plus the
committed unpacked-development identity.

The service has three explicit operations: check, install, and uninstall. Check is read-only.
Install is idempotent and updates a Ghostlight-owned stale path. Uninstall removes only a manifest
whose parsed host name is `org.sylin.ghostlight` and only a Windows registry entry that resolves to
a Ghostlight-owned manifest. Malformed or foreign state is reported and left untouched.

Windows uses one manifest below the current user's local application-data directory and one HKCU
key per browser. macOS and archive installs use the browsers' per-user `NativeMessagingHosts`
directories. A Debian package also owns the four fixed system manifest files under `/etc`; its
first ordinary user launch reconciles the per-user locations so a stale 0.8 file cannot shadow the
package. The product command itself has no elevation path.

### 2. The executable exposes a package seam, not another installer framework

`ghostlight native-host check|install|uninstall` is the whole package-facing command surface. It
does not edit MCP client configuration, download anything, launch a browser, or start a service.
The existing workbench remains the explicit, per-harness MCP registration surface.

Windows native packages invoke install only after all three sibling executables are in their final
location, and invoke uninstall before those files are removed. Debian owns its system manifests as
package files and performs the per-user reconciliation on first launch. Release archives retain
the command for manual and package-manager integration. A package must prove the registered path
names the connector inside that exact installed sibling set.

### 3. Demand-start replaces the old supervisor

No Run key, scheduled task, launchd agent, or systemd user unit is installed for 1.0. Either
connector demand-starts its trusted sibling orchestrator through the ADR-0104 seam. Upgrade
migration may remove a recognized 0.8 supervisor artifact, but it never creates a replacement.

Migration is narrow and ownership checked. Unknown commands, malformed files, foreign units, and
pre-existing deployment locks are preserved and reported. Package replacement uses the existing
fresh `deploy.lock` convention so old connectors cannot demand-start an image while its sibling set
is being replaced.

### 4. The desktop package carries all three executable shores

Tauri bundles the MCP and browser connectors as external binaries. A release staging script gives
them Tauri's required target-triple names from one locked workspace build. The UI receives no shell
permission and does not execute the sidecars; bundling is distribution only.

Windows NSIS uses an installer hook for the package-facing lifecycle command. Linux and macOS
artifacts must run the same command from their package integration or documented first-launch
handoff. A format is not release-ready merely because Tauri emitted a file: its clean install,
upgrade, uninstall, and real visible-browser journeys must pass on that operating system.

### 5. Historical evidence is re-expressed, not copied

The 0.8 source remains read-only implementation history. Current tests re-express its observable
contracts against the 1.0 module: exact paths and origins, missing/current/updatable/foreign state,
idempotence, ownership-safe removal, sibling completeness, and Linux package lifecycle. The
harvest inventory records the rest until an equivalent 1.0 proof exists.

## Consequences

- A package can install the complete browser shore without restoring the old always-running
  supervisor or a broad multi-client installer.
- Stale Ghostlight-owned connector paths become a first-class updatable state instead of looking
  healthy, while foreign state remains protected.
- The same lifecycle seam is usable by NSIS, Linux packages, Homebrew, Scoop, WinGet, and manual
  archives without duplicating registry or manifest policy in packaging scripts.
- macOS drag-to-trash cannot run an uninstall hook. Its package and documentation must state the
  cleanup path honestly, and the ownership-safe command remains available before removal.
- Release work now includes native packaging and real Linux revalidation. Those are product gates,
  not standing release bureaucracy.
