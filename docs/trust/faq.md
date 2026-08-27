# Ghostlight Trust Center: FAQ

These are the questions reviewers ask first. Each answer is written to be quoted: the first
paragraph stands on its own for pasting into an assessment portal, links follow, and a
closing evidence line names the artifact behind the claim. The recurring theme is that
Ghostlight's runtime executes on your infrastructure, so most vendor-side risk questions
have a structural rather than a procedural answer.

- **[Data and privacy](#data-and-privacy):** vendor access to data; model training;
  extension access; subprocessors; storage and retention; DPA and GDPR/CCPA.
- **[AI and agents](#ai-and-agents):** prompt injection; autonomy, pause, and kill;
  per-action logging and policy provenance; central fleet policy; the AI frameworks; the
  AI-browser question.
- **[Security posture](#security-posture):** certifications; vendor-side security;
  penetration testing and vulnerability handling; incident response.
- **[Continuity and viability](#continuity-and-viability):** vendor disappearance and
  license expiry; BC/DR.
- **[Supply chain](#supply-chain):** SBOM, checksums, and provenance; extension review and
  fleet distribution.
- **[Legal and support](#legal-and-support):** support commitments; license terms and
  expiry.

## Data and privacy

### Does any of our data ever reach the vendor?

No. Ghostlight generates zero vendor-bound traffic: the binary never phones home, carries no
telemetry, and initiates no network activity beyond your own tool calls and the optional signed
policy fetch from an endpoint your own organization hosts. Page content, credentials, audit records, and policy all stay on
your infrastructure or flow only to endpoints you choose. There is no vendor service to
receive your data, so there is no vendor-side copy of it to secure, subpoena, or breach.

See [data-flows.md](data-flows.md) and [docs/legal/PRIVACY.md](../legal/PRIVACY.md).

Evidence: ADR-0028 Decision 9 (never phone home, normative and permanent); docs/legal/PRIVACY.md.

### Is our data used to train AI models? Which model providers sit behind the product?

Your data is not used to train any model, and no model provider sits behind Ghostlight.
Ghostlight calls no LLM of its own: it is the governed bridge between your MCP client and a
browser session, and the model belongs to you through the client you already run.
There is no model-provider client in Ghostlight's dependency tree. Whichever model your MCP
client uses, and whatever that provider's training terms say, that relationship is between
you and the provider; Ghostlight neither mediates nor observes it.

See [docs/SPEC.md](../SPEC.md) for the architecture.

Evidence: the Cargo.toml dependency tree (no model-provider SDK present); docs/SPEC.md architecture section.

### What can the browser extension access, and where does that data go?

The extension is a thin executor with no policy logic and no cloud backend. It reads and acts
on the page you direct it to, and it sends what it observes only through Chromium native messaging
to the local Ghostlight service. There is no path from the extension to a vendor-hosted Ghostlight
service, because none exists. Every requested browser permission is justified individually in a
published permission-justification document.

See [docs/legal/PERMISSION_JUSTIFICATIONS.md](../legal/PERMISSION_JUSTIFICATIONS.md),
[extension/manifest.json](../../extension/manifest.json), and [data-flows.md](data-flows.md).

Evidence: extension/manifest.json (declared permissions and host access); docs/legal/PERMISSION_JUSTIFICATIONS.md; data-flows.md.

### Who are your subprocessors?

None. Ghostlight engages no subprocessors, because the vendor receives no customer data for a
third party to process on its behalf. The only third parties involved are ones you choose:
the MCP client and model you run, the SIEM you stream audit to, and the endpoint hosting
your central policy.

See [sub-processors.md](sub-processors.md).

Evidence: sub-processors.md (the empty subprocessor register, with reasoning).

### Where is our data stored and processed, and how is it retained or deleted?

Exclusively on your own infrastructure, under your own retention policies. Ghostlight writes a
small set of local artifacts: an audit JSON Lines file and, when you use central policy, a signed
policy cache and its status sidecar. All of these live on the endpoint. Retention and
deletion are yours to set, because there is no vendor-side store to govern.

See [data-flows.md](data-flows.md) for the artifact locations.

Evidence: data-flows.md (local artifacts and their owners); ADR-0028 Decision 9.

### Do you offer a DPA? How do you comply with GDPR/CCPA?

Yes, a DPA template is published, and it leads with the fact that the vendor processes no
customer personal data. Because the runtime never sends personal data to a Ghostlight
service, the conventional controller-processor mechanics do not engage; the DPA states that
fact directly instead of constructing clauses for a data flow that does not exist. Your
obligations under GDPR or CCPA attach to your own processing on your own systems, which
Ghostlight is built to keep local.

See [dpa.md](dpa.md) and [docs/legal/PRIVACY.md](../legal/PRIVACY.md).

Evidence: dpa.md (no-processing DPA template, pending counsel review); docs/legal/PRIVACY.md.

## AI and agents

### How do you mitigate prompt injection, including indirect injection from web content?

Prompt injection, including indirect injection from page content, is an unsolved problem
industry-wide, and I will not claim to have solved it. What Ghostlight does is bound the
blast radius so a successful injection cannot become an unbounded action. Sacred never-touch
domains are refused even when a policy or a prompt asks for them; capability grants scope what
the agent may do to which hosts; observe and enforce modes control whether actions run at
all, with shadow denials recording what enforcement would have blocked; and a panic kill
switch stops everything immediately. Injection can still mislead
the model, but governance decides what a misled model is permitted to do.

See [docs/SPEC.md](../SPEC.md) and the
[governance configuration guide](../guides/governance-configuration.md).

Evidence: docs/SPEC.md (sacred domains, capability model); ADR-0022 (capability classification); docs/guides/governance-configuration.md.

### What can the agent do autonomously? Can we pause or stop it mid-run?

Autonomy is bounded by policy and is interruptible at any point. Every tool call is classified
by capability (read, action, write, execute) and gated accordingly; observe and enforce
modes let you run the agent in a watching posture before granting it real actions, and under
observe a loaded policy runs in shadow, recording would-deny events without blocking
anything. A take-the-wheel pause hands control back to the human mid-run, and a panic kill
switch terminates the session outright. Enforce blocks refused work; observe deliberately runs and
audits ordinary would-deny decisions for rollout analysis.

See [docs/SPEC.md](../SPEC.md) and the
[governance configuration guide](../guides/governance-configuration.md).

Evidence: docs/SPEC.md (capability classification, modes, take-the-wheel, panic kill); docs/guides/governance-configuration.md.

### What is logged per agent action? Does the audit record capture the policy state at decision time?

Each terminal invocation produces an authority-attributed audit record, and yes: under managed governance that
record carries a `policy_seq` field, the org-signed publish sequence of the exact policy that
was in force when the decision was made. That ties every logged action to the precise policy
version that authorized it, so an auditor can reconstruct not just what happened but which
rules applied at that moment. The record is decision metadata only: it never contains page
content, typed values, or screenshots, so your SIEM does not become a sensitive-data store.
Ghostlight writes JSON Lines locally; SIEM delivery uses the endpoint's existing file collector.

See the [SIEM integration guide](../guides/siem-integration.md).

Evidence: docs/guides/siem-integration.md (record schema, policy_seq field); ADR-0055 Impl.9c (policy_seq on tool-call records).

### Can we enforce policy centrally across a fleet?

Yes. Your endpoint-management channel provisions a fixed `managed.json` bootstrap. It names a
customer-owned local file or HTTPS source and customer-owned public verification keys. Ghostlight
verifies the signed bundle, caches last-known-good, and retains it when the source is unreachable.
Monotonic publish sequence prevents rollback to an older policy, and the workbench Policy Passport
shows organization, verification, sequence, freshness, source class, and contacts.

See [ADR-0055](../adr/0055-managed-scheme-central-policy-distribution.md) and the
[governance configuration guide](../guides/governance-configuration.md).

Evidence: ADR-0121 (signed managed delivery and anti-rollback); docs/guides/governance-configuration.md; governance managed-policy tests.

### What is your posture under the EU AI Act, ISO/IEC 42001, and NIST AI RMF?

Ghostlight is a tool vendor; when you deploy it, you are the deployer, and these frameworks
place their operative duties on the deployer. Ghostlight supports those duties rather than
discharging them: the audit trail and policy provenance give you the record-keeping and
human-oversight evidence that, for example, EU AI Act Articles 12 and 26 expect a deployer to
maintain. Ghostlight holds no ISO/IEC 42001 certificate, and nothing here is legal advice; how
these frameworks apply to your deployment is a determination for you and your counsel. A
framework-by-framework orientation is published in the Ghostlight trust center.

See [controls.md](controls.md).

Evidence: controls.md (EU AI Act, ISO/IEC 42001, NIST AI RMF orientation); ADR-0057 Decision 11e (no legal advice).

### Analysts have advised blocking AI browsers. How is Ghostlight different?

The advice to block AI browsers targets replacement browsers that relocate a user's session
into vendor-controlled infrastructure. Ghostlight is the opposite pattern: it drives the
user's own Chrome, in place, subject to the hardening and policy you already apply, and never
moves the session anywhere. There is no separate browser to sanction, and because every agent
action is attributed in the audit trail, each automated click is at least as attributable as
a manual one.

See [security-overview.md](security-overview.md),
[ADR-0005](../adr/0005-policy-free-extension.md), and
[ADR-0096](../adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md).

Evidence: security-overview.md (current process and trust boundaries); ADR-0005 (thin,
policy-free extension); ADR-0096 (current MCP edge, neutral service, and browser relay topology).

## Security posture

### What certifications do you hold?

None yet. Ghostlight holds no SOC 2 report and no ISO/IEC 27001, ISO/IEC 42001, or CSA STAR
certification. Most of those attestations describe how a vendor protects data on its own
systems, and Ghostlight's runtime holds your data only on your systems, so their assurance
does not map onto this architecture. In their place I offer architecture-as-evidence
(documented decisions, runnable scenarios, fully open-source code); certification
is planned as the project grows, beginning with a CSA STAR Level 1 self-assessment
submission. The full inventory of absent attestations, with reasons, is published in the
Ghostlight trust center.

See [README.md](README.md) (what we do not have) and [controls.md](controls.md).

Evidence: README.md what-we-do-not-have section; controls.md framework orientation.

### How do you secure your own infrastructure?

The assets that matter on the vendor side are the source repository, release pipeline, and
distribution channels. Customer policy-signing keys are not vendor assets: Ghostlight generates
them locally for the customer and never receives them. Release binaries use per-file SHA-256 checksums
and build-provenance attestations tie each artifact to the exact source commit and workflow
run that produced it. Source and pipeline access is a single maintainer account with
multi-factor authentication, no shared accounts, and no third-party write access, and changes
reach a release only through recorded decision records and CI gates. There is no
customer-data store on the vendor side to defend, so the security effort concentrates on
the integrity of what I ship to you.

See [supply-chain.md](supply-chain.md).

Evidence: supply-chain.md (build and change management); ADR-0121 (customer-owned policy keys).

### Has Ghostlight been penetration tested? How do you handle vulnerabilities?

Ghostlight has not yet commissioned a third-party penetration test; one is planned and will be
run when funding allows. I make a standing commitment that any third-party security audit of
Ghostlight will be published in full, including findings, and until then the open-source license
functions as a standing audit right: you can read the code
that enforces policy. Suspected vulnerabilities go through the private disclosure channel
documented in the project's SECURITY.md. As a solo-maintainer project, acknowledgment, triage, and
remediation timelines are published there as best-effort targets rather than contractual windows.

See [SECURITY.md](../../SECURITY.md) and [security-overview.md](security-overview.md).

Evidence: SECURITY.md (disclosure channel); security-overview.md (publish-all-audits pledge, source-as-standing-audit-right).

### What is your incident response and breach notification commitment?

Ghostlight operates no customer-data store, so the meaningful vendor-side incident is a compromise
of what I ship: the source repository, build pipeline, offline signing keys, or distribution
channel. After confirming such a compromise, I aim to publish a GitHub Security Advisory promptly,
typically within a few business days, naming affected versions and remediation. This is a
best-effort solo-maintainer target, not a contractual notification window. Release notes and the
repository advisory feed are the subscription path. Incidents inside your own deployment remain
yours to detect through the audit trail Ghostlight produces.

See [security-overview.md](security-overview.md).

Evidence: SECURITY.md and security-overview.md (incident-response scope and best-effort advisory
target).

## Continuity and viability

### What happens if the vendor disappears?

Nothing stops working. The Continuity Promise states it directly:
"Ghostlight never phones home and license state never affects behavior. Enforcement, audit,
and your production workflows are never interrupted, degraded, or disabled by license expiry,
by the vendor's unavailability, or by the vendor ceasing to exist." Ghostlight 1.0 has no runtime
license state, license-status command, license gate, or license field in audit -- and since
[ADR-0140](../adr/0140-fully-open-source-licensing.md) there is no paid relationship whose end
could matter. The whole product is open source, so you already hold the code; no escrow trigger
stands between you and it. Managed-policy tests prove that enforcement continues from verified
last-known-good when your source goes dark.

See [ADR-0028](../adr/0028-tripwire-licensing-and-continuity-promise.md), the
[licensing guide](../guides/licensing.md), and [continuity.md](continuity.md).

Evidence: ADR-0028 Decision 6; docs/guides/licensing.md; continuity.md; managed-policy tests.

### What are your BC/DR commitments?

The conventional BC/DR question inverts here, because nothing of the vendor's runs in your
critical path: there is no vendor-hosted Ghostlight service whose outage could take your workflows
down.
Central policy continues to enforce from its last-known-good cache through a policy-source
outage, and a cold boot with nothing available fails closed to the protective state rather
than opening up. Your continuity therefore depends on your own infrastructure, which you
already plan for, not on ours. The same production verification path has tests for offline cache
recovery, cold-start failure, bad signatures, and rollback.

See [continuity.md](continuity.md).

Evidence: continuity.md (last-known-good cache and fail-closed cold boot); ADR-0121 Decision 4; managed-policy tests.

## Supply chain

### Do you provide an SBOM, checksums, and build provenance?

Yes. The release pipeline generates a CycloneDX software bill of materials for every release
and publishes it as a release asset (introduced 2026-07; earlier releases carry checksums and
attestations but no SBOM), alongside per-file SHA-256 checksums and build-provenance
attestations. You can verify what you downloaded against the published checksums and confirm its
provenance with one command before deploying. The public channels currently serve 1.1.0 (the Chrome Web Store adapter stays 1.0.0 by design).
The workflow builds and attests a candidate without publishing it; each channel advances only after
an explicit owner-approved operation and public reconciliation. The dependency tree is deliberately
lean; managed HTTPS uses rustls with the pure-Rust ring provider.

See [supply-chain.md](supply-chain.md) and the
[current build-only release workflow](../../.github/workflows/release.yml).

Evidence: supply-chain.md (releases, SBOM, dependencies); .github/workflows/release.yml (checksums, provenance, SBOM step).

### How do we review and force-install the extension?

Every extension permission is justified individually in a published permission-justification
document, so your security team can review the exact access before approving it. The extension is
Manifest V3, and all extension logic ships in the reviewed package. Its advertised
`browser_execute` accepts explicit page JavaScript from the local MCP client and evaluates it only
in the attached page; it does not fetch or install extension logic. Release archives use the
stable unpacked id pinned by the committed manifest key. The Chrome Web Store item has its own
stable store-assigned id. Fleet policy can force-install either the Web Store item or a reviewed,
self-hosted package by its corresponding id. Store-installed extensions follow Chrome's
auto-update; fleets that require version control over the extension can keep self-hosting.

See [docs/legal/PERMISSION_JUSTIFICATIONS.md](../legal/PERMISSION_JUSTIFICATIONS.md) and
[extension/manifest.json](../../extension/manifest.json).

Evidence: docs/legal/PERMISSION_JUSTIFICATIONS.md (per-permission and page-JavaScript rationale); extension/manifest.json (Manifest V3 and packaged extension logic); crates/orchestrator/src/install/native_host.rs (both official ids).

## Legal and support

### What support do you commit to?

Support is community support, because Ghostlight is free and open source with no paid tier.
GitHub Issues for reproducible defects, GitHub Discussions for questions, and best-effort email
at hello@sylin.org only for what cannot be public; there is no guaranteed acknowledgment or
resolution window. Suspected security vulnerabilities do not go to a public lane;
they go through the private disclosure channel documented in the project's SECURITY.md.

See [support-policy.md](support-policy.md).

Evidence: support-policy.md (channels and scope); SECURITY.md (security-report channel).

### What are the license terms?

Ghostlight is free and open source: the entire product, including the governance module, is
licensed Apache-2.0 OR MIT ([ADR-0140](../adr/0140-fully-open-source-licensing.md)). There are
no tiers, seats, activation, or paid options of any kind. Ghostlight 1.0 has no
activation server, license-status command, runtime license gate, or audit license stamp, so
nothing about licensing can alter browser behavior or local continuity -- there is no runtime
license state at all.

See the [licensing guide](../guides/licensing.md),
[ADR-0027](../adr/0027-open-core-business-model-and-licensing.md) (the superseded open-core
split), and [ADR-0028](../adr/0028-tripwire-licensing-and-continuity-promise.md).

Evidence: docs/guides/licensing.md; ADR-0140 (whole-product Apache-2.0 OR MIT); ADR-0028 (license state never gates behavior).

Last reviewed: 2026-08-25 against the 1.0 source candidate | Contact: hello@sylin.org
