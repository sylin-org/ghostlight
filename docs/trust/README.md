# Ghostlight Trust Center

This is the front door for anyone reviewing Ghostlight for procurement, security, or
compliance. Every document here is public. Nothing in this trust center is gated behind an
NDA, a form, or a sales call. You can read, quote, and file any page before you ever talk
to me.

Ghostlight is a governed browser automation tool: a local binary and a thin Chromium
extension that give your MCP client controlled access to a browser session you are already
signed into. The runtime executes entirely on your own infrastructure. That single fact
shapes almost every answer below, because most vendor risk questions assume a vendor-side
service that, here, does not exist.

Where a claim can be verified, the answer links the evidence. I would rather show you the
mechanism than ask you to trust a summary of it.

## How to read this

Each answer is written to be pasted. The direct, quotable paragraph comes first and stands
on its own, so a reviewer can drop it straight into an assessment portal without editing.
Supporting links come after the paragraph. Where a claim rests on something concrete, the
answer ends with an `Evidence:` line naming the artifact behind it -- the ADR that decided
it, the source path that implements it, the test that exercises it, or
the guide that documents it.

Review footers identify the source state against which a page was materially checked. They are
evidence, not a release restamping chore: an unchanged legal template or point-in-time assessment
keeps its original review date. Git history remains the change record.

Absences are stated as facts, not apologies. Where Ghostlight lacks a control or an
attestation, the page says so plainly and explains why it does or does not matter given the
local-only architecture.

## Documents

| Document | What it covers |
| --- | --- |
| [faq.md](faq.md) | The 22 questions reviewers ask first: data, AI and agents, security posture, continuity, supply chain, legal and support. |
| [security-overview.md](security-overview.md) | Architecture, trust boundaries, the governance layer, cryptography, and vendor-side security. |
| [data-flows.md](data-flows.md) | What runs where, the flows that exist, the flows that do not exist, and the local artifacts. |
| [sub-processors.md](sub-processors.md) | The subprocessor register: none, and why. |
| [supply-chain.md](supply-chain.md) | Signed releases, checksums, provenance, the CycloneDX SBOM, and dependency posture. |
| [continuity.md](continuity.md) | The Continuity Promise, why it holds structurally, and runnable proof. |
| [controls.md](controls.md) | Framework orientation: ISO/IEC 27001 Annex A, SOC 2 criteria, and the AI frameworks. |
| [questionnaire.md](questionnaire.md) | A CAIQ v4-shaped self-assessment you can file as vendor due diligence. |
| [support-policy.md](support-policy.md) | Community support channels and scope. |
| [openssf-self-assessment.md](openssf-self-assessment.md) | Point-in-time OSPS Baseline Level 1 evidence map, open checks, and claim boundary. |
| [msa.md](msa.md) | Retired with paid licensing; where the agreement's substance lives now. |
| [dpa.md](dpa.md) | Retired as a template; the no-vendor-processing facts it recorded, stated directly. |
| [tiers.md](tiers.md) | Retired with the pricing page; each former tier claim mapped to its shipped feature. |
| [SECURITY.md](../../SECURITY.md) | Vulnerability reporting: the private disclosure channel and its response times. |

## What we do not have

Ghostlight is early software from a one-person company, and this section is deliberate: a
reviewer can act on a stated absence, and cannot act on a page of green checkmarks.

- **No SOC 2 report, and no ISO/IEC 27001, ISO/IEC 42001, or CSA STAR certification.** These
  attestations largely describe how a vendor handles data on its own systems. Ghostlight's
  runtime handles your data only on your systems, so the assurance those reports provide does
  not map onto this architecture. In their place I offer architecture-as-evidence: the design
  is documented in decision records, the behavior is exercised by runnable scenarios, and every
  line of code -- governance included -- ships as open source you can read. Certification is
  planned as the project grows, beginning with a CSA STAR Level 1 self-assessment submission;
  see [controls.md](controls.md) for how a reviewer can orient these frameworks against
  Ghostlight today.
- **No completed third-party penetration test.** One is planned and will be commissioned when
  funding allows, and any third-party security audit of Ghostlight will be published in full,
  including findings. Until then, the open-source license functions as a standing audit right:
  you can read the code that enforces policy yourself.
- **No team beyond a solo founder.** One maintainer builds, signs, and supports Ghostlight
  today.
  The mitigation is structural rather than contractual: the Continuity Promise guarantees the
  software keeps working regardless of the vendor's status, and the whole product is
  Apache-2.0 OR MIT licensed, so a user's ability to keep operating never depends on the
  company's survival. See [continuity.md](continuity.md).

Last reviewed: 2026-08-25 against the 1.0 source candidate | Contact: hello@sylin.org
