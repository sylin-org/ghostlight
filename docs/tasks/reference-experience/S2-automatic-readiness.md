# S2: automatic readiness and bounded recovery

## Objective

Make an admitted browser operation succeed through ordinary missing-readiness conditions without
requiring the user to diagnose the stack. Keep recovery conservative, bounded, and owned by the
existing lifecycle seams.

## Prompt outline

1. Map the exact current path from an admitted canonical browser operation to a proven live browser
   adapter. Identify the one seam that knows readiness failed and can request recovery safely.
2. Implement the smallest closed recovery sequence for the decided cases, including on-demand
   browser launch when enabled, single-flight waiting for the inbound adapter, and safe repair of
   stale Ghostlight-owned registration where already authorized by the lifecycle contract.
3. Choose a browser only from deterministic evidence. Prefer an associated or uniquely obvious
   browser; do not guess across ambiguous installations or profiles.
4. Keep recovery authority-neutral and pre-effect. Never install software, open a store page,
   overwrite foreign configuration, broaden policy, or replay an uncertain browser effect.
5. Make every wait bounded and observable. On exhaustion, identify browser absence, launch failure,
   extension absence, native-host failure, wrong profile, handshake timeout, or ambiguity as
   precisely as the available evidence permits.
6. Prove concurrency, cancellation, disconnect, timeout, disabled automatic launch, and no-duplicate
   launch behavior through the real process seams where necessary.

## Completion evidence

- The common no-browser-connected operation recovers automatically with the default setting.
- Manual startup mode never launches a browser and returns one useful recovery outcome.
- Simultaneous requests create at most one launch/recovery attempt per owning scope.
- Cancellation or caller loss cannot leave an abandoned operation to continue later.
- Recovery never changes authority or foreign state.
- Focused and process-level tests cover every closed recovery result.

## Stop conditions

- Recovery logic must be repeated at operation call sites.
- The design needs a generic recovery framework, new daemon, or second lifecycle authority.
- The available evidence cannot choose one browser conservatively.
- A timeout would leave effect truth unknown and then retry automatically.
