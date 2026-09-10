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
