# Ghostlight Tiers: Retired

> **RETIRED 2026-08-25 by [ADR-0140](../adr/0140-fully-open-source-licensing.md).** The pricing
> page and its tiers were withdrawn; no claim is gated behind a tier anymore. This page records
> where each capability a tier once sold ships today.

Every capability the tiers once sold is simply a shipped feature of a free product now:

| Former claim | Where it lives today | Evidence |
| --- | --- | --- |
| Central policy | Signed file or HTTPS policy from an organization-owned source, activated by an administrator-provisioned bootstrap | [ADR-0121](../adr/0121-restore-rawx-policy-and-managed-fetch.md), [governance configuration guide](../guides/governance-configuration.md) |
| SIEM audit | Content-minimized JSONL with `policy_seq` and denial attribution, collected by the endpoint's existing file agent | [SIEM integration guide](../guides/siem-integration.md) |
| Support | Community support on GitHub, best-effort email for what cannot be public | [support-policy.md](support-policy.md) |
| Security questionnaires | The published CAIQ-shaped self-assessment and the evidence-linked FAQ, available to everyone | [questionnaire.md](questionnaire.md), [faq.md](faq.md) |
| MSA and DPA | Retired with paid licensing; the underlying facts (license grant, no vendor data flows) are documented directly | [msa.md](msa.md), [dpa.md](dpa.md), [data-flows.md](data-flows.md) |

Nothing about Ghostlight is enforced at runtime by any commercial term: it never phones home,
never counted seats, and license state never changed behavior ([ADR-0028](../adr/0028-tripwire-licensing-and-continuity-promise.md)).
Now there is also nothing to count.

Last reviewed: 2026-08-25 against the 1.0 source candidate | Contact: hello@sylin.org
