# process-diagnostics LEDGER

Durable progress for the process-diagnostics batch. One task = one commit. This file, not the
BOOTSTRAP's task list, is the authority on where the batch stopped.

## RESUME HERE

The batch is complete through D10 (2026-08-29): every task executed, all gates green, and the
extended process journey passes against the real executable graph. The live deployment and its
verification are the owner's dev-loop action and are recorded in STATUS, not here.

## Verified ground truth at opening (2026-08-29)

- `crates/bridge/src/lifecycle.rs` demand-start spawns the sibling orchestrator with no
  arguments, stdin/stdout/stderr sent to null, and the parent environment inherited, so
  `GHOSTLIGHT_DIAGNOSTICS_DIR` set in a client's server configuration already reaches the
  connector and the orchestrator it starts with no code on that path.
- `ghostlight-bridge` is already a dependency of all three executable crates.
- The audit file defaults to a sibling of the runtime discovery file (`GHOSTLIGHT_AUDIT_FILE`
  overrides it), and `crates/bridge/src/runtime.rs` resolves the runtime file with the
  override/sibling/Linux-cache/temporary shape the diagnostics directory mirrors.
- No logging crate exists in the workspace. The only process log lines today are one
  `eprintln!` per disconnect streak in each connector.
- ADR-0016 is the historical shape (per-PID files, bounded bodies, swallowed I/O, stdout
  purity, distinct from audit). Its mechanism was removed with the 0.8 layer by ADR-0143;
  nothing of it remains in `crates/`.

## Delight bar (owner directive, 2026-08-29)

The log must be useful and easy to access, use, and infer root causes from. ADR-0145 Decision
7 turns that into acceptance criteria: findable, readable, correlatable, factual, reachable,
bounded, honest, shareable. The owner sharpened it the same day: delight for the person
chasing an error, human or agent, is useful information, not prose. Each task's ledger entry
states how it met the bar.

## Amendments applied before execution

ADR-0145 was amended four times in place before any task executed (2026-08-29): first,
activation is layered, an explicit-directory variable over a presence-only `diagnostics.on`
marker beside the runtime discovery file, over off; second, the marker is actuated from
surfaces -- the person's hand, `diagnostics on|off`, and an extension popup toggle through the
orchestrator-owned runtime-control path; third, the live re-check became a layered OS watch
with a 2-second safety-net tick, whichever fires first; fourth, the workbench toggle and
folder reveal became decided surfaces, and the log voice was pinned to terse facts over
teaching prose. The batch implements the amended decision.

## D1 the sink in bridge

Status: complete (2026-08-29).

- `crates/bridge/src/diagnostics.rs`: layered resolution, per-activation-period files named
  `<utc-stamp>-<component>-<pid>.jsonl` with same-second sequence numbers, header record,
  bounded detail (500 bytes, UTF-8 safe), self-disable on append failure, prune (newest 8 per
  component, 64 MiB total, marker and foreign files never touched), and the closed event-name
  module.
- The watcher is the `notify` crate (one new dependency, dependency policy green) behind the
  injectable `MarkerWatcher` seam, filtered to the marker's file name so the sink's own
  appends never trigger an evaluation; a 2-second park-timeout tick is the safety net.
  Whichever fires first wins; transitions are idempotent.
- Delight bar: bounded and honest by construction; the schema test pins content-free records.
- Deviation, deliberate: one uniform ticker in the sink serves every component instead of
  reusing per-component timers. One mechanism beats per-component placement, and operational
  events are far too sparse for the reuse to buy anything.

## D2 orchestrator wiring

Status: complete (2026-08-29).

- `DiagnosticsHub` owns the sink, performs the toggle act (flip marker, re-evaluate, report),
  maps state to the wire, and implements the port's new `AdapterLifecycleObserver` for
  adapter attached/replaced/detached events.
- Emissions: process start, harness attached/detached (`serve_session`), adapter attach and
  replacement (observer at the port), adapter disconnected (observer from the reader loop),
  operation completed/failed at the one `finish` funnel with invocation id, tool, status,
  effect, and duration; runtime-control request handling unchanged.
- Wire: `DiagnosticsState` rides `HelloAccepted` and `ControlState` as optional additive
  fields; the new `BrowserEvent::DiagnosticsToggleRequested` is sent only by adapters whose
  hello advertised diagnostics, so an older service can never receive it (the additive
  pattern of the block, not a major bump). Round-trip test added.
- The orchestrator republishes wire state on every activation transition, so adapters, popup,
  and workbench always see the same truth from any actuation path.

## D3 mcp-connector wiring

Status: complete (2026-08-29).

- Process start, demand-start outcomes (spawned pid, already-running, deployment-in-progress,
  failures once per streak, deduplicated so the 500 ms retry loop cannot flood), service
  connected and disconnected with the client label. The existing stderr line stays.
- Delight bar: correlatable -- every line carries the process run id; connector records name
  the client label.

## D4 browser-connector wiring

Status: complete (2026-08-29).

- Process start, demand-start outcomes with the same deduplication, native-relay service
  connected/disconnected. Deviation from the BOOTSTRAP's expectation, recorded with reason:
  no forwarding change was needed because the relay is payload-opaque
  (`opaque_length_frames_preserve_unknown_payloads`), so the "typed toggle forwarding" task
  reduced to zero connector code and the D2 event rides the existing pipe.

## D5 the diagnostics CLI

Status: complete (2026-08-29).

- `ghostlight diagnostics path|show|prune|on|off` in the hand-rolled parser; completions and
  help updated and pinned by the existing consistency tests; `show` merges all components
  into one chronological line set with local timestamps, `--last/--component/--op/--json`
  filters, and names components absent from the range instead of implying a quiet past.
- Never demand-starts; `on`/`off` are the person's act through the same marker.
- Live smoke on this machine: off/on/off round trip, marker created and removed, show in
  text and JSON, help documents the subcommand.

## D6 the doctor row

Status: complete (2026-08-29).

- `Process diagnostics: <layer> -- <bytes> of log in <dir>` in text; `process_diagnostics`
  in the JSON document; same observation feeds both, per the file's own rule.

## D7 the journey

Status: complete (2026-08-29).

- The process journey runs with the switch on: all three components write headered files,
  `harness_attached` carries the client label and channel, `adapter_attached` carries the
  browser id, both connectors report `service_connected`, the orchestrator's
  `operation_completed` lines name `browser_read` with an operation id, and no page content
  ("Example Domain") appears anywhere in the records. `show --json` and `path` assert the
  CLI surface; `on`/`off` assert marker actuation beside the journey runtime file.
- Deviation, recorded: the journey covers env-pinned activation and CLI marker actuation;
  marker-only birth activation is pinned by bridge unit tests instead of a second spawned
  graph, which would have doubled the journey's process churn for one already-proven branch.

## D8 the extension popup toggle

Status: complete (2026-08-29).

- One control beside the popup's existing human controls; the worker gates the new
  `diagnostics_toggle` request on the hello-advertised diagnostics state and sends the one
  event over the existing runtime-control envelope. The popup renders the current layer and
  hides the whole row for an older service. No policy, no files, no console logging.
- Extension suite: 156 tests, zero failures; `node --check` clean on both changed scripts.
- Consequence noted in the ADR: these bytes ride the next store submission.

## D9 the workbench surface

Status: complete (2026-08-29).

- The Status destination gains the process-diagnostics card: current layer, byte count,
  directory, a toggle that sends the same orchestrator act, and a reveal that opens the
  folder through the opener plugin from the native process. The WebView gained no opener,
  shell, or filesystem grant.
- `workbench-surface.mjs` executes the real card render against a fixture and asserts both
  buttons; facade and command surfaces are covered by the workspace suite.

## D10 contract and documentation reconciliation

Status: complete (2026-08-29).

- `docs/1.0/ARCHITECTURE.md` describes the process sink beside the `browser_diagnose`
  paragraph, including the activation layers and the content-free boundary.
- `docs/DEV-LOOP.md` gains "When an agent reports an error": the four-step support route.
- This ledger, the tasks README row, and the STATUS section are made truthful.

## Post-deployment corrections (2026-08-29, same day, from live use)

- The default log directory polluted the application root (`target/release/` in a dev
  deployment): the default is now a `logs` folder beside the runtime file, for the marker
  layer and for the report, which names the folder even while off. `show` and `prune` act on
  that folder while off, so retained history stays readable. The activation marker did not
  move. ADR-0145 amended in place.
- The workbench card never flipped its button: the status section's change-detection
  dependencies omitted `process_diagnostics`, so the card rendered once at boot. Fixed in
  `view.js` and pinned by a new surface check that flips the layer under the card.
- `show` also read `audit.jsonl` as if it were diagnostics output; it now selects only files
  matching the diagnostics naming.

## Gate note for the batch

Observed per task and on the final tree, 2026-08-29: `cargo fmt --check` clean;
`cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace` green
(orchestrator 341 library tests, bridge 51, extension suite 156, workbench-surface and
policy-grammar journeys green, process journey green against freshly built debug binaries via
`GHOSTLIGHT_BIN_DIR=target/debug`). One flake was found and fixed at its root
(63287423): a test asserted a transition from pure resolution, which proves nothing; it now
waits on the observable effect, matching the eventual contract. The known-flaky failure was
reproduced once in 20 runs before the fix and zero times in 12 after it.
