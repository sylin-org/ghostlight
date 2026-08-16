# S1: experience contract

## Objective

Turn the approved direction into one durable product contract before changing behavior. Define the
experience in terms a user can observe, while keeping implementation placement open until the live
tree and owning ADRs have been read.

## Prompt outline

1. Audit the current install-to-operation, workbench landing, browser presence, tray and background
   lifetime, window-close, runtime control, recovery, and failure journeys. Name repetition, hidden
   dependencies, and surfaces made redundant by this epic.
2. Write the minimum ADR or marked amendments that establish:
   - progressive capability reveal;
   - the At a glance front door;
   - on-demand versus manual browser startup;
   - branded, minimal, and hidden browser-presence choices;
   - the future role of launcher, tray, notifications, idle lifetime, and window close;
   - running, paused, stopped, recovering, ready, and needs-attention meanings;
   - the boundary between canonical operation meaning and user-task intent;
   - the exact human-stop directive;
   - the limits of holding an MCP or CLI response while paused.
3. Reconcile the active 1.0 contracts and any superseded desktop or presentation statements. Keep
   historical ADRs immutable.
4. Define measurable acceptance for first use, automatic recovery, pause latency, truthful stop,
   diagnostic usefulness, keyboard use, and user comprehension.
5. Use compact state sketches or paper prototypes to test vocabulary before code. Record confusion
   as evidence, not as a reason to add explanatory chrome.

## Completion evidence

- Accepted decision record or amendments with no silent conflict.
- Updated active contracts and a list of obsolete or duplicated presentation to remove later.
- A state and configuration vocabulary small enough to enumerate completely.
- Owner-approved acceptance measures entered in the ledger.
- No production behavior change in this stage unless required to keep documentation truthful.

## Stop conditions

- A proposed state depends on Ghostlight inferring task-level intent.
- A proposed preference cannot be expressed as a small closed choice.
- Pause or stop cannot be defined consistently for plural sessions and operations.
- The stage would settle a protocol or lifecycle behavior without reading its owning ADR.
