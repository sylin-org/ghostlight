# Windows current-source and local-package pass -- 2026-08-15

Status: source, isolated-process, local-package, and native-window pass; installed release gates
remain open

This record covers the fullest non-installing Windows pass at source revision
`72402a7d1ed281436877546c0ea797736f25cc3a`. It did not deploy Ghostlight, change browser or MCP
client registrations, install or uninstall the NSIS package, publish an artifact, or read
machine-local project notes.

## Environment

```text
platform: Windows x86_64, 25H2, build 26200.8894
rustc: 1.95.0
cargo: 1.95.0
node: 24.7.0
npm: 11.4.2
PowerShell: 7.6.0
tauri-cli: 2.11.0
cargo-audit: 0.22.2
cargo-deny: 0.20.2
```

The repository candidate workflow remains pinned to Node 22. This host-local run used Node 24;
the provenance candidate's ordinary CI already passed on the pinned toolchain at the same product
source revision. The only commit after that product revision records its result in documentation.

## Source and dependency gates

- `cargo fmt --all -- --check` passed.
- Locked workspace/all-target Clippy passed with warnings denied.
- The locked workspace build passed in `.target-windows-1.0-pass`.
- All 288 Windows Rust tests passed: 246 orchestrator library, 4 orchestrator binary, 32 bridge,
  and 6 MCP connector tests. Six Linux-only orchestrator tests are compiled out on Windows.
- All 106 extension, 10 npm launcher, and 4 MCPB launcher tests passed.
- All 42 tracked JavaScript and module files parsed with `node --check`. All five tracked shell
  scripts parsed with Git for Windows `sh -n`.
- Offline public truth, regenerated 0.8 inventory equality, 0.8 behavior recovery, 0.8 artifact
  recovery, and whole-repository integrity passed.
- Dependency licenses, bans, and sources passed. `cargo audit` exited zero with the same 17
  documented allowed GTK/Tauri-chain warnings and no unallowed vulnerability.
- The online public check found GitHub, npm, the Chrome update feed, the official MCP Registry, and
  the website in agreement about the still-public 0.8 release and adapter.

## Isolated process journeys

Every process journey resolved executables from `.target-windows-1.0-pass/debug`:

- The process journey passed service interruption, stable relays, renegotiation, open/read,
  extension-owned recording save/discard, and close.
- The native CLI journey passed governed execution, CLI-attributed audit, batch identity, and
  channel refusal.
- The PowerShell journey passed separate-process open/list/read/capture/close. It produced real
  JPEG bytes and kept one CLI session across the commands.
- The executed workbench surface passed all 34 assertions, including plural integration targets,
  durable setup actions, policy presentation, failure containment, and retry.
- The policy grammar journey passed exact host-pattern readback and coverage behavior.

## Local release artifacts

An isolated locked optimized build under `.target-windows-package-pass` produced all three Windows
siblings. The local Tauri build used the pinned CLI, skipped signing deliberately, and produced an
NSIS package. `check-native-package.ps1` found the exact sibling set and exact legal payload.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Unsigned NSIS installer | 4,528,043 | `2ded94f504bf4537d82018c5ecf809085f2f5f1c747aea5c8bdb5478c41dc985` |
| Windows portable ZIP | 6,949,405 | `43b9c0c289e88250af374c5325ff72ea61d4e7c7c3eb7793a72fe1dca948c656` |
| Chromium extension ZIP | 1,405,054 | `8ef75eacefe06982717258b9b23d17a3b84edd28d1ed599a24f2c54fcd270a30` |

Two independent Windows extension-package runs were byte-identical. This host-built ZIP is not the
Ubuntu workflow's provenance artifact and has a different hash; this record makes no cross-host
archive-reproducibility claim. The provenance-bound candidate remains the publication authority.

## Native window smoke

The isolated optimized `ghostlight.exe` passed exact HWND inspection without touching installed
state:

- ordinary startup created one visible minimized `Ghostlight` / `Tauri Window` and no visible
  console;
- `ghostlight open` restored and foregrounded that exact HWND;
- Close destroyed only the workbench while the authority stayed alive; and
- a second `ghostlight open` created a new HWND under the same authority.

The test stopped only the exact isolated executable path and removed its unique runtime, lease, and
audit files.

## Remaining boundary

This is not the clean-machine Windows release pass. The following still require explicit installed
state and owner-visible interaction:

- install the provenance-bound NSIS candidate as an ordinary user;
- upgrade a real public 0.8 installation, then uninstall and prove ownership-safe cleanup;
- verify login and reboot, tray interaction, and native notifications;
- run the visible browser matrix with the matching store adapter; and
- run the public MCP harness matrix against the installed candidate.
