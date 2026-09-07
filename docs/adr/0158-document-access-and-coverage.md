# ADR-0158: Document-bound access and truthful coverage

- Status: Accepted; implemented and verified locally; not deployed
- Date: 2026-09-07
- Amends: ADR-0138, ADR-0151, ADR-0152, ADR-0131, and ADR-0109
- Builds on: ADR-0101, ADR-0103, ADR-0156, and ADR-0157

## Decision

The owner accepted H6's remaining choices on September 7: embedded host names in human-only
details, mask excluded screenshot regions, stop recordings at the access boundary, and refuse
unrestricted script execution when access to excluded documents cannot be bounded.

1. Existing grants authorize each accessed document for the actual RAWX requirements. The
   immutable authority snapshot resolves three monotonic handling modes: `permitted_content`,
   `complete_operation`, and `complete_page`. The registered setting is
   `content.frames.handling`. The default is `permitted_content`.
2. `content.frames.notice` selects `on_demand`, `when_affected`, or `when_excluded`; the default is
   `when_affected`. Human presentation cannot alter admission or machine-readable coverage.
   The existing monotonic policy layers resolve both choices and explain their source.
3. Browser-observed document identities, URLs, and routing facts precede content extraction.
   A negotiated document-scope mechanism binds collection and effects to those exact documents.
   Frame-id reuse does not preserve a target's authority. Relays remain opaque.
4. Results distinguish policy exclusions, unavailable documents, and output limits. Negative
   observations speak only about inspected content. Absence across unseen content is unproven.
   An unrelated excluded document does not make an admitted target operation incomplete under
   `permitted_content` or `complete_operation`.
5. Model results and durable audit contain bounded coverage metadata, never excluded host names,
   frame identities, URLs, labels, values, or handles. The local human can expand volatile
   coverage details in the existing workbench to see bounded host names and the reason. Restart
   may lose those volatile details; the bounded durable receipt remains truthful.
6. Screenshots mask excluded document regions before capture. The mask is visibly an exclusion,
   not blank website content. Unverifiable identity, geometry, or exclusion refuses capture.
   Target and magnified captures obey the same boundary. Coordinate actions recheck their real
   document subject; a screenshot cannot authorize access to a masked document.
7. Recording stops before retaining imagery beyond its authorized document boundary. Its prior
   permitted frames remain available. Capture availability and policy uncertainty cannot be
   represented as continued coverage. The extension applies a physical document constraint;
   policy remains in the orchestrator.
8. Unrestricted page scripts are refused when their document access cannot be safely bounded.
   No source scanner, script wrapper, or new sandbox is treated as confinement. All-open script
   behavior remains first-class.
9. Known ineligible targets in a multi-target effect are refused before the first effect.
   Later navigation or browser failures preserve partial effects and uncertainty. No rollback,
   website network filtering, automatic policy override, or host-containment promise is added.

## Implementation detail and limits

Restricted recordings bind to the admitted document set. A document change stops retention even
when a new invocation could permit the new document; the adapter cannot grant authority. Prior
frames remain available, subject to current source authority when exported. Every delivery
destination checks the recorded embedded sources. A full source buffer marks provenance incomplete
instead of dropping earlier source evidence. The unrestricted path may continue across navigation;
incomplete source evidence cannot later be disclosed under a host restriction.

Coverage counts retain observed maxima across physical preparation/execution without counting a
repeated read twice. Composition children each retain coverage, while volatile human details group
encountered exclusions under their invocation. See the [verification record](../tasks/security-hardening/h6-verification.md)
for actual browser, process, policy, privacy, capture, and deployment evidence.

## Evidence required

Policy resolution and mixed RAWX grants; denied content excluded before extraction; stale and
navigating documents; scoped negative results; all three handling and notice modes; human-only
host details; masked viewport, full-page, target, and region images; recording boundary behavior;
script refusal; audit minimization; and old-adapter refusal. Use real Sylin demo content for
browser journeys and controlled origins for independently authored host rules. Record native
installation, process-fixture, browser, and platform evidence separately in the H6 task.

Chrome's document identity changes across navigation while a frame id may remain the same.
Document-targeted messaging supplies the required binding:
[webNavigation](https://developer.chrome.com/docs/extensions/reference/api/webNavigation),
[tabs.sendMessage](https://developer.chrome.com/docs/extensions/reference/api/tabs#method-sendMessage).
