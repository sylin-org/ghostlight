# ADR-0196: Simple pipeline and contained boundaries

Decision 7 superseded by ADR-0197 Decision 5 (the break is strict, with no transition level).

Date: 2026-10-13. Status: Accepted by owner direction.

Supersedes ADR-0175 (workspace switching), the shared connector release version in ADR-0142,
and ADR-0193 Decision 3 where it replaces connectors on ordinary updates. Amends ADR-0181
Decisions 3, 5 and 7, ADR-0137 Decision 2, ADR-0172 Decision 3, ADR-0113 Decision 3, and
ADR-0114's waiting rule.

## Context

The owner's direction is simple. Commands from any source go in. Ghostlight checks whether
they may run. Then they run. The adapters at the edges are dumb and stable. The service can
be stopped and updated at any time, and nothing breaks.

An assessment on 2026-10-13 found the core of that pipeline intact: one executor, one
governance check, one browser port, one completion path. The boundaries around it leak:

- The extension service worker is 2,527 lines. It sequences fills, merges frames, adopts
  tabs and verifies results. Any feature change touches it, and an extension change needs a
  store release.
- Page code runs in the page's own JavaScript world. A page can replace Ghostlight's
  dispatcher and forge reads, credential checks and click coordinates.
- Page assets are split between `extension/lib` and the service.
- Every executable embeds the product version, so every release changes all three and every
  update stops every connector. The npm launcher deploys on any command, including `doctor`,
  and can downgrade.
- `browser_workspace switch` lets one session take over another session's tabs.
- A short deadline can quarantine a healthy browser connection. Four different situations
  are reported as "the browser disconnected".
- Several model-facing defects passed a green gate, because nothing tests what the model
  sees: invalid schemas, recovery hints that do not decode, and a summary that claims a tree
  the result does not contain.

## Decision

1. **One pipeline.** Every command, from MCP or the CLI, is decoded, checked against
   configured authority, run through one browser port, and answered with one result. No step
   outside that path changes what a session acts on.

2. **Four components, one job each.**
   - MCP connector: proxies MCP to the service.
   - Browser connector: relays native messaging to the service. It never parses adapter
     frames.
   - Service: every product decision, including the browser adapter below.
   - Extension: the browser's native side.

3. **The browser adapter is a service component.** There is one per browser family, Chromium
   today, under `crates/orchestrator/src/browser/adapter/`. It owns every asset that runs in
   a page, JavaScript and CSS, and the translation from abstract work to browser primitives.
   The extension's hello names its family, engine version and protocol. The adapter picks the
   matching asset set and wire level. Assets ship with the service, so a service update ships
   new page code without an extension update.

4. **Injected code runs in an isolated world.** Under adapter protocol 4 the extension
   installs the asset set with `Page.addScriptToEvaluateOnNewDocument` and the world name
   `ghostlight`, on the tab session and on every out-of-process frame session. It creates the
   world in existing documents with `Page.createIsolatedWorld`, removes superseded
   registrations, and calls into the world with `Runtime.callFunctionOn`. The page cannot
   see, call or replace that code. Every call answers `{ok:true, value}` or
   `{ok:false, error:{code, message}}` from a closed code set. `browser_execute` still runs
   the caller's script in the page's main world of the top frame, because that is its job.

5. **The extension decides how, never whether.** It owns tab groups, windows, focus,
   debugger lifecycle, worker lifecycle, trusted input, capture, dialogs, the diagnostics and
   recording buffers, and its own popup, options and setup pages. It does not decide whether
   Ghostlight may touch a tab. The service chooses and sends exact tab ids. The preserve-tabs
   setting stays as the person's local veto, like Pause.

6. **A small frozen primitive set.** Adapter protocol 4 offers: list, open, navigate, focus
   and close tabs; zoom; window size; frame inventory; install assets; call in the isolated
   world; evaluate in the main world; a trusted input batch; capture; dialogs; diagnostics;
   recording; a passive notice; cancel. Cross-frame merges, fill sequencing, retention
   checks, observation, settling and presentation routing move to the service adapter.
   Page-local loops stay inside one call, so a fill or a drag costs a few round trips, not
   one per keystroke.

7. **Old adapters keep working.** The adapter serves protocol 3 extensions through today's
   command set, unchanged. This legacy level stays until the published store adapter speaks
   protocol 4, and removing it later is its own decision. A service update never requires an
   extension update.

8. **Sessions are contained.** A session works only in its own workspace. `browser_workspace`
   is removed. A refusal for another session's handle points to the session's own tabs.

9. **Default reuse never takes a person's tab.** `reuse:"domain"` adopts only a tab
   Ghostlight opened and later released, in the same browser, with no page commit since the
   release. The service makes that choice and sends the exact tab id. The extension never
   picks a tab to adopt.

10. **Connectors are versioned by protocol, not by release.** They do not embed the product
    version. They decode only the envelope and pass the rest through, and they ignore unknown
    fields and message kinds. They change only when their protocol changes. While the
    service restarts, the MCP connector holds calls for a bounded window and answers
    `tools/list` from the last catalog it received.

11. **Updates replace the service, never downgrade, and never run from a status command.**
    The launcher runs the installed service when it is the same version or newer. It installs
    or updates only when the service is absent or older. `doctor`, `status`, `help` and
    `--version` never deploy. Installation refuses an older version. Unchanged connectors
    keep running through an update.

12. **Browser availability waits, then tells the truth.** A call waits a bounded grace
    period for the browser its session uses when there is evidence the browser is returning:
    it is reconnecting, still initializing, or the service has just started. One short
    deadline never quarantines a connection or fails other calls. Refusals say what is true:
    cancelled, not answering, not connected, busy, or a different browser profile.

13. **The guarantees are tested, not remembered.** CI fails when:
    - `extension/` or connector code changes without a protocol revision change;
    - the serialized wire shapes change without a protocol revision change;
    - a catalog example, or a next-step hint that names a tool and its fields, does not
      decode;
    - an advertised input schema rejects a known-good call or matches it ambiguously;
    - the current service cannot complete the process journey with the last published
      connectors.

## Migration order

Each step leaves a green tree and a working installation.

1. Service-only corrections that work with the published adapter: model-facing language,
   catalog schemas and self-checks, session containment, service-chosen tab reuse, and
   browser availability.
2. The adapter component: page assets move out of `extension/lib`, family and level are
   chosen at the handshake, and the legacy level carries today's protocol.
3. Connector freeze and update rules.
4. Adapter protocol 4: the isolated-world extension primitives and the Chromium translation in
   the service.
5. The CI gates in Decision 13, and the documentation that still describes code dev does not
   contain.

## Consequences

- The extension shrinks to about 3,000 lines. It changes when Chrome changes or a new
  primitive is needed, not when a feature changes.
- An isolated world cannot see JavaScript properties a page defines on its own elements.
  Attribute and ARIA fallbacks cover the cases Ghostlight uses.
- Diagnostics and recording buffers stay in the browser (ADR-0109), so URL sanitizing still
  happens at the source.
- Store submission text must say that the extension runs page code supplied by the locally
  installed Ghostlight service, in an isolated world, through the debugger. This ADR does not
  decide a store submission.
- The browser journeys that start their own Ghostlight authority stay in CI. Local acceptance
  uses the installation in place and the person's own browser.

## Appendix A: work dropped by merge 86952a8a

The 2026-10-05 merge kept 29 trial commits in history but not in the tree. Their disposition:

| Work | Commits | Disposition |
| --- | --- | --- |
| Hello-first native startup, single shared native connect | 7dd12972, 07b544a5 | Folded into the protocol 4 extension |
| Relay drain on write failure, reconnect spacing | d372e18e, part of 8c8d856c | Relanded with the connector freeze |
| Inventory excludes prerender, cached and deleted pages (ADR-0189) | 1485b8a4 | Superseded: the protocol 4 frame inventory comes from CDP, where those pages are separate targets |
| Pause and Resume while no browser is connected (ADR-0187 Decision 4) | 8d1e16ff, 4ecbe13e | Relanded in the service |
| Rejected injection results read as success | 22d27bd1 | Superseded by the Decision 4 envelope |
| Page runtime calling extension messaging from the main world (ADR-0188) | 7af79ed4 | Superseded by the isolated world |
| Cancel retires page work, generation-fenced leases (ADR-0190, 0191, 0192) | fcb4bce6, d936b708, f7f00f07, f2e42368 | Superseded. Requirement kept: cancel stops the service issuing steps, and page loops stop on a cancel call |
| Quiet coexistence, Show tab, Linux focus work (ADR-0186) | 8c8d856c, d66efa36, f3fedbb9 | Left archived |
| Capture-mask experiments | df27b57b, 1468b85b, ff223cba | Superseded by ADR-0194 |
