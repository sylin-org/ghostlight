# Linux failed-start custody and readiness

## Source milestone and mechanism proposal

Owner-delegated follow-up on test-03, starting at integrated source
`ac1becab78e5a8a6b89e553e82f6779d93dcb39c` on
`codex/fleet-test-03-startup`. Work is in the ordinary Ghostlight repository.
Generated evidence uses `.tmp/linux-startup-recovery/`; the existing native-musl
installation remains at its original path. No new fleet worktree or monitor.

The accepted Codex environment forwarding repair is already installed. This work
does not rediscover or replace it. The remaining defect has two owning seams:

- Bridge demand-start drops the lifetime-lease probe before spawning. Concurrent
  callers can all spawn, and failed children are not reaped by their callers.
- Service startup publishes discovery before native desktop initialization. A GTK
  failure then enters the same 15-second activation wait as a contending launch.
  Reconnecting callers repeatedly create these waiting processes.

The proposed mechanism stays inside the existing processes and contracts:

1. Add a per-runtime startup admission lock, distinct from the authority's lifetime
   lease. One existing reconnect worker owns a bounded spawn-to-ready exchange;
   other callers keep reconnecting. The child still acquires its own lifetime lease.
2. Reap a failed child before releasing startup admission. A shared five-second
   retry cooldown prevents concurrent clients from taking turns spawning failures.
   Keep the cooldown bounded and automatically retry after context is corrected.
   No captured environment or process-environment discovery is involved.
3. Treat a launch as successful only after new runtime discovery is published.
   Bound the startup exchange, stop only its own unready child on timeout, and
   preserve deployment quiescence and exact elected-sibling identity. Successful
   child cleanup is resource reaping, not a new service supervisor.
4. Prepare the service under its existing lifetime lease, but publish its runtime
   only from the native desktop Ready path after its interaction route exists.
   Retain the tray/workbench fallback and one disposable workbench contract.
5. Give lease contention a typed startup error. Only that error waits for another
   authority's activation. A genuine desktop failure is logged and exits promptly.
   Existing process diagnostics report startup failures and bounded retry state;
   no runtime tokens, page content or copied environment enter those logs.

This is an implementation proposal, not completed acceptance. No headless surviving
authority, new resident supervisor, broad permissions, or host-session guessing.
Bluefin owns Flatpak host activation and must compose with this shared admission
boundary. Direct task delivery to its remote host is unavailable here; this branch
is the coordination record. Shared integration branches remain untouched.

### Flatpak composition interface (coordinator relay)

The coordinator reserved ADR-0166 for this decision; Bluefin owns ADR-0165.
The shared bridge will expose these two entry points over the same admission,
readiness wait and cooldown implementation:

```rust
pub fn request_orchestrator_start() -> io::Result<StartDisposition>;
pub fn request_orchestrator_activation(
    activate: impl FnOnce() -> io::Result<()>,
) -> io::Result<StartDisposition>;
```

Native start still resolves and launches only its trusted elected sibling.
Activation calls its narrowly authorized OS activation closure once, only after
admission; it never invents a child PID or falls back to executing a host command.
Both wait for new authenticated discovery. Native startup can reap/stop its own
child; activation has no child handle and cannot stop the externally activated
authority. Existing runtime and deployment-lock discovery determine custody.
The callback must itself perform a bounded activation exchange.

Disposition variants will be `Spawned { process_id }`, `ActivationRequested`,
`AlreadyRunning`, `Starting`, `RetryDeferred`, and `DeploymentInProgress`.
The last two new waiting states mean keep reconnecting; they are not readiness.
`ActivationRequested` has no process id. No feature-specific wire revision is
needed. This interface is a planned source milestone, not yet implemented.

## Planned evidence

Preserve a native installed sanitized-environment failure with concurrent MCP
callers, including child counts, readiness exposure, cleanup and exact binary
hashes. Then repeat against the repaired installed siblings, retain concurrent
callers through cooldown and corrected-context recovery, and prove ordinary
configured Codex and browser-led startup, one authority, browser rejoin and native
workbench. Pure regressions cover failed spawn, early child exit, stalled startup,
concurrent admission, cooldown, private readiness publication and typed contention.
Run the ordinary gates plus real process journeys with the explicit fresh binary
directory. No physical reboot is planned unless a remaining question needs one.


## Native source milestone: failure and recovery pass

Baseline native-musl workspace gates and build passed before implementation.
`tests/linux/startup-recovery.mjs --record-before` preserved four real connector
callers creating 32 transient authorities in four seconds. Runtime discovery was
observed and retained diagnostics included three premature publications. This is
an isolated process lane using fresh executables, not another installed product.
The original installed binaries remain unchanged at this source milestone.

After the implementation, four sanitized callers ran for 11.5 seconds. They made
three failed attempts, with peak one transient authority, zero runtime publications,
zero completed MCP initialization and three factual native-startup failure events.
After replacing those callers with corrected-context callers, all four completed
MCP initialization against one authority in 5874 ms, including remaining cooldown.
No endpoint token or environment value appears in the report.

The ordinary format, warnings-denied Clippy, Rust workspace and 222 extension
checks pass. The real process journey also passes, including reconnect, audit
recovery, uncertain-effect preservation, and cold-start policy explanation. The
first after-test attempt overlapped that journey's shared deployment lock and
correctly spawned nothing; it could not measure retry cadence. Its failed record
is preserved as `after-deploy-lock-overlap`. The lanes were then run separately.
The new driver detects that interference explicitly rather than counting it as a
startup success or an unexplained product failure.

Source API is now implemented exactly as the composition milestone above.
`StartDisposition::diagnostic()` owns the shared content-free event/detail mapping,
so both shores stay generic when a platform activation disposition is added.
ADR-0166 records custody, deadlines, cooldown, private readiness and scope.
Installed deployment and Codex/browser/workbench acceptance are the next step.

Evidence under `.tmp/linux-startup-recovery/`: `baseline.json`, `baseline-*`,
`before/result.json`, `after/result.json`, `fixed-gates.json`, `fixed-*.log`,
`bridge-tests.log` and `process-journey.log`. The before/after JSON files record
all three exact binary hashes. The build directory is `.target-fleet-startup`
with explicit `x86_64-unknown-linux-musl` and `-C target-feature=-crt-static`.
