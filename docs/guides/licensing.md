# Ghostlight source licensing

Ghostlight's source tree carries one license: Apache-2.0 OR MIT, at your option, for the whole
product. This page explains what that covers; the exact terms remain in the license files and
[`../../LICENSING.md`](../../LICENSING.md).

## The whole tree

Everything -- the typed bridge, orchestrator application, browser engine, governance module,
MCP connector, browser connector, policy-free extension, desktop adapter, and bundled workbench
UI -- is offered under Apache-2.0 OR MIT. There is no separately licensed module.

You may use either permissive license. Preserve notices and follow the chosen license's terms
when redistributing.

The former open-core boundary (engine permissive, governance module commercially licensed) was
withdrawn by [ADR-0140](../adr/0140-fully-open-source-licensing.md) on 2026-08-25. The
`crates/orchestrator/src/governance/` directory remains an architecture boundary -- it is where
policy authority, admission decisions, runtime controls, and minimized audit live -- but it is no
longer a license boundary.

## Runtime behavior

Ghostlight 1.0 has no activation server, update ping, telemetry, license gate, or license-status
command. License state does not alter authority, audit, browser results, or continuity. An
installed copy keeps working without access to a Ghostlight-operated service.

Do not add a network call or behavior gate to enforce anything about the source: there is
nothing to enforce, and the never-phone-home rule ([ADR-0028](../adr/0028-tripwire-licensing-and-continuity-promise.md))
is permanent.

## Contributions

Contributions to any part of the repository use the Developer Certificate of Origin and
inbound-equals-outbound terms under Apache-2.0 OR MIT; sign off your commits (`git commit -s`).
See [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md).
