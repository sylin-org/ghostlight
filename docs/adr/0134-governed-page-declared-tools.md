# ADR-0134: Governed invocation of page-declared tools

- Status: Accepted
- Date: 2026-08-22
- Supersedes: ADR-0043 Decisions 1, 2, and 4
- Amends: ADR-0107's tool catalog (23 tools after ADR-0133) to 25
- Builds on: ADR-0005, ADR-0018, ADR-0022, ADR-0042, ADR-0078, ADR-0093, ADR-0103,
  ADR-0107, ADR-0111, ADR-0113, ADR-0114, ADR-0121, ADR-0122, ADR-0131, ADR-0132,
  and ADR-0133

## Context

WebMCP is the W3C draft that lets a web page declare callable tools to the browser's agent.
It is in public origin trial (Chrome 149 through 156) and its API surface has already moved
once mid-trial: `navigator.modelContext` became `document.modelContext` in Chrome 150.
ADR-0043 answered the question with a recorded stance: a future governed consumer, no
implementation during flux, and named re-evaluation triggers.

Those triggers have now fired in substance. The owner directed design work (trigger three),
and the landscape moved: first-party agentic browsing shipped in Chrome, while no neutral
agent can consume site-declared tools under organizational policy. Waiting longer forfeits
the design lead; coding against a moving API builds twice.

This ADR resolves that tension the way ADR-0133 does: it records the complete destination now
and gates implementation on evidence. It carries ADR-0043's central insight forward unchanged,
that consuming site-declared tools safely is precisely a governance problem, and replaces its
deferral and its dynamic-directory integration shape with concrete decisions.

Three facts shape every decision below. The declarer is untrusted content. The real effect of
an invocation may land out of band, on the site's servers. And the mechanism, WebMCP today and
something else tomorrow, must stay out of the authority vocabulary.

## Decision

### 1. Two fixed verbs; the catalog stays closed

Ghostlight adds `browser_list_page_tools` and `browser_invoke_page_tool`. The names describe
the authority relationship, this page declares a tool and you invoke it, and not the mechanism,
per ADR-0111's doctrine. The mechanism appears only in descriptions and in data. Descriptions
state where the tools come from (declarations of the currently loaded document, by web
standards such as WebMCP; the accepted set may grow), that page-supplied scopes and hints are
advisory claims, and that a listing expires when the document commits. The catalog grows from
23 to 25 tools. There are no aliases.

### 2. Discovery is generation-bound and bounded

A listing returns a bounded array. Each entry carries a declared id, a bounded title and
description, the advertised input schema when present within hard depth and byte bounds, and a
closed `source` value (`webmcp` today, additive later). A listing binds to the document
generation exactly as target and view handles do (ADR-0131): a commit invalidates it, and
stale ids refuse before dispatch with a re-list suggestion.

If the discovery seam cannot distinguish "the page declares nothing" from "the page offers no
such surface", one truthful sentence covers both. Ghostlight never invents a distinction the
browser cannot prove.

### 3. Invocation resolves one controlled tab and validates shallowly

Invocation resolves its tab through the existing selection rules: explicit handle first, then
the workspace's ordinary crossing rules (ADR-0114). Element targeting does not apply; tools
bind to documents, not elements.

An unknown tool id refuses and names what is declared on that tab. Parameters are validated
shallowly against the page-advertised schema with hard bounds; a mismatch refuses without
effect and returns the expected shape. A page's schema is untrusted input to the decoder: it
is bounded and refused when malformed, never treated as executable logic.

### 4. Independent RAWX membership

Listing joins the R set. Invocation joins the T set. Both are independent sets in the exact
action directory that drives enforcement, audit, catalog projection, explanation, and
simulation. No configured policy leaves both open, subject to the protected ceiling. Sequence
and flow steps compose them, and each step authorizes normally.

This separation is the feature. An organization must be able to say "declared tools yes,
arbitrary page JavaScript no". That requires T to exist apart from execute.

### 5. Effects are attested, never observed

The physical receipt records only what the browser observed: dispatch and the page-reported
result. The real effect may land on the site's servers, out of band. Audit therefore uses a
distinct effect class, `attested`, beside no-effect, committed, and unknown. The
unknown-disposition rules apply unchanged: a disconnect after dispatch yields unknown,
cancellation yields unknown, and nothing becomes repeat-safe because a page claimed
harmlessness.

Results are page content. They render through the normal envelopes (ADR-0132), obey the normal
output budget, and never enter audit or presentation. Outcomes speak person sentences
(ADR-0103): "Called the site's booking tool. It reported success."

### 6. Page-supplied scope claims follow a three-valued trust ladder

Pages may annotate their tools, a read-only hint for example. Such a claim is advisory input,
never authority.

- Default, informational: the claim is logged and surfaced as provenance, continuing the
  ADR-0042 and ADR-0078 lineage. Enforcement uses policy's own classification. The claim
  informs the model; it never informs the gate.
- Trusted hosts: local schema-3 gains an additive optional block designating specific hosts
  annotation-trusted, and managed bundles may carry the same designation. On a trusted host, a
  claim that narrows a class admits invocation under the lighter grant. Trust lives in policy
  about a host, never in the page's hands. No claim ever expands authority.
- Attested stays attested: even a trusted narrowing never upgrades receipt truth. Unknown
  stays unknown.

Adoption rides observe mode (ADR-0018). Claimed-versus-enforced divergence is recorded per
host, and promoting a host to trusted is a deliberate, auditable policy act.

### 7. Fringe placement and revision negotiation

All product semantics live in the orchestrator. The extension gains two physical primitives
behind explicitly advertised capabilities with minimum revisions, so an older adapter fails
before dispatch with a precise capability-version result (ADR-0093, ADR-0113, and the
ADR-0133 Decision 10 pattern). The opaque browser connector and the generic MCP edge do not
change.

Whether Chromium exposes declarations to extensions or only through CDP is Chromium's
decision. The browser port is defined so either implementation satisfies it. Listing and
invocation are ordinary local browser work; a page's tool call travels wherever that page
already sends its traffic. ADR-0028 gains nothing new.

### 8. Audit and privacy

An invocation record names time, invocation id, workspace, tab identity, the committed host,
the declared tool id, the source value, the complete RAWX requirement set, the authority
version, deciding tier, grant, rule, denial id, decision, status, effect class `attested`, and
claimed-versus-effective class when they differ. It excludes parameters, results, schemas, and
page content, like every other record.

### 9. Implementation is gated, and evidence is local

Implementation begins only when Chromium ships a stable exposure path in a release channel or
commits to a durable extension or CDP contract. Each slice adds decoder, governance, wire, and
extension tests, and runs live journeys against a bundled offline fixture page that declares
tools. No external site enters CI. An extension change makes any pending store draft stale;
store submission remains a separate owner confirmation (the ADR-0133 Decision 12 posture).

## Consequences

- The catalog reaches 25 tools. Tests, docs, and roster guards that pin the count need one
  mechanical sweep when the verbs land.
- Organizations gain their most likely posture for this decade: declared tools permitted,
  arbitrary script execution refused, both audited identically.
- Ghostlight becomes the neutral broker for site-declared tools across every supported
  harness, which no first-party agent can offer for its competitors' clients.
- Two descriptions join the token budget; they stay tight.
- If the standard dies, the loss is two dormant verbs, one fixture, and a superseding ADR.
  The `source` vocabulary and the browser port isolate the mechanism either way.

## Rejected alternatives

### Dynamic per-page catalog entries

Projecting each page's tools into the MCP catalog with change notifications was rejected. It
churns the catalog on every commit, inflates token budgets, confronts lower-capability models
with a shifting surface, and leaves cached catalogs stale mid-journey. The closed catalog is a
deliberate product property.

### Overload `browser_execute` with a mode

Rejected. One tool name cannot carry two authority classes. Policy could not say "tools yes,
JavaScript no" without mode-conditional rules, which derive enforcement from argument payloads
and break the exact-action-directory invariant. Receipts differ, annotations differ, and
lower-capability models would pay for the union.

### Trust page-supplied scope claims by default

Rejected. A hostile or careless page labeling destructive work read-only would lull both model
and auditor. Claims narrow admission only on hosts that policy designates trusted.

### Proxy invocations opaquely

Rejected. An opaque relay bypasses typed decoding, governance, outcome voice, and the one
completion path. Where a fringe already relays bytes, it does so without meaning.

### Name the tools `browser_webmcp_*`

Rejected. The name would fossilize one moving specification inside the authority vocabulary.
Mechanisms belong in descriptions and data, per ADR-0111.

### Design nothing until the API stabilizes

Rejected. ADR-0043 held that line and named its triggers, and they fired. Recording the
destination now costs nothing and is revisable by a newer ADR, while designing during
implementation would spend the integration window on decisions that deserve calm attention
first.
