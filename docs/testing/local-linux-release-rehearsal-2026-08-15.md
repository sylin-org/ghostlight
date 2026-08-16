# Local Linux 1.0 release rehearsal -- 2026-08-15

Status: development-host pass; native release gate remains open

This record covers the fullest safe local rehearsal available on the CachyOS development host.
It does not replace the provenance-bound Debian candidate or the Ubuntu GNOME Wayland L1-L9
lifecycle.

## Evidence header

```text
date_utc: 2026-08-15
source_revision: 51552025e2402c089cdf2a9658e5bba52829467e
architecture: x86_64
distribution: CachyOS rolling
kernel: 7.1.8-1-cachyos
desktop: KDE Plasma 6.7.4, Wayland
browser: Chromium 151.0.7922.137
toolchain: Rust/Cargo 1.95.0, Node 22.22.1, npm 10.9.4, PowerShell 7.6.0
ghostlight: 1.0.0 user installation
extension: 1.0.0 unpacked source adapter
```

The installed sibling hashes were:

- `ghostlight`: `51fb7289e6b33a4d5fdb6c8b7214918c964b4210e72d5e46afac88cf1a112a58`
- `ghostlight-mcp-connector`: `7c56fc3913a434bb87ab1a453bc96ba4e9ed16468c322da23dd303b0e8954987`
- `ghostlight-browser-connector`: `7e50d6d3424eafdfdcbb39c53d3bd9353ed56d59a5b8d5aa981701967ed8589a`

The deterministic extension ZIP is 1.0.0 with SHA-256
`3cdb3982c9772c84447923b9785ff7e0efc81ed2885fc2386090f4267cce0ab2`.

## Source and artifact gates

- Formatting and locked workspace/all-target Clippy with warnings denied passed.
- All 277 Rust tests passed: 237 orchestrator library, 4 orchestrator binary, 32 bridge, and 4
  MCP connector tests.
- All 106 extension, 10 npm launcher, and 4 MCPB launcher tests passed.
- Every extension, workbench, harness, npm, and MCPB JavaScript file parsed. Every shell script
  passed `sh -n`.
- Policy grammar and the executed workbench surface passed.
- Fresh isolated process, CLI, and PowerShell journeys passed. They covered demand start, relay
  reconnect, unknown-effect handling, recording save/discard, governed refusal, CLI audit
  attribution, batch identity, and separate-process open/list/read/capture/close.
- Offline public truth, 0.8 recovery, 0.8 artifact recovery, and repository integrity passed.
- `cargo deny` passed bans, licenses, and sources. `cargo audit` found no unallowed vulnerability
  error and reported the 17 already documented transitive warnings.

## Removal, reinstall, and local ownership

The supported uninstall path removed the active user installation's owned browser and detected MCP
registrations. Absence was checked before replacement. Only processes whose executable resolved
inside that exact installation were stopped. The complete prior version directory was preserved
on the second drive at
`/run/media/test/WORKBENCH/state/ghostlight-local-1.0-rehearsal/backups/pre-reinstall-v1.0.0`.
Retained audit and unrelated user state were not deleted.

The three current siblings were deployed together at `~/.ghostlight/bin/v1.0.0`. Installation
restored current Chrome, Edge, Brave, and Chromium registrations; Codex, Claude Code, and Visual
Studio Code registrations; and the owned XDG Applications entry and icon. Only Chromium is a
detected native browser on this host. Two later `install --all-browsers --no-open` calls changed
nothing. `doctor` reports the exact siblings, current registrations, current Applications entry,
and a running bridge-2 authority.

No installed Ghostlight supervisor exists. The visible test browser runs under one transient
user-systemd unit solely because processes launched directly by this test shell are collected with
the shell. The product did not install that unit.

## Protocol, CLI, governance, and recovery

- Direct stdio MCP initialization negotiated revision `2025-11-25`, named Ghostlight 1.0.0,
  returned exactly 22 tools, and completed `browser_tabs` through the installed connector.
- The installed CLI returned the same 22-entry catalog and completed a real browser call.
- All 13 maintained schema-3 policies validated. Explain used the production RAWX directory.
  Simulation evaluated 147 current audit records and reported 110 candidate denials. Non-JSON
  input was refused.
- Ed25519-only key generation produced a mode-0600 seed. Public-key derivation, explicit sequence
  1 signing, and publication advancing the same bundle to sequence 2 passed.
- A real authority configured with `research-read-only.json` exposed a narrowed 14-tool catalog,
  omitted `browser_execute`, and refused Example Domain with denial `D-43e8b2c9`. An invalid
  configured source started fail-closed and refused intake. Ordinary all-open authority was then
  restored with 22 tools.
- With the authority stopped and both connectors retained, the browser connector demand-started
  the exact installed sibling and a new call succeeded.
- Browser shutdown caused a bounded safe failure. Restart restored the same durable browser id,
  `browser_cc725b07bf554d53802b3da7f8347079`, the exact connector, and new work.

## Visible browser matrix

The installed CLI and unpacked adapter completed these ordinary-profile journeys:

- Example Domain open, list, bounded read, screenshot, and screenshot-bound coordinate hover;
- preserve-tabs refusal of model-driven close;
- direct MCP navigation, read, and tab listing;
- the complete published Foundry story: resize, recording, inspect, hover, click, zoom, checkbox
  input, text typing, drag, diagnostics, wait, screenshot, upload, form fill, completion, off-domain
  refusal, replay delivery, and recording erasure;
- prompt creation, dialog status, response, and page-side confirmation of the supplied response;
- pointer-only drag on the Foundry ticket; and
- native HTML drag and drop on a public W3Schools fixture, with its page-authored drag payload
  moving the source from `div1` to `div2`.

The first Foundry run exposed a real drag regression: a held move never received a terminal browser
receipt. The extension now constructs one bounded held-button packet plan, uses an action-scoped
native drag intercept, retains only content-free drag lifecycle booleans outside the worker, keeps
CDP drag data opaque, replays native enter/over/drop, and releases or cancels on every terminal
path. Unit coverage and both live drag lanes passed after the fix.

## Fresh Zed harness

A native CachyOS `zed 1.15.0-1` package supplied `/usr/bin/zeditor`, not `zed`. The untouched
installation therefore exposed an incomplete candidate list in Ghostlight's existing declarative
harness descriptor: doctor reported Zed as `NotDetected`. Adding `zeditor` beside `zed` in that one
descriptor made the generic detector report `Available` from an otherwise empty home and config
root. A regression test exercises that exact detector boundary.

Zed was launched against this repository, and the owner accepted Zed's project-specific trust
prompt without trusting the parent directory. Ordinary `ghostlight install --no-open`, with no
forced client selection, then created `~/.config/zed/settings.json` with the exact installed MCP
connector under `context_servers.ghostlight`. Zed's AI settings showed Ghostlight enabled with a
green active indicator. The live process chain was Zed -> its shell -> the exact
`~/.ghostlight/bin/v1.0.0/ghostlight-mcp-connector`, which proves Zed consumed the registration and
completed MCP startup.

A repeat install changed zero bytes in Zed, Codex, Claude Code, or Visual Studio Code
configuration. Explicit removal deleted only Ghostlight's Zed object, retained the empty
`context_servers` object, left those other three harness files byte-identical, and caused Zed to
stop the connector. Ordinary auto-detected reinstall restored the byte-identical Zed file plus the
native Chromium and Applications registrations. As the installer already says, Zed needed a
restart after re-add; it then spawned the exact connector again.

Zed renders a custom local server with its generated initial and a `C` marker. Its current stdio
settings type exposes command, args, environment, enablement, locality, and timeout, but no icon or
display-art field. This is an observed host limitation, not the desired product presentation.
[Research 26](../research/26-linux-harness-roster-and-integration-delight-2026-08.md) records the
visual-identity requirement and why Zed's deprecated MCP extension mechanism does not yet prove a
way to replace that badge.

## Boundary and release decision

This is strong current-source and visible development-host evidence. It is not a release pass for:

- the provenance-attested 17-artifact candidate;
- a clean Ubuntu desktop and package-manager install;
- the matching Chrome Web Store adapter;
- login and reboot;
- the full tray and native-notification matrix;
- simultaneous work through three public MCP harnesses; or
- Windows and Linux clean-machine publication smokes.

Do not publish 1.0 from this record alone. The next highest-value gate is the current candidate on
the owner's Ubuntu GNOME Wayland machine, followed by candidate provenance and public-channel
reconciliation.
