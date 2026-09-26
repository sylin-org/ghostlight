# ADR-0184: Chromium-Only Browser Offering

Date: 2026-09-26. Status: Accepted.

Supersedes ADR-0183's conditional Firefox return decision. The adapter removal and
historical-evidence preservation in ADR-0183 remain in effect.

## Context

ADR-0183 withdrew a partial Firefox adapter because it could not execute Ghostlight's
normal page work in the user's ordinary browser session. It described a parity gate
for a possible return. That still made Firefox sound like a paused product channel.

The owner has now chosen a simpler offering: Ghostlight is a Chromium browser product.
Current product contracts and release guidance should describe what people can use,
without a prospective Firefox adapter or return promise.

## Decision

1. Chrome, Edge, Brave, and Chromium are the browser offering. There is no Firefox
   adapter, release channel, candidate, or roadmap commitment in the active product.
2. Remove Firefox references from the current greenfield contract and public product,
   trust, comparison, and release guidance. State the supported browser set directly.
3. Keep the narrow migration cleanup that removes an old Firefox native-host
   registration only when Ghostlight ownership is proven. It serves existing installs
   and is not a supported adapter path.
4. Preserve historical ADRs, research, release records, the withdrawn listing copy,
   and Git history as evidence of the earlier work and decision.
5. Any future addition of another browser requires a new product decision and complete
   implementation and installed-product evidence. This ADR makes no such commitment.

## Consequences

- A new user sees one coherent browser capability promise.
- Release work has no dormant second browser channel to stage or publish.
- Existing Ghostlight-owned Firefox native-host registrations can still be removed
  safely during normal installation lifecycle work.
