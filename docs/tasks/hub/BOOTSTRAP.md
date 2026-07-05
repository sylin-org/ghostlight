# Ghostlight Hub batch: BOOTSTRAP

Ground rules for the executor implementing the Ghostlight Hub (the multi-client orchestrator).
Assume ZERO conversational context survives to you. Follow instructions literally; resolve nothing
by judgment. Read this file fully before touching any code.

## What you are building

The Hub turns today's single-session dual-role binary into: one persistent per-user SERVICE that
solely owns the single Chrome extension link, with heterogeneous clients as multiplexed SESSIONS.
The authoritative design is `docs/adr/0030-ghostlight-hub-orchestrator.md` (ADR-0030). You implement
it in nine tasks, H0 through H8, one task = one commit.

## Authority order (higher wins on conflict)

1. `docs/adr/0030-ghostlight-hub-orchestrator.md` -- the NORMATIVE design. Cite it; never restate or
   re-decide its semantics.
2. This BOOTSTRAP -- ground rules and procedure.
3. The per-task file `docs/tasks/hub/H<N>-<slug>.md`.
4. The LIVE TREE. The task files record facts as-of-authoring (2026-07-04). RE-READ the named files
   before relying on any line number or signature. If the tree contradicts a task's load-bearing
   assumption, follow that task's STOP precondition (see Failure protocol); do NOT improvise around it.

The "Preserved invariants" section of ADR-0030 and the NEVER-touch list below OVERRIDE everything.

## Environment facts

- Rust stable, one Cargo workspace, single portable binary `ghostlight`, zero runtime deps, no dylib.
- Work on the `dev` branch. One task = one commit.
- Code is ASCII only (the ghost glyph exists only as the `\u{1F47B}` escape); docs use no em-dashes.
- Verification commands (a task is not done until all four pass):
  - `cargo build --all-targets`
  - `cargo test` (plus the task's specific new test targets)
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`

## Task sequence (linear; do them in order)

H0 extract HubCore -> H1 transport-generic serve_session -> H2 service + adapter + multiplex
-> H3 GUID identity -> H4 binary-authoritative isolation -> H5 grace window + honest queue
-> H6 detached lifecycle + anti-squat -> H7 tab-group-per-session -> H8 local web API.

Dependencies that are also encoded as STOP preconditions: H1 before H2; H3 before H4; H2+H3+H4
before H8. H5 is orthogonal (any time after H2) but stays in sequence here. H2 is the one large
coupled commit (persistent service + adapter + multiplex + the kill-hook fan-out); it is pinned to
be landed whole, NOT split.

## Per-task procedure

For each H<N>:

1. Read `docs/tasks/hub/H<N>-<slug>.md` fully, and the ADR-0030 sections it cites.
2. RE-READ every source file the task names. Verify each as-of-authoring fact. If any STOP
   precondition's assumption is absent, STOP (Failure protocol).
3. Write the named tests FIRST (RED). Transcribe every pinned assertion / oracle from ADR-0030 or
   the task file verbatim -- never derive an expected value. If the task marks a value
   "AUTHOR MUST PIN" and it is still unpinned, STOP.
4. Implement to GREEN with the minimum change the task describes; keep the change inside the files
   the task names.
5. Run the full verification block. All four commands must pass.
6. Confirm you did not move a NEVER-touch fence and that the sacred tests
   (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`,
   `tests/architecture.rs::governance_core_has_no_forbidden_back_edges`) are green and unmodified.
7. Commit exactly this task: `feat(hub): H<N> <short title>` (or `refactor(hub):` for H0/H1).
8. Update `LEDGER.md`: move RESUME HERE to the next task, set this task's status to DONE with the
   commit hash, and log any numbered deviations.

## Completion criteria

- H0..H8 each landed as its own commit; every prefix left a green tree.
- The full suite is green, including the untouched sacred tests and every new `tests/hub_*.rs` /
  `tests/webapi_auth.rs` / `tests/channels_policy.rs` test named by the tasks.
- All-open output is byte-identical to before the batch (a lone all-open session's new code paths
  are pass-through no-ops).
- Two MCP clients multiplex concurrently through one service (H2), each with its own GUID-keyed
  session and, at H7, its own tab group; a kill fans out one audit record per live subject.

## Failure protocol (when a task cannot complete)

If a STOP precondition fires, the tree contradicts a load-bearing assumption, a NEVER-touch fence
would have to move, or an AUTHOR-MUST-PIN oracle is still unpinned:

1. REVERT the working-tree changes for this task (`git restore` / discard) so the tree stays green
   at the last completed task.
2. In `LEDGER.md`, set the task's status to BLOCKED and record: the exact assumption that failed
   (with the file/symbol you actually found), which STOP precondition or fence triggered, and what
   you would need to proceed.
3. HALT. Do NOT skip ahead -- later tasks depend on earlier ones. The frontier author reviews the
   ledger and re-issues or amends the task.

Never bypass a hook, never weaken a sacred invariant to make a task pass, and never invent an oracle
to make a test go green.

## NEVER touch (global; each names its single sanctioned exception if any)

- `src/transport/mcp/tools.rs` (TOOLS_JSON: the 13 trained schemas + `explain`) -- byte-frozen. No
  exception in any task.
- `tests/tool_schema_fidelity.rs` -- the schema fidelity pin. No exception; keep green untouched.
- `tests/all_open_golden.rs` and the all-open byte-identity invariant -- no exception; new paths are
  no-ops for a lone all-open session.
- `tests/architecture.rs` a7 (`governance_core_has_no_forbidden_back_edges`) -- `src/governance/**`
  names no browser/transport/mcp/native/url and no tabId/token/socket type. Session/isolation code
  lands in `src/hub`. SANCTIONED EXCEPTION: H8 only may add the `channels.webapi.from` POLICY
  allowlist (governs sources, never which tools exist).
- `src/transport/native/host.rs` framing (4-byte LE prefix, `MAX_MESSAGE_LEN`, `encode`/
  `read_message`) -- the native wire shared with the policy-free extension. No exception this batch.
- The MCP JSON-RPC wire and the pinned `notifications/tools/list_changed` line in `server.rs` -- the
  adapter is a byte relay, never a rewriter. No exception.
- `Browser::attach` single-EXTENSION-link rejection (`AttachOutcome::AlreadyAttached`) -- retained.
  SANCTIONED EXCEPTION: H2 may add the kill-hook multi-session fan-out but must NOT weaken the single
  physical-extension-link invariant.
