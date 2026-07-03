# T06: manifest hot-reload

## Goal

Implement ADR-0025: the manifest hot-reloads from its watched file sources (the org policy
path, and a user `file://` source when that is what was given); the live governance state
(grants/hash/mode/posture) becomes a swappable per-call snapshot like `Config` already is;
a swap that changes the advertised tool set emits `notifications/tools/list_changed`; and
the two session events (`manifest_reload`, `user_manifest_ignored`) are recorded. This is
one of the two sanctioned behavioral ADDITIONS of stage 4 (BOOTSTRAP rule 8).

## Authority

ADR-0025 is normative for every semantic (watched sources, selection re-evaluation,
keep-last-good, removal-as-transition, notification gating, session events, snapshot
idiom). ADR-0023 defines the single loader this builds on. Where this prompt and an ADR
disagree, THE ADR WINS.

## Depends on

t01 (single loader) and t03 (Governance shape) landed; t04/t05 (pipeline) landed per
sequence. STOP if `ConfigStore::load_initial_with_policy` does not exist or the five
`record_*` methods still exist on `Governance`.

## Current behavior (verified 2026-07-03 against `b4b2faf` as reshaped by t01-t05; re-read)

- `reload.rs`: `WatchSources { user_config, org_policy, manifest: None }`;
  `watched_paths()` already returns the three slots in fixed order and is recomputed
  each poll ("the manifest slot may change under G12"); `spawn_watcher` polls
  fingerprints every `POLL_INTERVAL` (read the constant) and calls `reresolve()` on a
  settled change; `plan_reload` encodes keep-last-good (org ERROR, user WARN); after t01
  the org slot re-parses via `parse_manifest` and derives config layers only.
- `server.rs::run`: builds ONE `Governance` at startup from `LoadedPolicy` (grants/hash/
  mode cloned), a single-writer stdout task over an
  `mpsc::UnboundedChannel<JsonRpcResponse>`, and `ConfigStore` (+`spawn_watcher`) with a
  recorder-reload subscription; `tools_list_result(&governance)` serves `tools/list`
  from the static instance.
- `ports.rs::SessionEventRecord`: `event_id, ts, identity, client, event: &'static str,
  manifest: Option<ManifestIdentity>` -- the doc pins that later session events add a
  new EVENT STRING, never a new record shape. Field order pinned by inline tests. The
  only producer today is `record_session_killed` (`event: "session_killed"`).
- `source.rs::LoadedPolicy { manifest, origin, user_manifest_ignored }`; `load_policy`
  performs org-wins selection; `user_manifest_ignored`'s promised audit note is
  unimplemented (doc comment says a future task records it).
- The MCP writer channel carries only `JsonRpcResponse` (no notification shape exists).

## Required behavior

### 1. Policy state in the store (`reload.rs`)

- `load_initial_with_policy` additionally receives and retains the resolved user source
  string (`user_source: Option<String>`). Caller ripple, complete: `server::run` threads
  the `run_server` value; `src/doctor.rs::governance_section_lines` passes the SAME
  env-resolved source string it already gives `load_policy` a few lines above (doctor
  never spawns the watcher, so the retained source is inert there); the `load_initial`
  convenience passes `None`. Verify completeness with
  `rg -n "load_initial_with_policy" src/ tests/`. It sets `sources.manifest` to the user
  manifest's PATH whenever `parse_source_string(user_source)` yields
  `UserSource::FilePath` -- INDEPENDENT of which origin won selection (ADR-0025
  Decision 1: an ignored user file must still be watched so the org-deletion fallback
  stays live and later edits to it reload; `env://` or no source leaves it `None`) --
  and stores the startup `Arc<LoadedPolicy>` as the initial value of a new
  `watch::channel<Arc<LoadedPolicy>>`.
- `pub fn policy(&self) -> watch::Receiver<Arc<LoadedPolicy>>` exposes it.
- `reresolve()` gains the manifest half: call
  `source::load_policy(self.user_source.as_deref(), self.domain_pattern_valid)` --
  full re-selection, so ORG-file creation/deletion mid-session transitions exactly as
  startup would (ADR-0025 Decision 1). Ok: derive the org config layers from it (t01
  path) AND, iff the manifest identity changed (compare name+version+hash+origin,
  including present<->absent transitions), publish the new `Arc<LoadedPolicy>` on the
  channel. Err: keep-last-good for BOTH config layers and the published policy, ERROR
  log (extend `plan_reload`'s org slot to carry this; keep its pure-function
  testability). Pinned edge (ADR-0025 Decision 1, fail closed): a configured user
  `file://` source that goes MISSING makes `load_policy` return `Err(LoadError::Io)` --
  that is the keep-last-good path, NOT a transition to all-open; only ORG-file deletion
  transitions. Add the pure test `user_manifest_deletion_keeps_last_good`.
- Delete/absorb the now-redundant org-file read inside the old path (single parse per
  change event: one `load_policy` covers org + user manifest).

### 2. Swappable governance (`server.rs`)

- Extract the existing construction into
  `fn build_governance(policy: &LoadedPolicy, recorder: Arc<dyn AuditSink>) -> Governance`
  (all-open vs governed, exactly today's wiring).
- Hold `Arc<Mutex<Arc<Governance>>>` (the ConfigStore snapshot idiom). Every
  `tools/call` and `tools/list` clones the current `Arc<Governance>` ONCE at entry and
  uses it for the whole call (torn never; ADR-0025 Decision 6).
- CLIENT IDENTITY SURVIVES THE SWAP: `Governance.client` is captured write-once at
  initialize (`set_client`) and stamped into every record; a rebuilt instance starts
  `None`, which would null the client on all post-swap records. The subscription task
  must copy the outgoing snapshot's client into the rebuilt Governance before swapping
  (add a crate-visible accessor or re-call `set_client` with the retained initialize
  values). The flagship test asserts a post-swap record still carries the initialize
  clientInfo.
- Spawn a policy-subscription task: on each published `LoadedPolicy`: build the new
  `Governance`, swap it in, then (a) record the `manifest_reload` session event through
  the sink (event string exactly `manifest_reload`; the record's `manifest` field
  carries the new identity; a swap TO all-open carries `manifest: None`), (b) recompute
  `advertise::advertised_tools` before/after and, iff the ADVERTISED SET changed, send
  the notification (section 3), and (c) if `user_manifest_ignored` transitioned
  false -> true, record the `user_manifest_ignored` session event.
- At startup, when `loaded_policy.user_manifest_ignored` is already true, record
  `user_manifest_ignored` once (implements the source.rs promised note; update that doc
  comment to point here instead of "a future audit task").

### 3. The notification

Widen the writer channel to an enum (pin):

    enum Outbound {
        Response(JsonRpcResponse),
        ToolsListChanged,
    }

The writer serializes `ToolsListChanged` as exactly
`{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}` plus the newline. It is
sent ONLY when the advertised set changed (ADR-0025 Decision 4); config-only reloads and
no-op manifest touches emit nothing. All-open sessions with no watched manifest change
emit nothing ever (goldens untouched).

### 4. Session-event vocabulary (`ports.rs` / `dispatch.rs`)

Add the two producers alongside `record_session_killed` (same record shape, new event
strings; NO field changes). Update the `event` field's doc list. Inline tests pin both
events' serialized key order identical to `session_killed`'s and the exact event
strings.

## Constraints

1. One commit: `feat(architecture): t06 manifest hot-reload`.
2. `check_call`, `DecisionRequest`, denial ids (which already embed the hash), the audit
   CALL-record format, tools.json/fidelity: untouched. `tests/all_open_golden.rs` and
   `tests/mcp_protocol.rs`: expectation-unchanged (quiet path stays quiet).
3. `tests/architecture.rs` green: the policy channel and store stay in governance; the
   subscription task and notification live in transport.
4. No new dependencies (tokio `watch`/`Mutex` only). ASCII. Fail-closed reload matrix
   preserved and tested.
5. Torn-snapshot rule: a call in flight completes on its begin-time governance AND
   config snapshots (grep the pipeline for any mid-call re-read; there must be none).

## Tests (minimum)

1. `plan_reload`-level pure tests: manifest identity diffing (changed / unchanged /
   appeared / removed), keep-last-good on a broken org file (policy channel does NOT
   publish), ORG removal publishes an all-open policy, and
   `user_manifest_deletion_keeps_last_good` (a missing configured user file:// source is
   an error, not a transition).
2. `manifest_reload_and_user_manifest_ignored_events_are_shaped` (dispatch/ports
   inline): both producers emit the frozen record shape with the pinned event strings;
   key order matches the session_killed pin. Plus the transition gating: a reload that
   keeps `user_manifest_ignored` true does NOT emit a second event (ADR-0025 Decision 5).
3. `advertised_set_diff_gates_the_notification` (server/pipeline inline or unit): a
   grants change that alters the set -> exactly one `ToolsListChanged`; a change that
   does not (e.g. two grants collapsing to the same capability union) -> none.
4. `org_policy_hot_swap_end_to_end` (NEW integration, `#[cfg(windows)]`, in a NEW file
   `tests/hot_reload.rs`, reusing t01's temp-ProgramData technique): ISOLATION PIN --
   spawn the child with BOTH `ProgramData` AND `LOCALAPPDATA` overridden to the temp dir,
   and put NO audit config entries in the policy under test (the deletion phase would
   remove them mid-test); the audit stream then lands at the default-path derivation
   `<tempdir>\browser-mcp\audit.jsonl`, stable across the governed -> all-open
   transition. Spawn governed via a temp org policy (read-only grant); `initialize` +
   `tools/list` (governed set, transcribed from the t01 oracle); REWRITE the policy file
   adding `"action","write"` capabilities; poll `tools/list` until the set changes
   (timeout >= 4x `POLL_INTERVAL`, which is 750ms at authoring -- read the constant);
   assert the stdout stream carried the exact `list_changed` notification line; then
   DELETE the policy file and assert the set returns to the full 14 (all-open) with a
   second notification; the temp audit file contains two `manifest_reload` events (the
   second with `manifest: null`), and a post-swap tools/call record still carries the
   initialize clientInfo. Generous timeouts; this is the stage's flagship test.
5. Full suite green.

## Verification

fmt/clippy/test green; ASCII scan; ledger entry; RESUME HERE -> t07; commit. Append TWO
deferred live checks to BROWSER-TESTS.md:

    ## t06-1: policy edit applies live (no restart)
    Changed: t06 added manifest hot-reload (ADR-0025); grants/mode swap on org-file change.
    Steps: with a governed org policy active and a live client session, edit the policy
    file to add the "action" capability to the active grant; within a few seconds run a
    computer left_click that was previously denied; then delete the policy file and
    re-run any denied call.
    Expect: the click flips from Denied (capability) to executing, with audit lines
    showing the new manifest hash and a manifest_reload session event; after deletion the
    session is all-open (14 tools; a client that honors list_changed refreshes its tool
    list) and a second manifest_reload event carries manifest null.

    ## t06-2: broken mid-edit policy never weakens the session
    Changed: t06 keep-last-good on reload (fail-closed org matrix extended to grants).
    Steps: with a governed session, save the policy file mid-edit as invalid JSON; run a
    call outside the grants; then fix the file.
    Expect: enforcement continues on the last-good manifest (same denials, same hash) with
    an ERROR in the server log and NO manifest_reload event until the fixed save, which
    swaps normally.

## Out of scope

- Watching env:// or managed:// sources; extension/popup surfaces; re-advertisement on
  config-only changes; any grant SEMANTIC change (ADR-0022).
- Deletions (t07), docs (t08).
