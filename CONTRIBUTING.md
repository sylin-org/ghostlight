# Contributing, questions, and requests

Input is genuinely wanted -- questions, requests, and contributions have three lanes.

## Where to reach us

| Lane                                    | Use it for                                                                                                                                                                 |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [GitHub Issues](https://github.com/sylin-org/ghostlight/issues)           | Bugs, defects, anything reproducible.                                                                                                                                      |
| [GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) | Questions, ideas, feature requests, policy patterns, show-and-tell.                                                                                                        |
| hello@sylin.org                         | Anything that cannot be public: security reports (see [SECURITY.md](SECURITY.md)), licensing and founding-program matters, or a compliance team that cannot post publicly. |

Public lanes are preferred when possible: an answered question becomes documentation,
and a discussed request becomes a visible roadmap decision. Founding and enterprise
licensees get the response times in [PRICING.md](PRICING.md); everyone gets best-effort,
honestly.

## How requests are evaluated

Every request gets a disposition, with reasoning: accepted (and roughly when), deferred
(and what would change that), or declined (and why). The filter is the project's
recorded vision, not taste of the day:

- **User delight first; governance that never punishes the ungoverned.** All-open stays
  first-class. Features that make the free path worse to upsell the paid one are
  declined on principle.
- **The trained compatibility boundary.** The original 13 tool identities keep their names,
  parameter names, parameter types, enum values, ordering, requiredness, and structural schemas
  ([ADR-0007](docs/adr/0007-sacred-tool-surface.md),
  [ADR-0094](docs/adr/0094-agent-readable-tool-definitions.md)). Descriptions and metadata are
  guidance: they may improve deliberately when clearer purpose, tool choice, side effects, or
  recovery helps an agent. New tools and optional parameters grow additively through the
  capability registry ([ADR-0034](docs/adr/0034-capability-transport-registry.md) Decision 7).
  Requests to rename or reshape a trained identity are declined; guidance and additive-growth
  proposals are evaluated against the current registry and fidelity tests.
- **Never phone home.** Telemetry, activation servers, and update pings are permanently
  out ([ADR-0028](docs/adr/0028-tripwire-licensing-and-continuity-promise.md)).
- **Lean engine.** Fewer, more meaningful moving parts win over feature count. Scope
  exclusions in [ADR-0014](docs/adr/0014-v1-scope-exclusions.md) stand until an ADR
  supersedes them.

A request that fits the vision and comes with a concrete use case (especially from a
team running Ghostlight governed in anger) carries real weight; the quarterly founding
questionnaire exists precisely to harvest those.

## Contributing code

Contribution terms follow the open-core boundary (ADR-0027 Decision 5):

- **Engine** (everything outside `crates/core/src/governance/`): contributions are accepted under
  the [Developer Certificate of Origin](https://developercertificate.org/); sign off
  your commits (`git commit -s`). Inbound = outbound under Apache-2.0 OR MIT.
- **Governance module** (`crates/core/src/governance/`): contributions require a Contributor
  License Agreement (the module is distributed under a commercial license, and only the
  copyright holder can sell that). The CLA will be in place before the first outside
  governance PR is merged; if you want to contribute there, open a Discussion first and
  we will sort the paperwork.

Practical expectations for PRs: `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`, and `cargo test` green; ASCII source (escapes for anything else); match the
surrounding code's style; and one logical change per PR. For anything larger than a
small fix, open a Discussion or Issue first so nobody builds the wrong thing.

### The dev loop: seeing your changes live

Ghostlight runs one stack ([ADR-0065](docs/adr/0065-one-stack-endpoint-is-the-engine.md)): one
native host, one endpoint, one engine (whichever `ghostlight` service currently holds the
endpoint). A Rust change and a JavaScript-extension change refresh differently -- a Rust change is
one command (`scripts/dev-loop.ps1` swaps in your fresh build; editors and the browser reconnect on
their own), a JS change is a Reload at `chrome://extensions`. [docs/DEV-LOOP.md](docs/DEV-LOOP.md)
is the full how-to, starting with a "when code changes, do this" table.

### Running tests locally

The suite has two tiers ([ADR-0032](docs/adr/0032-test-at-seams-and-inject-config-sources.md),
[ADR-0051](docs/adr/0051-verification-topology-fewer-moving-parts.md)):

- **Fast, in-process** -- the unit tests and the in-process integration tests. Plain `cargo test`
  runs them; they need no processes and are the everyday gate.
- **End-to-end (spawn)** -- `cargo run -p ghostlight-lightbox -- run --all` launches the real
  binaries over the IPC boundary. Lightbox builds them in an isolated target dir that a live dev
  service never locks. Use `--reuse-cache` only on a clean worker after `cargo build --workspace`.

## What not to report publicly

Suspected vulnerabilities go to hello@sylin.org with "SECURITY" in the subject, per
[SECURITY.md](SECURITY.md). Everything else is fair game in the open.
