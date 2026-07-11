# Maintenance and continuity

Ghostlight is a solo project by design, and it is built to keep working with little or no active
maintenance. This document is the honest picture of who keeps the lights on, what happens if that
person steps back, and how someone else could pick it up. It exists so that "bus factor 1" is a
known, planned-for condition rather than a hidden risk.

## Who maintains it

One maintainer (`lbotinelly`), with Dependabot handling routine dependency bumps. There is no team,
no on-call rotation, and no paid support desk. See [SECURITY.md](SECURITY.md) for how security
reports are handled and on what (best-effort) timelines.

## The Continuity Promise (why a quiet period is safe)

The single most important continuity property is a design one, not a staffing one:

- **The binary never phones home**, carries no telemetry, and initiates no network traffic beyond
  your own tool calls and any audit destination you configure (ADR-0028 Decision 9).
- **License state never changes behavior.** Ghostlight runs identically whether or not anyone ever
  pays, and whether or not the maintainer is active (ADR-0028 Decision 1).

So an installed copy does not degrade if maintenance goes quiet. It keeps doing exactly what it did
the day you installed it. Nothing expires, calls home, or waits for a server that might go away.

## What a coast period does and does not affect

**Keeps working with no maintainer action:** every installed copy; the automation and governance
engine; local audit; the docs, tests, and CI that verify each release.

**Needs the maintainer (or a successor) and may lag during a quiet period:**

- Cutting a new release (single npm maintainer, single signing key -- see "Single points of failure").
- Merging a dependency or security bump.
- Responding to a security report or a support question within any particular window (best-effort,
  not guaranteed -- [SECURITY.md](SECURITY.md)).
- Issuing a commercial license.

## Single points of failure (named plainly)

These are the things only the current maintainer can do today. They are real, and de-risking them
is planned work, not a solved problem:

1. **npm publishing** rides a single maintainer account. Mitigation intent: add a second npm
   publisher / an org-scoped token before any extended coast.
2. **Release signing** uses a single key that produces the SHA-256 / Sigstore build-provenance
   attestations adopters verify. Mitigation intent: back the key up in escrow so releases can
   continue if the primary is lost.
3. **Commercial license issuance** is founder-only offline signing. This halts if the maintainer
   goes fully minimal; it does not affect any running copy (see the Continuity Promise), only the
   ability to sell new commercial subscriptions.

Progress on 1 and 2 is tracked as continuity work; this file is updated when a backup publisher or
key escrow is in place.

## Picking it up (for a future maintainer or a fork)

The project is built to be resumable:

- **Everything needed to build and ship is in the repo:** the Rust workspace, the extension, the
  CI/release workflows (`.github/workflows/`), the packaging manifests, and a pinned toolchain
  (`rust-toolchain.toml`).
- **The design is written down:** [docs/SPEC.md](docs/SPEC.md) is authoritative, and the ADRs
  under [docs/adr/](docs/adr/) record why each decision was made.
- **The engine is permissively licensed.** Everything outside `crates/core/src/governance/` is
  Apache-2.0 OR MIT (see [LICENSING.md](LICENSING.md)), so it can be forked, ported, or continued
  freely. The vendor-neutral capability model ([RAWX](open-spec/rawx-capability-model.md)) is
  designed to outlive any single implementation.
- **Verification is automated, not tribal:** `cargo test`, the Playwright/lightbox tiers, and the
  schema-fidelity guard tell a newcomer whether a change is safe without needing the original
  author in the room.

## Reporting problems

- Bugs: [GitHub Issues](../../issues).
- Questions and ideas: [GitHub Discussions](../../discussions).
- Security: [SECURITY.md](SECURITY.md) (private channel).
- Anything that cannot be public: hello@sylin.org.
