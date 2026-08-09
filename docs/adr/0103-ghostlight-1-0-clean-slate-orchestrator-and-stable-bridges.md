# ADR-0103: Ghostlight 1.0 clean-slate orchestrator and stable bridges

- Status: Accepted
- Date: 2026-08-09
- Supersedes: ADR-0102's 0.9 implementation and fixed twenty-four-tool contract, and the remaining
  implementation direction in ADR-0101
- Amends: ADR-0024, ADR-0030, ADR-0032, ADR-0034, ADR-0051, ADR-0093, ADR-0096, and ADR-0100
- Preserves as product invariants: ADR-0005, ADR-0013, ADR-0022, ADR-0028, ADR-0079, ADR-0080,
  ADR-0081, ADR-0098, and ADR-0099

## Context

Ghostlight 0.8 is the working prototype. It proves that a local MCP client can use a visible,
authenticated Chromium browser while the user retains control and may add governance and audit.
It also accumulated product aliases, compatibility paths, overlapping registries, tool-specific
browser handlers, duplicated result interpretation, feature flags, and tests that freeze internal
shapes instead of user outcomes.

ADR-0102 attempted to simplify that system in place. Its one-surface language and typed-result
direction were useful, but the implementation remained constrained by the inherited pipeline and
its verification mass. A live 0.9 experiment made the boundary error concrete:
`browser_open_tab({url})` was treated as one physical adapter mechanism. The adapter created a
blank tab but did not complete the requested use case. The service truthfully reported a partial
effect, yet the client did not receive the simple outcome it requested.

The problem is broader than one operation. Product use cases still leak into the MCP edge and
browser adapter, while the service coordinates around their receipts. Tests then make those
accidental seams expensive to change. Continuing to repair that structure would preserve the
prototype's architecture as the foundation of 1.0.

Ghostlight 1.0 instead starts from a small bill of intent. Version 0.8 is harvested for observed
behavior and lessons, not copied as production code. The persistent service becomes the sole
owner of product language and use-case state machines. The MCP and native-browser bridges become
stable protocol shores. Governance and visual feedback keep their proven promises through simpler
ports.

This is an engineering clean-slate decision, not a legal provenance claim. The repository history
remains available, but 1.0 code is written from the accepted intent and executable behavior rather
than transplanted from 0.8 or the 0.9 experiment.

## Decision

### 1. Freeze 0.8 as the prototype baseline

The exact prototype baseline is Git tag `v0.8.0`, commit
`993135b048b60622157266b53b21f1719c9df4b3`. Its source, release artifacts, public documentation,
and tests remain available through Git history. It stays the stable install until 1.0 passes its
acceptance journeys.

The current uncommitted 0.9 operation-kernel work is an experiment. Before destructive reset, it
must be captured in an explicit archive ref or bundle with its documentation and live findings.
It is not an intermediate release and does not become a compatibility layer.

The 1.0 tree contains no 0.8 runtime fallback, legacy directory, old tool aliases, compatibility
serializer, duplicated catalog, or inherited test suite. Historical preservation happens through
Git, not through production files that every future change must carry.

### 2. Write the 1.0 bill of intent before production code

Four short documents become the implementation authority:

1. `docs/1.0/INTENT.md` -- user jobs, promises, non-goals, and delight principles;
2. `docs/1.0/LANGUAGE.md` -- exact tool names, descriptions, schemas, defaults, results, and
   recovery guidance;
3. `docs/1.0/ARCHITECTURE.md` -- boundaries, ownership, dependency direction, and operation
   lifecycle;
4. `docs/1.0/ACCEPTANCE.md` -- executable user journeys and safety invariants required for 1.0.

The existing ubiquitous-language and governance-language documents are research inputs. They do
not force a tool count or retain a decision merely because the 0.9 experiment implemented it.
Tools are selected by distinct user jobs, truthful authority, and measured ease of use. The bill
of intent is accepted before the new production implementation begins.

ADRs record only durable architectural decisions. They do not duplicate the four product
documents or grow into an implementation ledger.

### 3. Keep three process boundaries and remove product logic from the bridges

The three executable roles remain because they have distinct owners and lifetimes:

```text
MCP client
  <stdio>
ghostlight-mcp-connector
  <stable owner-only invocation bridge>
ghostlight orchestrator service
  <stable browser-primitive bridge>
ghostlight-browser-connector + policy-free Chromium adapter
  <Chrome APIs and CDP>
browser
```

`ghostlight-mcp-connector` owns only MCP framing, revision lifecycle, correlation, and rendering.
It obtains the current Ghostlight catalog from the service and forwards a stable invocation
envelope. It does not own tool defaults, per-tool decoders, use-case semantics, governance, or
recovery decisions. A new orchestrator feature does not require new MCP connector logic.

`ghostlight-browser-connector` remains a native-message frame relay. It has no operation,
governance, workspace, or presentation semantics.

The Chromium adapter implements a bounded catalog of policy-free physical primitives and renders
content-free presentation events. It does not implement model-facing tools or construct Ghostlight
results. A product feature composed from existing primitives changes only the orchestrator. A
genuinely new Chrome capability may require a new primitive and adapter change; the single-point
mutation rule does not pretend otherwise.

### 4. Make the service a domain-driven modular monolith and the single product mutation point

The orchestrator is one deployable service and one application runtime. Inside it, code is divided
by product domain rather than transport, protocol revision, tool name, or technical framework. The
initial bounded contexts are:

- language -- catalog, schemas, defaults, and operation decoding;
- work -- invocation lifecycle, sequences, deadlines, cancellation, and terminal outcomes;
- workspace -- controlled tabs, opaque handles, selection, ownership, and leases;
- governance -- authority, admission, landing decisions, runtime controls, and audit intent;
- browser -- typed physical primitives and observed browser facts;
- presentation -- content-free user feedback and its lifecycle.

These are modules inside the monolith, not services, crates, processes, plugin interfaces, or
independently versioned components by default. A new boundary must justify a distinct lifecycle,
trust boundary, replaceable external dependency, or independent consistency need.

All model-requested mutation enters through one application executor. Workspace mutation enters
through the workspace aggregate, browser effects through the browser port, authority decisions
through the governance facade, and terminal construction through one completion path. These are
the deliberate chokepoints. No handler writes another context's state, talks directly to a
transport, or assembles a client result around those chokepoints.

Contexts publish small typed domain events after meaningful state transitions. Events carry domain
facts such as work started, tab created, document committed, work blocked, work completed, hold
entered, or attention required. They let audit, presentation, workspace bookkeeping, and lifecycle
reactions remain separate from the use-case handler. Event handling is explicit and in-process.
There is no generic message broker, actor framework, event-sourcing store, CQRS split, reflection
registry, or eventually consistent authority path.

Events are not a substitute for ordinary calls. Invariants that must decide the current operation
-- validation, lease acquisition, authorization, ownership, browser dispatch, and terminal result
construction -- remain synchronous inside one unit of work. A domain event records a state change
or requests a separate reaction; it never grants authority or fabricates success.

The service owns:

- the Ghostlight catalog and agent-facing descriptions;
- argument schemas, defaults, validation, and typed operation decoding;
- one application handler for each user job;
- workspace ownership, scheduling, cancellation, and deadlines;
- governance classification, admission, landing decisions, and audit;
- orchestration over browser primitives;
- compensation and uncertainty handling;
- closed typed results, summaries, repeat guidance, and suggested next steps.

Every invocation follows one lifecycle:

```text
decode and default
-> validate intent
-> select scheduling resource
-> acquire lease
-> capture authority
-> admit
-> execute the use-case state machine
-> observe and govern resulting browser state
-> compensate when safe and useful
-> construct one typed result
-> bind opaque handles
-> audit and return
```

The executor retains the unit-of-work state until that lifecycle reaches one terminal outcome.
Provisional resources, emitted events, compensation, audit correlation, and the final result all
belong to that invocation. Opening a tab and loading its requested URL are therefore internal
steps of one `browser_open_tab` use case, not separate client-visible successes.

There are no architectural classes named direct, dynamic, local, compatibility, or vendor
operation. An operation handler may call zero, one, or many browser primitives. That is private
application behavior. One MCP call remains one client-visible unit of work.

Sequence execution calls the same operation executor for each child under one retained sequence
context. It does not have a second dispatcher, result projector, or authority model.

### 5. Use a small typed browser primitive port

The browser port expresses physical capabilities such as:

- tab creation, selection, focus, close, and topology observation;
- navigation dispatch and committed-document observation;
- page inspection and bounded text reads;
- target resolution and input;
- screenshot capture;
- dialog observation and resolution;
- bounded readiness observation;
- content-free presentation events.

Primitive requests and receipts contain typed browser facts and correlation only. They contain no
model prose, Ghostlight status, policy decision, suggested next step, or client tool name.

The adapter handshake negotiates one explicit 1.0 browser-port contract and its supported
primitive catalog. It does not add a feature flag for each orchestrated use case. Incompatible 0.8
and 1.0 peers fail loudly; they never fall back to an old grammar or guess support from related
features.

### 6. Reimplement governance behind one simple facade

The following governance promises survive as 1.0 intent:

- no policy means all browser capabilities are permitted;
- protected hosts remain an independent deny ceiling;
- one immutable authority snapshot governs a started operation;
- request restrictions can only tighten authority;
- queue admission, final ownership, hold, attention, and end-session checks remain service-owned;
- every committed landing is governed before its content or readiness is accepted;
- audit remains payload-free and separate from permission;
- invalid managed authority never falls back to all-open.

The new orchestrator consumes governance through one bounded facade for admission, landing, and
terminal audit. Policy source loading and settings normalization remain behind that facade. Tool
handlers do not know manifest layers, grant parsing, audit sinks, or presentation rules.

Governance is reimplemented from these accepted invariants and the 1.0 acceptance cases. The 0.8
module graph is not transplanted.

### 7. Reimplement visual feedback as a presentation port

Visual feedback remains a core Ghostlight promise: the user can see active scope and understand
what the agent is doing without page content becoming authority.

The orchestrator emits a small vocabulary of bounded, content-free presentation events such as
operation start, target indication, progress, completion, denial, and attention. The adapter owns
their browser-local rendering and document lifecycle. Tool handlers do not send DOM messages or
know renderer internals. Presentation failure never changes authority or fabricates operation
success.

The proven privacy, document-revision, and capture-barrier invariants from 0.8 become acceptance
tests. The old broker implementation and tool-specific visual message catalog do not carry over.

### 8. Derive tests from 1.0 intent

The 0.8 and 0.9 test suites remain historical evidence. They do not gate the clean 1.0 tree.

A test must earn its maintenance cost by protecting a distinct product contract, safety invariant,
failure branch, or user journey. The same fact is not re-proved in every crate, tool, protocol
revision, and process topology. Shared executor behavior is tested once at its owning seam; bridge
framing is tested once per bridge contract; orchestration tests cover materially different state
transitions rather than every tool permutation. Obsolete tests are deleted when their protected
contract disappears.

The new implementation has three test strata:

1. operation state-machine tests derived from `ACCEPTANCE.md`, including defaults, governance,
   cancellation, uncertainty, compensation, and recovery;
2. generic MCP and browser bridge conformance tests, written once for their stable envelopes and
   lifecycle contracts;
3. real visible-browser journeys that prove the user jobs and visual/governance invariants.

Tests assert client-visible outcomes, authority, physical facts at a port, and bounded recovery.
They do not freeze internal function names, source substrings, historical aliases, mechanism
counts, or temporary implementation topology.

The implementation process follows the same rule. Routine changes need a clear intent, focused
tests, formatting, and linting. They do not require a new ledger, checkpoint document, approval
round, exhaustive matrix, or full live-stack rerun unless the change crosses a durable architecture,
trust, release, or real-process boundary. ADRs record durable decisions, not ordinary coding steps.
Full-workspace and live-browser gates run at integration milestones and release readiness, not as
ceremony after every local edit.

### 9. Make 1.0 a real breaking release

Version 1.0 introduces a new service bridge major and browser-port contract. A 1.0 connector or
adapter refuses a 0.8 service, and a 1.0 service refuses 0.8 peers. Installation replaces the
prototype only after the complete acceptance suite and live local journey pass.

There is no automatic configuration reinterpretation. If 1.0 governance input differs from 0.8,
a separate explicit migration command may translate a file with a preview and validation. Runtime
compatibility code is not retained for that purpose.

Public release, store submission, tags, and `dev -> main` remain owner decisions.

## Non-goals

- preserving the 0.8 or 0.9 internal architecture;
- preserving an arbitrary tool count;
- supporting vendor-specific tool dialects;
- running old and new pipelines in one shipping binary;
- creating a workflow framework, plugin framework, microservice topology, generic event bus,
  actor system, CQRS split, or event-sourcing platform;
- using the rewrite to weaken governance, continuity, privacy, or truthful uncertainty.

## Implementation order

1. Capture the 0.9 experiment and reset the implementation branch to a clean 1.0 workspace.
2. Write and accept the four bill-of-intent documents.
3. Define the modular-monolith contexts, dependency direction, executor chokepoint, unit of work,
   and small domain-event vocabulary in `ARCHITECTURE.md`.
4. Establish the stable MCP invocation/catalog bridge and browser primitive bridge.
5. Implement one vertical journey: open a page, read it, and close its tab.
6. Verify that journey through the real local MCP edge, service, adapter, and visible browser.
7. Add the remaining accepted user jobs through the same executor.
8. Reimplement governance and visual feedback through their small facades and domain events.
9. Pass all operation, bridge, governance, presentation, and real-browser acceptance evidence.
10. Replace the local 0.8 deployment and prepare the 1.0 release only after those gates pass.

## Required evidence

- the accepted 1.0 bill of intent with no unresolved tool or authority ambiguity;
- a source search proving no 0.8 surface, alias, compatibility serializer, or duplicate pipeline
  exists in the 1.0 tree;
- one service-owned catalog and typed operation/result domain;
- one application executor and unit-of-work lifecycle through which every model-requested mutation
  passes;
- explicit bounded-context dependency direction and a small closed domain-event vocabulary, with
  no general-purpose event or workflow framework;
- unchanged generic MCP connector code across at least one added orchestrator-only operation;
- unchanged browser connector code across the complete implementation;
- branch-complete operation state-machine tests at the shared executor seam, plus one direct/sequence
  equivalence proof for that executor;
- focused governance tests for each distinct admission, redirect, hold, attention, cancellation,
  and uncertainty invariant;
- focused presentation tests for privacy, document lifecycle, and authority independence;
- the smallest set of real visible-browser journeys that covers each materially distinct browser
  and user-safety path;
- clean formatting, strict lint, dependency, license, diff, and ASCII gates;
- a guarded local 1.0 replacement followed by successful live use.

## Consequences

Ghostlight 1.0 is defined by its intended experience rather than the survivorship of prototype
code. The service becomes the clear application center. Protocol bridges become boring and
stable. Browser complexity is expressed as reusable physical primitives. Governance and visual
feedback remain differentiators without leaking their machinery into every tool.

The rewrite intentionally gives up incremental compatibility and much existing test coverage.
That risk is accepted because the old coverage frequently protects accidental structure. The
counterweight is a smaller bill of intent, explicit safety invariants, a vertical real-browser
proof before breadth, and acceptance tests derived directly from the desired product.

This is deliberately not a mandate for maximal coverage or process. Test count, document count,
review checkpoints, and gate count are not progress metrics. Fast feedback, clear ownership, and
proof of meaningful behavior are.

Development takes longer before the first releasable 1.0 build, while 0.8 remains the stable
product. In return, future product work has one mutation point and fewer reasons to touch either
bridge.
