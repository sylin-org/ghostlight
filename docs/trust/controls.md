# Ghostlight Controls Orientation

This page maps Ghostlight's properties onto the frameworks reviewers assess vendors against.
It is an orientation, not a certification: Ghostlight holds no SOC 2 report and no ISO/IEC
27001, ISO/IEC 42001, or CSA STAR certification, as stated on the trust center landing page
under [what we do not have](README.md). The purpose here is to help a reviewer find where each
framework's concerns land against a local-only, source-available tool, and where the evidence
for each answer lives.

## ISO/IEC 27001 Annex A orientation

The Annex A themes that matter for a vendor like Ghostlight cluster around supplier
relationships, secure development, cryptography, incident management, and continuity. For each,
the table states what your own control needs from a supplier, what Ghostlight provides, and
where to verify it.

| Annex A theme | What your control needs from a supplier | What Ghostlight provides | Evidence |
| --- | --- | --- | --- |
| Supplier relationships and ICT supply chain (A.5.19-A.5.23) | Assurance about the supplier and its build/release chain | Signed releases, per-file checksums, provenance, a per-release SBOM, and readable governance source | [supply-chain.md](supply-chain.md) |
| Access control on supplier assets (A.5.15-A.5.18) | Least privilege and strong authentication on the assets that affect you | MFA and least privilege on source, pipeline, and signing keys; keys held air-gapped | [security-overview.md](security-overview.md) |
| Secure development (A.8.25-A.8.28) | A disciplined SDLC behind what you deploy | Decision records for design, CI gates for correctness, signed artifacts for release | [supply-chain.md](supply-chain.md) |
| Use of cryptography (A.8.24) | Sound cryptographic practice for integrity | Composite Ed25519 + ML-DSA-65 signatures; signature-anchored trust; anti-rollback sequence | [security-overview.md](security-overview.md) |
| Logging and monitoring (A.8.15-A.8.16) | Records sufficient to reconstruct actions | Identity-bound tool-call audit with `policy_seq` policy provenance | [SIEM integration guide](../guides/siem-integration.md) |
| Incident management (A.5.24-A.5.28) | A defined vendor incident and notification commitment | Advisory within 3 business days of confirming a vendor-side compromise; private disclosure channel | [security-overview.md](security-overview.md) |
| ICT readiness for continuity (A.5.30) | Continuity that does not depend on the supplier surviving | The Continuity Promise, last-known-good cache, fail-closed cold boot | [continuity.md](continuity.md) |

On supplier auditing specifically, the governance module is source-available, so source access
is a standing audit right: you can read the code that enforces policy at any time rather than
schedule an audit window.

## SOC 2 orientation

No SOC 2 report exists for Ghostlight. If you are mapping the Trust Services Criteria, the
security criterion (CC-series) corresponds to the vendor-side controls in
[security-overview.md](security-overview.md) and [supply-chain.md](supply-chain.md);
availability corresponds to the continuity properties in [continuity.md](continuity.md); and
confidentiality and privacy largely do not engage, because the vendor holds no customer data,
as documented in [data-flows.md](data-flows.md). Use these as an orientation for your own
assessment, not as a substitute for an attestation Ghostlight does not have.

## AI frameworks

Ghostlight is the tool; when you deploy it, you are the deployer, and these frameworks place
their operative duties on the deployer. Ghostlight supports those duties rather than
discharging them.

- EU AI Act: the audit trail, its `policy_seq` policy provenance, and the Policy Passport (the
  `explain` surface answering "who governs me") give a deployer the record-keeping and
  human-oversight evidence that Articles 12 and 26 expect. This orientation supports your
  deployer duties; it is not legal advice, and how the framework applies to your deployment is
  a determination for you and your counsel.
- ISO/IEC 42001: Ghostlight holds no 42001 certificate. The governance layer (capability
  classification, modes, human-in-the-loop pause, audit) maps onto the operational controls an
  AI management system expects, as an orientation only.
- NIST AI RMF: govern maps to the policy and grant model; map maps to the capability
  classification of each action; measure maps to the audit trail and its provenance; manage
  maps to the modes, take-the-wheel pause, and panic kill switch.

See the [compliance team guide](../guides/compliance-team.md) and the
[governance configuration guide](../guides/governance-configuration.md).

Last reviewed: 2026-07-10 against v0.5.4 | Contact: support@sylin.org
