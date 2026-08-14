# The Ghostlight 1.0 development loop

Ghostlight has one orchestrator and two deliberately stable shores. Ordinary product changes belong
in `crates/orchestrator`; they must not require MCP connector, browser connector, shared bridge, or
extension changes when the existing contracts already express the needed work.

## Build

```powershell
cargo build --workspace
```

The sibling executables are:

- `target/debug/ghostlight` -- orchestrator plus Tauri workbench;
- `target/debug/ghostlight-mcp-connector` -- MCP stdio shore; and
- `target/debug/ghostlight-browser-connector` -- Chromium native-messaging shore.

Run the product with its workbench:

```powershell
target/debug/ghostlight
```

Run only the persistent orchestrator service:

```powershell
target/debug/ghostlight --headless
```

The normal launch always starts the complete desktop authority with its tray and a backgrounded
workbench: minimized on Windows and hidden on Linux. Connectors demand-start that same sibling
executable with no application arguments. A second direct launch opens and focuses the workbench
owned by the running authority. Windows restores its existing view; Linux reconstructs its
disposable view because Wayland cannot report or unset minimization. Closing destroys only the
workbench window; the next tray Open reconstructs it, and tray Quit ends the whole process.

## What to restart

| Changed area | Rebuild | Refresh |
| --- | --- | --- |
| Orchestrator domain, workbench, or bundled UI | `cargo build -p ghostlight` | Restart `ghostlight`; no relay or extension change |
| MCP connector protocol lifecycle | `cargo build -p ghostlight-mcp-connector` | Reconnect the MCP server in the harness |
| Browser connector relay lifecycle | `cargo build -p ghostlight-browser-connector` | Reload the extension so Chromium respawns the native host |
| Extension mechanism or presentation | none for JavaScript | Reload the unpacked extension explicitly |
| Shared bridge contract | `cargo build --workspace` | Restart only consumers affected by that versioned boundary |

Do not restart a shore merely because an orchestrator feature changed. That is the fringe-stability
contract, not an optimization.

## Replacing a live stack

A running stack holds its own executables open, so a release swap is build-elsewhere, stop, copy,
start. The order matters less than the scope.

1. Build into an isolated target: `CARGO_TARGET_DIR=.target-release-swap cargo build --workspace
   --release`.
2. **Decide what to replace from the source diff, not from the build output.** Every binary is
   rebuilt whenever a shared crate changes, so a differing file size proves nothing. Check
   `git diff --stat <base>..HEAD -- crates/<crate>` per crate, and check whether the connector even
   imports the bridge module that moved. A connector whose source did not change should be left
   running: it reconnects to the new service on its own, which is exactly what
   `process-journey.mjs` proves.
3. Place `deploy.lock` in the live directory before stopping anything. It suppresses demand-start
   for 30 minutes so a connector cannot start a replacement service mid-swap.
4. Stop only processes whose exact image path is that directory. Never stop by image name; an
   installed or user-owned stack may be running beside the build.
5. Copy the binaries that changed, remove `deploy.lock`, then start `ghostlight`. The tray appears
   and the workbench begins backgrounded.

Replacing `ghostlight-browser-connector` has a cost the other two do not: Chromium respawns the
native host within a second or two while the extension's service worker is awake, so the copy needs
a short kill-and-retry loop, and afterwards the extension must be reloaded explicitly at
`chrome://extensions` before any browser work succeeds. An MV3 worker that has since suspended will
not reconnect on its own. That is the reason step 2 matters: if the connector did not change, none
of this happens.

The repository script makes that narrow swap repeatable. It defaults to planning an orchestrator
replacement and makes no changes:

```powershell
pwsh scripts/dev-loop.ps1
```

Run the one-command build and exact-path swap after reviewing that plan:

```powershell
pwsh scripts/dev-loop.ps1 -Action Deploy
```

Name connector replacements only when their source or shared contract changed. Native-host
registration is also explicit because it changes browser registration on the current machine:

```powershell
pwsh scripts/dev-loop.ps1 -Action Deploy -Component orchestrator,mcp-connector
pwsh scripts/dev-loop.ps1 -Action Deploy -Component browser-connector -RegisterNativeHost -NoStart
```

The script builds selected packages in `.target-dev-loop`, creates `deploy.lock`, stops only exact
destination image paths under `target/release`, copies with a bounded retry, removes the lock, and
starts the orchestrator only when it was selected. Both directories must stay inside this
repository. A browser-connector replacement still requires an explicit extension reload.

## Automated gates

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test --prefix extension
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

Build all three executables side by side and ensure the platform native-messaging manifest points
to that `ghostlight-browser-connector`. Load `extension/` unpacked and reload it after every
extension edit. Its pinned development identity and `org.sylin.ghostlight` host name must not
change.

Run `ghostlight` again to restore the backgrounded workbench, open Status, and verify a compatible
browser instance appears. Use a
supported MCP client registration from MCP integrations, reconnect that client, and execute the
journeys in [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md). Use the `sylin.org` demo forms for visible
form, upload, dialog, navigation, policy-denial, and screenshot work.

Never treat a clean Ghostlight screenshot as evidence that presentation did not render: the
extension intentionally hides its feedback layer while capturing page pixels.

## Isolation and cleanup

For tests, set `GHOSTLIGHT_RUNTIME_FILE` and `GHOSTLIGHT_AUDIT_FILE` to repository-local temporary
paths. Stop only processes whose exact executable path is the build you started. Never terminate
all `ghostlight`, browser, or MCP-client processes by image name; an installed or user-owned stack
may be running at the same time.
