# Windows bounded startup follow-up

## Candidate and scope

Runtime source: `09890c9f162bca92f485c9a0dbfd1de07f03494f`.
Owned branch: `codex/fleet-windows-startup`, in the ordinary Ghostlight repository
on leo-desktop-02. The previous Windows branch, worktree, packages, failed attempts,
and acceptance evidence remain intact.

This is one installed Windows check of ADR-0166's generic startup/readiness
change. All three siblings changed, so the existing NSIS package path replaced
all three. No additional runtime or installer change was needed. The production
hook remains SHA-256
`c34cac3decccc7423b41139faf81f727e91cd962ba739c35413b56283a74f408`.

The baseline was the prior combined 1.3.5 installation: authority PID 14452,
VS Code connector PID 13700, no connected browser adapter, and no deployment lock.
The installed authority hash was
`186281aea560efe0bd29ef5cbf265a9a82e6f03aa72430c58670c16777782503`.
Baseline installed/configuration hashes and exact process identities were retained
before replacement.

## Build and package identities

The locked release workspace build passes in `.target-integration/release`.
Tauri CLI 2.11.0 and the existing NSIS compiler produce the package retained as
`.tmp/fleet-leo-desktop-02/startup-09890c9f-setup.exe`.
Package SHA-256:
`dc17b23cbded8f3cbd5b28ca1602192bb70984fa886e98d4362a41ee03dbc9cc`.

| Binary | Raw release SHA-256 | Installed SHA-256 |
| --- | --- | --- |
| Authority | `07791e5daa3a99372d4ec519ff6f086f425e770a58c456fa28e37708fd28eb87` | `3f21991c594d8ddf12a2f7c481cfe1d5a4439e49eb097f73f48ecfeceb69392f` |
| MCP connector | `5640b58748747fa25c35c141e209550bb154b03ab13c2572dcc476b0d48cbaf1` | `5640b58748747fa25c35c141e209550bb154b03ab13c2572dcc476b0d48cbaf1` |
| Browser connector | `54432d71edb75817941c5619b416fd124cbb636df657f6b50dc4541e82a6b260` | `54432d71edb75817941c5619b416fd124cbb636df657f6b50dc4541e82a6b260` |

All installed bytes match their expected values. As in the prior lane, Tauri
changes the authority's one `__TAURI_BUNDLE_TYPE_VAR_UNK` marker to
`__TAURI_BUNDLE_TYPE_VAR_NSS` during packaging. Hashing the complete build bytes
with that one substitution yields the exact installed authority hash above.

## Installed execution

1. Ordinary `/S /UPDATE` began at 18:46:16 PDT while VS Code was connected.
   The package exited 0 in 20.35 seconds and removed its deployment lock.
   The two old installed processes ended. No manual process termination or
   registration rewrite outside the normal package was used.
2. At 18:47:21, an OS process query confirmed no Ghostlight authority or connector
   was running, and no deployment marker remained. This establishes the cold
   baseline independently of the subsequent model result.
3. Actual Codex CLI invoked `policy_explain` exactly once through the installed
   MCP connector. It succeeded with `effect: none` and `history_storage: saved`;
   the CLI exited 0. The connector was PID 9408 and its child authority PID 24252.
   Across 134 OS samples over 25 seconds, the maximum observed authority count
   was one, with only that authority identity observed.
4. The ephemeral Codex run ended with no remaining Ghostlight process. This lane
   proves cold startup and a real model invocation, not authority survival after
   that client's shutdown. No duplicate or failed-start loop was observed.
5. VS Code 1.137.0's existing server entry had reported process termination during
   the upgrade. Its ordinary Start Server action recovered the unchanged entry:
   Running at 18:49:23, 23 tools discovered at 18:49:28. The settled installed
   graph is connector PID 23920 and exactly one authority, PID 1560.
6. Doctor confirms running 1.3.5 with current Chrome, Edge, Brave and Chromium
   registrations and no deployment lock. Native window enumeration finds one
   Ghostlight workbench, window 919046, and inspection reports it minimized.
   This is the expected initial backgrounded desktop route. Browser readiness
   remains Not connected; desktop readiness is not a browser connection claim.

Codex has no saved Ghostlight registration on this host. The actual CLI test
therefore reuses the same explicit per-run `-c mcp_servers.ghostlight.command=...`
configuration and installed physical connector path as the prior accepted run,
with `--ignore-user-config --ephemeral --sandbox read-only`. It does not claim
automatic discovery from a saved Codex registration. Both the saved Codex
`config.toml` and VS Code `mcp.json` hashes are unchanged. All 19 baseline VS Code
and Edge processes survive with matching PID and creation time.

## Validation and evidence

- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo test --workspace --locked`: 537 passed, none failed.
- Extension `npm test`: 222 passed, none failed.
- Real process journey: PASS with `GHOSTLIGHT_BIN_DIR` explicitly set to this
  candidate's `.target-integration/release`. This remains a synthetic-browser
  component lane, separate from the actual installed client checks above.

Ignored evidence stays under `.tmp/fleet-leo-desktop-02/startup-*`: build and
quality logs, frozen package, before/after hashes, cold-baseline process snapshot,
25-second sample series, full Codex JSONL and receipt summary, configuration hash
comparison, unrelated-process survival, final doctor, and final installed state.
No broad campaign rerun, saved client change, shared branch merge or public release
was performed. Only this report and project status change beyond the exact runtime
candidate.

No adapter was independently loaded since the preceding report. Ordinary browser
effects remain unverified. The prohibited extension-management route and rejected
held-file uninstall were not retried. This check adds no reboot, pristine-machine,
uninstaller or browser acceptance claim.
