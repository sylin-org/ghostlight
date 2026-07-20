Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z
Playbook version: 0.1.0

# Project profile

## Owner goal and non-goals

The owner is giving Ghostlight sustained focus for the next 90 days and wants to begin public
adoption work after the current release path is trustworthy. The near-term goal is not raw reach.
It is to help an unaffiliated MCP user install Ghostlight, reach a useful result in the browser,
understand the local and visible boundary, and describe the product accurately without founder
intervention.

The free local core should remain genuinely free. Organization-level governance is the paid
boundary. Donations, when enabled later, earn gratitude only and do not buy influence or special
treatment. Funding setup is intentionally deferred until receiving-account details are ready.

Deliberate non-goals are headless browsing, isolated browser profiles, cloud browser execution,
stealth scraping, bulk automation, and blindly unattended use. Ghostlight is built for a person
using their existing authenticated Chromium context and remaining responsible for it.

## Primary user and triggering problem

The first audience is an MCP practitioner using Codex, Claude Code, Cline, Cursor, VS Code, Zed,
OpenCode, Windsurf, or another stdio client. Their agent needs a site where the user is already
authenticated, but the client's own browser integration is absent, closed, or tied to another
client.

Secondary audiences are non-developer MCP users running documentation-heavy authenticated
workflows and security or platform teams that need local identity, domain and capability policy,
and an inspectable audit record.

The triggering problem is: "My MCP agent needs to work in the browser session I already use, and
I want to see, understand, and bound that work."

## First successful outcome

A stranger runs `npx -y ghostlight install`, completes the extension step, restarts or reloads the
MCP client, and asks for a read-only summary of a page in their current Chromium browser. They see
the managed tab group and visible action feedback, receive a compact useful answer, and can run
`ghostlight doctor` if any link is missing.

## Primary and secondary archetypes

- Primary: developer tool and MCP server.
- Secondary: Chromium extension, local infrastructure primitive, and governance or policy tool.
- Ecosystem objects: npm launcher, native GitHub release, MCP Registry entry, Chromium extension,
  Homebrew tap, package-manager manifests, RAWX capability specification, examples, demos, and
  Trust Center.

## Maturity, support surface, and maintainer capacity

Ghostlight is pre-1.0 at v0.6.0. Its trained browser schemas are treated as stable, while additive
tools and surrounding interfaces can still evolve. The latest public release was published on
2026-07-15. The repository is solo-maintained. Issues and Discussions are enabled, and email is
used for security, licensing, and private matters. Support is best effort outside separately
licensed commitments.

Windows and Linux are described as live verified in the README. macOS builds and tests in CI but
still lacks live-browser proof. The website-source contradiction found by this exercise is repaired
through the canonical `docs/public-status.json` fallback; deployment remains an external gate.

## Installation, use, and evidence

The fast path is the npm launcher plus a visible browser-extension installation. The extension is
currently loaded from a GitHub release while Chrome Web Store review remains open. The v0.6.0
release contains Windows, Linux, and macOS binaries, a store-ready extension ZIP, checksums, and a
CycloneDX SBOM. The release workflow supplies attestations.

Useful evidence is unusually strong for a young project:

- the README hero and live brief demo show visible browser work;
- 25 tools and their capability mappings are documented;
- 74 tracked test files and three GitHub workflows are present;
- architecture and product decisions are recorded in ADRs;
- the public Trust Center links claims to code, tests, and decisions;
- a candid comparison and decision aid explain fit and non-fit;
- the release pipeline covers npm, Homebrew, MCP Registry, extension assets, SBOM, checksums, and
  attestations.

## Constraints and open questions

- Broad publication waits for the Chrome Web Store path and a clean greenfield install proof.
- The website, README, registry, npm package, release, and store copy must agree on version and
  platform truth.
- The owner must choose the founder accounts and a launch window during which direct participation
  is possible.
- A small manual-extension proof cohort is allowed only when participants knowingly accept the
  pre-release installation friction.
- No external publication or outreach is authorized by this run.

## Evidence

- `../../README.md`
- `../../docs/guides/installation.md`
- `../../docs/STATUS.md`
- `../../docs/RELEASE.md`
- `../../docs/trust/README.md`
- `../../ROADMAP.md`
- `repository-inventory.json`
- https://github.com/sylin-org/ghostlight/releases/tag/v0.6.0
- https://sylin.org/ghostlight/
