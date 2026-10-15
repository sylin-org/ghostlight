# The Ghostlight 1.0 development loop

Ghostlight has one orchestrator and two deliberately stable shores. Ordinary product changes belong
in `crates/orchestrator`; they must not require MCP connector, browser connector, shared bridge, or
extension changes when the existing contracts already express the needed work.

Acceptance uses the installation in place and a browser launched independently through the normal
desktop. Build elsewhere when live files are locked, then deploy through this loop and verify the
installed graph. Do not start another product instance or browser profile to make an acceptance
check pass. The isolated lanes below retain their narrower component scope. The
[Windows boot investigation](testing/windows-boot-connection-2026-09-08.md) showed how an installer
and an agent-launched browser can share a redirected filesystem view and conceal a real failure.

## Build

`cargo build --workspace` builds the three executables: `ghostlight` (the service and its
workbench), `ghostlight-mcp-connector` and `ghostlight-browser-connector`. Tests and gates run
from any target directory. A build directory never serves on its own: `serve` runs only from
`~/.ghostlight/bin` or from the development build the marker selects (ADR-0197 Decisions 10
and 13).

## Deploying a build

```powershell
pwsh scripts/dev-loop.ps1            # build, then install it as the development build
pwsh scripts/dev-loop.ps1 -Plan      # build, then print what the install would do
pwsh scripts/dev-loop.ps1 -Restore   # go back to the installed service; builds nothing
```

The script builds in `.target-dev-loop` (`-Profile debug` for a debug build) and runs
`<build>/ghostlight deployment install --development`. The installer takes
`~/.ghostlight/install.lock` and checks the destinations of a 1.x migration first when it finds
one, then copies the 1.x policy, managed-policy and client-location files into the home and leaves
the originals for the 1.x service that is still running. It then
rename-swaps the service into `~/.ghostlight/dev` and writes the marker
`~/.ghostlight/development`. While the marker names an existing build, every other `ghostlight`
forwards to it, and `bin/ghostlight serve` hands over to it. Connectors live in `bin`. They are
replaced only when the build speaks another relay revision than `bin/install.json` records; at
the 2.0 break the whole build goes into `bin` as well.

As soon as `bin` holds the executables, and before anything old is stopped, the installer
reconciles every registration to name `bin`, so a browser or client that relaunches a connector
when the old one stops starts the new one. At the 2.0 break it then stops the 1.x images and looks
again for a few rounds for any that respawned. It asks the running service to quit, which gives
in-flight calls 10 s. If it is still running, that exact image is stopped. It then removes the
copied 1.x originals, archives the 1.x audit log, deletes the 1.x lifecycle files, the installation record last, starts
`bin/ghostlight serve --reason update`, and waits up to 30 s for a service that answers a control status with a
new instance. Nothing needs stopping by
hand. A connector that was not replaced keeps running and reconnects.

Agents that run inside Claude desktop or Codex: Windows runs every process those apps start in
the app's container, where registry and `%APPDATA%` writes reach only that app and no browser
sees them. Run `~/.ghostlight/bin/ghostlight.exe install` once from a regular terminal. It
writes the browser registrations, whose `HKCU` values name the manifest in
`~/.ghostlight/manifests`. Later dev loops then work from inside the container: they rewrite
only that manifest and report any registration they had to leave for a regular terminal.
`doctor` inside a container cannot see what other apps see, and says so.

`-Restore` runs `~/.ghostlight/bin/ghostlight deployment install --restore`. It is offline. It
refuses when the connectors in `bin` speak another relay revision than the service there.
Otherwise it removes the marker and restarts the installed service. `-Plan` with either form
prints one JSON document: the migration plan, the swaps, the connector replacements, the
registrations to rewrite and the processes to stop. Nothing changes.

Packages, npm and the get scripts run the same command without `--development`. A release refuses
an older payload and does nothing for the same intact version. It writes `bin/install.json` only
after the new service publishes, so a service that does not start is installed again on the next
launch.

## What to restart

| Changed area | Rebuild | Refresh |
| --- | --- | --- |
| Orchestrator domain, workbench, or bundled UI | `cargo build -p ghostlight` | Restart `ghostlight`; no relay or extension change |
| MCP connector protocol lifecycle | `cargo build -p ghostlight-mcp-connector` | Reach `bin` (see below), then reconnect the MCP server in the harness |
| Browser connector relay lifecycle | `cargo build -p ghostlight-browser-connector` | Reach `bin` (see below), then reload the extension so Chromium respawns the native host |
| Chromium extension mechanism or presentation | none for JavaScript | Reload the unpacked extension explicitly in `chrome://extensions` |
| Shared bridge contract | `cargo build --workspace` | Restart only consumers affected by that versioned boundary |

Do not restart a shore merely because an orchestrator feature changed. That is the fringe-stability
contract, not an optimization.

Rebuilding does not deploy a connector. The installer replaces the connectors in `bin` only when the
relay revision differs from the one `bin/install.json` records (ADR-0197 Decision 14). Under one
relay revision, a connector source change reaches an installed home only through a revision change
or a forced replacement. To force one, remove `~/.ghostlight/bin/install.json` and run
`pwsh scripts/dev-loop.ps1 -Plan` first; the plan must list the connector replacements before you
run the loop for real. The record is rewritten once the new service publishes. Until the connectors
are replaced, the old ones keep running the old code.

An extension reload invalidates content-script receivers in already-open pages. The current
adapter does not bootstrap them back into those documents. Refresh a disposable verification tab
after reloading; do not automatically refresh user pages that can contain unsaved drafts. A fresh
navigation is also enough. Reloading the extension alone does not prove existing pages are ready.

## Container builds and host deployment

When build dependencies live in Toolbox but the installed authority runs on the
host, keep the deployment controller on the host. Delegate only its Cargo command
to Toolbox. A shared filesystem or PID namespace does not prove that the container
can read a host process's exact executable path. On Bluefin that path was unavailable
inside Toolbox: the controller found no matching image and copying failed with
`Text file busy`. The host controller identified and stopped the exact same PID,
then completed the ordinary locked swap. Do not replace exact-path ownership checks
with process-name termination to work around this boundary. See the
[Bluefin evidence](testing/bluefin-fleet-2026-09-09.md).

## When an agent reports an error

Turn the process log on, reproduce, read the story:

1. `ghostlight diagnostics on` (creates the marker; every running component picks it up within
   about two seconds) or set `GHOSTLIGHT_DIAGNOSTICS_DIR` in the client's server configuration.
2. Reproduce the failure.
3. `ghostlight diagnostics show --last 10m` reads one merged timeline; `--json`, `--component`,
   and `--op` filter it. `doctor` names the active layer and folder.
4. The folder is content-free by construction (no page content, no payloads), so it can be
   attached to a report as-is. Prune is automatic; `ghostlight diagnostics prune` forces it.

## Automated gates

The complete hardening suite has one local entry point:

```text
node tests/hardening-suite.mjs
```

Set `GHOSTLIGHT_TEST_BROWSER` to Chrome for Testing. The runner builds into the isolated
`CARGO_TARGET_DIR` (default `.target-ghostlight-1.0`) and passes that exact build to every journey.
It includes the omitted real-browser and continuity lanes, writes per-gate evidence, and fails if
a required lane cannot run or source changes during execution. See the
[feature coverage guide](tasks/security-hardening/regression-suite.md). Installed validation uses
`node tests/live-journey.mjs` separately after the changed components are active.

On Windows, the full and process lanes include the `desktop-windows` gate. It requires
PowerShell 7 and a Windows desktop. The journey launches an isolated authority, sends concurrent
Open requests during startup, counts its real native workbench windows, and checks responsiveness,
minimize/restore, and close/reopen while the authority stays alive. It uses the fresh
`GHOSTLIGHT_BIN_DIR` build and changes only its own processes, windows, endpoint, and WebView
profile. It also starts both connectors with held input, delivers the Windows end-session sequence
to all three exact processes, and requires bounded successful exit, including the worst ordering
where Tao's event target ends before the authority's dedicated listener. It verifies the spawned
WebView uses that isolated profile; the installed stack and browser registration stay intact. To
run it directly after building:

```powershell
pwsh -NoProfile -File tests/windows-desktop-journey.ps1
```

Linux installed-browser, desktop, and package-consumer acceptance drivers live in
[`tests/linux/`](../tests/linux/README.md). Its runner accepts per-environment exports for
test areas, browser executables, candidate packages, and guest build tools. These opt-in
journeys remain separate from the ordinary hardening gates; their generated state stays ignored.

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test --prefix extension
node --test "crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js"
sh -n scripts/*.sh
cargo build --workspace --target-dir .target-ghostlight-1.0
node tests/process-journey.mjs
node tests/workbench-surface.mjs
node --check crates/orchestrator/ui/app.js
node --check tests/workbench-preview-server.mjs
```

`process-journey.mjs` uses repository-local runtime and audit files and the isolated target above.
It resolves the executables from `.target-ghostlight-1.0/debug` unless `GHOSTLIGHT_BIN_DIR` points
elsewhere, so a build into a different target directory needs that variable or the journey passes
against stale binaries.
It places the deployment lock to isolate the established reconnect proof, starts the real
executables, interrupts an in-flight operation by stopping the service, proves both relays stay
alive and renegotiate, then completes new open/read/close work. Bridge and orchestrator tests cover
demand-start admission; the live journey starts from no service and proves adapter demand-start.

The workbench preview server supplies only representative local test facts to the exact bundled
HTML, CSS, JavaScript, and artwork:

```powershell
node tests/workbench-preview-server.mjs
```

Open `http://127.0.0.1:41737/` for layout and interaction review. It never connects to the
orchestrator or changes harness configuration.

## Live browser validation

For the isolated script-correctness lane, use Node 22 or newer and a Chromium executable:

```powershell
cargo build --workspace --target-dir .target-ghostlight-1.0
node tests/script-browser-journey.mjs
```

`GHOSTLIGHT_TEST_BROWSER` can name the browser executable; `GHOSTLIGHT_BIN_DIR` can name a
different fresh build directory. The lane opens a visible browser with its own temporary profile
and HTTP fixture,
executes the shipped evaluator through Chromium CDP, and carries results through the real browser
connector, orchestrator, and MCP connector. It checks effect counts, uncertain failures, and REPL
compatibility. Native framing is a test adapter: this is not an installed MV3/native-host journey.
It isolates runtime, audit, policy, diagnostics, and registration paths, then removes its own
fixtures. It neither reloads an existing extension nor changes a person's browser profile.

Build all three executables side by side and ensure the platform native-messaging manifest points
to that `ghostlight-browser-connector` (on Windows the manifest is
`~/.ghostlight/manifests/org.sylin.ghostlight.json`). Load `extension/` unpacked and reload it after every
extension edit. Its pinned development identity and `org.sylin.ghostlight` host name must not
change.

Run `ghostlight open` to restore the backgrounded workbench, open Status, and verify a compatible
browser instance appears. Use a
supported MCP client registration from MCP integrations, reconnect that client, and execute the
journeys in [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md). Use the `sylin.org` demo pages for visible
form, embedded-frame form, upload, dialog, navigation, policy-denial, and screenshot work.

Never treat a clean Ghostlight screenshot as evidence that presentation did not render: the
extension intentionally hides its feedback layer while capturing page pixels.

## Isolation and cleanup

For tests, set `GHOSTLIGHT_TEST_HOME` to an absolute, repository-local temporary directory and
`GHOSTLIGHT_AUDIT_FILE` to a temporary path. The test home replaces `~/.ghostlight` for every
process that sees it: the endpoint (a pipe or socket named from that home) and
`run/service.lock` both follow it, so a test service never meets the running one. A connector run
with it demand-starts `bin/ghostlight` under that home, so put the build under test there --
a test home with no service executable is a loud failure, never a fallback to the running tree.
Stop only processes whose exact executable path is the build you started. Never terminate all
`ghostlight`, browser, or MCP-client processes by image name; an installed or user-owned stack may
be running at the same time.
