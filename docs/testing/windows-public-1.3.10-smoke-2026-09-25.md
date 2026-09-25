# Windows public 1.3.10 consumer smoke -- 2026-09-25

Status: PASS. The public npm launcher downloaded, verified, and executed the exact Windows
1.3.10 candidate binaries from an isolated state directory.

## Scope

- Host: Windows x64.
- Entry: `npx --yes ghostlight@1.3.10 --help` from an isolated non-repository working directory.
- State boundary: `GHOSTLIGHT_HOME` pointed at a new directory under the repository's ignored
  `.tmp` area. The smoke did not use the normal Ghostlight state directory.
- Mutation boundary: the launcher populated only that isolated versioned download cache. It did
  not run install or uninstall and did not alter browser or MCP client registrations.

## Public bytes

The launcher downloaded all three Windows executables from public release `v1.3.10`, verified its
package-bound checksums, and printed the 1.3.10 CLI help. Independent SHA-256 recomputation returned:

- `ghostlight.exe`:
  `40c6ea63c72a970f4e218fc68f4cd81b93f49952010b4aceb0c388402fe0c7d2`.
- `ghostlight-mcp-connector.exe`:
  `61cd61cc7a56158f60e9539af82ea8ad7a6c7d8cb10f271a1789705e0e327fcf`.
- `ghostlight-browser-connector.exe`:
  `718c2c221af333af5bdd989931d67fe5a229047368b9063fcfe335973ecbe3a2`.

Each hash matches the frozen 1.3.10 candidate manifest and the GitHub release asset digest.

## Limits

This smoke proves public npm resolution, release download integrity, launcher cache verification,
and CLI execution on Windows. It does not repeat installer registration, uninstall, reboot, live
browser control, or physical Linux acceptance. Candidate CI covers Debian 12 and Ubuntu 24.04
package lifecycle; the pre-publication Windows journey covers the native shutdown behavior.
