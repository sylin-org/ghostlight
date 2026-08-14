# Ghostlight 0.8 test recovery matrix

Status: complete inventory disposition; native release evidence still in progress

This matrix connects the 1,388 harvested 0.8 entries to the 1.0 architecture. It does not claim
that a smaller 1.0 test count is line-for-line equivalent to the old suite. It records where the
behavior now belongs, which old mechanisms are intentionally gone, and which promises still need
native or visible-browser proof.

The machine-readable source is [`test-recovery.json`](test-recovery.json). Run
`scripts/check-0.8-recovery.ps1` to prove that all twelve inventory groups and all 34 Lightbox
scenarios still have a disposition. The source inventory remains
[`test-inventory.json`](test-inventory.json).

## Category disposition

| Historical area | Entries | 1.0 treatment | Remaining release proof |
| --- | ---: | --- | --- |
| Browser E2E | 4 | Keep the real-browser contract; replace the old fixture runner with current catalog journeys. | Signed-package visible Chromium on Linux and a second family. |
| Browser hub | 176 | Re-express through browser coordination, workspace, work, and one completion path. | Concurrent real browsers, restart, and unknown-effect journeys. |
| Extension | 177 | Re-express in the policy-free adapter and its current 99-test suite. | Store-package reload, browser restart, group reuse, and local interlock. |
| Governance | 344 | Re-express immutable snapshots, monotonic layers, final-boundary controls, and payload-free audit. Retire network activation. | Managed expiry, redirected landing, hold, and close-interlock journeys. |
| Installation | 62 | Re-express harness merge, native host, ownership-safe removal, stale-path update, and migration. Retire resident supervisors. | Clean install, 0.8 upgrade, reboot, and uninstall on all packages. |
| Integration | 149 | Re-express at current Rust, process, CLI, workbench, package, and browser seams. | Three public MCP harnesses and the visible release matrix. |
| Lightbox helpers | 5 | Retire the old runner implementation; retain its process contracts below. | Keep current process journeys blocking on Windows and Linux. |
| MCP edge | 101 | Keep generic JSON-RPC/MCP 2025-11-25 behavior and typed service framing. Defer the old proposed 2026 transcript. | Signed-candidate compatibility in three public harnesses. |
| Process contracts | 34 | Give every named scenario an explicit disposition below. | Resolve live-gate rows before release. |
| Supporting units | 105 | Retain observable behavior. Retire service-owned GIF/recording helpers after browser-local recording. | Browser-local recording save/export/discard in a visible browser. |
| Tool execution | 137 | Re-express through the 22-tool language, executor, typed outcome, and physical receipt seams. | Run every accepted browser job against a visible browser. |
| Transport | 94 | Re-express stable typed bridges, framing, correlation, reconnect, and demand-start. Retire named instances, UDS discovery, watchdog, and systemd self-heal. | Linux packaged demand-start and reconnect. |

Total: 1,388 historical entries. The checker derives that total from the inventory instead of
trusting this prose.

## High-value translations already restored

- The four Chromium native-host layouts, both fixed extension identities, atomic writes,
  ownership-safe removal, and stale-path updates now live under `install::native_host`.
- A 0.8 connector path is `updatable`, not `installed`. The workbench offers an explicit Update
  action and never overwrites a foreign entry.
- Recognized 0.8 Windows Run/task and Linux systemd artifacts are retired narrowly. No 1.0 package
  installs a resident supervisor.
- Windows NSIS and Linux Debian candidates carry the exact three executable shores. Package
  inspection rejects missing siblings and leaked target-triple staging names.
- Scrubbed Linux launcher environments cannot split discovery in 1.0: all three sibling processes
  derive one runtime document from their installation directory without XDG or D-Bus state.
- Missing, malformed, unmarked, or expired managed authority fails closed. A started invocation
  keeps its immutable snapshot; the next invocation reads the changed file.
- Windows and Linux source, extension, and process CI, dependency policy, deterministic extension
  packaging, compatibility data, and public-state reconciliation are active again.
- The 0.8 publication packet, exact release commit, public channels, channel drift, and recovery
  rules remain first-class inputs. Candidate, submitted, public, and observed are separate states.

## Lightbox process scenarios

`reexpressed` means a current automated seam proves the durable behavior.
`superseded-invariant-retained` means the mechanism is gone but its safety rule remains.
`superseded` means the 1.0 architecture makes the old process contract inapplicable.
`deferred` means it is outside the 1.0 contract and is not silently claimed.

| Scenario | Status | 1.0 disposition |
| --- | --- | --- |
| `legacy-read-page-redaction` | reexpressed | Governance and audit expose bounded observations without page payloads. |
| `legacy-late-extension-wait` | reexpressed | The process journey starts disconnected and accepts the later adapter. |
| `legacy-form-fill-parent-audit` | reexpressed | All form work crosses the one executor and one payload-free audit path. |
| `legacy-console-index` | superseded | The localhost console was replaced by the bundled Tauri workbench. |
| `legacy-console-assets` | superseded | The workbench loads packaged local assets and has no remote WebView route. |
| `legacy-console-not-found` | superseded | There is no management HTTP server or route surface. |
| `legacy-console-method-not-allowed` | superseded | There is no management HTTP method surface. |
| `legacy-console-websocket-rejected` | superseded | Sequenced workbench changes use the Tauri adapter, not a public WebSocket. |
| `legacy-console-config-registry` | reexpressed | The workbench facade supplies closed diagnostics and integration state. |
| `legacy-console-dns-rebind-denied` | superseded | The local HTTP origin boundary no longer exists. |
| `legacy-console-live-sessions` | reexpressed | The plural workbench projection owns current sessions and operations. |
| `mcp-edge-two-client-multiplex` | reexpressed | Sessions and workspaces are plural and owner-bound. |
| `legacy-hub-kill-audit-fanout` | reexpressed | Human end-session control is authoritative and terminal at final boundaries. |
| `mcp-edge-reconnects-future-call` | reexpressed | The real process journey reconnects the same MCP edge after service restart. |
| `service-survives-mcp-edge` | reexpressed | Connector loss releases its session, not the authority process. |
| `mcp-edge-anti-squat` | superseded | Authenticated random loopback endpoints and the service lifetime lease replaced named UDS ownership. |
| `mcp-2026-exact-transcript` | deferred | 1.0 implements MCP 2025-11-25 only. |
| `browser-relay-restart` | reexpressed | The browser relay remains live and reconnects across service restart. |
| `legacy-control-status` | reexpressed | Workbench Status reports service, browser, and authority health. |
| `legacy-org-policy-boot` | reexpressed | Configured local and managed authority is read before admission and snapshot. |
| `legacy-org-policy-hot-reload` | reexpressed | Existing work stays immutable; the next snapshot reads the changed authority. |
| `mechanism_wire_new_new` | reexpressed | Current browser bridge negotiation and capability tests cover the matching pair. |
| `mechanism_wire_new_service_old_extension` | reexpressed | Capability-gated older-adapter behavior remains explicit. |
| `mechanism_wire_old_service_new_extension` | reexpressed | Incompatible bridge majors fail during hello, before work. |
| `managed-activation-local` | superseded-invariant-retained | Explicit managed files replace activation; configured invalid authority still fails closed. |
| `managed-activation-network` | superseded | 1.0 has no activation service or unrelated network call. |
| `fail-closed-cold-boot` | reexpressed | A missing configured managed file denies from a fresh facade. |
| `continuity-source-unreachable` | reexpressed | Unreachable configured authority never falls back to all-open. |
| `rollback-guardian` | superseded-invariant-retained | Monotonic local/managed intersection prevents authority expansion. |
| `update-on-reresolve` | superseded | There is no remote policy resolver or update poll. |
| `no-clobber-on-reresolve` | superseded | There is no remote authority cache to clobber. |
| `sidecar-propagation` | superseded | Managed authority is an explicit local document, not a fetched sidecar. |
| `passport-freshness` | superseded-invariant-retained | Managed documents require a future expiry and fail closed otherwise. |
| `license-expiry-continuity` | superseded | 1.0 has no activation/license continuity service. |

The machine-readable matrix points each row at a current evidence file and prevents an inventory
or scenario from disappearing during later edits.

## Still owed before 1.0 publication

- Build the unsigned Linux candidates in their native CI environments and inspect them.
- Sign or attest every platform candidate, then run clean install, 0.8 upgrade, login/reboot,
  demand-start, and uninstall on ordinary user accounts.
- Run the full visible-browser acceptance matrix with the matching store extension, two Chromium
  families where available, and at least three MCP harnesses.
- Reconcile independently downloaded public bytes and each publication channel only after owner
  approval. Nothing in the recovery matrix authorizes a tag, push, store mutation, or publication.
