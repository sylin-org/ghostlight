# ADR-0145: A shared process diagnostics log

- Status: Accepted
- Date: 2026-08-29
- Builds on: ADR-0016, ADR-0028, ADR-0107, ADR-0127, ADR-0143

## Context

Model clients report errors, and today there is nothing to look at afterward. The blindness is
structural, one gap per layer:

- The demand-started orchestrator is silent. `crates/bridge/src/lifecycle.rs` spawns it detached
  with stdin, stdout, and stderr sent to null, and a test pins the zero-argument launch
  (ADR-0127's one invoked desktop authority). Anything the authority would say while starting or
  serving is destroyed on arrival.
- The connectors log almost nothing. No logging crate exists anywhere in the workspace. The only
  process diagnostic is one stderr line per disconnect streak in each connector, which lands in
  an MCP client's log if that client shows one, and effectively nowhere for the connector that
  Chromium itself spawns.
- The extension is silent by design. Its only diagnostics are the bounded volatile ring behind
  `browser_diagnose` (ADR-0107): in memory, pull-based, gone when the browser restarts.

The governance audit does not cover this need and should not be stretched to. It is
orchestrator-only, content-minimized, and records decisions: allowed, denied, effect, status.
The failures agents actually report -- a demand-start that never came up, an adapter dying, a
framing error, a revision negotiation miss, a deadline firing -- are operational events that
cross all four parts and appear in no record.

The project has built this before. ADR-0016 gave the 0.8 layer an opt-in observability mode:
`BROWSER_MCP_DEBUG=1` enabled a per-PID state snapshot and an append-only JSONL event firehose,
kept stdout pure, swallowed sink faults, clipped every body, and stayed deliberately distinct
from the audit subsystem. ADR-0143 retired the 0.8 layer and that mechanism with it; the design
evidence survives in the record, and several of its lessons carry forward (bounded bodies,
swallowed I/O, per-process files, stdout purity).

Constraints that shape the new decision:

- The desktop authority launches with no arguments (ADR-0127), so a CLI flag cannot reach it
  through demand-start.
- Never phone home (ADR-0028): the sink is local files or nothing.
- Content minimization is a standing discipline: audit is metadata-only, and diagnostic payloads
  never enter audit or presentation.
- Fewest meaningful moving parts: no new process, crate, service, or event bus for this.

## Decision

### 1. One local diagnostics directory

Every Ghostlight process that has something to say about its own operation writes into one
directory. The default is the directory holding the runtime discovery file -- the same place
`audit.jsonl` already lives -- resolved with the same shape the runtime file uses:
`GHOSTLIGHT_DIAGNOSTICS_DIR` override, then the runtime-file sibling with the Linux
system-package cache location, then a temporary-directory fallback. Local-only, permanently
(ADR-0028).

### 2. The switch is an environment variable, honored from process birth

`GHOSTLIGHT_DIAGNOSTICS_DIR` set to a usable directory turns the sink on for that process. All
three executables check it at startup, before any connection exists, because the most valuable
lines are written exactly when nothing connects. Propagation through the MCP client chain is
free: demand-start inherits the parent's environment, so a variable set in a client's server
configuration reaches the connector and the orchestrator it starts with no code on that path.

A CLI flag is rejected: the no-argument authority launch is pinned by test, and Chromium spawns
the browser connector with the browser's own environment, which a flag on some other component
cannot reach either.

Deferred, not decided: handing the configuration to an already-connected browser connector at
handshake, and a registered setting with a workbench toggle for the "turn this on, reproduce,
send me the folder" support flow. Either would be a new ADR.

### 3. Per-process-instance bounded JSONL files

Each process instance owns exactly one file, opened at startup and named
`<utc-start>-<component>-<pid>.jsonl`, so a plain directory listing reads chronologically. No
two processes ever write one file, so there is no cross-process locking. The first record is a
header: schema marker, component, product version, pid, run id, start time. Every later record
is one line: timestamp, run id, component, event name, level (info, warn, error), optional
operation id, and one bounded detail string clipped at 500 bytes on a UTF-8 boundary.

Retention is automatic, and the numbers are pinned: at process startup and at
`ghostlight diagnostics prune`, keep the newest 8 files per component and no more than 64 MiB
total, pruning oldest first.

### 4. A closed event vocabulary with cause-first detail

Event names form a closed, named set beside the sink, in the bridge crate that all three
executables already depend on. Initial families: process lifecycle (started, ready, stopping);
demand-start attempts and outcomes; connection lifecycle (attached, detached, replaced,
reconnect); protocol framing and negotiation (revision agreed, malformed frame refused);
operation boundaries (started, completed, failed, timed out) carrying the operation id; and
liveness (heartbeat lost, heartbeat resumed).

Detail strings are plain sentences that lead with the cause and, where one is known, name the
remedy -- the same teaching voice the model-facing language already uses. Records are
content-free by construction: identifiers, counts, durations, states, and typed reasons from
the product's own failure vocabulary. Never URLs, page content, payloads, selectors, or
credentials. A schema test pins the record shape the way the audit record shape is pinned.

### 5. A sink fault never disturbs the product

Off by default the sink is a no-op with no files and no cost. On: an append failure disables
the sink for that process's lifetime instead of surfacing an error into a product path. stdout
purity holds where it matters -- the MCP connector's stdout stays pure protocol; the sink
writes files only.

### 6. One human front door

`ghostlight diagnostics` gains three subcommands, all read-only over the directory and never
demand-starting (the doctor rule):

- `path` prints the effective directory and says whether the switch is present.
- `show` merges every component's files into one chronological timeline rendered as plain
  sentences with a component tag and local timestamps. It supports `--last`, `--component`,
  `--op`, and `--json` for tooling. Where coverage is missing it says so -- "no orchestrator
  log in this range; diagnostics were probably off" -- rather than displaying a quiet past.
- `prune` applies the retention bounds on demand.

`doctor` gains one row: whether diagnostics are active, where the directory is, and how large
it is, so support has one command that finds the folder.

### 7. What useful means here

The owner's bar: a log earns its place only if it answers "what happened and why" without
ceremony.

- Findable: one stable place; `doctor` and `diagnostics path` both name it.
- Readable: `diagnostics show` is one command to one chronological story; no manual stitching
  of four files.
- Correlatable: run ids bind a process instance's lines, and operation ids let one search
  follow one tool call across orchestrator and connectors.
- Cause-first: lines lead with the reason and the remedy, not a code path.
- Bounded and honest: retention is automatic, and missing coverage is stated, never implied
  away.
- Shareable: content-free by construction, so the folder can be attached to a report without
  leaking page content. Public trust claims about that property wait until the tree proves it.

### Boundaries

Audit stays the only governance record. `browser_diagnose` stays the only page-evidence
surface, and its ring stays in memory. The extension gains no files and no push channel under
this decision. A later decision may let `browser_diagnose` persist its collected ring into
this directory as a timestamped dump, which would give the extension a voice here without a
standing channel.

## Consequences

- An agent-reported error becomes traceable end to end: set the variable in the client's
  server configuration, reproduce, run `ghostlight diagnostics show --last 10m`, read or
  attach the result.
- The demand-started authority stops being a black box: it writes its own file from birth when
  the switch is present.
- All three executables grow one small shared write path in the bridge crate. No new process,
  crate, or service; connector contracts are unchanged; the extension is untouched.
- ADR-0016's mechanism stays retired. This decision revives its observability half with a new
  design on the current tree: an environment switch instead of a launch flag and
  `BROWSER_MCP_DEBUG`, per-instance JSONL files with headers instead of a live snapshot plus
  an event firehose, and a read-only CLI reader instead of a rendered snapshot.
- Off by default and bounded when on: no files appear unless asked for, and none outlive the
  retention bounds.
- The Chrome-spawned browser connector logs only when its own environment carries the
  variable; until a handshake or setting decision lands, turning diagnostics on for that
  process means setting the variable at the user level before starting the browser. This
  limitation is accepted for now and recorded rather than papered over.
