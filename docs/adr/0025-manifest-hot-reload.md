# 0025. Manifest hot-reload

- Status: Accepted
- Date: 2026-07
- Builds on: ADR-0018 (observe-then-enforce; the a5 hot-reload substrate), ADR-0019
  (layered configuration; the watcher and fail-closed reload matrix), ADR-0020 (org
  policy experience), ADR-0022 (schema-3 manifests), ADR-0023 (one loader: the
  watcher now holds a full fresh `Manifest` on every org-file change), ADR-0024 (the
  swap must fit the authorize/AuditScope shape).

## Context

Stage 2 built hot-reload for CONFIG (the a5 substrate: `ConfigStore` atomic-swap
snapshots, a debounced file watcher, a fail-closed reload matrix, live re-resolution
of sacred domains, audit destination, mode, and redaction) but deliberately deferred
the MANIFEST: `WatchSources.manifest` is a hardcoded `None` with a G12
integration-point comment, `Governance` is constructed once at startup from frozen
clones of the grants, hash, and mode, and the advertisement module doc records that
dynamic re-advertisement is not implemented.

The stage-2 live sessions showed the cost: changing policy meant killing the server
and letting the client respawn it, and REMOVING a policy file mid-session required
the same dance. The org-policy experience ADR-0020 sells an iterate loop (edit,
explain, simulate, observe); ADR-0023 makes the watcher parse the full manifest on
every org-file change anyway. Leaving the parsed grants on the floor is now the
anomaly. Hot-reload was a stage-2 delight-resolved decision ("hot-reload
first-class"); this ADR completes it.

## Decision

### 1. Watched sources

The manifest hot-reloads from the sources whose paths are known and local:

- the org policy path (already polled by the a5 watcher), and
- a user-supplied `file://` manifest source (the `--manifest`/`BROWSER_MCP_MANIFEST`
  path given at startup).

An `env://` source has no file to watch and stays fixed for the session. The user
`file://` path is watched whenever it was GIVEN at startup, independent of which
source won selection (an ignored user manifest must still be watched so the
org-deletion fallback stays live and later edits to it reload). The org-wins
selection rule (ADR-0023 / shared format 1.3) is re-evaluated on every reload
event: creating an org file mid-session overrides a user manifest; deleting the ORG
file falls back to the user source if one was given, else to all-open. Org-file
creation and deletion are both legitimate, first-class transitions. A configured
user `file://` source that goes MISSING is a load error, not a transition: reload
keeps the last-good state (fail closed), exactly like an invalid edit.

### 2. Swappable governance state, config-store idiom

The live governance state (grants, manifest hash, manifest mode, governed/all-open
posture) becomes a swappable snapshot exactly like `Config`: one atomic snapshot
read per call at the dispatch chokepoint, torn never (a call that started under the
old manifest completes under it, including its audit attribution). The swap
mechanism follows the existing `ConfigStore` idiom (std/tokio primitives already in
the tree; no new dependencies). The pure decision core (`check_call`,
`DecisionRequest`, denial-id derivation) is untouched: denial ids already
incorporate the manifest hash, so ids issued under different manifest versions
differ by design.

### 3. Reload semantics: fail-closed, keep-last-good

On a settled change to a watched manifest source: one `parse_manifest` (ADR-0023's
single loader). Success: atomically swap the governance snapshot AND re-derive the
config layers from the same parse (one parse feeds both consumers). Failure
(unparseable, invalid, schema violation): keep the last-good governance snapshot and
config layers, log at ERROR -- the same keep-last-good matrix the config reload
already implements. Startup remains fail-loud (ADR-0023 Decision 4). File deletion
is not a failure: it re-runs source selection per Decision 1.

### 4. Re-advertisement

The advertised tool surface is a function of the grants (ADR-0022 Decision 8). When
a manifest swap changes the advertised set, the server emits the MCP
`notifications/tools/list_changed` notification through the existing single-writer
stdout task, and subsequent `tools/list` calls serve the new set. Clients that
ignore the notification are merely stale until their next `tools/list`; nothing
breaks. No notification is emitted when the advertised set is unchanged (a grants
edit that does not change the set, or a config-only edit). Under all-open with no
watched manifest sources, no notification is ever emitted: the all-open goldens are
untouched.

### 5. Audit: session events for policy transitions

Two additions to the session-event record vocabulary (the `SessionEventRecord`
shape, distinct from per-call `AuditRecord`s, exactly like `session_killed`):

- `manifest_reload`: recorded on every successful swap. The record's existing
  `manifest` identity field carries the new manifest's name/version/hash; the origin
  (org path vs user source) goes to the operational log, not the record -- the
  session-event record SHAPE is frozen by design ("later session events add their own
  string, never a new record shape"), and this ADR honors that. A failed reload
  records nothing new (the ERROR log carries it; the audit stream records what IS in
  force, not what failed to be).
- `user_manifest_ignored`: recorded once at startup when the condition holds, and
  again whenever a reload newly re-establishes it after it had lapsed (a TRANSITION,
  not a repeat: consecutive reloads under the same ignored user manifest record
  nothing new) -- implementing the note promised in the stage-2 source-selection
  code ("a future audit task notes this in the session's first record").

Field order and the existing session-event shape rules apply unchanged.

### 6. Interaction with in-flight state

- Holds and the kill switch are browser-session state, orthogonal to the manifest;
  a swap changes neither.
- The navigate landing re-check uses the same snapshot its call started with.
- `policy explain`/`simulate`/`doctor` are one-shot CLI paths; they parse fresh on
  each run and need nothing from this ADR.

## Consequences

- Positive: the policy iterate loop (edit, watch it apply, observe denials, adjust)
  works live against a running session; the stage-2 "kill the server to change
  policy" dance dies. Org policy can be installed, updated, and removed under a
  running client.
- Positive: symmetric mental model: config swaps live (stage 2), policy swaps live
  (this ADR), one watcher, one parse, one matrix.
- Negative: `Governance` moves from construct-once to snapshot-per-call; every
  consumer that held `&Governance` state across an await must be audited for
  torn-snapshot correctness (the rule is the same one `Config` already follows:
  snapshot once at call entry).
- Negative: a new notification type crosses the MCP surface. `list_changed` is
  standard MCP and additive; the all-open goldens prove the quiet path stays quiet.
- Risk, accepted: rapid successive edits produce multiple swaps; the debounce the
  a5 watcher already applies bounds this, and keep-last-good means a half-saved
  file cannot take the session down.

## Future work (explicitly not this ADR)

- Watching `managed://`/registry-delivered policy (no such source exists yet).
- Re-advertisement push on CONFIG-only changes (nothing in the advertised set
  depends on config today).
- A UI affordance in the extension popup showing the active policy name/version
  (extension changes are out of scope for stage 4 entirely).

## Provenance

Scoped into stage 4 by the user (2026-07-03) alongside ADR-0023/0024, with
break-and-rebuild sanctioned and delight as the north star; hot-reload first-class
was already a stage-2 delight-resolved decision recorded in the stage-2 plan.
User-decided: inclusion in scope. Recommended-and-accepted: watching org + file://
sources only; the config-store snapshot idiom over new dependencies; keep-last-good
for invalid edits with removal as a legitimate transition; `list_changed` gated on
actual set change; the two session-event additions including the promised
`user_manifest_ignored`.
