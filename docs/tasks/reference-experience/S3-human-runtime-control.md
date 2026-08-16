# S3: human runtime control

## Objective

Give pause, resume, and stop one domain meaning across GUI, tray, browser, CLI, and every supported
MCP era. The orchestrator owns the state; surfaces request and render it.

## Prompt outline

1. Audit the existing runtime-control, hold, cancellation, settlement, and completion paths. Reuse
   the existing executor and synchronization seam rather than adding a parallel scheduler.
2. Define exact operation, session, and global scopes for plural work. A one-session presentation
   may collapse controls visually but may not create singleton behavior.
3. Make pause prevent the next browser effect. Let an already-dispatched effect settle truthfully,
   keep the caller pending while its transport permits, and define caller timeout or disconnect as
   terminal rather than something that may resume later.
4. Make resume revalidate leases, browser generation, transient handles, authority gates, and any
   other current seam facts required before the next effect.
5. Make stop terminal and idempotent. Every affected invocation completes through the typed outcome
   path beginning with: `The user asked to interrupt the process. Wait for further instructions.`
   Preserve completed, partial, or uncertain effect facts after that sentence where necessary.
6. Decide and test which pause state survives workbench close, browser reconnect, harness reconnect,
   and orchestrator restart. Favor explicit human control over silent resumption.
7. Verify older and current MCP clients receive the same semantic result even when transport
   mechanics differ.

## Completion evidence

- One tested state machine owns running, paused, resumed, and stopped transitions.
- No browser effect begins after pause admission until a valid resume.
- Stop wakes held callers and no automatic retry is recommended.
- Caller cancellation or loss cannot create background continuation.
- Every control surface delegates to the same owner and contains no independent state rule.
- Plural-session tests prove scoped and global controls are unambiguous.

## Stop conditions

- A surface must compute whether Ghostlight is paused.
- A connector or extension must author the interruption sentence.
- Supporting one protocol era would weaken another.
- The implementation requires a second execution queue or workflow engine.
