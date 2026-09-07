# ADR-0159: Audit health and recovery

- Status: Accepted
- Date: 2026-09-07
- Amends: ADR-0103, ADR-0121, ADR-0122, ADR-0156

## Decision

The owner accepted H7 after discussion and directed implementation. An action can succeed while
its history cannot be saved. State both facts without changing the action's status, effects,
repeat safety, or known progress. For example: "Form submitted. History could not be saved."

`audit.availability` has two closed values. `keep_working` is the default: browser work continues
with visible history health and storage confirmation on each result. `require_audit` refuses
subsequent browser work while the shared destination has a known failure. Organization and user
layers compose monotonically; either can require audit. Grant observe mode does not relax this
explicit prerequisite. Each invocation keeps its immutable policy snapshot, while current storage
health is checked before browser preparation and at the final dispatch boundary after queuing.

Flows and sequences preserve earlier effects. A later audit refusal ends the composition even
under Continue. Parent and child storage are distinct: saving a parent does not confirm that its
children were saved. Policy explanation, service diagnostics, and human controls remain reachable.
Audit failure does not change global Pause/Stop or create a session-attention incident.

## Storage and recovery

One recorder serializes terminal receipts, asynchronous browser receipts, and recovery. The
JSONL sink reopens the configured path for each attempt and acknowledges only a successful write
and `sync_all`. A failed append or sync means storage is unconfirmed: bytes may exist. Live history
keeps its bounded receipt with that qualification. It never represents an unconfirmed receipt as
a failed browser action or automatically repeats the action.

Recovery attempts the destination at most once per five seconds while a failure is known. It
retains only one content-free gap counter and timestamps, never a backlog of failed receipts.
After synchronizing a recovery marker, it allows newly requested work. It neither replays work
nor backfills receipts or silently changes earlier unconfirmed rows to saved. Gaps remain visible.
Marker identity makes retries after an uncertain sync distinguishable from separate incidents.

Storage failure at startup does not prevent the service, workbench, or controls from starting.
History reading preserves valid neighboring records around malformed or oversized lines, reports
omission, and bounds the memory used per line. An unreadable source is explicit. Recovering writes
does not claim that previously unreadable history was loaded; a later restart can reload it.

The workbench uses the existing At a glance history and Status surfaces, without repeated native
notifications. Healthy operation adds no proactive notice. Strict unavailability also appears in
the shared readiness answer. The Policy editor and model-facing policy explanation expose the
effective choice. Result and health fields use closed content-free types; filesystem errors and
paths do not enter model results, gap markers, or retained action summaries.

## Placement and limits

`audit.rs` replaces the workbench's projecting-sink decorator and owns persistence health.
Governance owns the setting and immutable requirement. `language/audit_health.rs` owns the
storage vocabulary and explanations; completion still owns action truth. The browser and MCP
connectors, relay contracts, and extension acquire no policy or audit logic.

This is a known-failure prerequisite, not an atomic transaction between a website and local disk.
Another admitted action can already be running when a failure becomes known. A crash while the
destination is unwritable can lose volatile receipts and gap counts before a marker can be saved.
Storage synchronization is subject to operating-system and filesystem guarantees. The JSONL is
local, content-minimized, and editable by the host owner; it is not tamper proof or remotely backed up.

Implementation evidence and deployment limits live in
[H7 verification](../tasks/security-hardening/h7-audit-health.md).
