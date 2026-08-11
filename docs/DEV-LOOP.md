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

`--show` is retained as an explicit alias for the same human launch behavior.

Run only the persistent orchestrator service:

```powershell
target/debug/ghostlight --headless
```

A direct launch starts visibly or focuses the workbench owned by the running authority. Connectors
demand-start the sibling executable with `--background`, which keeps the workbench hidden and the
tray available. Closing the window hides it; the tray Quit action ends the whole process.

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

## Automated gates

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test --prefix extension
cargo build --workspace --target-dir .target-ghostlight-1.0
node tests/process-journey.mjs
node --check crates/orchestrator/ui/app.js
node --check tests/workbench-preview-server.mjs
```

`process-journey.mjs` uses repository-local runtime and audit files and the isolated target above.
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

Run `ghostlight --show`, open Status, and verify a compatible browser instance appears. Use a
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
