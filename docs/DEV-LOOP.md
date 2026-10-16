# The Ghostlight 1.0 development loop

Ghostlight has one service and two deliberately stable shores. Ordinary product changes belong
in `crates/orchestrator`; they must not require edge, relay, or extension changes when the
existing contracts already express the needed work.

Acceptance uses the installation in place and a browser launched independently through the normal
desktop. Build elsewhere when live files are locked, then deploy through this loop and verify the
installed graph. Do not start another product instance or browser profile to make an acceptance
check pass. The isolated lanes below retain their narrower component scope. The
[Windows boot investigation](testing/windows-boot-connection-2026-09-08.md) showed how an installer
and an agent-launched browser can share a redirected filesystem view and conceal a real failure.

## Build

`cargo build --workspace` builds the three executables: `ghostlight-service` (the service and its
workbench; it links no UI toolkit), `ghostlight` (the edge: the MCP relay, the browser relay and
the front door that hands every other command to the service) and `ghostlight-desktop` (the shell:
the window, the tray and the notices; on Linux its build needs the WebKitGTK 4.1 development
packages). Tests and gates run from any target directory. A build
directory never serves on its own: `serve` runs only from `~/.ghostlight/bin` (ADR-0197
Decisions 10 and 13).

## Deploying a build

```powershell
pwsh scripts/dev-loop.ps1            # build, then install it into bin as the development channel
pwsh scripts/dev-loop.ps1 -Plan      # build, then print what the install would do
```

There is zero or one Ghostlight on a machine, and a deploy replaces it in place (ADR-0199, the
development channel amendment). The script builds in `.target-dev-loop` (`-Profile debug` for a
debug build) and runs `<build>/ghostlight-service deployment install --development`. The
installer takes `~/.ghostlight/install.lock` and checks the destinations of a 1.x migration first
when it finds one, then copies the 1.x policy, managed-policy and client-location files into the
home and leaves the originals for the 1.x service that is still running. It then rename-swaps the
build into `~/.ghostlight/bin` the way a release install does:

- The service image is replaced every time. A development install never stops at "same version"
  and never refuses an older or a newer one.
- The edge is replaced when `bin/install.json` is absent, records no edge, records another relay
  revision than the build speaks, or when the edge in `bin` is missing or is not the one the record
  names. Otherwise the edge stays and keeps running. The same holds for the shell when the build
  has one: it is replaced when its file differs.
- Once the new service publishes, `bin/install.json` is written with `"channel": "development"`.

Nothing is written into `dev` and no marker is written. `ghostlight doctor` and
`ghostlight status` print `Channel: development` while the installation is a development build.
A release package never installs over it: its carrier runs the installed service, and
`deployment install` from a release package refuses and names the restore commands below.

As soon as `bin` holds the executables, and before anything old is stopped, the installer
reconciles every registration to name `bin`, so a browser or client that relaunches its host
when the old one stops starts the new one. At the 2.0 break it then stops the 1.x images and looks
again for a few rounds for any that respawned. It asks the running service to quit, which gives
in-flight calls 10 s. If it is still running, that exact image is stopped. It then removes the
copied 1.x originals, archives the 1.x audit log, deletes the 1.x lifecycle files, the
installation record last, starts `bin/ghostlight-service serve --reason update`, and waits up to
30 s for a service that answers a control status with a new instance. Nothing needs stopping by
hand. An edge that was not replaced keeps running and reconnects.

Earlier development builds ran from `~/.ghostlight/dev`, selected by the marker
`~/.ghostlight/development`, and `bin/ghostlight-service` stayed an old image that forwarded to
them. A deploy that finds either one stops what runs from `dev` (the service first, by the quit,
then any edge or shell), removes the directory and the marker, and reports an entry a running
program kept; the next deploy removes it.

The 1.x launchers unpacked each release into `~/.ghostlight/bin/v<version>`. Every deploy and
`ghostlight install` stops what runs from such a directory (a directory directly under `bin`
named `v` and a semantic version; nothing else in `bin` is touched), moves its `audit.jsonl` to
`~/.ghostlight/state/audit-v<version>.jsonl`, and removes the directory. A log that cannot be
moved keeps its directory, and `--plan` lists each directory and each move.

Agents that run inside Claude desktop or Codex: Windows runs every process those apps start in
the app's container, where registry and `%APPDATA%` writes reach only that app and no browser
sees them. Run `~/.ghostlight/bin/ghostlight.exe install` once from a regular terminal. It
puts `ghostlight` on the user PATH, writes the browser registrations, whose `HKCU` values
name the manifest in `~/.ghostlight/manifests`, and writes the Start-menu shortcut and the
installed-apps entry, saying each one it changed (`--dry-run` says which it would). Later dev loops then work from inside the
container: they rewrite only that manifest and report any registration they had to leave for a
regular terminal.
`doctor` inside a container cannot see what other apps see, and says so.

Going back to a release needs a release carrier, because restore installs that carrier's own
package. Run it from the carrier:

```powershell
npx ghostlight deployment install --restore                     # the npm package
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.ps1))) -Restore
curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.sh | sh -s -- --restore
```

Until 2.0 is published, the published npm package and the main-branch get scripts have no 2.0
`--restore`, so the way back is a 2.0 build's own `deployment install --restore`, run from that
build's directory.

`--restore` takes the same optional payload directory as a release install. It installs that
package into `bin` whatever version or channel is recorded (the one allowed move down), replaces
the edge by the usual rule, restarts the service and records `"channel": "release"`. `-Plan` and
`--plan` print one JSON document: the migration plan, the swaps, the registrations to rewrite, the
processes to stop and the files to delete. Nothing changes.

Packages, npm and the get scripts run the same command without `--development`. A release
refuses an older payload, refuses a development installation, and does nothing for the same
intact version with an intact edge. It writes `bin/install.json` only after the new service
publishes, so a service that does not start is installed again on the next launch.

## What to restart

| Changed area | Rebuild | Refresh |
| --- | --- | --- |
| Orchestrator domain, workbench, or bundled UI | `cargo build -p ghostlight-service` | Run the dev loop: it replaces the service in `bin` every time; no relay or extension change |
| Edge as the MCP relay | `cargo build -p ghostlight` | Reach `bin` (a relay revision change or a forced replacement, see below), then reconnect the MCP server in the harness |
| Edge as the browser relay | `cargo build -p ghostlight` | Reach `bin` (a relay revision change or a forced replacement, see below), then reload the extension so Chromium respawns the native host |
| Chromium extension mechanism or presentation | none for JavaScript | Reload the unpacked extension explicitly in `chrome://extensions` |
| Shared bridge contract | `cargo build --workspace` | Restart only consumers affected by that versioned boundary |

Do not restart a shore merely because an orchestrator feature changed. That is the fringe-stability
contract, not an optimization.

Rebuilding does not deploy the edge by itself. A development deploy replaces the service in `bin`
every time, and the edge only when the relay revision differs from the one `bin/install.json`
records, there is no record, or the edge in `bin` is missing or is not the one the record names
(ADR-0197 Decision 14). Under one relay revision, an edge source change reaches an installed home
only through a revision change or a forced replacement. To force one, remove
`~/.ghostlight/bin/install.json` and run `pwsh scripts/dev-loop.ps1 -Plan` first; the plan must
list the edge among the swaps before you run the loop for real. The record is rewritten once the
new service publishes. Until the edge is replaced, the old one keeps running the old code.

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
`node tests/live-journey.mjs` separately after the changed components are active. It runs the
installed `~/.ghostlight/bin/ghostlight mcp` (an edge outside the home's `bin` is a carrier, and
`mcp` through a repo-built one would carrier-install that build as a release), records the SHA-256 of the installed edge, service and shell, and needs
all-open authority. `GHOSTLIGHT_BIN_DIR` may name that `bin` directory; it must be the home's own.
`tests/settlement-live-journey.mjs` drives the installed stack the same way.

On Windows, the full and process lanes include the `desktop-windows` gate. It requires
PowerShell 7 and a Windows desktop. The journey copies the edge, the service and the shell from
the fresh `GHOSTLIGHT_BIN_DIR` build into a test home, sends concurrent `ghostlight-service open`
requests while the service starts, counts the real native workbench windows (they belong to the
shell), and checks responsiveness, minimize/restore, and close/reopen while the service stays
alive. It changes only the processes, windows, endpoint and WebView profile of that home. It also
starts both edge roles with held input and delivers the Windows end-session sequence to them and
to the service, requiring a bounded successful exit and that the shell leaves with the service. It
verifies the spawned WebView uses the home's profile; the installed stack and browser registration
stay intact. To run it directly after building:

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

On an elevated Windows token (CI's runners), a debug build refuses to serve unless
`GHOSTLIGHT_TEST_ALLOW_ELEVATED=1`; release builds never read it, and `tests/elevation-journey.mjs`
proves the refusal there.

`GHOSTLIGHT_TEST_BROWSER` can name the browser executable; `GHOSTLIGHT_BIN_DIR` can name a
different fresh build directory. The lane opens a visible browser with its own temporary profile
and HTTP fixture,
executes the shipped evaluator through Chromium CDP, and carries results through the real browser
relay, service, and MCP relay. It checks effect counts, uncertain failures, and REPL
compatibility. Native framing is a test adapter: this is not an installed MV3/native-host journey.
It isolates runtime, audit, policy, diagnostics, and registration paths, then removes its own
fixtures. It neither reloads an existing extension nor changes a person's browser profile.

Build both executables side by side and ensure the platform native-messaging manifest points
to that `ghostlight` (on Windows the manifest is
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
`run/service.lock` both follow it, so a test service never meets the running one. An edge run
with it demand-starts `bin/ghostlight-service` under that home, so put the build under test there --
a test home with no service executable is a loud failure, never a fallback to the running tree.
Stop only processes whose exact executable path is the build you started. Never terminate all
`ghostlight`, browser, or MCP-client processes by image name; an installed or user-owned stack may
be running at the same time.
