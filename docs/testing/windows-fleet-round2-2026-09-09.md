# Windows acceptance round two

Starting source: `ac1becab78e5a8a6b89e553e82f6779d93dcb39c`.
Working branch: `codex/fleet-windows-acceptance` in the ordinary repository.
The prior fleet worktree and its artifacts remain intact; no new worktree was made.

## Milestone: installed baseline and hook audit

The existing installed authority remains 1.3.5 from the previous lane. Read-only
doctor says Not connected, and no native browser connector is running. No adapter
installation or changed control capability was observed. The single human step
needed for the ordinary-browser lane is to load the current repository's
`extension` directory as the unpacked adapter in the ordinary Chrome profile.
Extension management remains prohibited to the available Browser Use tool; no
alternate route or repeat of the rejected held-file uninstall test was attempted.

The installed binaries are unchanged from the previous report:

| Binary | SHA-256 |
| --- | --- |
| Authority | `adc4a0691c0d24192246bdb5cce922abe88d4e9ea775ed1c7517e150e016db99` |
| MCP connector | `d01465b67c3535985f20205f933126f90ce709bcacb6141a09d0600f191a92ed` |
| Browser connector | `7595fa7a8149386a496254cf8f4681ac620d605db54a107fc12eabe311f382d4` |

The installed authority is the only running Ghostlight process at baseline, and
there is no deployment marker. Developer tools and runtimes are preinstalled;
this machine is not a pristine consumer lane. Exact process and path observations
are retained in the ignored `round2-baseline.json` and doctor output.

## Executed NSIS contract

`tests/windows/nsis-hooks.ps1` compiles and runs the production hooks through a
small NSIS fixture. It creates only new directories under repository `.tmp`,
never writes an uninstaller, and never registers a host or replaces a product.
Its evidence is retained even on failure. Run it with installed Tauri NSIS or
set `GHOSTLIGHT_MAKENSIS` to an existing compiler path.

| Case | Result |
| --- | --- |
| Empty destination | PASS: preparation succeeds and its marker is removed |
| Another deployment's existing marker | PASS: exit 1, original marker bytes preserved |
| Directory occupying an executable path | PASS: access preflight refuses, owned marker removed |
| Explicit failure after lock acquisition | PASS: exit 23, failure callback removes owned marker |
| Existing authority outside fixture | PASS: original PID and creation time survive all four cases |

This directly exercises success and failure behavior, rather than reading the
hook source as a proxy. It does not exercise the previously rejected installed
uninstaller, a held file, interactive Cancel, or final package deletion. Those
paths remain unverified. Review found no additional demonstrated runtime defect
in the paths above, so the product hook is unchanged.

The exact hook source SHA-256 is
`c34cac3decccc7423b41139faf81f727e91cd962ba739c35413b56283a74f408`.
The fixture executable SHA-256 is
`77784cb9662af929ffa9d3f163a39aa13c1351e1857b50948d28d54e4b7bb060`.
Its compiler deliberately reports unused uninstall code because no uninstaller
is written; both production function families are compiled, only install runs.

Formatting, warnings-denied workspace Clippy, all 533 Windows Rust tests, and all
222 extension tests pass on the combined baseline. A fresh release build and
package identity are the next independent step; this milestone does not claim
that combined binaries are installed. No connector source, policy, credential,
browser state, public release, or shared integration branch was changed.

## Combined release package and installed acceptance

The release workspace build passes with locked dependencies. Both executable
journeys run with `GHOSTLIGHT_BIN_DIR` set to this repository's
`.target-integration/release`, never a default or stale target. The process
journey passes recovery across real executable boundaries with a synthetic
browser. The provenance journey passes with 12 service receipts and three
OS-observed caller images. Neither proves ordinary browser behavior.

The runtime source is exactly `ac1becab78e5a8a6b89e553e82f6779d93dcb39c`.
The package was assembled after test/document commit `13e378de`, which changes no
product source, using Tauri CLI 2.11.0 and the installed NSIS compiler. The
bridge and both connector source trees are unchanged from the prior Windows
lane. Different build hashes do not imply connector feature changes.

Package: `.target-integration/release/bundle/nsis/Ghostlight_1.3.5_x64-setup.exe`.
SHA-256: `4d7ea43bfae3e44f94faeb6e557b978f8cd801219e0dc4c056434c6b5f7a128e`.
A frozen copy is retained as `round2-13e378de-setup.exe` in the ignored evidence
directory `.tmp/fleet-leo-desktop-02`.

The ordinary `/S /UPDATE` package ran against the existing installation while
VS Code 1.137.0 was connected and had discovered 23 tools. Before replacement,
the installed authority was PID 18980 and its MCP connector was PID 13752.
Both ended during package preparation. All three siblings were replaced.

The installer was still running at the initial 30-second observation deadline;
its marker and already-replaced binaries were preserved as intermediate
evidence. By the subsequent observation at 17:26:47 PDT it had exited and its
marker was absent. The launcher did not retain the eventual exit code. This is
an observation limit, not a claimed zero exit or demonstrated installer failure.
No retry was made. File hashes, registration, and client operation independently
verify the resulting installation.

| Installed binary | Expected and actual SHA-256 |
| --- | --- |
| Authority | `186281aea560efe0bd29ef5cbf265a9a82e6f03aa72430c58670c16777782503` |
| MCP connector | `31e60a04ad794b35de53680b281aa8462d939b87bb218b03f893d3f046c67a74` |
| Browser connector | `2ebf6aef491d1e05d7eda84565a0dbefa9f0d81edb89a994d874c08e3cdd3c8a` |

The authority's raw build hash is
`de3e091fa44bf78ea6451ed58f76a19d91314d6f91933504b183d677e62ffd7f`.
Its expected installed hash is computed from the full build bytes with the one
Tauri bundle marker changed from `__TAURI_BUNDLE_TYPE_VAR_UNK` to
`__TAURI_BUNDLE_TYPE_VAR_NSS`. The installed authority matches that complete-byte
expectation; both connectors match their raw builds. Chrome, Edge, Brave, and
Chromium registry entries still point to the installation's physical manifest
under the Codex package LocalCache, as required by the prior Windows repair.

An actual Codex CLI model invoked `policy_explain` exactly once through the
installed MCP connector. The result succeeded, reported four available
capability areas over zero policy layers, `effect: none`, and
`history_storage: saved`. No browser tool or durable client configuration change
was involved. Evidence is `round2-codex-policy.jsonl` and its exit-result file.

VS Code's existing connector reported termination during replacement. Starting
the same configured MCP server through VS Code then reached Running and
rediscovered 23 tools at 17:27:53 PDT. This is transport/catalog evidence; Copilot
was not signed in, so it is not a VS Code model-call claim. The settled installed
graph has MCP connector PID 13700 and authority PID 14452. Doctor reports 1.3.5
running and Not connected because no browser adapter is attached.

## Remaining limits

- Load the current repository `extension` directory as an unpacked adapter in
  the ordinary Chrome profile to enable this machine's browser acceptance.
  Available tool policy still blocks agent-managed extension setup.
- The final installed uninstaller and held-file refusal remain unverified after
  automatic approval review rejected the earlier command as `blocked by policy`.
  This round did not retry or use another route.
- No reboot, pristine Windows consumer environment, real browser effect, or
  cross-browser/profile acceptance is claimed for this combined candidate.
- No additional product defect was demonstrated in this round. The committed
  change is executable NSIS failure-path coverage and evidence, not a new
  installer implementation. The earlier Windows FFI and installer repairs
  remain part of the integrated runtime source.

The package and report stay on the owned Windows acceptance branch. No shared
branch merge, credential change, public release, or recurring monitor was made.
