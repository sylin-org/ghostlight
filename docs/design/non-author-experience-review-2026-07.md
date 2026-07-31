# Retrospective non-author experience review, 2026-07

Status: Directional qualitative evidence. Not a formal usability study.

## Method and limits

This document reconstructs one pre-release review from the owner's recollection of a video call
with another developer. There is no recording, transcript, or contemporaneous note set. The
participant knew the product was pre-release. The owner pointed her to the existing browser
extension installation instructions, then she completed setup and use on her own.

The observations are useful because they came from a person who did not author the product, but
they are not representative evidence. No demographic, timing, error-rate, or task-completion claim
should be inferred. Recalled quotations below are approximate unless the owner later confirms
otherwise. Do not publish the participant's quotations or identity without her consent.

## What worked

### The product became useful quickly after installation

The participant found the pre-store extension path straightforward, although predictably
cumbersome.
Once both halves were installed, Kline recognized Ghostlight quickly. Her comparison, "It's just
like Claude Cowork!", was an immediate recognition moment: Ghostlight made a familiar browser-agent
experience available in another MCP client without requiring a Ghostlight account.

That is the strongest product signal in this review. Pre-install communication caused friction;
post-install connection speed, visible operation, and usefulness did not.

### Visible action feedback earned trust and delight

The content-read scan, its gentle blue glow, and the fluid action effects were specifically liked.
The blue tint was described as "Better than Claude's." The feedback made agent intent legible
without feeling mechanical. A camera glyph after a screenshot was suggested as a small extension
of the same idea.

### The mascot invited experimentation

The participant said the mascot made her want to try the product because it was cute. This is more
than decoration: it can make first contact feel approachable. The mascot should invite and orient;
it should not become the symbol of governance authority.

## Where the experience broke cohesion

### Installation was discoverable only after the owner supplied the right starting point

The participant could not find the pre-store extension instructions from the main product page.
The process was understandable once found, but the journey crossed surfaces whose visual identity
differed enough that she wondered whether they belonged to different sites.

This is assisted discovery followed by unassisted execution, not a fully unassisted install win.
The product page should show the complete journey and link directly to the supported extension
listing.

### Narration looked and behaved heavier than its meaning

The action effects felt fluid, but the narration ribbon felt bolted on. Its progress line drew
attention and created anxiety by implying a countdown that mattered. The participant's proposed
signal was simpler: "Three vanishing dots would be enough" to communicate that narration is
temporary.

The issue is not merely styling. A progress line carries duration semantics. Narration only needs
to communicate presence and ephemerality. Its motion should be a one-shot entrance or quiet
vanishing beat, not a continuously depleting meter.

### The denial ribbon looked modal without being modal

The participant could not tell whether subsequent actions remained blocked until she closed the
denial ribbon. Today, one denied call is denied; the next mutating call can proceed and also clears
the ribbon. The ribbon itself spans the center of the viewport, persists, and obscures content.
The presentation therefore suggests a stronger enforcement state than the service actually holds.

She wanted a clear but more organic signal: a shield, a responsive hover state, and some whimsy.
She also asked about dismissing one notice, suppressing repeats for the session, or suppressing
repeats for that site during the session. Those are valid attention controls, but none may grant a
capability or override a denial.

The resulting design hypothesis is two-stage:

1. An ordinary denial produces a non-blocking, centered sticker for about three seconds. A newer
   denial replaces it. The sticker has a clear title, supporting detail, and an icon for the kind
   of boundary that held.
2. A burst of denials in a bounded window places that MCP session into a real service-side pause.
   Only then does Ghostlight show a blocking overlay with recovery controls.

The threshold, scope, and recovery semantics require an ADR and live evaluation. The prior-art
study in [research 16](../research/16-denial-burst-circuit-breaker.md) proposes a safe shape but
does not authorize implementation.

## Questions the review exposed

### Why is there no Ghostlight sign-in?

Ghostlight is local software, not a hosted account service. The service admits local relays through
same-user IPC and pins allowed extension origins; websites continue to use the user's existing
browser profile. This is authentication and ownership without a Ghostlight cloud identity.

The participant correctly observed that a fully compromised user machine already compromises the
browser. There is still a narrower threat between "healthy machine" and "fully compromised
machine": a malicious or runaway MCP client can ask Ghostlight to do harmful things. Users should
connect only MCP clients they trust. Governance constrains that narrower use, but no extra sign-in
would repair a compromised local service or browser.

This explanation belongs in onboarding. Otherwise the absence of a login can look like missing
security rather than an intentional local architecture.

### What should recording show?

The participant proposed a red-dot `REC` treatment and a cute picture-in-picture square showing
what is being recorded. Ghostlight currently records one tab's rendered viewport in memory. A live
indicator is a sound expectation. A literal in-page mirror is not: it can recursively capture
itself, duplicate sensitive pixels, and obscure the page.

A better candidate is a tab-scoped extension badge plus a small, stylized viewfinder card saying
"Recording this tab" and "Local memory." Whether any in-page treatment appears in the resulting
recording must be decided explicitly. The visual proposal records this as a design question, not a
shipped promise.

### Can the screenshot feel photographic?

Yes. The existing shutter/frame effect runs after capture, so it is visible to the user but absent
from the model's screenshot. Adding a small camera glyph to that after-capture treatment preserves
the current trust invariant and adds the requested delight.

## Public-language findings

The README led too strongly with organizations and licensing. The participant's distinction was
plain: developers read repository READMEs; organizational buyers look at product and procurement
pages. Seeing licensing near the entrance made her worry that another nominally free service would
later become paid.

The repository entrance should therefore:

- lead with the practitioner problem, the local visible experience, and the shortest useful path;
- state the browser automation core's Apache-2.0 OR MIT license and the absence of an account,
  activation, telemetry, or subscription without vague "free" promises;
- explain the separately licensed organizational governance layer later, with a direct link to
  exact terms; and
- leave procurement evidence to the product site and Trust Center.

This is an ordering change, not permission to blur the license boundary. Exact public wording must
remain consistent with ADR-0027 and the live license files.

The participant also looked for a way to support the project. GitHub Sponsors is the natural
repository-native option; Ko-fi could be a secondary link. Neither should unlock features or be
published before the recipient, entity, accounting, and tax handling are decided. Public wording
should say "support" or "sponsorship," not imply a charitable donation.

### OpenCode was named as a developer-friendly reference

The participant later named anomalyco/opencode as an example of repository content that feels
developer-friendly. Its current README, checked on 2026-07-14, uses a terse product sentence and
visual proof, then puts installation immediately in the main flow. It explains working modes,
documentation, and contribution after the reader can already see how to start. Source:
https://github.com/anomalyco/opencode

The useful pattern is fast repository orientation, not its exact layout or tone. Ghostlight has a
two-part install and a more important local trust boundary, so hiding those details to match a
one-command product would be misleading. The actionable adaptation is an immediate install anchor,
the visible four-stage journey, one read-only proof task, and practitioner capabilities before
organization governance. Procurement and licensing depth stay available later rather than
disappearing.

## Product conclusions

1. Protect the post-install strength. Recognition, connection speed, and visible action feedback
   are already product advantages.
2. Treat install discovery and cross-surface identity as the largest current user gap.
3. Make semantic weight match visual weight. Narration is light, an ordinary denial is brief, and
   a blocking overlay means the service is actually paused.
4. Explain the local trust model early. "No account" is a benefit only when the alternative
   identity and threat boundary are legible.
5. Lead repository readers as practitioners. Put organizational depth where organizational
   readers actually look.
6. Preserve whimsy in confirmation and invitation. Keep authority calm, precise, and unmistakable.
7. Keep repository orientation fast: show the product, make installation findable immediately,
   then explain deeper modes and organizational concerns.

## Follow-up evidence

The next non-author review should be observed and noted with consent. It should start from the main
product or repository page, not from a supplied deep link, and record at least:

- whether the extension step is found without help;
- whether each surface is recognized as Ghostlight;
- whether "no account" is understood correctly;
- what the user believes an ordinary denial and an escalated pause will do; and
- whether recording scope and persistence are clear before capture begins.
