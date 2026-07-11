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

The `ghostlight` binary, the bundled Chromium extension, and the install scripts in this
repository. The reference/ directory is third-party study material and out of scope.

## What to expect from the product

Ghostlight is a local-only tool: it never phones home, carries no telemetry, and
initiates no network traffic beyond the user's own tool calls and configured audit
destinations (ADR-0028 Decision 9). The extension holds no policy logic; enforcement and
audit live in the binary (docs/SPEC.md). License state never changes behavior (ADR-0028
Decision 1).

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

The latest tagged release. Pre-1.0, fixes land on the tip; there are no backport
branches.
