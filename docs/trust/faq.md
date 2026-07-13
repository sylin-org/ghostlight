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
telemetry, and initiates no network activity beyond your own tool calls, the audit
destinations you configure, and the optional central-policy fetch from an endpoint your own
organization hosts. Page content, credentials, audit records, and policy all stay on
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
small set of local artifacts: audit records (JSON Lines files, or a syslog stream, or nothing,
per your configuration) and, when you use central policy, a local policy cache and its status
sidecar. All of these live on the endpoint or the destinations you configure. Retention and
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
industry-wide, and we will not claim to have solved it. What Ghostlight does is bound the
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
switch terminates the session outright. Nothing runs that the active policy does not permit.

See [docs/SPEC.md](../SPEC.md) and the
[governance configuration guide](../guides/governance-configuration.md).

Evidence: docs/SPEC.md (capability classification, modes, take-the-wheel, panic kill); docs/guides/governance-configuration.md.

### What is logged per agent action? Does the audit record capture the policy state at decision time?

Each tool call produces an identity-bound audit record, and yes: under managed governance that
record carries a `policy_seq` field, the org-signed publish sequence of the exact policy that
was in force when the decision was made. That ties every logged action to the precise policy
version that authorized it, so an auditor can reconstruct not just what happened but which
rules applied at that moment. The record is decision metadata only: it never contains page
content, typed values, or screenshots, so your SIEM does not become a sensitive-data store.
Audit streams to syslog (RFC 5424 over UDP) or JSON Lines files today; HTTP delivery is
deferred.

See the [SIEM integration guide](../guides/siem-integration.md).

Evidence: docs/guides/siem-integration.md (record schema, policy_seq field); ADR-0055 Impl.9c (policy_seq on tool-call records).

### Can we enforce policy centrally across a fleet?

Yes, through the `managed://` scheme. A central policy bundle, signed by your organization, is
provisioned to endpoints by your existing management channel (GPO, Intune, Jamf), fetched from
an endpoint you host, and enforced with a last-known-good cache so a device keeps its policy
even when the source is unreachable. The trust anchor is the signature on the bundle, not the
transport, and a monotonic publish sequence prevents rollback to an older, more permissive
policy. The runnable scenario managed-activation-local shows a device activating a managed
policy end to end:

    cargo run -p ghostlight-lightbox -- run managed-activation-local

See [ADR-0055](../adr/0055-managed-scheme-central-policy-distribution.md) and the
[governance configuration guide](../guides/governance-configuration.md).

Evidence: ADR-0055 (managed:// design, signed trust, anti-rollback); docs/guides/governance-configuration.md; lightbox scenario managed-activation-local.

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

See [docs/SPEC.md](../SPEC.md) and [ADR-0001](../adr/0001-single-binary-thin-extension.md).

Evidence: docs/SPEC.md architecture (drives the user's real session, never relocates it); ADR-0001 (single binary, thin extension).

## Security posture

### What certifications do you hold?

None yet. Ghostlight holds no SOC 2 report and no ISO/IEC 27001, ISO/IEC 42001, or CSA STAR
certification. Most of those attestations describe how a vendor protects data on its own
systems, and Ghostlight's runtime holds your data only on your systems, so their assurance
does not map onto this architecture. In their place we offer architecture-as-evidence
(documented decisions, runnable scenarios, source-available governance code); certification
is planned as the customer base grows, beginning with a CSA STAR Level 1 self-assessment
submission. The full inventory of absent attestations, with reasons, is published in the
Ghostlight trust center.

See [README.md](README.md) (what we do not have) and [controls.md](controls.md).

Evidence: README.md what-we-do-not-have section; controls.md framework orientation.

### How do you secure your own infrastructure?

The assets that matter on the vendor side are the source repository, the release pipeline,
and the signing keys for licenses and policy bundles, and those are what we protect. The
license- and policy-signing keys are held offline on an air-gapped machine and never touch CI
or any online system. Release binaries are protected differently: per-file SHA-256 checksums
and build-provenance attestations tie each artifact to the exact source commit and workflow
run that produced it. Source and pipeline access is a single maintainer account with
multi-factor authentication, no shared accounts, and no third-party write access, and changes
reach a release only through recorded decision records and CI gates. There is no
customer-data store on our side to defend, so our security effort concentrates on the
integrity of what we ship to you.

See [supply-chain.md](supply-chain.md).

Evidence: supply-chain.md (build and change management); ADR-0028 Decision 10 (air-gapped license signing).

### Has Ghostlight been penetration tested? How do you handle vulnerabilities?

Ghostlight has not yet commissioned a third-party penetration test; one is planned and will be
run when funding allows. We make a standing commitment that any third-party security audit of
Ghostlight will be published in full, including findings, and until then the source access
granted by the governance license functions as a standing audit right: you can read the code
that enforces policy. Suspected vulnerabilities go through the private disclosure channel
documented in the project's SECURITY.md, with a 48-hour acknowledgment, triage within 7
days, and a 30-day fix target for confirmed critical issues.

See [SECURITY.md](../../SECURITY.md) and [security-overview.md](security-overview.md).

Evidence: SECURITY.md (disclosure channel); security-overview.md (publish-all-audits pledge, source-as-standing-audit-right).

### What is your incident response and breach notification commitment?

Because no customer data ever reaches the vendor, there is no customer-data breach for us to
notify you of; the meaningful incident on our side is a compromise of what we ship, that is,
the build, the signing keys, or the update channel. For that class of event we commit to
publishing a security advisory within 3 business days of confirming a vendor-side compromise,
with the affected versions and the remediation, as a GitHub Security Advisory on the
repository, named in release notes; watching the repository's releases is the subscription
path. Because Ghostlight never phones home, your deployment learns nothing on its own, so
the advisory channel is deliberately a pull channel. Incidents inside your own deployment
are yours to detect through the audit trail Ghostlight produces.

See [security-overview.md](security-overview.md).

Evidence: security-overview.md (incident response; advisory window); ADR-0057 Decision 11a.

## Continuity and viability

### What happens if the vendor disappears, or we stop paying?

Nothing stops working. The Continuity Promise, binding on all tiers, states it directly:
"Ghostlight never phones home and license state never affects behavior. Enforcement, audit,
and your production workflows are never interrupted, degraded, or disabled by license expiry,
by the vendor's unavailability, or by the vendor ceasing to exist." An expired license changes
exactly one thing: license-state notices appear in your own tooling and audit records. Because
the governance module is source-available, you already hold the code; no escrow trigger
stands between you and it. The runnable scenario continuity-source-unreachable proves the
promise's policy leg, enforcement continuing from the last-known-good cache when your policy
source goes dark:

    cargo run -p ghostlight-lightbox -- run continuity-source-unreachable

See [ADR-0028](../adr/0028-tripwire-licensing-and-continuity-promise.md), the
[licensing guide](../guides/licensing.md), and [continuity.md](continuity.md).

Evidence: ADR-0028 Decision 6 (Continuity Promise wording); docs/guides/licensing.md; continuity.md; lightbox scenario continuity-source-unreachable.

### What are your BC/DR commitments?

The conventional BC/DR question inverts here, because nothing of the vendor's runs in your
critical path: there is no vendor-hosted Ghostlight service whose outage could take your workflows
down.
Central policy continues to enforce from its last-known-good cache through a policy-source
outage, and a cold boot with nothing available fails closed to the protective state rather
than opening up. Your continuity therefore depends on your own infrastructure, which you
already plan for, not on ours. The runnable scenario fail-closed-cold-boot demonstrates the
cold boot directly:

    cargo run -p ghostlight-lightbox -- run fail-closed-cold-boot

See [continuity.md](continuity.md).

Evidence: continuity.md (last-known-good cache, fail-closed cold boot); ADR-0055 Decision 5; lightbox scenario fail-closed-cold-boot.

## Supply chain

### Do you provide an SBOM, signed releases, and build provenance?

Yes. The release pipeline generates a CycloneDX software bill of materials for every release
and publishes it as a release asset (introduced 2026-07; earlier releases carry checksums and
attestations but no SBOM), alongside per-file SHA-256 checksums and build-provenance
attestations. You can verify what you downloaded against the published checksums and confirm
its provenance with one command before deploying; the package-manager channels (npm,
Homebrew, Scoop, winget) distribute the same tagged artifacts. The dependency tree is
deliberately lean, the signature cryptography is pure Rust, and a build flag yields an
air-gap binary with no HTTP or TLS stack at all.

See [supply-chain.md](supply-chain.md) and the
[release workflow](../../.github/workflows/release.yml).

Evidence: supply-chain.md (releases, SBOM, dependencies); .github/workflows/release.yml (checksums, provenance, SBOM step).

### How do we review and force-install the extension?

Every extension permission is justified individually in a published permission-justification
document, so your security team can review the exact access before approving it. The
extension is Manifest V3, ships no remotely hosted code, and its extension ID is stable
across installs, pinned by a committed manifest key. Today it is distributed as a versioned
zip asset on each GitHub release, for review and self-hosted deployment; a Chrome Web Store
listing is in preparation, and once it is live, fleet force-install through Chromium's
`ExtensionInstallForcelist` pinned to the same ID applies. Store-installed extensions follow
Chrome's auto-update; fleets that require version control over the extension can keep
self-hosting.

See [docs/legal/PERMISSION_JUSTIFICATIONS.md](../legal/PERMISSION_JUSTIFICATIONS.md) and
[extension/manifest.json](../../extension/manifest.json).

Evidence: docs/legal/PERMISSION_JUSTIFICATIONS.md (per-permission rationale); extension/manifest.json (Manifest V3, no remote code).

## Legal and support

### What support do you commit to?

Support runs by email at support@sylin.org, and the commitment is an acknowledgment time, not
a resolution time: a first human acknowledgment within 3 business days for Team and within 2
business days for Enterprise. From there we work the issue with you; the acknowledgment clock
is what we promise to meet. Suspected security vulnerabilities do not go to the support lane;
they go through the private disclosure channel documented in the project's SECURITY.md.

See [support-policy.md](support-policy.md).

Evidence: support-policy.md (acknowledgment commitments, scope); SECURITY.md (security-report channel).

### What are the license terms, and what happens at expiry?

Ghostlight is open-core. The automation engine is licensed Apache-2.0 OR MIT, both of them
open source licenses. The governance module is source-available under the Ghostlight
Commercial License: you can read and audit its code, but the license grants narrower rights
than the engine's. At
license expiry nothing about enforcement or audit changes; the only effect is that
license-state notices appear in your own tooling and audit records until you renew, per the
Continuity Promise.

See the [licensing guide](../guides/licensing.md),
[ADR-0027](../adr/0027-open-core-business-model-and-licensing.md), and
[ADR-0028](../adr/0028-tripwire-licensing-and-continuity-promise.md).

Evidence: docs/guides/licensing.md; ADR-0027 (open-core split, source-available governance); ADR-0028 (expiry changes only the audit stamp).

Last reviewed: 2026-07-10 against v0.5.6 | Contact: support@sylin.org
