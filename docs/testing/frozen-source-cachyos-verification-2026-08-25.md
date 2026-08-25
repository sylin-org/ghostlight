# Frozen source CachyOS verification -- 2026-08-25

Status: BLOCKED pending owner disposition of three frozen release-tooling defects. The exact
frozen product binaries and visible browser graph passed every independently runnable source and
live check in this lane. No product or extension source changed.

## Environment

```text
runner_head: 2bf2c2b174374b21f5089c8f4af06ceec645af4c (docs-only descendant)
frozen_revision: e7d8986bb96625335cd9cff7d04d7e8b083f845d
architecture: x86_64
distribution: CachyOS rolling
kernel: 7.2.0-1-cachyos
desktop_and_display_protocol: KDE Plasma 6.7.4, Wayland
browser: Chromium 151.0.7922.173, ordinary graphical profile
rust_and_cargo: 1.95.0
node_and_npm: 22.22.1 and 10.9.4
powershell: 7.6.5 portable, temporary verification runtime
ghostlight_version: 1.0.0 development candidate e7d8986b
extension_version: 1.0.0 unpacked source adapter, unchanged and not reloaded
```

`scripts/assert-freeze.ps1` passed before the gates. The runner head is a clean docs-only
descendant of the declared freeze, with no product or packaging diff. `extension/` is
byte-identical to `70869631`, the source state covered by the pending staged Store review.

## Source gates

The exact frozen source passed:

- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo test --workspace`: 400 tests -- 342 orchestrator library, 11 orchestrator binary,
  39 bridge, 7 MCP connector, and 1 cross-platform win-peer test;
- `npm test --prefix extension`: 137 tests;
- `npm test --prefix packaging/npm`: 10 tests;
- `node --test packaging/mcpb/test/launcher.test.js`: 4 tests;
- every `scripts/*.sh` file through `sh -n`;
- a fresh isolated debug workspace build under `.target-linux-frozen-e7d8986`;
- process, CLI, CLI PowerShell, workbench-surface, policy-grammar, and capability-matrix journeys
  against the exact isolated binaries;
- JavaScript syntax for the bundled workbench and preview server;
- `cargo deny check licenses bans sources`;
- `cargo audit`, which exited zero with exactly the 17 accepted GTK/Tauri-chain warnings;
- offline public-surface truth;
- repository integrity: 852 tracked files readable, local links valid, source version aligned,
  the 25 historical ASCII exceptions unchanged, every extension permission justified, and the
  capability matrix complete; and
- complete 0.8 recovery: all 1,388 inventory entries in 12 groups and all 34 Lightbox scenarios
  dispositioned.

The extension packager produced byte-identical ZIPs across two local runs, each with SHA-256
`90d11790fb2a18c68ab99fcf98ce2b4602f1f6e29d7a4a04256ea44f4af377d3`. This is host-local
determinism evidence, not a replacement Store package or a G2 candidate artifact.

## Exact user candidate

An optimized locked workspace build produced all three siblings. They were installed without
removing the prior candidate under `~/.ghostlight/bin/v1.0.0-dev-e7d8986`:

| Sibling | SHA-256 |
| --- | --- |
| `ghostlight` | `bd2ac2fd036a70cd0cbdf45ed404ae92a66e6f0d7b5cbc1f9145ac7848d87fcc` |
| `ghostlight-mcp-connector` | `66feeee60a9821c1aa9b8c18f9bbc4641115741ee0883eb35e14830c39064c4c` |
| `ghostlight-browser-connector` | `0530fcc6836227de322c94e845044d52805169eb7c36941dbdd6289bc89df264` |

The ownership-checked installer updated the command, Applications entry, four browser
registrations, and detected owned MCP registrations to the exact candidate. It preserved the
known foreign Cline entry. `doctor --json` then reported the three exact siblings and native
Chromium current. No Ghostlight process was live during replacement. Starting ordinary Chromium
without automation flags naturally launched the candidate browser connector and moved readiness
to `Ready`; the frozen unpacked extension was neither edited nor reloaded.

## Visible whole-catalog result

The normal-paced command

```text
scripts/demo-foundry.sh --ghostlight ~/.ghostlight/bin/v1.0.0-dev-e7d8986/ghostlight
```

exited zero with all 41 scripted beats green against the visible ordinary Chromium profile.
The run included the mid-story key beat, governed off-domain refusal, replay save and erasure,
sequences, flow, history, and both desk-bell dialog dispositions. The exact active processes were
the revision-qualified orchestrator and browser connector.

The script does not call the newly added 24th catalog tool, `policy_explain`, despite printing
`Whole catalog rehearsed`. A separate call against the same live graph succeeded with effect none
and explained four capability areas over zero configured layers. The exact catalog listed 24
tools.

## Frozen release-tooling findings

No finding below was fixed because coordination message `[0022]` requires defects found after the
freeze to be reported with evidence for owner disposition.

1. `scripts/release-preflight.ps1 -TargetDirectory <custom>` restores `GHOSTLIGHT_BIN_DIR` before
   its queued journey stages execute. The process and CLI journeys therefore silently resolve the
   default `.target-ghostlight-1.0/debug` directory instead of the fresh custom build. On this host
   that stale directory exposed 23 tools and stale readiness language, while direct runs against
   `.target-linux-frozen-e7d8986/debug` passed. Seeding the exact `GHOSTLIGHT_BIN_DIR` in the caller
   also made all three process journeys pass. The frozen product is green; the documented runner
   parameter is not truthful.
2. `scripts/release-preflight.ps1 -IncludeDependencyGates` runs broad `cargo deny check`, which
   contradicts the authoritative `RELEASE.md` split and fails on the already accepted advisory
   set. The authoritative `cargo deny check licenses bans sources` passed, and `cargo audit`
   exited zero with exactly the 17 documented warnings.
3. `scripts/demo-foundry.sh` and `scripts/demo-foundry.ps1` omit `policy_explain` but claim the
   whole catalog. All 41 browser beats passed and the missing 24th tool passed separately, but the
   standing G1 runner no longer proves its literal claim.

## Limits

This is frozen-source, revision-qualified user-candidate, unpacked-adapter, and visible CachyOS KDE
evidence. It is not a provenance-bound G2 candidate, Ubuntu GNOME Wayland, matching Store adapter,
clean machine, native package lifecycle, publication, or release evidence. No merge, tag, upload,
submission, publication, or release action occurred.
