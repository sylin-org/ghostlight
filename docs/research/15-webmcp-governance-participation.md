# WebMCP governance participation, 2026-07

Status: Research and outbound-feedback draft. No product implementation is authorized.

## Why act now

ADR-0043 correctly rejects production support while WebMCP is changing. It does not require
Ghostlight to remain silent while the security and consent model is being formed.

Chrome's public origin-trial material now explicitly asks implementers and agent builders for API,
security, and consent feedback. The current material recognizes prompt injection, recommends
`readOnlyHint` and `untrustedContentHint`, limits cross-origin exposure, and describes future user
interaction. Those are useful beginnings. Ghostlight has implementation experience with several
questions that remain open: runtime consequence classification, identity-bound decisions,
origin/frame attribution, audit, and in-band data provenance.

Primary sources reviewed 2026-07-12:

- https://developer.chrome.com/blog/ai-webmcp-origin-trial
- https://developer.chrome.com/docs/ai/webmcp/secure-tools
- https://developer.chrome.com/docs/ai/webmcp/evals
- https://github.com/webmachinelearning/webmcp

## Where Ghostlight can contribute

### A declaration is a claim, not a verdict

A page can label a tool read-only, but a governed consumer cannot treat a page-supplied hint as an
authorization fact. Classification may depend on arguments, authenticated state, destination, and
the concrete action selected at runtime. WebMCP should preserve hints for agent ergonomics while
allowing a consumer to apply its own deterministic policy.

### Origin and frame identity belong on every call

Tools are registered dynamically by documents and may be exposed across frame boundaries. A
consumer needs the registering origin, executing frame, top-level origin, exposure path, and tool
generation attached to discovery, decision, execution, result, and audit. A bare tool name and
schema are not enough.

### Dynamic tools create a time-of-check problem

A tool can be replaced or removed between discovery and invocation. A governed consumer needs a
stable registration identity or generation so the thing it classified is the thing it executes.
Name equality is insufficient.

### Read-only does not mean harmless

A read can reveal private information or return attacker-controlled instructions. The current
`untrustedContentHint` helps with model handling, while resource-aware policy answers a different
question: whether this subject may read this information from this origin at all. Both signals are
needed.

### Consequences deserve a prepare boundary

Some tools prepare a reversible UI change; others commit a purchase, publication, deletion, or
message. A generic user-interaction callback is useful, but consumers also need enough consequence
metadata to decide when interaction is required and to render a truthful preview.

### History and provenance are part of control

The WebMCP goals mention visibility, history, and control. The invocation contract should make it
possible for a consumer to record which registered tool ran, under which origin and user gesture,
with what declared and independently assigned consequence, and which prior structured results fed
its arguments. It must not imply visibility into data carried through the model's context.

## Proposed non-shipping experiment

1. Join Chrome's early preview program as the Ghostlight project.
2. Register a controlled Ghostlight demo origin for the public trial after owner review.
3. Build a small research page, not a shipped Ghostlight adapter, with four tools:
   - a private read;
   - an additive UI update;
   - a prepared but uncommitted write; and
   - a committed external consequence.
4. Exercise dynamic replacement, same-name tools in different frames, untrusted output, and one
   structured cross-tool data flow.
5. Classify each call with RAWX and record where static hints are sufficient or insufficient.
6. Publish findings only after the owner approves the exact text.

This experiment adds no extension permission, no Ghostlight tool, and no production WebMCP
consumer. ADR-0043 remains intact.

## Draft feedback for the WebMCP explainer

Suggested title: `Runtime consequence classification and provenance for WebMCP consumers`

> We maintain Ghostlight, a local governed browser-automation MCP server. Our enforcement experience
> suggests a useful separation between page declarations and consumer verdicts.
>
> Tool hints are valuable for model selection and UX, but a page-supplied hint cannot be an
> authorization fact. Consequence can depend on arguments, current authenticated state, destination,
> and whether an operation prepares or commits an external effect. Would the API preserve a stable
> registration identity or generation, plus registering origin, executing frame, and top-level
> origin, from discovery through invocation? That would let a consumer classify the exact
> registration it later executes and avoid same-name replacement races.
>
> We would also value a standard event shape that lets a consumer record the declared hints, its own
> runtime classification, the origin/frame identity, user-interaction result, and structured source
> call identifiers used in later arguments. This is not content inspection: it can attest only flows
> through the consumer's own structured substitution path.
>
> Finally, `readOnlyHint` and `untrustedContentHint` answer different questions. A private read may
> be non-mutating but still require resource-aware authorization; an allowed read may return
> attacker-controlled content that needs model-side handling. Keeping those axes separate would help
> governed consumers use both correctly.
>
> We are preparing a small origin-trial experiment around private reads, reversible UI updates,
> prepare-versus-commit actions, dynamic replacement, and cross-frame identity. We would be glad to
> share concrete results rather than speculate from the API shape alone.

## Owner actions before anything leaves the repository

- Approve or edit the draft above.
- Join the Chrome AI early preview program using the project identity.
- Choose the controlled origin for an origin-trial experiment.
- Confirm whether the experiment may live on sylin.org or should use a separate test origin.
- Approve any GitHub issue, Chromium report, or public findings note before submission.
