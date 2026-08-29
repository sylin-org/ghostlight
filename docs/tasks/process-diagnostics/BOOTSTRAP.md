# process-diagnostics BOOTSTRAP

The batch that gives every Ghostlight process one place to say what happened: a shared local
diagnostics directory, bounded content-free operational logs from process birth, and one human
front door that turns them into a single readable story. It implements
[ADR-0145](../../adr/0145-shared-process-diagnostics-log.md).

Authority order: this BOOTSTRAP, then ADR-0145, then the current tree. The ledger is the
authority on progress; a task here describes intent, the ledger records what happened.

## Ground rules

- One task = one commit, and every commit leaves a green tree: formatting, warnings-denied
  Clippy, the full Rust suite, the extension suite, and `node --check` on any changed extension
  JavaScript. This batch expects to change no extension bytes.
- The delight bar is acceptance criteria, not flavor. The log must be findable (`doctor` and
  `diagnostics path` name the directory), readable (`diagnostics show` is one command to one
  chronological story), correlatable (run ids and operation ids survive into the rendered
  line), cause-first (details lead with the reason and, where known, the remedy), bounded
  (retention is automatic), and honest (`show` states missing coverage instead of implying a
  quiet past). A task that lands a line type or a command without meeting the bar is not done.
- Content-free discipline is enforced by a schema test, the way
  `audit_record_has_no_payload_fields` pins the audit record. Any new record field needs a
  justification in that task's ledger entry.
- A sink fault must never disturb the product: append failures disable the sink for the
  process lifetime, and no product path returns an error because diagnostics could not write.
- Activation is a person's act: the product never creates the `diagnostics.on` marker, and
  every component evaluates the activation layers once, at process birth.
- stdout purity: the MCP connector's stdout stays pure protocol. The sink writes files only.
- The extension changes nothing: no files, no push channel, no console logging.
  `browser_diagnose` and its ring are out of scope.
- ASCII only in every new file and line. Plain sentences in user-visible output; no Rust
  identifiers where plain words work.
- Nothing leaves the machine. Local commits are normal; pushes, releases, and anything
  external wait for the owner.
- A task that cannot close honestly is BLOCKED in the ledger with the reason. Do not improvise
  around a changed tree; STOP and record.

## STOP preconditions

Verify these against the tree before starting. They were true on 2026-08-29 and are recorded
in the ledger; if any has moved, STOP and record what changed.

- `crates/bridge/src/lifecycle.rs` demand-start still spawns the sibling orchestrator with no
  arguments, nulls its stdio, and does not clear its environment.
- All three executable crates still depend on `ghostlight-bridge`.
- The audit file still defaults to a sibling of the runtime discovery file
  (`crates/orchestrator/src/service/mod.rs`), and `crates/bridge/src/runtime.rs` still
  resolves the runtime file with the override/sibling/Linux-cache/temporary shape.
- The orchestrator CLI is still hand-rolled argument parsing in `crates/orchestrator/src/main.rs`.

## Tasks

1. D1 the sink in bridge: `crates/bridge/src/diagnostics.rs` resolves activation in layers
   (`GHOSTLIGHT_DIAGNOSTICS_DIR` override, then a presence-only `diagnostics.on` marker beside
   the runtime discovery file activating at the default directory, then off), names files
   `<utc-start>-<component>-<pid>.jsonl`, writes the header record, appends bounded lines
   behind a mutex, disables itself on append failure, and prunes to the newest 8 files per
   component within a 64 MiB total ceiling, touching only component log files and never the
   marker. Unit tests cover the layer matrix (variable over marker over off), file naming,
   detail clipping, self-disable, prune order and its marker safety, and a schema test that
   pins the record fields (timestamp, run id, component, event, level, optional operation id,
   bounded detail).
2. D2 orchestrator wiring: initialize the sink at startup when the switch is present, and emit
   the process, connection, framing and negotiation, operation boundary, and liveness families
   at the seams that already exist (service host start and readiness, harness attach and
   detach, browser adapter hello and replacement, deadline firing). Event names come from the
   closed vocabulary; no literal event strings at call sites.
3. D3 mcp-connector wiring: emit process start, demand-start attempts and outcomes, service
   connection lifecycle, and framing errors. The existing disconnect-streak stderr line stays;
   the sink records the same fact.
4. D4 browser-connector wiring: emit process start, demand-start attempts and outcomes,
   native-messaging connection lifecycle, and framing errors. Browser identity is recorded as
   the relay sees it (ids and epochs), never page facts.
5. D5 the `ghostlight diagnostics` CLI: `path`, `show`, and `prune` in the orchestrator's
   hand-rolled parser. `path` names the active layer (explicit directory, marker, or off) and
   the directory. `show` merges all files into one chronological timeline, renders plain
   sentences with a component tag and local timestamps, supports `--last`, `--component`,
   `--op`, and `--json`, and states missing coverage in range. It never demand-starts.
6. D6 the doctor row: the active layer (explicit directory, marker, or off), the directory,
   and its size, in both text and `--json` output.
7. D7 the journey: extend `tests/process-journey.mjs` to run the graph with the switch set,
   asserting per-component files with header records, lines for a real operation from both
   connector sides, prune behavior, and `diagnostics show --json` output containing that
   operation. One case activates by marker alone with the variable absent from every spawned
   environment, proving the browser-side path needs no propagation. Pass `GHOSTLIGHT_BIN_DIR`
   explicitly; the journey must exercise fresh binaries.
8. D8 contract and documentation reconciliation: `docs/1.0/ARCHITECTURE.md` describes the
   process sink beside the `browser_diagnose` diagnostics paragraph; `docs/DEV-LOOP.md` gains
   a short "when an agent reports an error" route; the tasks README row and the STATUS section
   are made truthful about the batch's final state. Public trust claims wait: nothing is
   claimed publicly until the tree proves it.
