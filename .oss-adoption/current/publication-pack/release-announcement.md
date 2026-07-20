Status: complete
Run ID: 20260718T214944Z
Project: Ghostlight
Created: 2026-07-18T21:49:44Z

# Release announcement draft

Release: use for the first store-backed release after v0.6.0, not as a retroactive v0.6.0 post

## Candidate headline

> Ghostlight [version]: install from the Chrome Web Store, then let your MCP client use the browser
> you already have open

## What shipped

- A reviewed Chrome Web Store package as the default extension path.
- The current native service and relay for Windows, Linux, and macOS release targets.
- One-command MCP client and browser registration through the npm launcher.
- Visible managed-tab scope and unified action feedback.
- Compact browser tools for reading, acting, forms, files, scripts, waits, recording, and audit.
- Checksums, CycloneDX SBOM, and build-provenance attestations for release artifacts.

Replace this list with the exact changelog before publication.

## Who benefits and how

MCP users can connect Codex, Claude Code, Cline, Cursor, OpenCode, VS Code, Zed, Windsurf, or
another stdio client to their existing authenticated Chromium session without creating a
Ghostlight account or moving work into a cloud browser.

Organizations can optionally apply local identity, domain, and capability policy and retain a
structured audit record. The unrestricted free path remains first-class.

## Compatibility and migration impact

- Chromium browsers version 116 or newer.
- Windows and Linux live verification must be recorded for this exact release.
- macOS scope must state whether live verification completed or remains owed.
- Existing manual extension installs should receive explicit instructions for switching to the
  store package without losing the native host connection.
- Stable trained schemas remain unchanged; additive tool and implementation changes must be named
  from the actual changelog.

## How to try it

```text
npx -y ghostlight install
```

Follow the extension walkthrough, restart the MCP client, then ask:

> In my current browser, summarize the active page and tell me which tab you used. Do not click or
> change anything.

Use `npx -y ghostlight doctor` if the browser or client connection is incomplete.

## Limitations and known issues

- Ghostlight is pre-1.0 and Chromium-only in v1.
- It is for visible authenticated user-context work, not headless, stealth, isolated, cloud, or
  bulk automation.
- Governance constrains capability and destination but does not infer user intent.
- Support is solo-maintainer and best effort outside separately licensed commitments.
- Fill this section from the exact release acceptance record before publishing.

## Acknowledgements and support route

Thank proof users, issue reporters, and contributors only with their permission. Use GitHub Issues
for reproducible defects, Discussions for questions and workflows, and the private SECURITY.md
route for suspected vulnerabilities.

This draft is not authorization to publish a release or announcement.
