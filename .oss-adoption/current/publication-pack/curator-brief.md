Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z

# Curator brief draft

Target curator: a writer or maintainer covering MCP tools, browser automation, local-first
software, Rust developer tools, or agent governance after independent use exists

## Why this fits the curator's audience

Ghostlight addresses a specific gap between first-party browser integrations and automation
browsers: any stdio MCP client can use the user's existing logged-in Chromium profile while the
work remains local, visible, interruptible, and optionally governed.

## Verifiable project facts

- Project: Ghostlight by sylin.org.
- Current release: v0.6.0, published 2026-07-15.
- Runtime: native Rust service and relay plus a thin Chromium MV3 extension.
- Distribution: npm, GitHub Releases, official MCP Registry, Homebrew tap, and package-manager
  manifests.
- License: automation core Apache-2.0 OR MIT; organization governance source-available under
  separate terms.
- Account/telemetry: no Ghostlight account, activation service, product telemetry, or update ping.
- Tool surface: 25 tools; the 13 trained core schemas are preserved byte-for-byte.
- Release evidence: per-asset checksums, SHA256SUMS, CycloneDX SBOM, and GitHub provenance
  attestations.

## What changed and why it matters

The current experience combines model-efficient semantic actions and compact receipts with one
coherent human visual language: managed-scope border, page scan, typing and click feedback,
screenshot and recording signals, narration, and work badges. Window placement now reuses the
user's last-focused eligible Chromium window rather than spawning unnecessary windows.

The result is browser agency that can feel natural to an MCP model and understandable to a person
watching it.

## Evidence, license, demo, and maintainer links

- Repository: https://github.com/sylin-org/ghostlight
- Demo: https://sylin.org/ghostlight/demo/brief/
- Install: https://sylin.org/ghostlight/install.md
- Release: https://github.com/sylin-org/ghostlight/releases/tag/v0.6.0
- Decision aid: https://sylin.org/ghostlight/decision-aid/
- Trust Center: https://github.com/sylin-org/ghostlight/tree/main/docs/trust
- Comparison: https://github.com/sylin-org/ghostlight/blob/main/docs/COMPARISON.md
- Contact: hello@sylin.org

## Factual description available for adaptation

> Ghostlight is a local browser-automation MCP server for the Chromium profile a user already has
> open. It gives stdio MCP clients a shared set of compact browser tools, keeps work visible in
> managed tabs, and offers optional identity, domain, capability, and audit governance for
> organizations. The automation core is Apache-2.0 OR MIT and requires no Ghostlight account.

## Non-claims

- Not a headless, isolated, stealth, cloud, or bulk automation runtime.
- Not a semantic-intent or prompt-injection prevention system.
- macOS live-browser verification is not complete.
- Chrome Web Store acceptance must be confirmed before outreach.
- No independent penetration test or formal security certification exists yet.

## Contact authority

No outreach is authorized by this draft. Select each curator individually and contact only after
the store path, proof cohort, and public truth are current.
