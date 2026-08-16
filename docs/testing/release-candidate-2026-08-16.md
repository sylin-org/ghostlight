# Ghostlight 1.0 build-only release candidate -- 2026-08-16

Status: candidate, provenance, and headless Debian package gates passed; visible clean-machine gates
remain open

Source revision: `fd8640336b11ed12cd47fe96deb7eb06adfbdcd1` on `dev`.

Nothing was tagged, released, submitted, or published. This record covers the manual build-only
candidate workflow and its matching ordinary CI run.

## GitHub results

- [CI run 31920645118](https://github.com/sylin-org/ghostlight/actions/runs/31920645118)
  passed all nine jobs: Windows and Linux Rust, Windows and Linux process journeys, both extension
  jobs, formatting, release truth, and supply chain.
- [candidate run 31920647296](https://github.com/sylin-org/ghostlight/actions/runs/31920647296)
  passed its quality gate, Ubuntu 22.04 Debian-package build, Windows 2025 NSIS build,
  deterministic extension build, Debian 12 lifecycle smoke, Ubuntu 24.04 lifecycle smoke, and
  final candidate assembly.
- Both runs bind the exact source revision above. The candidate artifact id is `9256574239`; its
  downloaded archive size is 47,897,004 bytes and its GitHub retention is 14 days.

The first rehearsals did useful release work instead of being retried blindly. Release truth found
that the restored `mcp_2026_07_28.rs` path still had an absent historical-artifact disposition.
Windows then found one Linux-only import without a target guard and four path-serialization test
assumptions. Commits `994ed4f`, `34b3aa0`, `ca8ec86`, and `fd86403` corrected those seams. The final
Windows run compiled with warnings denied and passed the complete Rust and process-journey jobs.

## Candidate unit

`release-candidate.json` has schema 1, status `release-candidate`, version `1.0.0`, the exact source
revision above, and 17 artifacts:

- six raw Linux and Windows executable shores;
- Linux Debian and portable packages;
- Windows NSIS and portable packages;
- the deterministic Chromium extension ZIP;
- the npm launcher tarball and Claude Desktop MCPB;
- four CycloneDX workspace SBOMs.

The candidate manifest SHA-256 is
`7f78de08416eaf4896fb0b0c6fb9a0137ff2e2dedfd9bc926f8553a05e67e985`. The `SHA256SUMS` SHA-256 is
`34065a8c2d3e69aad1a5a9563d3aff619d53f9c3001b7580ac3f8bfbb0e1c7a3`. Local verification matched
all 17 asset hashes. Important channel hashes are:

| Artifact | SHA-256 |
| --- | --- |
| Linux Debian package | `06831f102cae18e1d3a5c41bc26daf7b9ae3664af981b408e1f43b5a22729196` |
| Windows NSIS installer | `6513d2e27c01013d1498b1c48d283607cd36d5472b041075ce39ea058fe81de9` |
| Chromium extension ZIP | `3cdb3982c9772c84447923b9785ff7e0efc81ed2885fc2386090f4267cce0ab2` |
| npm launcher tarball | `d72a4bef127f18ddb519388eb770c5445b8f233b02be7212b256813f977f2ceb` |
| Claude Desktop MCPB | `8fc67a147dbbcd5489b7ccb8515d3ed0868223c7d715af02a55dcbe3b2b019fd` |

GitHub attestation verification passed for all 17 assets, `release-candidate.json`, and
`SHA256SUMS`. Verification pinned repository `sylin-org/ghostlight`, signer workflow
`.github/workflows/release.yml`, source digest `fd8640336b11ed12cd47fe96deb7eb06adfbdcd1`, and source
ref `refs/heads/dev`.

## Remaining boundary

The Debian 12 and Ubuntu 24.04 headless package gates and package provenance are now closed for
this candidate. They do not prove a graphical desktop session. Before publication, Ghostlight
still needs:

- the candidate Debian package and matching store adapter on Ubuntu GNOME Wayland through L1-L9;
- clean Windows install, public-0.8 upgrade, uninstall, login/reboot, tray, and notification checks;
- the public MCP harness matrix against this candidate; and
- final public metadata reconciliation and explicit channel-by-channel publication approval.
