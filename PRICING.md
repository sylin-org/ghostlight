# Ghostlight pricing

**You need a paid license for exactly one thing: an organization of more than five
people using the governance features (policy manifests, org policy, audit) for real
work.** Everything else is free:

- Individuals and solo businesses: free, including operational use.
- Evaluation, development, and testing: free, for any organization, at any size.
- All-open operation (no policy configured): free, for any organization, at any size.
- Nonprofits and open-source projects: free.
- Teams of up to 5 people: free, including operational governance use.

"Operational use" means your organization relies on it for real work -- what most people
call production; internal tools count. The engine itself (everything outside the
governance module) is open source, Apache-2.0 OR MIT, with no conditions at all.

## Tiers

| Tier | Price | What you get |
|---|---|---|
| Evaluation | Free (self-signed key) | Try, build, and test everything, at any org size. |
| Community | Free (self-serve key) | Operational governance use for teams of up to 5. |
| Founding | Free for 12 months, then 50% of list permanently | Enterprise terms for the first 10 organizations. See below. |
| Team | ~$12 per user/month, billed annually, 5-seat minimum | Central policy, SIEM audit, email support (3-business-day acknowledgment). |
| Enterprise | From ~$10k/year | Everything in Team, plus security questionnaires, MSA, DPA, 2-business-day acknowledgment, deployment help, and roadmap input. |

Every procurement document behind these tiers is published openly in the
[trust center](docs/trust/README.md): the security questionnaire, the evidence-linked FAQ,
the MSA and DPA templates, and the [support policy](docs/trust/support-policy.md) that
defines the acknowledgment commitments. Review first, talk to us after.

Billing is annual. Your price is locked at signup for as long as you hold a continuous
license. Two standing accommodations:

- If paying would genuinely be hard -- an early-stage startup, a classroom,
  public-interest work -- email us and we will work something out, including free.
- Outgrew Community mid-year? Finish the year free and buy when you renew. Nothing is
  owed retroactively.

## The Continuity Promise

> Ghostlight never phones home and license state never affects behavior. Enforcement,
> audit, and your workflows are never interrupted, degraded, or disabled by license
> terms, by the vendor being unreachable, or by the vendor ceasing to exist. The 1.0 runtime
> contains no activation, license check, behavior gate, or license marker. Your deployment works
> as-is, offline, indefinitely.

This commitment is recorded in
[ADR-0028](docs/adr/0028-tripwire-licensing-and-continuity-promise.md) (Decision 6) and
quoted in every renewal email.

## The founding program (open now: 10 slots)

The first ten organizations get:

- 12 months of enterprise-equivalent licensing, free.
- Renewal at 50% of the then-current list price, permanently.
- In exchange: a short emailed questionnaire (5-8 topics) once a quarter, and one
  reference -- a named case study if your policies allow it, anonymized-but-quotable
  otherwise. No calls, no meetings.

Apply: email **hello@sylin.org** with the subject "Founding org", a sentence about your
use case, and a rough seat count. The agreement is one page.

## How licensing works

Licensing is contractual, not an activation mechanism. The 1.0 runtime has no key file, activation
server, license-status command, telemetry, network traffic, or audit marker. Every documented
feature works without entering a key; the terms in [LICENSING.md](LICENSING.md) define permitted
use. Commercial customers receive the applicable agreement through the ordinary procurement
process.

## Questions

**What happens when a commercial term expires?** Nothing technical stops or changes; see the
Continuity Promise. Renewal is a contractual and procurement matter.

**Do we need a license to evaluate?** No. Evaluation, development, testing, and all-open
operation are free at any organization size, with no key and no registration.

**What exactly counts as "operational use"?** Your organization relies on the policy for
real work: it is enforcing (or observing) on tasks people actually depend on. Internal
tools count. A pilot your team is still deciding on does not.

**Why is the governance module source-available instead of closed?** It enforces your
policy and writes your audit trail, so security teams should be able to read it before
trusting it. The engine is plain open source; the governance module's source is public
under a commercial license (see [LICENSING.md](LICENSING.md)).

**Will prices go up?** List prices may change with notice; your price is locked at
signup for as long as you hold a continuous license. Founding organizations keep the 50%
discount permanently.

**Who do we talk to?** Use hello@sylin.org for founding applications, quotes, and procurement
paperwork. Licensed support requests go to support@sylin.org; suspected vulnerabilities follow the
private channel and subject line in [SECURITY.md](SECURITY.md).
