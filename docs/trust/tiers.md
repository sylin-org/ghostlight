# Ghostlight Tiers: Claims to Evidence

This page maps each claim on the [pricing page](../../PRICING.md) to the tier that carries
it, the feature that delivers it, and the evidence behind it.

| Pricing-page claim | Tier | Shipped feature | Evidence |
| --- | --- | --- | --- |
| Central policy | Team and above | Signed file or HTTPS policy from a customer-owned source, activated by an administrator-provisioned bootstrap | [ADR-0121](../adr/0121-restore-rawx-policy-and-managed-fetch.md), [governance configuration guide](../guides/governance-configuration.md) |
| SIEM audit | Team and above | Content-minimized JSONL with `policy_seq` and denial attribution, collected by the endpoint's existing file agent | [SIEM integration guide](../guides/siem-integration.md) |
| Email support | Team and above | support@sylin.org, acknowledged within 3 business days (Team) or 2 (Enterprise) | [support-policy.md](support-policy.md) |
| Security questionnaires | Enterprise | The published CAIQ-shaped self-assessment, the evidence-linked FAQ, and one completed questionnaire per year on request | [questionnaire.md](questionnaire.md), [faq.md](faq.md) |
| MSA | Enterprise | Master software agreement template (draft, pending counsel) | [msa.md](msa.md) |
| DPA | Enterprise | No-processing data processing addendum template (draft, pending counsel) | [dpa.md](dpa.md) |
| Deployment help and roadmap input | Enterprise | Enterprise extras | [support-policy.md](support-policy.md) |

Seat and licensee counts are contractual terms, never enforced at runtime: Ghostlight never
phones home, never counts seats, and license state never changes behavior (ADR-0028).

Last reviewed: 2026-08-14 against the 1.0 source candidate | Contact: support@sylin.org
