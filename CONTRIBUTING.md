# Contributing, questions, and requests

I want the input. Questions, requests, and contributions have three lanes.

## Where to reach me

| Lane                                    | Use it for                                                                                                                                                                 |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [GitHub Issues](https://github.com/sylin-org/ghostlight/issues)           | Bugs, defects, anything reproducible.                                                                                                                                      |
| [GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) | Questions, ideas, feature requests, policy patterns, show-and-tell.                                                                                                        |
| hello@sylin.org                         | Anything that cannot be public: security reports (see [SECURITY.md](SECURITY.md)), licensing and founding-program matters, or a compliance team that cannot post publicly. |

Use a public lane where you can: an answered question becomes documentation, and a
discussed request becomes a visible roadmap decision. Founding and enterprise licensees
get the response times in [PRICING.md](PRICING.md). Everyone else gets my best effort.

## How requests are evaluated

Every request gets a disposition and the reasoning behind it: accepted (and roughly
when), deferred (and what would change that), or declined (and why). The filter is the
project's recorded vision, not taste of the day:

- **User delight first; governance that never punishes the ungoverned.** All-open stays
  first-class. Features that make the free path worse to upsell the paid one are
  declined on principle.
- **One meaningful 1.0 language.** Tool names, schemas, defaults, terminal truth, and recovery
  language are owned by the orchestrator and recorded in
  [docs/1.0/LANGUAGE.md](docs/1.0/LANGUAGE.md). Requests are evaluated as user jobs, not as
  client-specific aliases or low-level browser commands.
- **Stable fringes.** Product and journey evolution belongs in the orchestrator. The MCP
  connector, browser connector, shared bridge, and extension change only when their actual edge
  mechanisms or versioned contracts must change.
- **Never phone home.** Telemetry, activation servers, and update pings are permanently
  out ([ADR-0028](docs/adr/0028-tripwire-licensing-and-continuity-promise.md)).
- **Lean engine.** Fewer, more meaningful moving parts win over feature count. No generic event
  bus, actor framework, workflow engine, CQRS split, reflection registry, or microservice is added
  without a demonstrated boundary that the current modular monolith cannot express.

A request that fits the vision and comes with a concrete use case (especially from a
team running Ghostlight governed in anger) carries real weight; the quarterly founding
questionnaire exists precisely to harvest those.

## Contributing code

Contribution terms follow the open-core boundary (ADR-0027 Decision 5):

- **Engine** (everything outside `crates/orchestrator/src/governance/`): contributions are accepted under
  the [Developer Certificate of Origin](https://developercertificate.org/); sign off
  your commits (`git commit -s`). Inbound = outbound under Apache-2.0 OR MIT.
- **Governance module** (`crates/orchestrator/src/governance/`): contributions require a Contributor
  License Agreement (the module is distributed under a commercial license, and only the
  copyright holder can sell that). The CLA will be in place before the first outside
  governance PR is merged. If you want to contribute there, open a Discussion first and
  I will sort out the paperwork with you.

Practical expectations for PRs: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and
`npm test --prefix extension` green; ASCII source and documentation; match the surrounding code's
style; and protect each distinct contract once at its narrowest seam. For anything larger than a
small fix, open a Discussion or Issue first so nobody builds the wrong thing.

### The dev loop: seeing your changes live

Ghostlight 1.0 runs one orchestrator with independently reconnecting MCP and browser shores. An
orchestrator, workbench, or bundled-UI change rebuilds and restarts `ghostlight`; it does not
require a relay or extension edit. A browser mechanism or presentation change requires an explicit
Reload at `chrome://extensions`. [docs/DEV-LOOP.md](docs/DEV-LOOP.md) has the exact refresh matrix
and isolated process journey.

### Running tests locally

The suite has these useful layers:

- **In-process Rust contracts:** `cargo test --workspace`.
- **Policy-free extension mechanisms:** `npm test --prefix extension`.
- **Agent Plugin package contract:** `node tests/agent-plugin-contract.mjs`.
- **Agent Plugin installed-command topology:** after the isolated workspace build, run
  `node tests/agent-plugin-journey.mjs`.
- **Real process topology:** build into `.target-ghostlight-1.0`, then run
  `node tests/process-journey.mjs`. It keeps both relay processes alive through an orchestrator
  interruption and proves they renegotiate without replaying the interrupted effect. The journey
  looks for the executables in that directory; if you built somewhere else, say so with
  `GHOSTLIGHT_BIN_DIR`, or it will pass against stale binaries and tell you nothing.

Visible browser journeys and native tray/notification smoke tests remain release gates because a
green unit suite cannot substitute for the user's actual desktop and browser.

## What not to report publicly

Suspected vulnerabilities go to hello@sylin.org with "SECURITY" in the subject, per
[SECURITY.md](SECURITY.md). Everything else is fair game in the open.
