# Ghostlight Security Questionnaire (CAIQ v4)

This is a self-assessment in the shape of the Cloud Security Alliance CAIQ v4, organized by
its control domains. It is a self-assessment, not a third-party attestation. Where your
process accepts a domain-level self-assessment, it can be filed as-is; where it requires the
full question-level set, I complete one questionnaire per year on request
([support-policy.md](support-policy.md)). A CSA STAR Level 1 registry submission is
planned as a later step; no date is committed. Many domains that assume a hosted
software-as-a-service vendor do not apply to Ghostlight, because the runtime executes on your
infrastructure and the vendor operates no service; those are marked as not applicable with the
structural reason, rather than left blank or answered as if a service existed.

| CAIQ v4 domain | Response | Evidence |
| --- | --- | --- |
| A&A (Audit & Assurance) | No third-party audit has been completed. Source access to the governance module functions as a standing audit right, and any third-party audit will be published in full. | [security-overview.md](security-overview.md), [controls.md](controls.md) |
| AIS (Application & Interface Security) | The extension is Manifest V3 and all extension logic ships in the reviewed package. Explicitly requested page JavaScript is evaluated only in the attached page through the local automation path. Each permission is justified individually; all policy lives in the binary, not the extension. | [docs/legal/PERMISSION_JUSTIFICATIONS.md](../legal/PERMISSION_JUSTIFICATIONS.md) |
| BCR (Business Continuity Management & Operational Resilience) | The Continuity Promise, a last-known-good policy cache, and a fail-closed cold boot keep enforcement running independent of the vendor. | [continuity.md](continuity.md) |
| CCC (Change Control & Configuration Management) | Design changes are recorded as decision records; CI gates formatting, linting, tests, a dependency audit, and the scenario runner; releases are checksummed and provenance-attested. | [supply-chain.md](supply-chain.md) |
| CEK (Cryptography, Encryption & Key Management) | Customer policy bundles require Ed25519; an optional ML-DSA-65 key makes both signature legs mandatory. Trust is anchored in customer-owned public keys, and private keys never enter Ghostlight. | [security-overview.md](security-overview.md), [governance configuration guide](../guides/governance-configuration.md) |
| DCS (Datacenter Security) | N/A -- structurally impossible: Ghostlight operates no datacenter and no hosted service; the runtime executes on your endpoints. | [data-flows.md](data-flows.md) |
| DSP (Data Security & Privacy Lifecycle Management) | N/A for vendor-held data -- structurally impossible: the vendor receives, stores, and processes no customer data; the full data lifecycle is governed by your own policies. | [data-flows.md](data-flows.md) |
| GRC (Governance, Risk & Compliance) | Ghostlight is early software from a solo-founder company; governance is documented in decision records and an explicit register of absent controls and attestations. | [README.md](README.md) |
| HRS (Human Resources Security) | One maintainer builds, signs, and supports Ghostlight. N/A for workforce-scale personnel controls, which assume a staffed organization; the access surface is a single locked-down account. | [security-overview.md](security-overview.md) |
| IAM (Identity & Access Management) | Vendor side: a single maintainer account with MFA on source and pipeline accounts; no shared accounts or third-party write access. Product side: identity-described capability grants with host polarity. | [security-overview.md](security-overview.md), [governance configuration guide](../guides/governance-configuration.md) |
| IPY (Interoperability & Portability) | Audit is standard JSON Lines suitable for endpoint collectors; the protocol is MCP; the engine is Apache-2.0 OR MIT and the governance module source is available. | [data-flows.md](data-flows.md), [continuity.md](continuity.md) |
| IVS (Infrastructure & Virtualization Security) | N/A -- structurally impossible: there is no vendor infrastructure or virtualization plane to secure; nothing of the vendor's runs in your path. | [data-flows.md](data-flows.md) |
| LOG (Logging & Monitoring) | Each terminal invocation produces a content-minimized JSONL record; managed decisions carry `policy_seq`, tier, grant, rule, mode, and denial attribution. SIEM delivery uses the endpoint's existing file collector. | [SIEM integration guide](../guides/siem-integration.md) |
| SEF (Security Incident Management, E-Discovery & Cloud Forensics) | A confirmed vendor-side compromise triggers a prompt public security advisory, typically within a few business days as a best-effort solo-maintainer target. N/A for cloud forensics, since there is no vendor cloud to investigate. | [security-overview.md](security-overview.md), [SECURITY.md](../../SECURITY.md) |
| STA (Supply Chain Management, Transparency & Accountability) | Per-release CycloneDX SBOM, per-file checksums, provenance attestations, and no subprocessors. | [supply-chain.md](supply-chain.md), [sub-processors.md](sub-processors.md) |
| TVM (Threat & Vulnerability Management) | Vulnerabilities are reported through a private channel. Acknowledgment, triage, and remediation follow the best-effort solo-maintainer targets in SECURITY.md; there is no bug bounty today, and a third-party penetration test is planned when funded. | [SECURITY.md](../../SECURITY.md), [supply-chain.md](supply-chain.md) |
| UEM (Universal Endpoint Management) | N/A -- structurally impossible: Ghostlight manages no fleet of vendor endpoints; endpoint management is yours, and central policy ships through your own MDM. | [governance configuration guide](../guides/governance-configuration.md) |

Last reviewed: 2026-08-14 against the 1.0 source candidate | Contact: hello@sylin.org
