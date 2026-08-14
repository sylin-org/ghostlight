# Mapping RAWX governance to the OWASP agentic threat taxonomy

Status: informational mapping, first published 2026-07-07 (ADR-0041 Decision 1: meet external
vocabularies with bridges). Licensed Apache-2.0 OR MIT like everything in open-spec/.

Agent-governance evaluations increasingly use the OWASP Agentic Security Initiative's threat
taxonomy as their checklist (Microsoft's open-source Agent Governance Toolkit, for one, maps
itself against all ten of its risks). This note maps the [RAWX capability
model](rawx-capability-model.md) and the governance overlay of its reference implementation
(Ghostlight) onto that taxonomy's themes, plus the 2026 University of Washington findings on
agentic-browser security. The OWASP taxonomy's exact naming evolves; check the initiative's
current publication for canonical wording. This mapping is written honestly in both
directions: it also states plainly what a governance overlay does NOT mitigate.

## The threat themes and what governs them

**Tool misuse / excessive agency.** The core RAWX case. Every action is classified by
an intrinsic capability set drawn from read / action / write / execute, and grants set per-host
allowances. An agent on a read-only grant cannot type, submit, or execute script anywhere the
grant applies, no matter what it was talked into. Advertisement filtering removes ungranted
tools from the agent's view entirely.

**Goal hijacking / intent manipulation (prompt injection).** A governance layer cannot stop a
model from BELIEVING injected instructions; that battle happens inside the model and its
harness. What it does is cap the blast radius: an injected agent still cannot exceed its
capability allowance, cross host polarity, touch sacred never-touch domains, or act while a
take-the-wheel hold is in effect. Injection turns from "attacker controls the browser" into
"attacker controls a session confined to what the human granted."

**Privilege compromise / identity abuse.** Grants are identity-described and host-scoped. Audit
carries opaque workspace and authority ids plus channel, tier, grant, and decision attribution.
Ghostlight does not claim federated identity: policy identity is only as strong as the endpoint
configuration that supplied it.

**Cross-origin data movement (the UW findings; taxonomy: data exfiltration / cascading
effects).** The 2026 UW study showed four of seven agentic browsers create same-origin-policy
bypass conditions: content from one origin steering actions or data on another. Host-polarity
grants confine which hosts can receive each capability set. Ghostlight does not implement
cross-call data-flow provenance or content inspection. Data the model carries in its context
between calls is out of band for this local mechanism.

**Repudiation / untraceability.** The structured audit stream is the spine: every call --
allowed, denied, shadow-denied, or held -- produces one record with opaque authority and workspace
ids, tool, complete capability set, governed host, grant or denial id, timing, and managed publish
sequence. Denials carry stable ids the security team can reference.

**Human-in-the-loop bypass / overwhelming.** The take-the-wheel pause and the panic kill
switch are user gestures enforced ahead of all policy machinery; a held call never queues or
replays. Write-class actions can require explicit grants rather than per-call nagging, which
is what makes the human checkpoint sustainable instead of click-through.

**Unexpected code execution.** Explicit page JavaScript is the independent `execute` capability,
grantable separately from everything else and deniable per host. Execute is not a tier and does
not imply Read, Action, or Write.

**Memory poisoning.** Out of scope: Ghostlight holds no cross-session agent memory.

**Supply chain.** Out of scope for the governance layer itself; addressed at the project
level (checked release unit, checksums, and build provenance) rather than by RAWX.

## What this layer does not do

No content inspection or DLP; no protection against the model being deceived within a granted
scope (a write grant misused on the granted host is within policy); no attestation of
out-of-band data flows through model context; no substitute for browser-level origin
isolation, which is the browser vendor's layer. Governance bounds agency and makes actions
attributable; it does not make the agent smart or the page trustworthy.

## Sources

- OWASP Agentic Security Initiative (threats and mitigations): https://owasp.org
- Microsoft Agent Governance Toolkit: https://github.com/microsoft/agent-governance-toolkit
- UW agentic-browser study coverage (2026-07): https://www.technology.org/2026/07/03/some-agentic-ai-browsers-come-with-major-cybersecurity-risks-uw-study-finds/
- Ghostlight ADRs 0013, 0018, 0022, and 0121 (docs/adr/) for the mechanisms named here.
