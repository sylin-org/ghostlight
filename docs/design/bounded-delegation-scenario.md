# Bounded delegation: release-candidate triage scenario

Status: Scenario exploration. Not an accepted design.

## Why this scenario

"Give the agent access" is too vague to design a responsible experience. A useful delegation has a
purpose, a boundary, a lifetime, and an understandable end state. This scenario tests whether the
existing tighten-only session overlay can become a user-facing delegation contract without turning
ordinary work into policy administration.

## The person and the job

Mira maintains an open-source library. She asks her MCP client:

> Prepare the 0.6 release candidate. Review the open GitHub pull requests and CI failures, compare
> them with the Linear release checklist, and update the checklist with your findings. Do not merge,
> publish, delete, change repository settings, or message anyone.

Mira is already signed into GitHub and Linear in Chromium. She wants the agent to do useful work in
that real context, but she does not intend to delegate release authority.

## What Ghostlight should make visible

Before the work begins, the client or Ghostlight presents a compact contract:

```text
Purpose: prepare the 0.6 release candidate
For: this Codex session
Until: 30 minutes from now or session end

May use:
  github.com       read
  linear.app       read, write

Will stop before:
  merge or close pull requests
  publish a release or package
  delete or change settings
  send comments, messages, or review decisions

Limits:
  at most 12 writes
  no execute capability
  no other hosts
```

The summary is the primary interface. A manifest-shaped detail view is available for inspection,
copying, or organizational review, but Mira does not need to author it.

## Expected journey

1. The agent translates Mira's request into a proposed boundary. It explains that updating Linear
   requires write capability, while merging, publishing, and communication remain outside scope.
2. Mira accepts the proposal once. The resulting policy can only tighten the authority already
   available from user, organization, and managed tiers.
3. Ghostlight binds the contract to the authenticated MCP subject and current session. A copied
   session identifier is not authority.
4. The agent reads the release checklist, pull requests, and CI state. The activity indicator stays
   visible in the browser without interrupting every read.
5. The agent updates checklist fields and adds a private release note inside the specified Linear
   project. Each write consumes the visible budget.
6. A pull request looks ready. The agent attempts to merge it. Ghostlight stops the operation and
   explains: "Merging is outside this delegation. The current session may inspect pull requests but
   may not merge or close them."
7. The agent continues with the work still inside scope instead of treating the denial as a fatal
   error.
8. At completion, Mira receives a digest: hosts visited, findings, checklist changes, unused write
   budget, denied attempts, and the contract's expiration.

## What makes this delightful

- Mira describes the job, not access-control syntax.
- The proposed boundary uses the same vocabulary as the work.
- Routine reads do not trigger repetitive approval prompts.
- A denial preserves momentum and explains the next valid move.
- The agent cannot turn a request for more authority into authority by itself.
- Expiry is automatic and visible.
- The final digest answers "what did I entrust, and what happened?" in one place.

## How the current architecture helps

ADR-0060 already provides the core safety property: a session overlay composes by intersection and
can only reduce authority granted by higher tiers. Identity-bound policy, host polarity, RAWX
classification, audit correlation, and the persistent service supply most of the remaining
substrate.

The scenario deliberately uses read and write but excludes execute. It also distinguishes a Linear
record update from external communication, showing why a useful contract may need intent descriptors
more specific than RAWX alone. RAWX remains the capability floor; named consequences refine it.

## Questions the scenario exposes

1. Who proposes the contract: the MCP client, Ghostlight, or the model through an additive tool?
2. How does a client establish or replace the session overlay after initialization without
   reconnecting?
3. Which clients can render a native confirmation through MCP elicitation, and what is the graceful
   fallback for clients that cannot?
4. Are time and write budgets part of the manifest, a separate delegation envelope, or both?
5. How are semantic consequences such as merge, publish, delete, and communicate declared and
   verified across built-in and future WebMCP tools?
6. Can the user extend a contract without creating an escalation path controlled by the agent?
7. What minimum digest remains useful without retaining sensitive page content?
8. How should saved scripts request a delegation that is narrower than their hash-bound approval?

## Disposition

Do not write a delegation ADR from vocabulary alone. Test this scenario in a design prototype first,
along with one personal scenario and one organization-managed scenario. The ADR should pin the user
journey and authority transition together; an elegant policy envelope with an awkward approval flow
would miss the point.
