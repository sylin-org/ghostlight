# Ghostlight Security Overview

This is the architecture and security whitepaper for a reviewer who has read the FAQ and
wants the mechanism. The through-line is that Ghostlight runs entirely on your
infrastructure, so its security model is about the integrity of local execution and of what
the vendor ships, not about protecting a vendor-side data store.

## Architecture and trust boundaries

Ghostlight is three processes across two protocol boundaries: your MCP client talks to the
Ghostlight binary over stdio; the binary talks to the browser extension over Chromium native
messaging; the extension drives the browser over the DevTools protocol. Everything runs on
the endpoint. The MCP client and the model behind it are yours: Ghostlight is the governed
bridge between that client and a browser session you are already signed into, and it never
relocates the session to a vendor-controlled or fresh-profile browser. The trust boundaries
that matter are the two protocol hops, both local, and the policy decision point inside the
binary that every action passes through.

## The governance layer

All policy lives in the binary; the extension holds mechanism but makes no access decision.
Every tool call is classified by capability (read, action, write, execute) and evaluated
against the active policy before it runs. Identity-bound grants scope what an actor may do to
which hosts, with host polarity distinguishing where an action is allowed from where it is
refused. Sacred never-touch domains are refused unconditionally, even against an
organization's own policy, so a misconfiguration or a hostile prompt cannot reach them.
Observe, shadow, and enforce modes let you run automation in a watching or simulated posture
before granting real actions; a take-the-wheel pause returns control to the human mid-run;
and a panic kill switch stops the session immediately. Every decision, permit or deny,
produces an audit record.

## Cryptography

Licenses and managed policy bundles are signed with a composite Ed25519 + ML-DSA-65
signature, pairing a classical curve with a post-quantum lattice scheme so a break in either
alone does not forge a bundle. Trust is anchored in the signature, not in the transport: a
bundle is authentic because it is validly signed, so the policy source can be any host the
fleet already reaches without the transport becoming the trust anchor. A monotonic publish
sequence inside each signed bundle prevents rollback, so a stale or malicious mirror serving
an older but validly signed policy cannot downgrade a device to a more permissive past
version. The local policy cache is itself signed and its signature is verified on load from
cache, not only on fetch, which closes the back-door where an attacker swaps the on-disk
policy to weaken enforcement. The signature is the integrity control here; a device enforces
only a policy whose signature checks out.

## Vendor-side security

The assets worth attacking on the vendor side are the source repository, the release signing
keys, and the release pipeline, and those are the focus of vendor-side security. The signing
keys live offline on an air-gapped machine and never enter CI or any online system; the
founder batch-signs releases there. Source and pipeline access uses multi-factor
authentication and least privilege. Change management is the discipline that reaches a
release: architecture decision records for design, CI gates (formatting, linting, tests, and
the lightbox scenario runner) for correctness, and signed artifacts for distribution. There
is no customer-data store on the vendor side, so this is where the security budget goes.

## Incident response

Because no customer data reaches the vendor, there is no customer-data breach to report; the
consequential incident on the vendor side is a compromise of what we ship, namely the build,
the signing keys, or the update channel. For that class of event we commit to publishing a
security advisory, naming the affected versions and the remediation, within 3 business days of
confirming a vendor-side compromise. Suspected vulnerabilities reach us through the private
channel documented in SECURITY.md. We also make a standing transparency commitment: any
third-party security audit of Ghostlight will be published in full, including findings.

See [SECURITY.md](../../SECURITY.md), [supply-chain.md](supply-chain.md), and
[data-flows.md](data-flows.md).

Last reviewed: 2026-07-10 against v0.5.4 | Contact: support@sylin.org
