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
earlier Windows fixture results. This branch requires its own source, process,
installation and candidate checks. Public delivery is not yet claimed.

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
