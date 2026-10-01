# ADR-0188: Page-world-independent local form diagnostics

Date: 2026-10-01. Status: Accepted for the unpublished local improvement trial.

Amends ADR-0145's structural form tracing amendment. Preserves ADR-0181's service-owned
page runtime, closed browser mechanisms, and adapter-local physical responsibilities.

## Context

The service installs its page runtime in Chromium's MAIN world. The form observer still used
extension-context `chrome.runtime.sendMessage` for startup and row delivery, while the worker
sent state through `tabs.sendMessage`. There is no content-script message receiver in that
world. Other installed extensions can also expose webpage messaging APIs there. Calling those
APIs without an extension ID can throw before Ghostlight installs its dispatcher. A Node VM
regression reproduces this initialization failure using the actual runtime source.

An earlier installed localhost read failed with `document_unavailable`. After Chrome restarted
and the existing adapter was reloaded, a fresh fixture had an installed dispatcher, no exposed
page messaging API, and successful read/fill. The source defect is confirmed; it is not proven
to be the cause of that earlier live failure.

## Decision

1. The service-owned page runtime never calls extension messaging APIs. Its form observer starts
   disabled. The adapter pushes the existing closed diagnostic state through the ordinary content
   primitive after determining the existing ownership and human diagnostic preference.
2. A small isolated-world relay carries only the named structural diagnostic row event to the
   extension worker. It is idempotent, bounded, exception-isolated, and separate from browser work.
   It supplies no policy, task intent, ownership, or product decision.
3. The runtime, relay, and worker project the closed metadata. The worker uses Chromium's sender
   tab/document identity, rejects unauthorized senders, and retains the existing bounded local
   ring. Page fields cannot supply that identity. No row reaches native IPC, governance audit,
   action history, or a model-facing result.
4. Disabling diagnostics or releasing a controlled tab stops its observer. The isolated receiver
   preserves wake-on-row delivery and the existing local export while the service is disconnected.
   A browser-session key retains at most 256 exact traced tab/document identities through worker
   suspension. Custody must persist before enabling observation; a storage failure leaves it off.
   Physical synchronization is serialized per tab. A failed or malformed stop retains its retry
   identity; cleanup requires the exact document's disabled acknowledgement. Unrelated tabs cannot
   consume cleanup capacity.
   No external webpage messaging grant, generic event bus, new browser protocol capability,
   automatic tab adoption, or page reload is introduced.

## Consequences

Ordinary page-world API differences cannot abort automation startup. Structural diagnostics use
the browser context that actually owns extension messaging, rather than silently depending on a
retired content-script route. Existing opt-in, privacy, retention, and failure-isolation promises
remain in force. The local adapter changes and requires reload after deployment.
