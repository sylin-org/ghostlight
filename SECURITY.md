# Security policy

## Reporting a vulnerability

Email hello@sylin.org with "SECURITY" in the subject. Do not open a public issue for a
suspected vulnerability.

Ghostlight is a solo project, so the timelines below are **best-effort targets, not contractual
guarantees** -- a report that arrives during a quiet maintenance period may take longer, and that
is an honest description rather than a hidden asterisk:

- Acknowledgement: typically within a few days.
- Assessment and severity triage: as soon as the issue is understood, usually within a week or two.
- Confirmed critical issues: prioritized for a fix and a coordinated release.
- You will be credited in the release notes unless you ask not to be.

## Scope

The `ghostlight-mcp-connector` protocol edge, the `ghostlight` orchestrator and desktop workbench,
the browser-only `ghostlight-browser-connector`, the bundled Chromium extension, explicit harness
configuration management, and release packaging in this repository. Historical, research, and
quarantined reference material is out of scope.

## What to expect from the product

Ghostlight is local-first: it never phones home and carries no telemetry. Network traffic is the
browser activity the user requested. Audit and configured authority are local files; the runtime
does not fetch policy or send audit. The extension holds no policy logic, the desktop WebView owns
no product state, and enforcement and audit live in the orchestrator. License state never changes
behavior.

## Enforcement model and residual risk

Governance authorizes each invocation by capability class (`read`, `action`, `write`, or
`execute`) and target host. It constrains where an agent may act and what class of action it may
take. It does not infer whether page instructions match the user's intent. One residual risk
follows directly.

**In-domain prompt injection executes within policy.** When an identity is granted `write` on a
domain (for example, `mail.example.com` for email triage), page content on that domain that
contains injected instructions can drive further writes on the same domain. Those writes are a
permitted capability on a permitted domain and are authorized. This is inherent to capability-based
authorization: tightening a policy does not remove it, because the authorized actions are exactly
those the grant permits.

Controls that reduce this risk:

- **Least privilege.** Grant the narrowest hosts and capabilities a task requires. Use explicit
  deny hosts for sensitive applications and request restrictions for a narrower unit of work.
- **Visibility and interruption.** Every action is rendered in the browser as it happens; an
  operator can pause the session (take-the-wheel) or trigger the kill switch at any point.
- **Audit.** Every terminal invocation produces a payload-free local record of the decision,
  outcome, and effect class without URL or page content.
- **Intent confirmation is a client responsibility.** Determining whether an action matches the
  user's intent requires the user's intent, which resides in the MCP client and the model, not in a
  tool beneath the model. Current security research is consistent that server-side intent inference
  from page content or DOM heuristics is unreliable and evadable by injection. Ghostlight enforces
  capability and destination, provides visibility, and gives the operator direct control.

Transaction-bound confirmation is not part of the 1.0 runtime. A future design must preserve the
same client-owned intent boundary and cannot be claimed until accepted and implemented.

## Disclosures and advisories

There is no bug-bounty program. As a solo-founder project, Ghostlight cannot administer or
fund a bounty; reports are handled through the private channel above and credited in the
release notes.

Because the runtime holds no customer data on the vendor side, the vendor-side incident that
matters is a compromise of what we ship (the build, the signing keys, or the update channel).
For that case the aim is to publish a security advisory, with the affected versions and the
remediation, promptly (typically within a few business days) of confirming a vendor-side
compromise -- again a best-effort target for a solo maintainer, not a contractual SLA. Advisories
are published as GitHub Security Advisories on this repository and named in release notes;
watching the repository's releases is the subscription path. The vendor-side security
posture is documented in docs/trust/security-overview.md.

## Supported versions

The latest tagged release is supported. The unreleased 1.0 source candidate is for development and
release validation; security fixes land on its active branch until 1.0 is tagged. There are no
promised backport branches.
