# Ghostlight security overview

Ghostlight is a local governed bridge between an MCP client and the Chromium profile where the
user is already signed in. It operates no vendor runtime or customer-data store.

## Architecture and trust boundaries

The MCP relay terminates stdio MCP and sends typed work over a per-user named pipe or Unix
socket, which only the same user can open, to the persistent `ghostlight-service`. Both relays are
the `ghostlight` edge. The service owns workspaces, language, policy, audit, execution,
and completion. The browser relay carries typed browser work between that service and the
extension through Chromium native messaging. The extension owns Chrome APIs, page-local DOM work,
browser durability, and content-free presentation. It owns no policy.

Every model-requested action crosses one executor, one immutable authority snapshot, one browser
port, and one completion path. Unknown effects are reported as unknown and are never replayed
automatically.

## Tabs across restarts

A browser hands tab ids out again after it restarts, so a tab Ghostlight remembers across a
browser or extension reconnect, or across a service restart, is a hint until the browser confirms
it.
It is kept only when the browser shows a tab with that id and either the extension reports the
same browser run it was made in, or the tab shows the page Ghostlight last saw there. Otherwise
the agent's handle names no open tab, and the agent opens a fresh one. The one case this cannot
tell apart is a tab of the person's that received the same id after a browser restart and shows
the same address, such as a restored copy of the agent's page; that tab is bound.

An MCP session's resume token is unsigned and opaque, written by the service and held by the edge
in memory. It is a hint, never authority. A program running as the same user can write one, but it
binds only a tab whose id shows the page it names, or, carrying the browser's current run id, a tab
of that run. Such a program can already act as the user, and it could open the same site in a new
tab with the person's cookies; what a forged token adds is an existing tab's in-page state.

A client reuses only tabs that sessions of the agent it claims to be kept open. That is a courtesy
between well-behaved clients, not authority: a same-user program that claims another agent's name
can adopt that agent's kept tabs. The adopt loads a page in the kept tab, so the origin's
sessionStorage and the tab's history cross with it; cookies are shared across the profile anyway.
A client that claims no name reuses nothing. Ghostlight makes no promise of tab ownership against
programs running as the same user.

## Window on loopback

The service serves the Ghostlight window's pages over HTTP on one loopback port. It binds
127.0.0.1 only and answers only when the `Host` header is exactly that address and port, which
defeats DNS rebinding. It sends no CORS header, refuses WebSocket upgrades and embedding, and
needs a JSON body and a same-origin request for every change. The page carries no MCP and starts
no browser work, and governance refuses agent navigation to its address whatever the policy says.

Changes need a bearer: 256 random bits made at each service start, which the service hands only
to the Ghostlight window, over the per-user pipe that checks the peer's user. It never appears
in a URL, a log, an audit line or a response. A request without it is a watcher's, and a wrong
bearer is refused rather than treated as one.

"Others can watch" is on by default. While it is on, any program or account on this computer can
open the address in a browser and see what the window's glance shows: readiness, which agents and
browsers are connected, what agents are doing and on which hosts, the client names, and the policy
rules and settings. The never-touch list and excluded hosts show only as counts. A watcher can
search, check a policy against history, re-check clients, ask to bring the Ghostlight window
forward and pause agents; every other change needs the window. A person turns watching off in
the window's Settings > Sharing, and then a browser tab shows only the About page.
`ghostlight status` shows the address and whether watching is on.

## Governance

Read, Action, Write, and Execute are independent capabilities. Ordered schema-4 grants bind a
complete requirement set to exact, suffix-wildcard, or universal host patterns. Managed and
local tiers intersect. A request cannot narrow its own authority: request restrictions are refused,
and audit records carry no restriction fields. Localhost, loopback, and link-local HTTP(S)
destinations follow those policies, with one exception: agents can never open or read
Ghostlight's own workbench address. That address and non-HTTP(S) schemes are Ghostlight's own
ceiling, credited to no policy layer. Configured sacred domains are never-touch for every agent,
and each refusal of one is the decision of the layer that listed it.

A policy can turn off work from MCP agents or from the command line, and every request on a
channel that is off is refused before any browser work. The channel is declared by Ghostlight's
own programs when they connect, and any program running as the same user could declare another,
so a channel switch is a guardrail for well-behaved agents, not a barrier against hostile software
on the same account.

Every policy denial receives deterministic attribution to authority, tier and rule, and to the
grant when a grant decided it; a never-touch refusal names the layer that listed the host.
Repeated refusals do not pause a session; later permitted work remains available. A tab a refused
landing left on the refused page is held: it refuses reading and acting until the agent navigates
it to an allowed page or closes it. A person's Pause and End session remain authoritative. Controls
are rechecked at browser transmission, while already dispatched work retains its actual effect
evidence.

Fill and typing require the caller to acknowledge explicit user authorization for credential
entry through `user_authorized_credentials`, false by default. Existing conversation authorization
is sufficient. This per-request acknowledgement is not independently verified proof of intent
and cannot override policy or human controls. Without it, the request returns guidance without
creating a session hold. Credential-like metadata on a file control does not block explicitly
requested file uploads or recording saves. In-page feedback is passive and cannot intercept user
input. [ADR-0185](../adr/0185-explicit-user-control-and-passive-browser-feedback.md) records this
source behavior; [STATUS](../STATUS.md) distinguishes verification and deployment evidence.

Audit is a local append-only JSONL flight recorder. It carries the complete RAWX requirement set,
decision attribution, managed sequence, governed host, and bounded outcome measurements. It does not
contain credentials, arbitrary page text, full URLs, typed values, scripts, screenshots, or
recordings, including when an action raises a dialog or its expected condition falls short, and it
keeps no client's own words or another session's name. SIEM delivery uses the endpoint's existing
file collector.

## Signed managed policy

Managed policy activates only when an administrator provisions `managed.json`. The bootstrap
names a customer-owned file or HTTPS source and customer-owned public verification keys. Ed25519
is required. If an ML-DSA-65 key is present, both signature legs must verify. Ghostlight embeds no
policy trust anchor and runs no policy service.

Monotonic publish sequence prevents rollback. Verified bundles are cached atomically and verified
again on read. Unreachable, malformed, unsigned, bad-signature, or rollback updates retain the
last verified policy. A configured cold start with no valid cache or source fails closed. Signed
policy has no clock expiry; staleness stays visible in the Policy Passport instead of erasing
protection.

The HTTP client uses rustls with the pure-Rust ring provider, refuses redirects, bounds response
size, and supports an organization CA pin. No managed bootstrap means no policy network traffic.

## Release integrity

Release artifacts are bound by per-file SHA-256 and keyless GitHub build-provenance attestations.
A 2.0 release carries the three raw binaries for Windows and for Linux (`ghostlight`,
`ghostlight-service` and `ghostlight-desktop`), the WinGet archive, the extension package, one
SBOM per workspace crate, the mandatory npm launcher tarball, the Claude Desktop MCPB,
`release-candidate.json` and `SHA256SUMS`. [supply-chain.md](supply-chain.md) shows how to check
them. Ghostlight has no Windows Authenticode certificate; that is a disclosed distribution fact,
not a hidden readiness claim.

CI gates formatting, warnings-denied Rust linting, Rust and extension tests, process journeys,
dependency policy, and release metadata. Organization policy signing keys belong to the customer
and never enter Ghostlight source, releases, or CI.

## Incident response

The consequential vendor-side incident is compromise of source, build pipeline, or distribution.
Confirmed issues are handled through the private route in [SECURITY.md](../../SECURITY.md) and a
public GitHub Security Advisory with affected versions and remediation. The timing target is
best-effort for a solo maintainer, not a contractual notification window. Any third-party security
audit of Ghostlight will be published in full, including findings.

See [data-flows.md](data-flows.md) and [supply-chain.md](supply-chain.md).

Last reviewed: 2026-10-19 against the current source; local-destination policy follows ADR-0155,
and tabs across restarts follow ADR-0203.
Contact: hello@sylin.org
