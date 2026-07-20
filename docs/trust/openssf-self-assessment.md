# OpenSSF OSPS Baseline self-assessment

Assessment date: 2026-07-18

Baseline: [OSPS Baseline 2026.02.19](https://baseline.openssf.org/versions/2026-02-19.html)

Scope: the `sylin-org/ghostlight` source repository, GitHub Actions workflows, and official
Ghostlight release assets.

Status: working self-assessment, not a certification, badge, or submitted attestation.

## How to read this

The current OSPS Baseline defines Level 1 for projects with any number of maintainers or users.
Ghostlight is assessed at that level because it has one maintainer. The Baseline allows projects to
self-attest, but compliance is point-in-time. This document therefore distinguishes repository
evidence from settings that require an owner check. An unchecked privileged setting prevents a
full compliance claim.

Status vocabulary:

- Met: repository evidence or a verified setting satisfies the control.
- Owner check: the required account setting is not provable from the public tree or repository API.
- Partial: some evidence exists, but the control is not fully met or its application is unclear.
- Not met: current evidence shows that the control is not enforced.
- Not applicable: the stated condition does not exist; the reason is recorded.

## Level 1 control map

| Control | Status | Evidence and follow-up |
| --- | --- | --- |
| OSPS-AC-01.01 | Owner check | Organization-wide 2FA enforcement is not enabled. Confirm the sole administrator account itself uses MFA or a passkey before assessing the control. |
| OSPS-AC-02.01 | Met | The organization default repository permission is read, and the repository has one explicitly assigned administrator. Recheck when adding a collaborator. |
| OSPS-AC-03.01 | Not met | The active `Main` ruleset prevents deletion and non-fast-forward updates but does not require a pull request or otherwise block a direct fast-forward push. |
| OSPS-AC-03.02 | Met | The active `Main` ruleset targets the default branch, enforces deletion protection, and has no bypass actor. |
| OSPS-BR-01.01 | Met | Pull-request CI does not interpolate branch names, titles, commit text, or other untrusted metadata into commands. Workflows use static commands and checked-in configuration. |
| OSPS-BR-01.03 | Met | Pull-request CI declares `contents: read` and has no privileged release credentials. Release publication is isolated to the trusted tag workflow and separates unprivileged assembly from privileged publication. See `.github/workflows/ci.yml`, `.github/workflows/release.yml`, and `docs/trust/supply-chain.md`. |
| OSPS-BR-03.01 | Met | Official repository, website, package, registry, support, and documentation URLs use HTTPS. SSH is an optional authenticated Git transport, not a public content channel. |
| OSPS-BR-03.02 | Met | GitHub, npm, Homebrew, and MCP Registry delivery use authenticated HTTPS. Releases also carry `SHA256SUMS`, Sigstore attestations, and a CycloneDX SBOM. |
| OSPS-BR-07.01 | Partial | Secrets are excluded from tracked configuration and release workflows use GitHub secrets or local environment files. GitHub secret scanning and push protection are currently disabled, so the preventive control rests on repository boundaries and review alone. |
| OSPS-DO-01.01 | Met | `README.md`, `docs/guides/`, CLI help, configuration docs, and the canonical install guide cover installation and basic use. Consequential capabilities and residual risks are called out. |
| OSPS-DO-02.01 | Met | `SUPPORT.md`, `CONTRIBUTING.md`, and structured Issue forms explain defect and installation reporting. |
| OSPS-GV-02.01 | Met | GitHub Discussions is the public lane for proposed changes and usage obstacles; Issues handle reproducible defects. |
| OSPS-GV-03.01 | Met | `CONTRIBUTING.md` explains contribution terms, validation, design boundaries, and the DCO/CLA split. |
| OSPS-LE-02.01 | Not met | The automation engine is Apache-2.0 OR MIT, but the repository also contains the deliberately source-available governance module under a commercial license. A claim that all project source meets the OSI/FSF definition would be false. `LICENSING.md` defines the boundary. |
| OSPS-LE-02.02 | Not met | Public release binaries contain both the permissively licensed engine and the separately licensed governance module. The license is clear and included, but not every released-software license meets the OSI/FSF definition required by this control. |
| OSPS-LE-03.01 | Met | Root `LICENSE`, `LICENSE-APACHE`, `LICENSE-MIT`, `LICENSE-GOVERNANCE`, and `LICENSING.md` keep each source boundary in a standard location. |
| OSPS-LE-03.02 | Met | Source archives carry the root license files; release packaging and the hash manifest associate assets with the release. |
| OSPS-QA-01.01 | Met | The authoritative source is publicly readable at `https://github.com/sylin-org/ghostlight`. |
| OSPS-QA-01.02 | Met | Git history publicly records changes, authors, and timestamps. Pull requests and signed-off commits provide additional review history. |
| OSPS-QA-02.01 | Met | `Cargo.toml`, `Cargo.lock`, and npm package manifests enumerate direct language dependencies. |
| OSPS-QA-04.01 | Not applicable | Ghostlight's released codebase is this repository. The website and package-manager tap are distribution companions, not hidden Ghostlight source subprojects; `docs/RELEASE.md` maps them explicitly. |
| OSPS-QA-05.01 | Met | Generated executables are release assets, not tracked source files. Repository images and the README GIF are documentation media, not executable artifacts. |
| OSPS-QA-05.02 | Partial | Tracked PNG/GIF product media is binary and cannot receive line review. Reproducible sources or capture recipes exist for key assets, but the owner should confirm the Baseline's intended treatment before a full Level 1 claim. |
| OSPS-VM-02.01 | Met | `SECURITY.md`, `SUPPORT.md`, and `https://sylin.org/.well-known/security.txt` publish security contacts and the private reporting route. |

## Current conclusion

Ghostlight has strong public evidence for build isolation, authenticated distribution, user and
contributor documentation, source history, dependencies, disclosure, and release integrity. It
must not claim full OSPS Level 1 compliance yet. One account control needs owner verification,
direct pushes are not blocked, secret push protection is disabled, the open-core license makes two
Level 1 controls inapplicable as compliance claims, and tracked binary media still needs an
explicit Baseline interpretation.

## Closure checklist

- [ ] Verify MFA or passkeys for every privileged GitHub account.
- [x] Verify least-privilege collaborator defaults.
- [ ] Extend the active `Main` ruleset to prevent direct pushes if the project adopts that OSPS
      control; preserve an explicit emergency path if GitHub supports it without routine bypass.
- [x] Verify deletion protection through the active ruleset.
- [ ] Enable GitHub secret scanning and push protection, or document and verify an equivalent
      preventive control.
- [x] Record OSPS-LE-02.01 and OSPS-LE-02.02 as Not met. Do not restructure or mislabel the
      open-core project merely to obtain a badge.
- [ ] Resolve OSPS-QA-05.02 treatment for necessary documentation media.
- [ ] Re-run this assessment against the then-current named Baseline version.
- [ ] Only after every Level 1 control is Met or validly Not applicable, consider an external
      self-attestation or badge submission.

This assessment should be reviewed when repository ownership, branch rules, release workflows,
licensing boundaries, or the current OSPS Baseline version changes.

Last reviewed: 2026-07-18 against v0.6.0 | Contact: support@sylin.org
