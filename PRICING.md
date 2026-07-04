# Ghostlight pricing

The short version: the automation engine is open source (Apache-2.0 OR MIT) and free
forever, for everyone. The governance module is free for individuals, small teams, and
all evaluation; organizations that run it in production buy a license. Nothing is ever
gated at runtime: a license is a legal and visibility artifact, not a switch.

## The founding program (open now: 10 slots)

Ghostlight is new, and the first ten organizations get the best deal it will ever offer:

- **12 months of enterprise-equivalent licensing, free.**
- After the free year, renewal at **50% of the then-current list price, permanently**.
- In exchange: reply to a short emailed questionnaire (5-8 topics) once a quarter, and
  give one reference (a named case study if your policies allow it; an
  anonymized-but-quotable reference is fine too). No calls, no meetings.

Apply: email **hello@sylin.org** with the subject "Founding org", a sentence about your
use case, and rough seat count. There is a one-page agreement; no procurement gymnastics.

## Tiers

| Tier | Price | Who it is for |
|---|---|---|
| Development | Free (self-signed) | Evaluation and development anywhere, any org size. Never production. |
| Community | Free (self-serve key) | Production use in organizations of 5 or fewer people. |
| Founding | Free 12 months, then 50% of list forever | The first 10 organizations. See above. |
| Team | ~$12 per user/month, billed annually, 5-seat minimum | Central policy, SIEM audit, email support (2-business-day response). |
| Enterprise | From ~$10k/year | Everything in Team, plus procurement paperwork (security questionnaires, MSA, DPA), 1-business-day support, deployment help, and roadmap input. |

Billing is annual only. Every customer is grandfathered at their signup price for as
long as they hold a continuous license. Individuals never need a license for anything.

## The Continuity Promise

> Ghostlight never phones home and license state never affects behavior. Enforcement,
> audit, and your production workflows are never interrupted, degraded, or disabled by
> license expiry, by the vendor's unavailability, or by the vendor ceasing to exist. An
> expired license changes exactly one thing: license-state notices appear in
> `ghostlight doctor`, `ghostlight license status`, and your own audit records until it
> is renewed. Your deployment works forever, offline, as-is.

This is a permanent, binding commitment (ADR-0028 Decision 6), not marketing copy. It is
quoted in every renewal email we send.

## How licensing works

A license is a small signed JSON file that the binary verifies offline (Ed25519). There
is no activation server, no telemetry, no network traffic of any kind in the license
path, ever. License state is purely observational: with an abnormal state (expired,
missing where org policy is present), the binary keeps working exactly as before and
appends a `license` field to your own audit records so your compliance process sees it.

License key verification ships in an upcoming release. Today, nothing checks anything:
every feature described in the documentation works without a key, and evaluation
requires no contact with us at all. The commercial terms in
[LICENSING.md](LICENSING.md) apply regardless of tooling.

## Questions

**What happens when a license expires?** Nothing stops. See the Continuity Promise.
Your audit records carry an `"license": "expired"` marker until renewal; that is the
entire consequence.

**Do we need a license to evaluate?** No. Evaluation, development, testing, and all-open
operation are free at any organization size, with no key and no registration.

**Why is the governance module source-available instead of closed?** Because it is the
code that enforces your policy and writes your audit trail. Security teams should be
able to read it before trusting it. The engine is plain open source; the governance
module's source is public under a commercial license (see
[LICENSING.md](LICENSING.md)).

**Will prices go up?** List prices may change with notice; your price is locked at
signup for as long as you hold a continuous license. Founding organizations keep their
50% discount permanently.

**Who do we talk to?** hello@sylin.org for everything: founding applications, quotes,
procurement paperwork, and questions.
