# ADR-0079: Denial attention circuit breaker

Date: 2026-07-14
Status: Accepted
Builds on: ADR-0018 (human pause and kill controls), ADR-0030 (multiplexed local sessions),
ADR-0057 (honest governance presentation), and ADR-0072 (agent narration). Preserves
ADR-0005's policy-free extension and ADR-0028's no-phone-home Continuity Promise.

## Context

One denied browser action is ordinary feedback. A persistent full-page ribbon makes that ordinary
event feel more severe than it is and interrupts legitimate work. Repeated denials in a short
window are different. They can indicate a confused loop, a badly prompted client, or an MCP client
that is no longer acting as the user expects. Continuing to dispatch browser work while only
stacking notifications is the wrong failure mode.

The existing take-the-wheel hold cannot represent this state. It is a global user control shared
by every MCP session. A denial burst belongs to the one session producing it and must not stop an
unrelated client. Governance decisions also cannot move into the extension. The extension may show
state and relay a human choice, but the service must count, pause, admit, audit, and resume.

## Decision

### D1. Isolated denials use a transient sticker

An enforced sacred-domain or policy denial produces a centered, non-modal sticker. It has a clear
title, one bounded detail line, and an icon for the denial category. It replaces an older sticker
and disappears after three seconds. It does not obscure or disable the page.

The sticker is presentation of a decision already made by the service. It is not a prompt, grant,
or warning that the page itself can dismiss.

### D2. The service owns one circuit per MCP session

Each live session has an in-memory attention circuit keyed by its opaque session guid. It observes
only enforced denials, never shadow denials, schema errors, transport failures, or dry-run verdicts.
It maintains two rolling counters:

1. matching denials: same session, top origin, capability set, and denial category;
2. all enforced denials in the session.

The circuit opens when either threshold is reached:

- 3 matching denials within 60 seconds; or
- 5 total denials within 120 seconds.

These values are named constants and pinned by deterministic-clock unit tests. They are not hidden
configuration knobs. A later threshold change is a product decision and requires evidence plus a
marked amendment.

The circuit and its history are memory-only. Session teardown removes both. Nothing survives a
service restart.

### D3. An open circuit blocks new browser dispatch for that session

After the transition commits, no new browser request for that session may enter the extension.
The MCP result is an ordinary successful text result that says human attention is required and
names the available recovery action. It is distinct from a policy denial and from the global
take-the-wheel hold so orchestration reports the state honestly.

The browser transport performs the final admission check at its send boundary. A pipeline check
provides the helpful structured outcome early, but it is not the security boundary. Pause
activation and final admission share synchronization so a concurrent call cannot pass a stale
check and enqueue after the circuit opens.

An already-dispatched action is not rolled back or cancelled. The transition prevents later
dispatch; it makes no transaction claim.

### D4. The human gets four explicit dispositions

The page overlay and extension popup offer:

- Keep paused;
- Resume;
- Resume and quiet identical site repeats for the rest of this session;
- End session.

Resume closes the circuit and clears its rolling history. Quieting changes presentation only for
the matching origin and denial category. It never grants a capability, changes a manifest, skips
the sacred-domain check, changes a policy result, or suppresses audit. A later burst may open the
circuit again even when its isolated stickers were quieted.

End session uses the existing panic control. Page controls are a convenience; the popup is the
trusted recovery surface when injection, navigation, or page rendering fails.

### D5. The extension remains mechanism-only

The service sends exact presentation state, including session identity, bounded labels, reason,
and allowed controls. The extension renders it, replays it after navigation, and returns the
selected disposition. It does not calculate thresholds, classify denials, infer origins, decide
which controls are legal, or persist governance state.

Page UI is placed in a closed shadow root, uses `textContent`, ignores page styles, and follows
reduced-motion preferences. The blocking overlay dims and softens the page behind it so the visual
state agrees with the actual interaction state. Agent narration remains visually distinct and can
never impersonate this surface.

### D6. Audit records transitions without page content

The audit destination receives a separate attention-event record for open, resume, quiet, and
session-end transitions. It may contain time, client identity, denial category, capability names,
threshold kind, count, window length, and disposition. It does not contain the opaque session guid
because ADR-0047 keeps that correlation secret off disk. It must not contain page text,
semantic queries, form values, screenshots, full URLs, or denial descriptions.

The governing origin may be represented only through the existing normalized resource rules. No
new content inspection or page-derived hash is introduced.

### D7. Related visual feedback becomes quieter and truthful

Agent narration becomes a compact caption with one transient three-dot activity cue and no
progress bar. A screenshot shows a short camera glyph after capture. Recording shows an honest REC
state tied to the actual screencast lifecycle. V1 uses extension chrome and popup feedback so the
indicator is not recursively captured into the GIF. A literal live picture-in-picture preview is
rejected because it would imply an independent capture view that does not exist.

## Acceptance criteria

1. Unit tests pin both thresholds, rolling-window expiry, independent sessions, quieting semantics,
   resume clearing, and teardown clearing.
2. A final synchronized transport check proves no request is enqueued after an open transition.
3. `script` and `browser_batch` report `attention_required`, not `denied` or `held`.
4. Extension tests prove sticker replacement/expiry, overlay replay, popup fallback, multiple
   paused sessions, disposition relay, and reduced-motion behavior.
5. Audit tests prove transition facts are present and content payloads are absent.
6. The global take-the-wheel hold and panic kill behavior remain unchanged.
7. The 13 trained tool schemas remain byte-stable.

## Consequences

- Normal denials become visible without taking over the user's page.
- A runaway client stops itself at the browser boundary while other clients keep working.
- Recovery remains local, explicit, and available outside the page.
- The service gains bounded per-session memory and a synchronized admission seam.
- Quieting can reduce repetition without weakening governance.

## Rejected alternatives

- Reuse the global take-the-wheel hold. Rejected because one client would pause every session.
- Block on the first denial. Rejected because an expected denial is useful feedback, not evidence
  of a loop.
- Let the extension count denials. Rejected because that moves policy-adjacent state and authority
  out of the service.
- Make the isolated sticker clickable to grant access. Rejected because presentation cannot become
  an authorization path.
- Automatically resume after a timeout. Rejected because the circuit exists to require a human
  decision.

## Provenance

Accepted by the owner after a non-author developer review and an explicit prior-art/design pass on
2026-07-14. The owner approved the per-session design and both numeric thresholds before production
implementation.
