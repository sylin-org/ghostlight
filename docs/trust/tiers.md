# Ghostlight Tiers: Claims to Evidence

This page maps each claim on the [pricing page](../../PRICING.md) to the tier that carries
it, the feature that delivers it, and the evidence behind it.

| Pricing-page claim | Tier | Shipped feature | Evidence |
| --- | --- | --- | --- |
| Central policy | Team and above | `managed://` signed central policy, provisioned by your MDM | [ADR-0055](../adr/0055-managed-scheme-central-policy-distribution.md), [governance configuration guide](../guides/governance-configuration.md) |
| SIEM audit | Team and above | Identity-bound audit to syslog (RFC 5424 over UDP) or JSON Lines files, with `policy_seq` provenance; HTTP delivery is deferred | [SIEM integration guide](../guides/siem-integration.md) |
| Email support | Team and above | support@sylin.org, acknowledged within 3 business days (Team) or 2 (Enterprise) | [support-policy.md](support-policy.md) |
| Security questionnaires | Enterprise | The published CAIQ-shaped self-assessment, the evidence-linked FAQ, and one completed questionnaire per year on request | [questionnaire.md](questionnaire.md), [faq.md](faq.md) |
| MSA | Enterprise | Master software agreement template (draft, pending counsel) | [msa.md](msa.md) |
| DPA | Enterprise | No-processing data processing addendum template (draft, pending counsel) | [dpa.md](dpa.md) |
| Deployment help and roadmap input | Enterprise | Enterprise extras | [support-policy.md](support-policy.md) |

Seat and licensee counts are contractual terms, never enforced at runtime: Ghostlight never
phones home, never counts seats, and license state never changes behavior (ADR-0028).

Last reviewed: 2026-07-10 against v0.7.3 | Contact: support@sylin.org
