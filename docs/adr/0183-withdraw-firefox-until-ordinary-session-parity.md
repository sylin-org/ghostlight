# ADR-0183: Withdraw Firefox Until Ordinary-Session Parity

Date: 2026-09-26. Status: Accepted.

Supersedes ADR-0179's Firefox product decision and ADR-0181 Decision 6. Builds on ADR-0003,
ADR-0050, ADR-0093, ADR-0138, and the Firefox browser-adapter dossier.

## Context

The Firefox extension proved that a native-messaging shell can provide a small set of tab and
window operations. It could not provide Ghostlight's complete browser workspace in the ordinary
visible Firefox session a person already uses. Page reading, physical input, capture, scripting,
network and console observation, presentation, and complete document-scope enforcement remained
absent.

Calling that smaller surface Firefox support would optimize for a compatibility badge instead of
the user's job. A person choosing Firefox would discover missing capabilities only after install,
then be told to switch browsers for normal work.

Firefox's privileged automation route does not repair that experience. Its Remote Agent is enabled
at browser startup with a remote-debugging command-line flag. Building around it would require a
managed launch or restart, special profile custody, and a second connection and recovery topology.
That contradicts Ghostlight's ordinary-session promise and its no-startup-ritual default.

## Decision

1. Ghostlight's active browser product supports Chromium-family browsers only: Chrome, Edge,
   Brave, and Chromium.

2. Remove the Firefox extension, native-host registration, installer discovery, adapter identity,
   CI lane, release artifact, publisher scripts, and current support claims. Do not submit the
   prepared Firefox package to Mozilla Add-ons.

3. Preserve the Firefox research, historical ADRs, release records, and Git history. They remain
   evidence, not an active product surface.

4. Firefox can return only when one implementation provides every currently advertised Ghostlight
   capability revision in an ordinary visible, authenticated, already-running Firefox session. It
   must not require headless execution, a disposable profile, a managed launch, a browser restart,
   or a user-visible remote-debugging ritual.

5. A returning adapter must pass the same handler-derived capability checks, process journeys,
   installed-product recovery journeys, and full visible-browser acceptance suite as Chromium.
   Partial capability branding is not an acceptable intermediate release.

6. The browser connector remains an engine-neutral opaque relay. The current `BrowserPlatform`
   vocabulary is narrowed to Chromium because no other active adapter exists; a future platform
   value is added only with its complete implementation.

## Consequences

- Firefox users are not invited into an experience that stops at the first normal page task.
- Installation, diagnostics, release custody, and support language have one browser capability
  truth instead of parallel full and partial products.
- The active system loses unused Gecko branches and Mozilla-specific registration and publication
  machinery.
- Firefox work resumes from preserved evidence if its platform can satisfy the ordinary-session
  parity gate later.
