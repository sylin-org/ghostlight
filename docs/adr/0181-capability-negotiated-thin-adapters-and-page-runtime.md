# ADR-0181: Capability-Negotiated Thin Adapters and an Acknowledged Page Runtime

Date: 2026-09-25. Status: Accepted; Decision 6 superseded by ADR-0183.

Supersedes ADR-0178 Decisions 1 and 2 and amends ADR-0179 Decision 2. Builds on ADR-0005,
ADR-0093, ADR-0101, ADR-0113, ADR-0114, and ADR-0138.

## Context

ADR-0178 selected raw WebDriver BiDi as an internal Rosetta Stone and proposed a BiDi-to-CDP
translation layer. The live product never constructed the raw BiDi or CDP bridge commands. It
continued to use the smaller closed `BrowserCommand` vocabulary. Keeping both shapes created a
second ungoverned route without delivering a capability.

The service also sent the embedded page code as an uncorrelated, fixed-id preload request. It
published adapter readiness first, discarded send errors, and never waited for a receipt. Browser
work could therefore race the page runtime it required.

Firefox exposed a larger problem. Its hello advertised document scope, capture, script, and
presentation revisions that its command dispatcher did not implement. Its document-scope handler
invented one top document and reported every allowed document as visited without enforcing that
scope. Its preload handler swallowed registration errors and acknowledged success. Capability
negotiation cannot protect the product when declarations are separate from executable handlers.

Firefox Manifest V3 dynamic registration accepts packaged JavaScript file paths. It does not
provide Chromium's debugger-bound route for installing arbitrary service-supplied source into the
interactive browser session. Ghostlight must report this physical difference rather than hide it
behind a platform abstraction.

## Decision

1. The browser bridge keeps one closed Ghostlight mechanism vocabulary. Raw BiDi and raw CDP are
   not product contracts. An adapter may use a vendor protocol privately to execute a closed
   mechanism, but that protocol never crosses the orchestrator boundary.

2. Each adapter owns one closed executable command table. Its advertised capability revisions are
   derived from that table and its passive frame handlers. A command absent from the table is not
   dispatchable and cannot contribute a capability declaration. A capability revision requires
   production handlers and conformance coverage for the complete revision.

3. Page behavior shared across browser families is a service-owned `PageRuntimeBundle`. The
   process builds it once and reuses its revision, SHA-256 identity, and source for every capable
   adapter. `InstallPageRuntime` and `PageRuntimeInstalled` are ordinary correlated browser
   mechanisms. The adapter verifies the checksum before any browser effect and echoes the exact
   revision and hash in its receipt.

4. A runtime-capable connection is registered privately while it initializes. It is not returned
   by browser discovery and does not emit `adapter_attached` until the exact runtime receipt
   arrives. Failure, timeout, disconnect, or a different receipt retires that connection. An
   adapter without the `page_runtime` capability remains usable only for the independent physical
   capabilities it truthfully advertises.

5. Chromium installs the runtime through its existing extension-owned debugger session. It adds
   the source for future documents and evaluates it in the root page plus every flat
   out-of-process iframe session before acknowledging. A newly attached child is paused only until
   that exact runtime is installed. The runtime has a source fingerprint guard so reconnecting the
   same bundle does not duplicate page-local state. Chromium 125 is the minimum supported version
   because that is the first version with flat child debugger sessions for extensions.

6. Firefox advertises only its complete shell-level handler families: tabs, navigation revision
   1, window geometry, liveness, and attention. Fake document inventory, fake scope coverage,
   screenshot, raw script, presentation, atomic open, and preload handlers are removed. Firefox
   can gain another capability only when its interactive-session adapter executes that complete
   mechanism family and passes the same conformance rule.

7. `BrowserPlatform` remains bounded identity and presentation information. Product dispatch is
   selected by capability and revision, never by a platform strategy branch in the executor.

8. The process topology does not change. The browser connector remains an opaque relay. No plugin
   kernel, generic event bus, protocol translation service, or runtime package manager is added.

The adapter protocol major becomes 3 because the raw commands are removed and runtime installation
has acknowledged semantics.

## Consequences

- Browser readiness now proves page-runtime installation when the adapter claims that mechanism.
- The page runtime changes with the service while Chromium's store adapter remains stable unless a
  privileged browser mechanism or browser-specific correctness rule changes.
- Firefox remains useful for the smaller set it truly executes. It no longer appears to support
  governed page observation or capture when it cannot prove document coverage.
- A new adapter starts with no capabilities and earns each family by adding real handlers and
  tests. Capability drift becomes a failing seam rather than a documentation problem.
- The unused BiDi/CDP bridge module, variants, outcomes, and Chromium raw-CDP escape hatch are
  removed.
