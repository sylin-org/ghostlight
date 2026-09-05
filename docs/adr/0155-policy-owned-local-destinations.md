# ADR-0155: Local browser destinations follow policy

- Status: Accepted (implemented in this revision)
- Date: 2026-09-05
- Amends: ADR-0121's protected-resource scope and ADR-0122 Decision 2's permanent ceilings
- Builds on: ADR-0060 and ADR-0121

## Context

Ghostlight refused localhost, its subdomains, loopback addresses, and link-local addresses before
evaluating any authored host policy. The refusal applied with no policy configured and could not
be lifted by the owner. It prevented agents from testing a local development server.

Local services can expose sensitive capabilities, but that concern does not justify a permanent
product ban on owner-authorized browser work. The existing rule classified URL names and literal
addresses, not resolved destinations or every page-generated network request. It was a bounded
navigation guard, not network isolation.

After reviewing the rationale, the owner directed removing the restriction completely: policies
exist to decide which destinations an agent may use.

## Decision

1. Remove the built-in localhost, loopback, and link-local address restrictions, including the
   IPv4-embedded IPv6 cases. No policy means these HTTP(S) destinations are available alongside
   remote sites. This includes link-local metadata addresses; no address-specific exception stays.
2. Apply the existing host grants, capability sets, request restrictions, authority-layer
   intersection, observe/enforce modes, and policy-defined sacred destinations to local browser
   work. Do not add a local-access setting, approval flow, special grant, or replacement blocklist.
3. Keep non-HTTP(S) schemes outside the browser automation contract. Credential handoff, runtime
   controls, ownership, stale-handle checks, and browser-local interlocks retain their contracts.
4. Make the effective-policy projection, model explanation, workbench, and active documentation
   state the same remaining boundaries. Preserve older ADRs and dated evidence as history.
5. The change belongs in the orchestrator governance owner. Existing execution and completion
   paths already carry it. No bridge, connector, extension, or protocol revision is needed.

## Consequences

- Agents can open, read, and operate a local development server without a policy exception.
- Deployments that need host restrictions must author them. Existing wildcard grants also apply
  to local destinations, and ordinary denials in observe mode remain observations.
- Policy host matching retains its existing grammar and URL-host semantics. This decision adds
  no DNS resolution, CIDR matching, origin/port rules, or network interception.
- The source is newer than the held 1.3.3 candidate. That frozen artifact does not acquire this
  change through documentation or a local development deployment.

## Acceptance

- Governance tests admit remote, localhost, localhost-subdomain, IPv4/IPv6 loopback, link-local,
  and IPv4-embedded IPv6 HTTP(S) addresses under all four capabilities without configured policy.
- Tests retain non-HTTP(S) refusals and prove local host/capability denials, request restrictions,
  observe behavior, and policy-defined never-touch destinations.
- Effective-authority and workbench tests show the remaining scheme boundary and any authored
  never-touch hosts, with no built-in local-address ceiling.
- The process journey opens and reads localhost and refuses an authored local-host denial before
  browser dispatch through the real MCP, orchestrator, and browser-connector executables.
- A live browser check opens and reads a disposable local HTTP fixture after the orchestrator
  deployment. It exercises no real local administrative or metadata service.
