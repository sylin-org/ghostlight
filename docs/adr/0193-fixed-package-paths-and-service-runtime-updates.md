# ADR-0193: Fixed package paths and service runtime updates

Date: 2026-10-02. Status: Accepted for implementation by the owner.

Amends ADR-0115 and ADR-0167's package-directory selection. Builds on ADR-0181.

## Context

The npm launcher and download installers used `~/.ghostlight/bin/v<version>`.
Registrations then named one release directory. Updates accumulated executable
copies and made installation identity depend on the release being served.

The owner rejected release directories and an additional deployment framework.
The browser extension is already an injector: the service owns the page runtime
and sends its exact bundle on reconnect. Matching release directories or updating
the packaged extension cannot be prerequisites for ordinary service updates.

## Decision

1. npm and the download installers use `~/.ghostlight/bin` permanently. Release
   versions remain artifact identities and download coordinates, not install paths.
2. Downloaders verify all three siblings in temporary storage before calling the
   existing package/deployment seam. `ghostlight deployment install <directory>`
   copies from its own verified sibling set. No permanent helper, release selector,
   version symlink, or new wire protocol is introduced.
3. The seam uses existing deployment and startup custody. Only changed exact image
   paths are stopped. Unchanged connectors keep their streams through a service
   replacement. Temporary copies are removed after replacement. Ordinary file
   errors restore displaced siblings; this is not a power-loss atomicity claim.
4. Migration retires the recorded release's exact processes and redirects owned
   registration through the existing installers. Only the three binary files in
   the former version child of this bin directory may be removed. Existing state,
   foreign files and unrelated installations are preserved. Development selection
   still wins over package updates.
5. The packaged extension keeps the fixed `org.sylin.ghostlight` host name. The
   native-host manifest resolves it to the permanent connector path. A service
   reconnect installs and acknowledges the current service-owned page runtime
   through ADR-0181. No extension update or reload is required for that handoff.
6. Browser-privileged mechanisms still belong to the extension. This decision
   does not claim that an old injector can execute a mechanism it lacks. Public
   version records and historical compatibility attestations remain evidence.

## Evidence

Acceptance must cover fixed paths across updates, failed download preservation,
changed-component replacement, live Windows executable locks, unchanged connector
continuity, old owned-registration migration, development custody and retained
history. Injector acceptance must replace two distinct bundles through one loaded
adapter without restamping or reloading it. Component fixtures and actual native
browser acceptance remain distinct evidence.

[The implementation report](../testing/fixed-installation-2026-10-02.md) records
Windows process/download results, component injector evidence, and unexecuted lanes.
