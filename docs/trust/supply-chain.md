# Ghostlight Supply Chain

This page is the supply-chain evidence a reviewer needs: how releases are built and signed,
what a software bill of materials covers, the dependency posture, and how changes reach a
release.

## Releases

Every release publishes signed, verifiable artifacts. Each downloadable carries a per-file
SHA-256 checksum so you can confirm you received exactly what was published, and each release
includes build-provenance attestations that tie the artifacts back to the workflow that
produced them. Releases are distributed across the package managers Ghostlight supports
(GitHub releases, npm, and the platform package managers), all built from the same tagged
source. The release pipeline is defined in
[.github/workflows/release.yml](../../.github/workflows/release.yml).

## SBOM

Starting with this documentation batch, each release includes a CycloneDX software bill of
materials generated in the release pipeline. It is published as a release asset named
`ghostlight-v<version>-sbom.cyclonedx.json`, alongside the binaries and their checksums, so
you can ingest the exact dependency set of a given release into your own supply-chain tooling.

## Dependencies

The dependency tree is kept deliberately lean, favoring fewer, well-understood crates over
broad transitive graphs. The signature cryptography is pure Rust, with no native TLS stack
pulled into the default build; the network stack used for managed policy fetch is isolated
behind a feature gate rather than compiled into every build. As a dated data point, the npm
package scored 100/100 on all axes on Socket.dev at publication (2026-07); see
[the npm package](https://www.npmjs.com/package/ghostlight). That is a snapshot of that
moment, not a standing guarantee, and the SBOM above is the authoritative, per-release
dependency record.

## Build and change management

Changes reach a release through a disciplined path. Design decisions are recorded as
architecture decision records before they are implemented. CI gates every change on
formatting, linting, the test suite, and the lightbox scenario runner, so a regression in
governance behavior fails the build. Development flows through a trunk-and-release branch
model. Release signing keys are held offline on an air-gapped machine and never enter CI or
any online system, so a compromise of the build infrastructure cannot produce a validly signed
release.

See [security-overview.md](security-overview.md) for the vendor-side security posture.

Last reviewed: 2026-07-10 against v0.5.4 | Contact: support@sylin.org
