Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z

# Claim ledger

| Claim | Scope | Evidence | Version or date | Confidence | Safe wording | Excluded wording | Next proof |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Uses the user's existing authenticated browser session | Ghostlight-managed tabs in supported Chromium profiles | README, installation guide, architecture, extension mechanism | v0.6.0 | High | "Use the Chromium profile where you are already signed in, inside Ghostlight-managed tabs." | "Ghostlight can access every tab" or "isolated sandbox" | Recheck store build against managed-tab tests |
| Works with any stdio MCP client | Protocol compatibility; installer has nine named integrations | README, install guide, hand-rolled stdio MCP transport | v0.6.0 | High with scope | "Any stdio MCP client can connect; the installer auto-configures nine named clients." | "Every MCP client is auto-detected" | Record proof from three representative clients |
| Local and no Ghostlight account | Runtime path, excluding user-configured destinations and normal browser traffic | README, ADR-0028, trust data flows | v0.6.0 | High | "No Ghostlight account or vendor runtime service is required." | "No network traffic of any kind" | Logged-out greenfield install |
| No product telemetry or update pings | Ghostlight runtime | README, ADR-0028, source and trust docs | v0.6.0 | High | "Ghostlight does not phone home or send product telemetry." | Claims about the browser, npm, GitHub, or website analytics | Recheck release tree before reuse |
| Visible and interruptible work | Managed tabs and supported visible effects | Hero, demo, extension tests, live Windows verification | v0.6.0 | High | "Browser actions stay visible, with scope and activity feedback and user controls." | "Every possible browser effect is visible" | Proof cohort observation |
| Free local core with separate organization governance license | Engine versus governance module | LICENSE, LICENSING, PRICING, ADR-0027 | v0.6.0 | High | "The local automation core is Apache-2.0 OR MIT; organization governance has separate terms." | "Everything is open source" or "free for every governed production use" | Legal copy review before broad launch |
| 25 browser tools with stable trained core schemas | Declared MCP surface | README tool table, schema fidelity test, ADR-0007 | v0.6.0 | High | "Ghostlight exposes 25 tools; the 13 trained schemas are preserved byte-for-byte." | "Models are trained on every additive tool" | Release schema test |
| Identity, domain, capability policy, and per-call audit | Governance-enabled local service | examples, Trust Center, tests, ADRs | v0.6.0 | High | "Optional governance authorizes by identity, host, and capability and records each call." | "Understands whether every click matches user intent" or "prevents prompt injection" | Managed proof user and audit sample |
| Protects ordinary tabs through managed scope | Extension and service managed-tab boundary | README, ADR-0066, extension tests, project memory | v0.6.0 | High | "Ghostlight works only in tabs it manages; ordinary tabs remain outside that surface." | "The tab group is a security sandbox" | Regression test at each extension release |
| Windows and Linux live verified | End-to-end live browser path | README and internal status | 2026-07-18 | Medium until reconciled | "Windows and Linux have live verification" only after public surfaces agree | Current website statement or unqualified cross-platform claim | Repeat and record clean greenfield acceptance |
| macOS package exists and CI passes | Build and automated tests, not live browser | release assets, README, CI | v0.6.0 | High | "macOS builds and passes CI; live-browser verification is still owed." | "macOS is end-to-end verified" | Visible macOS test |
| Release artifacts are verifiable | v0.6.0 release | GitHub assets, SHA256SUMS, SBOM, release workflow, attestations | 2026-07-15 | High | "Releases include checksums, a CycloneDX SBOM, and GitHub build-provenance attestations." | "Reproducible build" or independent audit | Verify a downloaded archive and attestation |
| Fast first use after installation | Observed author and one non-author path | interview feedback and live development sessions | 2026-07 | Medium | "Once installed, tested MCP clients recognized Ghostlight quickly." | Universal time-to-value or percentage | Time five to ten proof users |
| More suitable than headless tools for user-context work | Product fit comparison | README, comparison, competitor first-party docs | 2026-07-18 | Medium, inferential | "Choose Ghostlight when the job requires the user's visible authenticated Chromium context." | "Better than agent-browser/Playwright" | Publish mutual capability table with current versions |

## Non-claims and known limits

- Ghostlight does not infer semantic user intent from a browser action.
- Governance constrains capability and destination; it does not eliminate in-domain prompt
  injection.
- The browser tab group is visible organization, not the security boundary.
- Ghostlight is not a headless, isolated, stealth, cloud, or bulk automation runtime.
- The Chrome Web Store path is not public yet.
- macOS live-browser verification is not complete.
- There is no SOC 2, ISO certification, completed third-party penetration test, bounty program, or
  maintainer team.
- Organization governance is source-available under separate terms, not Apache-2.0 OR MIT.
- Donations are not enabled and would not buy access or influence.

## Claims requiring owner confirmation

- The exact Windows/Linux clean-install acceptance date and named test matrix.
- Whether to call the current state "pre-release," "public preview," or simply "pre-1.0."
- Which support lane should be primary for first-use feedback.
- Whether any non-author quote or workflow may be used publicly and with what attribution.
