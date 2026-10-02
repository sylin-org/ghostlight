# Deployment-only 1.3.13 -- 2026-10-02

The owner authorized publication and then rejected unrelated browser investigation.
This release uses the published 1.3.12 source plus the fixed-path installation repair
from b4340ba0 and its Linux process enumeration correction. Unpublished browser and
workbench changes remain in Git history and on main. They are outside this release.

Production extension files, bridge contracts, browser relay and service page runtime
remain identical to the published baseline. Only an injector regression test changes.
The public 1.3.12 adapter supports service 1.3.13; no store update is required.
Installations keep one permanent sibling set at `~/.ghostlight/bin`. The service
owns registration migration and runtime handoff through existing seams (ADR-0193).

The original [fixed-installation report](fixed-installation-2026-10-02.md) records
earlier Windows fixture results. The narrowed branch's independent source, process,
installation, candidate and public-delivery evidence follows below.

## Current source and Windows installation evidence

The narrowed branch passes formatting, strict workspace Clippy, all 567 Rust
tests, all 295 extension tests and all 11 npm launcher tests. JavaScript syntax,
offline public-surface checks and repository integrity pass. Fresh binaries were
built into the existing isolated `.target-dev-loop` directory and explicitly used
by every process fixture.

The Windows fixed-installation journey passes: former owned registrations migrate,
history survives, a running service image is replaced, both unchanged connector
PIDs and streams survive, the service runtime is acknowledged again, and no new
release directory or temporary payload remains. The shipped PowerShell downloader
passes verified complete-payload installation, a profile containing spaces,
corrupt final-download preservation and temporary cleanup. The complete process
journey passes against this source, including restart through one native peer.

These are isolated user/process fixtures, not deployment into the owner's ordinary
installation. Linux installation and package checks remain candidate workflow gates.

The unmodified Chromium frame journey passes all 69 checks with pinned Chrome
152.0.7977.82 and this branch's freshly built service and relays. It loads the
unchanged production 1.3.12 MV3 adapter, installs the service runtime and completes
governed reads, forms, captures, scripts, recording and cleanup. Its native port
transport is a fixture shim around real relay framing; it is component evidence,
not an installed Chrome Web Store acceptance claim.

## Candidate and public delivery

Candidate run [37068025269](https://github.com/sylin-org/ghostlight/actions/runs/37068025269)
passes all seven jobs from `6656ef87a85437ff79e8dc6ed32cec28121b4872`. Linux source,
process, fixed-path and shell-downloader checks pass. Windows release executables
pass fixed-path replacement and PowerShell downloader checks. Native packages
build, and Debian 12/Ubuntu 24.04 package lifecycle checks pass. Assembly verifies
18 artifacts and attests all 20 files including the manifest and checksum list.

Tag `v1.3.13` names that exact source. The publisher verifies all 20 provenance
records, creates a GitHub draft and re-downloads every asset to compare hashes
before publication. GitHub 1.3.13 is public. npm accepts version 1.3.13 at latest;
the public tarball SHA-256 matches the candidate. A fresh consumer with isolated
user state installs that public package, downloads and verifies all three real
Windows binaries at fixed paths, and invokes version 1.3.13 twice. This consumer
does not register against the owner's browser or deploy into their installation.

The official MCP Registry publishes `org.sylin/ghostlight` 1.3.13. Candidate-derived
Scoop metadata is reconciled in the repository. WinGet's three 1.12 manifests pass
local validation and are submitted in
[PR #445923](https://github.com/microsoft/winget-pkgs/pull/445923); its initial manifest
and existing CLA checks pass, with further checks/review pending. No new legal
agreement is signed. Website delivery is verified after the public fallback push.

The production adapter is unchanged 1.3.12. Its candidate ZIP SHA-256 is
`9d5521b3f34562051ea6b07d9d5d0e07267ff6d7bacf274bd38234d43659a25d`.
No Chrome package is submitted or published for this ordinary service update.
Main's separate source adapter 1.3.13 is an unpublished trial, outside this release.
