# Ghostlight Support Policy

This page states the support channels, what to expect, and what support covers.

## Channels

Support is community support, because Ghostlight is free and open source
([ADR-0140](../adr/0140-fully-open-source-licensing.md)) and has no paid tier.

- [GitHub Issues](https://github.com/sylin-org/ghostlight/issues) for reproducible defects.
- [GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) for questions,
  installation and configuration help, policy authoring, and central-policy deployment.
- hello@sylin.org only for what cannot be public. Suspected security vulnerabilities always go
  through the private channel documented in [SECURITY.md](../../SECURITY.md), never a public lane,
  so that a report is handled under disclosure rules. The current private reporting address and
  required subject line are stated in SECURITY.md.

## What to expect

The maintainer answers best-effort, with no guaranteed acknowledgment or resolution window.
There are no paid response tiers to fall back on; there never will be a reason to buy one,
because there is nothing to buy. Where an issue turns out to be a defect in Ghostlight, it is
handled through the normal release process: fixes land on the latest tagged release.

## Scope

Community support covers the things you need to run Ghostlight: installation, configuration,
authoring and troubleshooting policy, and signed managed-policy deployment. It does not cover
custom development, nor the operation of your own MCP client or the model behind it, which are
outside Ghostlight and belong to you and your provider.

Last reviewed: 2026-08-25 against the 1.0 source candidate | Contact: hello@sylin.org
