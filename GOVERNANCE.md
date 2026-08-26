# Project governance

Ghostlight is a maintainer-led free and open-source project. I maintain it (`lbotinelly`), and the release,
roadmap, security, licensing, and final merge calls are mine. There is no foundation, no steering
committee, and no community consensus behind those decisions -- one person makes them, and you
should know that before you build on it.

## How decisions are made

- Bugs and small corrections can begin as Issues or pull requests.
- Ideas, feature requests, workflows, and policy patterns begin in Discussions when practical.
- Material architecture or product-boundary changes require an ADR. Accepted ADRs are never
  silently rewritten; a later decision amends or supersedes them.
- Requests receive a reasoned disposition: accepted, deferred with a re-evaluation condition, or
  declined against the recorded product constraints.
- The complete orchestrator-owned 1.0 language, no-phone-home promise, stable thin connectors,
  policy-free extension, clean-room rule, and first-class unrestricted engine are standing
  boundaries.

## Contributions and licensing

The whole repository is Apache-2.0 OR MIT ([ADR-0140](docs/adr/0140-fully-open-source-licensing.md)).
Contributions everywhere use the DCO and the repository's Apache-2.0 OR MIT terms.
Read [CONTRIBUTING.md](CONTRIBUTING.md) before starting substantial work.

## Accountability and continuity

Every architectural decision and its reasoning live in [docs/adr/](docs/adr/). Where the project
stands right now lives in [docs/STATUS.md](docs/STATUS.md), release history in
[CHANGELOG.md](CHANGELOG.md), and [MAINTENANCE.md](MAINTENANCE.md) names what depends on me alone.
Security reports follow [SECURITY.md](SECURITY.md).

This structure can change if sustained outside participation creates a real need for shared
authority. Until that authority exists, I will not call it shared governance.
