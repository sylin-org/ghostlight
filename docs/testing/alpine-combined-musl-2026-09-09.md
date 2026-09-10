# Alpine combined-candidate compatibility

## Candidate and scope

The coordinator delegated one bounded musl compatibility pass of exact integrated
source `0129ff15bf0c6348a78e8c0d2e4e1d2716695ae8` from `origin/codex/fleet-next`.
Work uses the ordinary repository on `codex/fleet-test-03-combined`. The previous
startup repair and installed acceptance ended at `f3396e31`; this round guards the
new Linux activation dependencies, native routing and explicit Flatpak installer.
No product change was required by the checks below.

This is the existing Alpine 3.24.1/KDE Wayland native-musl feasibility lane. It adds
no Alpine Flatpak support or public musl support promise. No physical reboot,
packaging campaign, public release, Flatpak setup or permission grant was performed.
The previous installation and adapter worktree remain at their established paths.

## Source gates and real executable boundaries

All three siblings were built at the exact candidate with:

```sh
RUSTFLAGS='-C target-feature=-crt-static' cargo build --workspace --locked \
  --target x86_64-unknown-linux-musl --target-dir .target-fleet-startup
```

Each binary is an x86-64 ELF dynamically linked through `/lib/ld-musl-x86_64.so.1`.
The same target and Rust flags were used for warnings-denied workspace/all-target
Clippy and the Rust workspace suite. Formatting, Clippy, all 564 Rust tests and all
222 extension tests passed. One opt-in private-bus test was ignored by the ordinary
suite and then run explicitly, successfully. Extension source did not change.

Both process journeys used the explicit absolute
`GHOSTLIGHT_BIN_DIR=.target-fleet-startup/x86_64-unknown-linux-musl/debug` resolved
from this repository, and ran sequentially to avoid deployment-lock interference:

- `node tests/process-journey.mjs` passed real connector/authority reconnect,
  open/read/flow, capture, recording, audit recovery and cold policy explanation.
- `node tests/linux/startup-recovery.mjs` passed four sanitized concurrent callers:
  three failed attempts in 11.5 seconds, peak one transient authority, no discovery
  publication and no successful MCP initialization. All four corrected-context
  callers recovered to one authority in 5893 ms.
- `cargo test -p ghostlight-bridge --locked --target x86_64-unknown-linux-musl
  --target-dir .target-fleet-startup --test desktop_activation -- --ignored
  --nocapture` passed the source private-session-bus test. Its fixture alone
  registers the fixed name under a temporary home and bus, proves deployment
  suppression, exact activation, one owner and cleanup. This is mechanism evidence,
  not an installed Alpine Flatpak lane or a host permission grant.

The earlier startup report's `after/result.json` was preserved around the new run;
this round has its own result copy. No previous failure record was discarded.

## Installed native acceptance

All three consumers import the changed bridge lifecycle, so all three were selected
for replacement. The source extension is unchanged. Deployment used the established
`dev-loop.ps1` controller adaptation for the preserved installation and musl target
layout, verified to differ from source only at those exact path/layout seams.
The ordinary lock, exact-image stop, bounded copy retry and cleanup remain intact.
No native-host registration was rewritten.

Chromium was closed normally for the configured Codex cold-start check. Following
deployment, a controller-owned authority-only quiesce left a verified empty installed
stack. Actual `codex exec --ephemeral --json` using the saved registration started
one authority (PID 1759) and completed exactly one `policy_explain` call with
`succeeded` and effect `none`. It exited successfully in 14.08 seconds including
model/network time. No existing connector needed to be paused in this run.

Normal Applications launch of the existing native Chromium profile rejoined that
same authority. An explicit adapter reload rejoined again with browser identity
`browser_fec1a57a95d64265a452c5569c428714` and adapter 1.1.2. A fresh local fixture
navigation and read through the installed MCP connector succeeded and returned
`ALPINE-MUSL-135`. This direct protocol check is distinct from the actual Codex call.
`ghostlight open` showed exactly one active, unminimized native workbench owned by
PID 1759. Doctor reported Ready, connected and idle, service 1.3.5.

Final hash comparisons prove the saved Codex configuration, native Chromium host
registration and every loaded adapter source file are unchanged. All installed
binary hashes match the exact combined build. No deployment marker or paused
connector remains; diagnostics remain off. Alpine still has no Flatpak executable,
Ghostlight D-Bus activation file, Flatpak Chromium override/native registration or
activation custody file. The private test bus left no installed activation state.

## Exact combined installed binaries

| Executable | SHA-256 |
| --- | --- |
| ghostlight | `6499ba5c32a97f965734176c66186f15f4b9ceaa253a279ed59ab66c34ba0731` |
| ghostlight-mcp-connector | `e71d33d2ab7660ca78e71bf713e3780c59c306e4b2bfc33262cecd8f22635f0f` |
| ghostlight-browser-connector | `504bdb6f316a858c1290947cf0ca06a13b06dc057b11b428b06de30619c041ee` |

The existing installed path is
`/home/test/ghostlight-fleet/test-03/installed/1.3.5-musl-debug`.
The old installed hashes are retained in the round baseline; they match the prior
[startup acceptance record](linux-startup-recovery-2026-09-09.md).

## Evidence

Machine-local generated evidence is under `.tmp/linux-combined-musl/`:
`baseline.json`, `build-hashes.json`, `gates.json`, `journeys.json`, their named logs,
`startup-recovery-result.json`, `deploy.log`, `cold-prepare.log`,
`installed-codex-result.json`, `installed-browser-result.json` and
`preservation-result.json`.
The actual Codex event stream is retained locally; the report uses its sanitized
result summary. No session address, endpoint token, credential or browser content
is copied into the tracked report.
