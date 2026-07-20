# Project governance

Ghostlight is a maintainer-led open-core project. The current maintainer is `lbotinelly`, who makes
release, roadmap, security, licensing, and final merge decisions. There is no foundation, steering
committee, or claim of community consensus.

## How decisions are made

- Bugs and small corrections can begin as Issues or pull requests.
- Ideas, feature requests, workflows, and policy patterns begin in Discussions when practical.
- Material architecture or product-boundary changes require an ADR. Accepted ADRs are never
  silently rewritten; a later decision amends or supersedes them.
- Requests receive a reasoned disposition: accepted, deferred with a re-evaluation condition, or
  declined against the recorded product constraints.
- The byte-stable trained schemas, no-phone-home promise, policy-free extension, clean-room rule,
  and first-class unrestricted engine are standing boundaries.

## Contributions and licensing

Contributions to the open engine use the DCO and the repository's Apache-2.0 OR MIT terms.
Contributions to `crates/core/src/governance/` require a CLA because that module has a separate
commercial license. Read [CONTRIBUTING.md](CONTRIBUTING.md) before starting substantial work.

## Accountability and continuity

Architectural reasoning lives in [docs/adr/](docs/adr/). Current state lives in
[docs/STATUS.md](docs/STATUS.md). Release history lives in [CHANGELOG.md](CHANGELOG.md), and
[MAINTENANCE.md](MAINTENANCE.md) names the solo-maintainer and publisher risks plainly. Security
reports follow [SECURITY.md](SECURITY.md).

This structure may evolve if sustained outside participation creates a real need for shared
authority. It will not be described as shared governance before that authority exists.
