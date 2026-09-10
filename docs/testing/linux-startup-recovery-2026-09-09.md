# Linux failed-start custody and readiness

## Implemented mechanism and evidence

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

The implemented mechanism stays inside the existing processes and contracts:

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

This implementation creates no headless surviving authority, new resident
supervisor, broad permissions, or host-session guessing.
Bluefin owns Flatpak host activation and must compose with this shared admission
boundary. Direct task delivery to its remote host is unavailable here; this branch
is the coordination record. Shared integration branches remain untouched.

### Flatpak composition interface (coordinator relay)

The coordinator reserved ADR-0166 for this decision; Bluefin owns ADR-0165.
The shared bridge exposes these two entry points over the same admission,
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

Disposition variants are `Spawned { process_id }`, `ActivationRequested`,
`AlreadyRunning`, `Starting`, `RetryDeferred`, and `DeploymentInProgress`.
The last two new waiting states mean keep reconnecting; they are not readiness.
`ActivationRequested` has no process id. No feature-specific wire revision is
needed. This interface is implemented at source commit `d67e3b33`.

## Evidence plan and boundaries

Preserve a native isolated sanitized-environment failure with concurrent MCP
callers, including child counts, readiness exposure, cleanup and exact binary
hashes. Then repeat against fresh repaired siblings, retain concurrent
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
The original installed binaries were unchanged at that source milestone.

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
Installed deployment and Codex/browser/workbench acceptance subsequently passed
as recorded below.

Evidence under `.tmp/linux-startup-recovery/`: `baseline.json`, `baseline-*`,
`before/result.json`, `after/result.json`, `fixed-gates.json`, `fixed-*.log`,
`bridge-tests.log` and `process-journey.log`. The before/after JSON files record
all three exact binary hashes. The build directory is `.target-fleet-startup`
with explicit `x86_64-unknown-linux-musl` and `-C target-feature=-crt-static`.


## Installed native acceptance complete

All three changed executables from `d67e3b33` were deployed to the installation
in place through the ordinary dev-loop controller. The local controller adaptation
only names the preserved external installation and the musl target subdirectory;
its deployment lock, exact-image custody and copy behavior remain the source path.
The old worktree, native registrations and saved Codex configuration remain in place.

The first deployment failed because Codex respawned its MCP connector after the
controller stopped it. Thirty attempts to copy a running Linux executable all
failed with `Text file busy`. The controller now rechecks only the selected exact
destination after a failed copy, stops a respawned instance and retries after its
exit. A real executable-lock regression proves the negative control fails, the
selected executable is replaced, and a neighboring executable keeps running.
The second installed deployment passed. Both the failure and correction are kept.

Actual configured Codex then cold-started from an empty installed stack with the
browser closed and completed `policy_explain` with a succeeded, effect-none result.
The entire real Codex invocation took 11.57 seconds, including model/network time;
it is not a measurement of the startup deadline alone.

Browser-led cold startup also passed from no authority or browser connector. The
existing Codex connector was temporarily paused to isolate which real caller won,
then resumed in a finally block. Normal Applications launch of Chromium in its
existing profile started browser connector PID 17825, which started authority
PID 17830. An earlier preparation attempt correctly detected Codex winning before
browser launch and stopped that measurement; its record is retained separately.

An explicit extension reload exposed a browser setup condition: Chromium disabled
the existing unpacked adapter with `unsupportedDeveloperExtension` because Developer
mode was off. The browser reported no manifest or runtime errors. Enabling its normal
Developer mode setting restored the adapter. A second explicit reload then rejoined
without restarting the authority, with the same browser identity and adapter 1.1.2.
No extension source, host permission or native registration was changed.

A fresh disposable local page navigation and read through the installed MCP connector
both succeeded and returned the fixture marker `ALPINE-MUSL-135`. This boundary driver
is separate from the actual Codex proof above. `ghostlight open` produced one active,
unminimized native workbench. Final process inspection still found only authority
PID 17830, all three installed image hashes matched the repaired build, no connector
was left paused, and the deployment marker was absent. Doctor reported Ready and
service 1.3.5. Temporary process diagnostics were turned off after evidence capture.

### Exact repaired installed identities

| Executable | SHA-256 |
| --- | --- |
| ghostlight | `b7a8924a60dc7e345f5e1801fc491228172a7ff00177c228fed5b17ef3a46aca` |
| ghostlight-mcp-connector | `d73f9cf98e09798a7007bcc3720383bb7c3bf6cf246ace70183d3fb93b74dce3` |
| ghostlight-browser-connector | `ce436a3f6cfe3982c4215395ec82937e372228342e011e4719457a6a35e1362b` |

Final required gates pass: formatting, warnings-denied workspace/all-target Clippy,
547 Rust workspace tests, 222 extension tests, and startup-driver syntax. No extension
JavaScript changed. The real process journey and the Linux executable-lock regression
also pass. Before/after retry evidence is an isolated native process lane; installed
acceptance uses the actual saved Codex registration and normal Chromium profile.

Additional evidence under `.tmp/linux-startup-recovery/`:
`installed-deploy.log`, `installed-deploy-retry.log`, `copy-retry-test.log`,
`installed-codex-result.json`, `browser-cold-mcp-won-prepare.log`,
`installed-browser-cold-result.json`, `installed-final-browser-result.json`,
`installed-final-state.json`, `installed-diagnostics.log`, `final-gates.json`, and
`final-*.log`.

This closes generic failed-start amplification and premature discovery on this lane.
An unconfigured client that strips desktop context still cannot initialize GTK;
it now fails truthfully with bounded retries and recovers when context is corrected.
Bluefin's separate named host activation implementation must compose at the published
callback boundary during central integration. No second physical reboot, release,
package publication, Flatpak acceptance or expanded public musl support is claimed.
