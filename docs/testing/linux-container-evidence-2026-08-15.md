# Linux container release evidence -- 2026-08-15

Status: local package evidence passed; visible Ubuntu lifecycle still required

This record gets every useful headless Linux question out of the visible release lane. It does not
turn a container into desktop evidence. The remaining Ubuntu machine should spend its time on the
window, tray, browser, extension, login, reboot, and notification behavior that requires a real
graphical user session.

## Candidate

- Source revision: `a9bd73424198cb144154117ad4dcae682d18baf5`
- Debian package SHA-256:
  `a6c898f9072ae50363b12e8d422f74a6718d2bce3a874bd82d6d25b9658338e9`
- Portable archive SHA-256:
  `7bf2994067c148191d797c572abd1a3604b487497c4bef8e2a44fb04548f8d10`
- Orchestrator SHA-256:
  `8653c1f3b11b483e6356ef842d896d9d2a5e95ff84d128a29704f7a6f7c0384d`
- MCP connector SHA-256:
  `25dda20be9cd2a0adca11a49b8c04d54f0c9168ccdfc82aa10e2e34ae08f9cbe`
- Browser connector SHA-256:
  `95763cb9be04407570e4b08ac319a47a6e03fb3910c2bd40f6fa74284800ffee`

The package was built from a `git archive` of that exact revision in the selected Ubuntu 22.04
builder. Cargo and Tauri ran with the locked dependency graph and persistent caches. The three
release binaries were built once, then used for the portable and Debian artifacts.

## Isolation model

The lab is rootless and lives outside the repository on the second drive. It uses an unprivileged
user namespace, Bubblewrap, and one overlay per guest. No Docker daemon, host package mutation, or
sudo access is required. Tool archives and OCI images are checksum-bound, base filesystems are
read-only, writable overlays are disposable by test family, and candidates and evidence survive
guest teardown.

The exact amd64 OCI image digests were:

| Guest | Digest |
| --- | --- |
| Ubuntu 22.04 builder | `sha256:3b06811b2afd352be909dd088a004166d665dc76d38b13eada33522a9d915c6f` |
| Debian 12 | `sha256:813017f3d62be4b5891a7acca6a01bdcd4b8513daa81b1ab99d3a50385b26931` |
| Debian 13 | `sha256:34cd9e9fd437c0a095ec39cb2e73422c9f30821b0d0848ed74fd0d43bae4d958` |
| Ubuntu 24.04 | `sha256:561618e2c15bf2397621dd04f96926663a3b5616c189cf7e38db7e82f5c538ea` |
| Ubuntu 26.04 | `sha256:678c6550cc43645e08669028bc177f50be4e7c5b8cca677067b1914d4afc7a03` |

Persistent overlays need a new ordinary-user state root for each candidate and journey. A stale
runtime file from another candidate can otherwise produce a correct but irrelevant not-reachable
result before the new authority publishes its endpoint.

## Coverage plan and result

| Layer | What it answers | Result |
| --- | --- | --- |
| Repository gate | Formatting, warnings, unit and contract behavior | Passed: 276 Rust tests and 103 extension tests |
| Oldest builder | Selected glibc and WebKitGTK build floor | Passed on Ubuntu 22.04; every binary requires at most GLIBC 2.34 |
| Artifact inspection | Payload, metadata, legal files, conffiles, checksums, modes, RPATH, and dependency declarations | Passed |
| Required package matrix | Debian 12 and Ubuntu 24.04 dependency and package-manager lifecycle | Passed |
| Advisory package matrix | One newer Debian and Ubuntu generation | Debian 13 and Ubuntu 26.04 passed the same journey |
| Ordinary UID | Runtime permissions, service start, doctor, native-host report, and MCP initialize | Passed in all four guests |
| Package ownership | Remove, reinstall, purge, owned manifest cleanup, and retained user state | Passed in all four guests |
| Network independence | Installed authority, CLI, connector, and package lifecycle with no network namespace | Ubuntu 24.04 passed |
| Portable route | XDG application/icon install, byte-idempotent reinstall, version-path update, runtime, and ownership-safe uninstall | Ubuntu 24.04 passed |
| Public upgrade | Real public 0.8 install, supervisor retirement, manifest update, idempotency, unrelated-state preservation, and old-binary preservation | Ubuntu 24.04 passed |
| Debian advisory | `lintian` after package correction | Only intentional platform-path, internal-manpage, Rust, and duplicate-legal-payload findings remain |

The reusable guest mechanism is
`scripts/check-debian-package-lifecycle.sh`. The release workflow calls it for the accepted Debian
12 and Ubuntu 24.04 gates. Local evaluation can pass the same candidate to newer guests without
making every available distribution a release gate.

## Defects found and closed

Container evidence found release defects that source tests did not:

- `dpkg-deb` listings do not consistently prefix paths with `./`.
- Packaged siblings could not put runtime discovery beside read-only `/usr/bin`.
- Browser discovery treated a non-executable path entry as a native browser.
- The Debian artifact had placeholder descriptions, a malformed maintainer, no libc floor,
  unstripped binaries, no standard changelog or copyright, and unmarked `/etc` configuration.
- Tauri 2.9 put `changelog.gz` under the display-name directory instead of the Debian package-name
  directory.
- Repacking with a new `dpkg-deb` default produced zstd members that Debian 12's `lintian` rejected.

The native-package verifier now guards the corrected metadata and payload. Debian finalization is
one bounded script: it marks the four browser manifests as conffiles, normalizes the changelog
path, regenerates payload checksums, and emits xz members.

## Expected advisory findings

The remaining `lintian` errors are the Chrome and Edge system native-messaging locations under
`/etc/opt`; those paths are browser contracts and cannot be moved. The connector executables are
internal sibling shores, so three missing-manpage warnings do not justify a documentation system
for 1.0. C fortify notices do not describe Rust hardening, and the exact legal payload stays in the
cross-platform application resources as well as the Debian-standard copyright location. Binary
spelling notices are string-table false positives.

## What containers cannot close

Do not spend more headless matrix rows on questions that share the same glibc and Debian package
seams. A container shares the host kernel and has no ordinary GNOME Wayland login, real tray,
store-installed extension, browser profile, logout, or reboot transition. A booted headless VM
would add only package-survives-reboot evidence; it would not close the visible product promise.

The prepared Ubuntu machine should run
[the L1-L9 lifecycle](linux-live-lifecycle.md) against this checksum-bound package and its matching
extension. Provenance verification, clean graphical install, Applications activation, tray and
window recovery, native browser journeys, login/reboot demand-start, notification containment,
and final uninstall remain release-environment work.
