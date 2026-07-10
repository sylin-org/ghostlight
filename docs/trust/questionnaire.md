# Ghostlight Security Questionnaire (CAIQ v4 shape)

This is a self-assessment in the shape of the Cloud Security Alliance CAIQ v4, organized by
its control domains, and it is suitable for filing directly as vendor due diligence. It is a
self-assessment, not a third-party attestation. A CSA STAR Level 1 registry submission is
planned as a later step; no date is committed. Many domains that assume a hosted
software-as-a-service vendor do not apply to Ghostlight, because the runtime executes on your
infrastructure and the vendor operates no service; those are marked as not applicable with the
structural reason, rather than left blank or answered as if a service existed.

| CAIQ v4 domain | Response | Evidence |
| --- | --- | --- |
| A&A (Audit & Assurance) | No third-party audit has been completed. Source access to the governance module functions as a standing audit right, and any third-party audit will be published in full. | [security-overview.md](security-overview.md), [controls.md](controls.md) |
| AIS (Application & Interface Security) | The extension is Manifest V3 with no remotely hosted code; each permission is justified individually; all policy lives in the binary, not the extension. | [docs/legal/PERMISSION_JUSTIFICATIONS.md](../legal/PERMISSION_JUSTIFICATIONS.md) |
| BCR (Business Continuity Management & Operational Resilience) | The Continuity Promise, a last-known-good policy cache, and a fail-closed cold boot keep enforcement running independent of the vendor. | [continuity.md](continuity.md) |
| CCC (Change Control & Configuration Management) | Design changes are recorded as decision records; CI gates formatting, linting, tests, and the scenario runner; releases are signed. | [supply-chain.md](supply-chain.md) |
| CEK (Cryptography, Encryption & Key Management) | Licenses and policy bundles are signed with composite Ed25519 + ML-DSA-65; trust is anchored in the signature; signing keys are held air-gapped. | [security-overview.md](security-overview.md), [supply-chain.md](supply-chain.md) |
| DCS (Datacenter Security) | N/A -- structurally impossible: Ghostlight operates no datacenter and no hosted service; the runtime executes on your endpoints. | [data-flows.md](data-flows.md) |
| DSP (Data Security & Privacy Lifecycle Management) | N/A for vendor-held data -- structurally impossible: the vendor receives, stores, and processes no customer data; the full data lifecycle is governed by your own policies. | [data-flows.md](data-flows.md) |
| GRC (Governance, Risk & Compliance) | Ghostlight is early software from a solo-founder company; governance is documented in decision records and an honest absences register. | [README.md](README.md) |
| HRS (Human Resources Security) | One maintainer builds, signs, and supports Ghostlight, with least privilege on crown-jewel assets. N/A for workforce-scale personnel controls, which assume a staffed organization. | [security-overview.md](security-overview.md) |
| IAM (Identity & Access Management) | Vendor side: MFA and least privilege on source, pipeline, and keys. Product side: identity-bound capability grants with host polarity. | [security-overview.md](security-overview.md), [governance configuration guide](../guides/governance-configuration.md) |
| IPY (Interoperability & Portability) | Audit is standard JSON Lines or RFC 5424 syslog; the protocol is MCP; the engine is Apache-2.0 OR MIT and the governance module is source-available, so there is no lock-in. | [data-flows.md](data-flows.md), [continuity.md](continuity.md) |
| IVS (Infrastructure & Virtualization Security) | N/A -- structurally impossible: there is no vendor infrastructure or virtualization plane to secure; nothing of the vendor's runs in your path. | [data-flows.md](data-flows.md) |
| LOG (Logging & Monitoring) | Each tool call produces an identity-bound audit record; under managed policy it carries `policy_seq` provenance. Audit streams to syslog (RFC 5424 over UDP) or JSON Lines files today; HTTP delivery is deferred. | [SIEM integration guide](../guides/siem-integration.md) |
| SEF (Security Incident Management, E-Discovery & Cloud Forensics) | Vendor-side compromise triggers a security advisory within 3 business days of confirmation. N/A for cloud forensics, since there is no vendor cloud to investigate. | [security-overview.md](security-overview.md), [SECURITY.md](../../SECURITY.md) |
| STA (Supply Chain Management, Transparency & Accountability) | Per-release CycloneDX SBOM, signed artifacts, provenance attestations, and no subprocessors. | [supply-chain.md](supply-chain.md), [sub-processors.md](sub-processors.md) |
| TVM (Threat & Vulnerability Management) | Vulnerabilities are reported through a private channel; there is no bug bounty today; a third-party penetration test is planned when funded. | [SECURITY.md](../../SECURITY.md), [supply-chain.md](supply-chain.md) |
| UEM (Universal Endpoint Management) | N/A -- structurally impossible: Ghostlight manages no fleet of vendor endpoints; endpoint management is yours, and central policy ships through your own MDM. | [governance configuration guide](../guides/governance-configuration.md) |

Last reviewed: 2026-07-10 against v0.5.4 | Contact: support@sylin.org
