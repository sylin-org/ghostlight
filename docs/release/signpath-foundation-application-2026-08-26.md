# SignPath Foundation application -- 2026-08-26

Dated record of an outward action. The owner submitted the free
SignPath.io OSS subscription application through https://signpath.org/apply
on 2026-08-26. Personal application details (contact identity and email) are
recorded only in the gitignored `local/signpath-application.md`, never here.

## What was applied for

A free OSS SignPath.io subscription with a code signing certificate issued to
SignPath Foundation, to sign Ghostlight's Windows release artifacts (the NSIS
installer and the three Windows executables). Today those artifacts ship
checksum-bound with keyless Sigstore provenance and carry no Authenticode
signature; this is the first code-signing certificate the project has held.

## Why the project qualifies now

The ADR-0140 relicensing (2026-08-25) is the enabling decision. SignPath
Foundation requires an OSI-approved license without commercial dual-licensing
and forbids proprietary components by affiliated parties; the former
open-core split (source-available governance module) failed both conditions.
The whole-product Apache-2.0 OR MIT tree passes them. The other published
conditions -- actively maintained, publicly released (0.8.x line since July
2026), documented, no malware or hacking tools, uninstallation provided, no
data collection -- were assessed as met; see the eligibility assessment in
the working session of 2026-08-26.

## What was submitted

The application form stated the project facts that are already public:
repository, homepage, download and privacy URLs, tagline, a description of
the governed browser automation model, a reputation summary (npm launcher,
Chrome Web Store listing, MCP Registry entry, SHA256SUMS + Sigstore
provenance + CycloneDX SBOMs on GitHub releases), maintainer type
(independent community project), and GitHub Actions as the build system.
All three consent checkboxes were accepted by the owner.

## Conditions and follow-ups

Pending foundation review. On acceptance, before the first signed release:

1. Add a "Code signing policy" section (attribution sentence, Author/
   Reviewer/Approver roles, privacy link) and surface it on the GitHub
   Releases page -- the application form states the download page must
   mention SignPath Foundation signing.
2. Give every signed Windows binary consistent ProductName/ProductVersion
   metadata and enforce it through SignPath artifact-configuration metadata
   restrictions.
3. Verify MFA on the GitHub and SignPath accounts (also closes the OpenSSF
   OSPS-AC-01.01 owner check).
4. Wire the release workflow to SignPath.io with a manual approval step as
   the final signing gate.
5. Then update `docs/trust/supply-chain.md` and `security-insights.yml`,
   which today truthfully state that releases carry checksums and provenance
   but no Authenticode signature.
