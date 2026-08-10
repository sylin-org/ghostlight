# Ghostlight source licensing

Ghostlight's source tree has one deliberate open-core boundary. This page explains that boundary;
the exact terms remain in the license files and [`../../LICENSING.md`](../../LICENSING.md).

## Engine

Everything outside `crates/orchestrator/src/governance/` is offered under Apache-2.0 OR MIT. This
includes the typed bridge, orchestrator application and browser engine, MCP connector, browser
connector, policy-free extension, desktop adapter, and bundled workbench UI.

You may use either permissive license. Preserve notices and follow the chosen license's terms when
redistributing.

## Governance module

`crates/orchestrator/src/governance/` is offered under the Ghostlight Commercial License in
[`../licenses/LicenseRef-Ghostlight-Commercial.txt`](../licenses/LicenseRef-Ghostlight-Commercial.txt).
The source is visible, but it is not covered by the engine's Apache/MIT grant.

The standing free-use terms cover individuals, teams of up to five, evaluation, development,
all-open operation, and qualifying noncommercial use. Larger organizations using configured
governance operationally should consult [`../../PRICING.md`](../../PRICING.md) or contact
hello@sylin.org.

## Runtime behavior

Ghostlight 1.0 has no activation server, update ping, telemetry, license gate, or license-status
command. License state does not alter authority, audit, browser results, or continuity. An
installed copy keeps working without access to a Ghostlight-operated service.

Commercial terms and technical enforcement are intentionally separate. Do not add a network call,
behavior gate, or audit payload to enforce a source license.

## Contributions

Engine contributions use the Developer Certificate of Origin and inbound-equals-outbound terms.
Governance-module contributions require a contributor agreement so the copyright holder can keep
offering the commercial license. See [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md).
