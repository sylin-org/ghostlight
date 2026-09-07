# ADR-0157: Session attention and dispatch control

- Status: Accepted
- Date: 2026-09-06
- Amends: ADR-0102, ADR-0121, ADR-0126, ADR-0156

## Decision

The owner approved H5's review-and-resume experience. Workspace lifetime owns automatic attention,
including the triggering invocation and closed cause. The governance facade retains the existing
denial thresholds; observed denials do not count. The human workbench offers review of the exact
history group and explicit session resume. Resume admits new requests without replay or permission
changes; a stale incident reference cannot clear a newer incident. Global human Resume does not
clear session attention. Global Pause/Stop remain independent and keep their fixed directives.

Observe-mode allowance cannot skip stricter request limits. Capture the final restriction decision
and the evaluated policy layers in the same H4 permission evidence.

The common browser port invokes an application-owned control check after waiting for its writer,
immediately before command transmission. Once dispatch has been admitted, a later control change
cannot imply rollback or establish that its effects did not occur. Do not hold an authority lock
while waiting for a browser receipt. Existing adapter human interlocks remain in force.

Model requests cannot clear attention. Human recovery is an allowlisted Tauri action on the same
WorkbenchFacade. No generic event bus, new process, connector protocol, policy setting, or extension
policy logic is introduced. Audit health, frame policy, and automatic flow resumption remain outside
this decision. See the H5 task for validation and deployment state.

## Implementation and evidence (2026-09-07)

Implemented locally. Incidents carry an id separate from their triggering invocation, and a
threshold-triggering child ends its composition even if human recovery races completion. Final
control checks also guard close compensation. A separate postcondition observation cannot erase
the action receipt that preceded it. The [H5 record](../tasks/security-hardening/h5-runtime-controls.md)
contains regression, process, UI evidence, and the untested installed/browser/platform lanes.
No connector protocol or extension code changed; no deployment or publication occurred.

Completed landings are still checked against policy after dispatch. A later runtime control
change does not create a permanent policy hold on an otherwise permitted tab; the next command
checks the live control again. Recovery tests prove the same tab remains usable after Resume.
