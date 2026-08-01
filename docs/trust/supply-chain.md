# Ghostlight Supply Chain

This page is the supply-chain evidence a reviewer needs: how releases are built and signed,
what a software bill of materials covers, the dependency posture, and how changes reach a
release.

## Releases

Every release publishes verifiable artifacts. Each payload artifact is covered by the release's
SHA-256 manifests so you can confirm you received exactly what was published, and each release
includes keyless Sigstore build-provenance attestations that tie the artifacts back to the
exact source commit and workflow run that produced them (attestation coverage spans every
release asset from 2026-07 onward; earlier releases attest the packaged archives). Releases
also carry a canonical `SHA256SUMS` manifest. A read-only assembly job creates the complete
release bundle, including the SBOM; the privileged publisher can only download that bundle,
verify its exact file list and hashes, attest it, and create the release.
are distributed today through GitHub Releases, npm, the MCP Registry, and the Sylin Homebrew tap,
all resolving to artifacts from the same tagged source. Scoop and winget manifests are prepared in
the repository but are not public distribution channels until their packages ship. Fixes land on
the latest tagged release; pre-1.0 there are no
backport branches (see [SECURITY.md](../../SECURITY.md)). The release pipeline is defined in
[.github/workflows/release.yml](../../.github/workflows/release.yml).

## Verify a release

Both checks run against any release asset, straight from a shell:

    sha256sum -c ghostlight-v<version>-<target>.tar.gz.sha256
    gh attestation verify ghostlight-v<version>-<target>.tar.gz --repo sylin-org/ghostlight

The first prints `OK` when the archive matches its published checksum (on Windows,
`Get-FileHash` computes the same SHA-256). The second, using the GitHub CLI, proves the
artifact was built by this repository's release workflow and prints the source commit and
workflow run that produced it.

## SBOM

Each release includes a CycloneDX software bill of materials generated in the release
pipeline (introduced 2026-07; releases through v0.5.4 predate it and carry no SBOM asset).
It is published as a release asset named `ghostlight-v<version>-sbom.cyclonedx.json`,
alongside the binaries and their checksums, so you can ingest the exact dependency set of
the `ghostlight` package for a given release into your own supply-chain tooling.

## Dependencies

The dependency tree is kept deliberately lean, favoring fewer, well-understood crates over
broad transitive graphs. The signature cryptography is pure Rust. The network stack used for
managed policy fetch (a Rust HTTP client over rustls, whose default cryptographic provider
includes a C library) sits behind a feature gate that is on by default in shipped binaries;
building with `--no-default-features` yields a pure-Rust, air-gap-only binary with no HTTP
or TLS stack at all. No formal export classification (ECCN) has been made; the cryptographic
source is public in this repository. As a dated data point, the npm
package scored 100/100 on all axes on Socket.dev at publication (2026-07); see
[the npm package](https://www.npmjs.com/package/ghostlight). That is a snapshot of that
moment, not a standing guarantee, and the SBOM above is the authoritative, per-release
dependency record.

## Build and change management

Changes reach a release through a disciplined path. Design decisions are recorded as
architecture decision records before they are implemented. CI gates every change on
formatting, linting, the test suite, a dependency audit, and the lightbox scenario runner,
so a regression in governance behavior fails the build. Development flows through a
trunk-and-release branch model. License- and policy-signing keys are held offline on an
air-gapped machine and never enter CI or any online system, so a compromise of the build
infrastructure cannot forge a license or a policy bundle. Release binaries are protected
differently: checksums and build-provenance attestations tie each artifact to the source
commit and workflow run that produced it. Compromise of the release pipeline itself is the
residual supply-chain risk for binaries; it is the scenario covered by the best-effort advisory
target in [SECURITY.md](../../SECURITY.md).

See [security-overview.md](security-overview.md) for the vendor-side security posture.

Last reviewed: 2026-07-14 against v0.7.2 | Contact: support@sylin.org
