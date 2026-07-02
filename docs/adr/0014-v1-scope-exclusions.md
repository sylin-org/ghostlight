# 0014. v1 scope exclusions

- Status: Accepted
- Date: 2026-07

## Context

A capability manifest plus a governance overlay invites scope sprawl: runtime
identity providers, remotely fetched policy, session multiplexing, content
inspection, cryptographic manifest signing, other browsers, and every tool the
reference or official extension ever shipped. Each addition brings a network
dependency, a failure mode, or a maintenance burden that undercuts the
single-binary, zero-runtime-dependency premise. v1 needs a hard boundary.

## Decision

v1 explicitly excludes (SPEC sec 10):

- Built-in IdP integration (OIDC, SAML, LDAP). Identity resolves at deployment
  time via manifest push, not at runtime; runtime IdP would add a network
  dependency and credential management inside the binary.
- Remote policy service. A manifest fetched over HTTP per call adds a network
  dependency and failure mode; manifest changes propagate via the deployment
  channel (Intune/GPO refresh) instead.
- Multi-user / multi-session multiplexing. One binary, one identity, one
  manifest, one profile, one active session; a second concurrent session is
  rejected cleanly rather than shared over a relay.
- Content inspection / DLP. Governance is structural (which domains, which
  tools), not semantic; content DLP is a separate discipline.
- Manifest signing / attestation. Enterprise uses tamper-resistant
  `chrome.storage.managed`; signing is a v2 enhancement for file-based manifests.
- Cross-browser support. v1 targets Chromium browsers only; Firefox uses a
  different extension and native-messaging model.
- The `upload_image` tool. Deferred as niche; addable later without schema
  changes.

Correction to stale rationale: SPEC sec 3.2 justifies excluding `upload_image`
(and `gif_creator`) as "non-functional stubs in the reference." That rationale
is stale: the official Claude-in-Chrome v1.0.78 implements both fully. The
exclusion of `upload_image` stands, but on scope and niche grounds: it is
deferred as a niche capability, not because it is a stub. (`shortcuts_list`,
`shortcuts_execute`, and `switch_browser` remain genuine reference stubs.)

## Consequences

- Positive: v1 stays a self-contained single binary with no runtime network
  dependency and no credential storage; identity is the deployment channel.
- Positive: the single-active-session rule keeps the process model simple and
  the audit trail unambiguous.
- Negative: shared machines require separate OS/browser profiles; a second
  concurrent agent session is refused, not queued or shared.
- Negative: niche tools and file-manifest tamper-resistance are unavailable
  until a user or org actually needs them.
- Follow-up: manifest signing, dynamic grant refresh, and `upload_image` are v2
  candidates (SPEC sec 11), gated on demand.
