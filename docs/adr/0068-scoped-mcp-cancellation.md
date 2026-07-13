# ADR-0068: Scoped MCP cancellation at composition boundaries

Status: Proposed (2026-07-12). Deferred and evidence-gated. Extends ADR-0049's MCP conformance
posture. If accepted and implemented, it amends ADR-0035 and ADR-0050 only in how `script` and
`browser_batch` stop between steps. It does not change any trained tool schema.

## Context

MCP defines `notifications/cancelled`: a client may name an in-flight request whose result it no
longer needs, and the receiver should stop work when it can do so honestly. Ghostlight currently
spawns each `tools/call` request independently and ignores unknown notifications. It has no
request-id-to-cancellation-token registry.

Most Ghostlight browser operations are effectively atomic at the service boundary. Once a click,
key press, navigation, upload, or form write has been dispatched to the extension, the page may
already have acted. Cancelling the Rust future cannot reverse that effect. A blanket
`JoinHandle::abort` would be worse than no cancellation: it could suppress the response and drop the
audit scope while the browser mutation still commits.

The useful cancellation boundary is composition. `script` and `browser_batch` run an explicit
sequence of separately authorized and audited steps. A cancellation received while step N is
running can let step N settle, record its real outcome, and prevent step N+1 from starting. That
saves work and can prevent later mutations without pretending the current mutation was rolled back.

`gif_creator export` may eventually have another honest boundary between service-side encoding
stages, but only if measurement shows export latency is material. The ordinary browser tools do not.

## Proposed decision

### 1. Implementation is gated on live client evidence

Before implementation, use local debug instrumentation to verify whether supported clients actually
send `notifications/cancelled` for in-flight Ghostlight calls. Test Codex, Claude Code, and at least
one other supported MCP client. Record only message type, request id shape, timing, and client name;
never page content or tool arguments, and never send data off the machine.

If common clients do not emit cancellation notifications, this ADR remains Proposed. Protocol
support with no caller has no current product value.

### 2. Cancellation is cooperative, never rollback

The first implementation scope is `script` and `browser_batch` only:

- The current step is allowed to settle normally.
- No later step starts after the cancellation token is observed.
- Every started step keeps its normal authorization, result classification, and audit record.
- No output may say or imply that an already-dispatched browser action was undone.
- Atomic tools may ignore cancellation, as MCP permits when work cannot be cancelled honestly.

`form_fill` stays one semantic intent and is not made cancellable between its internal field writes.
Stopping it halfway would create a partially filled form while presenting the parent intent as
cancelled. `wait_for` also stays unchanged in v1: interrupting its extension-side wait would require
a new wire cancellation mechanism for little practical gain.

### 3. The server tracks request cancellation without aborting audit-bearing tasks

The MCP session owns an in-flight registry keyed by the request's exact JSON-RPC id. A tool-call task
registers a cooperative cancellation token before execution and removes it on every terminal path.
`notifications/cancelled` validates the referenced id in the same session, marks its token, and
otherwise remains fire-and-forget. Unknown, malformed, already-completed, and cross-session ids are
ignored.

The notification handler does not call `JoinHandle::abort`. The tool-call task remains alive long
enough to let its current step settle and to close every audit scope. Once cooperative cancellation
has been accepted, the server suppresses that request's MCP response. A completion/cancellation race
is harmless: if completion already won and removed the registry entry, the late notification is
ignored; if cancellation won, the client has declared the result unused.

Browser pending-request entries must remain cancellation-safe. Dropping a caller must not leave a
sender in the pending map until the ordinary tool timeout. The implementation must use an RAII-style
pending guard or an equivalent single cleanup path; it must not scatter best-effort removals among
branches.

### 4. Batch state gains an internal cancelled stop reason

The shared sequential executor used by `script` and `browser_batch` gains a `cancelled` stop reason.
After the active step settles, every remaining step is classified `not_run` with cancellation as the
reason. The parent run records how many steps completed and at which boundary cancellation stopped
the sequence.

The normal cancelled response is suppressed at the MCP edge. The internal outcome still exists for
tests, debug instrumentation, and audit completion. If a completion race causes a response to win,
that response must be truthful: it may report completed steps and the cancelled boundary, never a
generic success summary.

### 5. Audit truth is a prerequisite to acceptance

Every started step and the parent composition call must finish their existing audit scopes. A future
implementation plan must pin one additive audit representation for the parent cancellation before
this ADR can move to Accepted. The preferred shape is an always-present `cancelled` boolean on tool
audit records, false by default and true on the cancelled parent. An alternative representation is
acceptable only if it gives the same durable answer without treating cancellation as a policy deny,
a hold, an error, or a successful completion.

Cancellation is execution control, not authorization. It creates no new governance decision and no
denial id. Completed steps retain their real allow, deny, shadow-deny, held, or error outcomes.

### 6. No extension cancellation message in v1

The first implementation adds no native-message cancellation frame. The service does not attempt to
interrupt a CDP command or page handler already in flight. This keeps the extension policy-free and
avoids false rollback semantics. A later extension-side mechanism requires its own evidence and ADR
amendment.

### 7. Verification gates

An implementation is not complete until tests prove:

1. Cancellation before step 1 runs no steps.
2. Cancellation during step N lets N settle and never starts N+1.
3. Started steps and the parent produce complete, truthful audit records.
4. A cancelled request produces no MCP response after cancellation wins.
5. A completion/cancellation race has one terminal owner and leaks no registry entry.
6. Unknown, malformed, late, and cross-session cancellation ids are ignored.
7. Atomic tools continue with their existing behavior and never claim rollback.
8. Dropped callers leave no stale browser pending entry.
9. Adapter reconnect replay and the no-initialize-before-use behavior from ADR-0045/0049 remain
   unchanged.

Use the existing in-process `Browser` over duplex seam for most coverage. Keep only irreducible
stdio/session race assertions in the spawn E2E tier.

## Non-goals

- Undoing or compensating for browser effects.
- Cancelling every tool merely because MCP has a cancellation notification.
- Aborting Rust tasks without audit finalization.
- Making `form_fill` partially cancellable.
- Adding MCP task augmentation or progress notifications in the same change.
- Adding extension-side or CDP cancellation in v1.

## Consequences

- Cancellation has narrow but real value: it prevents future steps in composed workflows.
- Atomic browser operations stay simple and truthful.
- The server gains a small request-lifecycle registry and the shared batch executor gains one stop
  reason, but the browser wire protocol stays unchanged.
- Audit correctness makes the implementation more involved than a task abort. That cost is accepted
  only after live clients demonstrate demand.
- Progress notifications, task augmentation, GIF-export interruption, and broader cancellation stay
  separate future decisions.

## Revisit triggers

Revisit this ADR when any of the following is true:

- Two supported MCP clients are observed sending cancellation for Ghostlight tool calls.
- Users report unwanted later `script` or `browser_batch` steps after cancelling a request.
- A composed workflow becomes long enough that avoiding remaining steps has measurable value.
- GIF export latency becomes material and exposes a clean service-side cancellation boundary.

## References

- MCP 2025-11-25 cancellation:
  https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation
- ADR-0035: `script` sequential composition.
- ADR-0049: MCP conformance and protocol-version posture.
- ADR-0050: `browser_batch` as the trained front door over the shared sequential executor.
