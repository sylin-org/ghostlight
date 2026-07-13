# Ghostlight Security Overview

This is the architecture and security whitepaper for a reviewer who has read the FAQ and
wants the mechanism. The through-line is that Ghostlight runs entirely on your
infrastructure, so its security model is about the integrity of local execution and of what
the vendor ships, not about protecting a vendor-side data store.

## Architecture and trust boundaries

Ghostlight runs as a persistent local service, a small relay executable in two roles, and a browser
extension. Your MCP client talks over stdio to the agent relay, which forwards over owner-scoped
local IPC to the service. Chromium talks through the browser relay using native messaging. Inside
the browser, the extension drives pages over the DevTools protocol. Everything runs on the
endpoint. The MCP client and the model behind it are yours: Ghostlight is the governed bridge
between that client and a browser session you are already signed into, and it never relocates the
session to a vendor-controlled or fresh-profile browser. The trust boundaries that matter are the
local protocol hops and the policy decision point inside the service that every action passes
through.

## The governance layer

All policy lives in the binary; the extension holds mechanism but makes no access decision.
Every tool call is classified by capability (read, action, write, execute) and evaluated
against the active policy before it runs. Identity-bound grants scope what an actor may do to
which hosts, with host polarity distinguishing where an action is allowed from where it is
refused. Sacred never-touch domains are refused unconditionally, even against an
organization's own policy, so a misconfiguration or a hostile prompt cannot reach them.
Observe and enforce modes let you run automation in a watching posture before granting real
actions, and under observe a loaded policy runs in shadow, recording would-deny events
without blocking; a take-the-wheel pause returns control to the human mid-run; and a panic
kill switch stops the session immediately. Every decision, permit or deny, produces an audit
record under the default configuration: audit is on by default and is disabled only by
explicit configuration.

Identity in this model is declared, not federated: the audit record's identity block carries
the principal and how it was resolved, from the active policy (local configuration or your
MDM-provisioned managed policy). There is no OIDC, SAML, or LDAP integration; directory
integration is a deliberate exclusion, documented in the spec, so attribution is exactly as
strong as the configuration your organization provisions.

## Cryptography

Licenses and managed policy bundles are signed with a composite Ed25519 + ML-DSA-65
signature on production key generations, pairing a classical curve with a post-quantum
lattice scheme so a break in either algorithm alone does not forge a production-signed
bundle. (Ed25519-only signing remains for the public development and evaluation-grade key
generations.) Trust is anchored in the signature, not in the transport: a bundle is
authentic because it is validly signed, so the policy source can be any host the fleet
already reaches. A monotonic publish
sequence inside each signed bundle prevents rollback, so a stale or malicious mirror serving
an older but validly signed policy cannot downgrade a device to a more permissive past
version. The local policy cache is itself signed and its signature is verified on load from
cache, not only on fetch, which closes the gap where an attacker could swap the on-disk
policy to weaken enforcement.

## Vendor-side security

The assets worth attacking on the vendor side are the source repository, the release
pipeline, and the signing keys for licenses and policy bundles, and those are the focus of
vendor-side security. The license- and policy-signing keys live offline on an air-gapped
machine and never enter CI or any online system; the founder batch-signs license artifacts
there. Release binaries are protected by a different mechanism: per-file SHA-256 checksums
and keyless build-provenance attestations, generated in the release pipeline, tie each
artifact to the exact source commit and workflow run that produced it. Source and pipeline
access is a single maintainer account with multi-factor authentication, no shared accounts,
and no third-party write access. A change reaches a release only through a disciplined path:
architecture decision records for design, CI gates (formatting, linting, tests, dependency
audit, and the lightbox scenario runner) for correctness, and checksummed,
provenance-attested artifacts for distribution. There is no customer-data store on the
vendor side, so the security effort concentrates on the integrity of what we ship.

## Incident response

Because no customer data reaches the vendor, there is no customer-data breach to report; the
consequential incident on the vendor side is a compromise of what we ship, namely the build,
the signing keys, or the update channel. For that class of event we commit to publishing a
security advisory, naming the affected versions and the remediation, within 3 business days of
confirming a vendor-side compromise. Advisories are published as GitHub Security Advisories
on the repository and named in release notes; watching the repository's releases is the
subscription path, and because Ghostlight never phones home, this pull channel is
deliberately the only channel. Suspected vulnerabilities reach us through the private
channel documented in SECURITY.md. We also make a standing transparency commitment: any
third-party security audit of Ghostlight will be published in full, including findings.

See [SECURITY.md](../../SECURITY.md), [supply-chain.md](supply-chain.md), and
[data-flows.md](data-flows.md).

Last reviewed: 2026-07-10 against v0.5.6 | Contact: support@sylin.org
