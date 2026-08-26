# Ghostlight security overview

Ghostlight is a local governed bridge between an MCP client and the Chromium profile where the
user is already signed in. It operates no vendor runtime or customer-data store.

## Architecture and trust boundaries

The MCP connector terminates stdio MCP and sends typed work over authenticated loopback TCP to the
persistent `ghostlight` service. The service owns workspaces, language, policy, audit, execution,
and completion. The browser connector relays typed browser work between that service and the
extension through Chromium native messaging. The extension owns Chrome APIs, page-local DOM work,
browser durability, and content-free presentation. It owns no policy.

Every model-requested action crosses one executor, one immutable authority snapshot, one browser
port, and one completion path. Unknown effects are reported as unknown and are never replayed
automatically.

## Governance

Read, Action, Write, and Execute are independent capabilities. Ordered schema-3 grants bind a
complete requirement set to exact, suffix-wildcard, or universal host patterns. Managed policy,
local policy, and request restrictions intersect. Protected loopback and link-local destinations
and configured sacred domains remain hard ceilings.

Observe mode records ordinary would-deny decisions without blocking them; enforce mode blocks.
Every enforced policy denial receives deterministic attribution to authority, tier, grant, and
rule. Repeated denials pause the affected workspace for human attention. Runtime pause, resume,
resume quietly, and end-session remain locally controlled.

Audit is a local append-only JSONL flight recorder. It carries the complete RAWX requirement set,
decision attribution, managed sequence, governed host, and bounded outcome measurements. It does
not contain credentials, arbitrary page text, full URLs, typed values, scripts, screenshots, or
recordings. SIEM delivery uses the endpoint's existing file collector.

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
The release unit includes platform packages, portable archives, raw binaries, the extension,
component SBOMs, the mandatory npm launcher, and the MCPB package. Ghostlight has no Windows
Authenticode certificate; that is a disclosed distribution fact, not a hidden readiness claim.

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

Last reviewed: 2026-08-14 against the 1.0 source candidate | Contact: hello@sylin.org
