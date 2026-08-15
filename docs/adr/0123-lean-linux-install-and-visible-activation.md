# ADR-0123: Lean Linux install and visible application activation

- Status: Accepted
- Date: 2026-08-15
- Amends: ADR-0104 Decision 3, ADR-0112 Decisions 1 and 3, ADR-0115 Decisions 1 and 2,
  and ADR-0116's Linux release evidence
- Builds on: ADR-0102, ADR-0104, ADR-0115, ADR-0116, and ADR-0118
- Research input: [25-delightful-linux-experience-2026-08.md](../research/25-delightful-linux-experience-2026-08.md)

## Context

Ghostlight's Linux architecture is already small: three sibling executables, one orchestrator-owned
installer, connector demand-start, and no resident supervisor. The CachyOS KDE development lane
proved that shape. The remaining gaps are at Linux integration seams.

First, no-argument startup is deliberately backgrounded so an MCP client or Chromium can start the
authority without interrupting the user. On Linux, ADR-0118 implements that as a hidden workbench
plus a tray. GNOME may not expose a tray, so an Applications launcher that invokes the same
no-argument executable can appear to do nothing on first use.

Second, the per-user installer registers browsers and MCP clients but does not install an XDG
desktop entry or icon. The Debian package gets desktop integration from Tauri, so the two Linux
delivery routes do not converge on the same user entry point.

Third, registration state is not browser availability. A native-host manifest can be current when
the browser is absent or installed as a Snap or Flatpak that cannot start an arbitrary host native
messaging executable. Reporting that state as usable sends the user to the wrong recovery step.

Finally, the release workflow builds Linux artifacts on Ubuntu 24.04 while the public surface does
not declare the resulting glibc floor. Tauri requires building on the oldest base intended for
support and identifies Ubuntu 22.04 and Debian 12 as suitable WebKitGTK 4.1 baselines.

Research 25 found that completeness does not require more package formats. It requires one common
experience, a precise support statement, and a small evidence matrix that separates ABI, package,
desktop, and browser-package promises.

## Decision

### 1. Explicit human activation composes the existing lifecycle seams

`ghostlight open` is the explicit local-human intent. If an authority exists, it sends the existing
authenticated workbench activation request. If none exists, it asks the existing bridge lifecycle
seam to start the exact sibling `ghostlight` executable with no arguments, waits for that ordinary
backgrounded authority, then sends the same activation request.

This is not another service mode or startup implementation. Connectors still launch the exact
sibling executable with no arguments. The spawned authority still follows ADR-0112 and ADR-0118.
The explicit command merely composes demand-start with activation, both of which already exist.
The lifetime lease still admits one authority and the workbench presentation port still owns the
only reveal mechanism.

No-argument startup remains the connector-safe backgrounded startup. The Linux Applications entry
uses `ghostlight open`. A tray click continues to use the same presentation port.

### 2. The installer owns one XDG application integration

On Linux user installations, `ghostlight install` owns exactly two XDG files:

- `org.sylin.ghostlight.desktop` below the user applications directory; and
- the byte-identical 128-pixel Ghostlight icon below the user hicolor icon directory.

The desktop entry invokes the exact installed orchestrator with the `open` intent. It does not
depend on shell `PATH`, edit a shell profile, start a service, or introduce a wrapper. An upgrade
rewrites an owned stale entry to the new versioned executable path.

Ownership is explicit in the desktop entry and byte-exact for the icon. Install refuses a foreign
file at either product location. Uninstall removes only an owned desktop entry and an exact
Ghostlight icon. Both operations are idempotent and use the existing per-user XDG root precedence.

The Debian package remains system-owned. It uses the same desktop-entry template with
`ghostlight open`, and the per-user installer does not shadow it when running from `/usr/bin`.
Package scripts still do no per-user work.

### 3. Browser package provenance is a typed install fact

The native-host report adds one closed browser-installation state: native, Snap, Flatpak, multiple
sandboxed forms, not detected, or not checked on the current operating system.

Linux discovery uses only local filesystem facts:

- supported executable names found on `PATH`, excluding Snap and Flatpak export roots;
- fixed `/snap/bin` executable locations; and
- fixed system and per-user Flatpak desktop-export locations for the four supported browser ids.

No package manager is invoked and no network work occurs. A native installation wins when native
and sandboxed copies coexist because it can use the registered host.

The ordinary install path registers detected native browsers. An explicit sandbox-only browser
selection is refused before manifest mutation with a fixed explanation and remedy. `--all-browsers`
retains the deliberate pre-registration behavior for administrators and package testing. Windows
keeps its existing registration behavior and reports package provenance as not checked.

`doctor` always shows package provenance separately from registration state. A sandboxed browser is
never described as merely disconnected or as a usable current registration.

### 4. One old-enough builder and two package smokes prove the ABI/package seam

The Linux release artifact is built on Ubuntu 22.04, the selected x86_64 glibc/WebKitGTK 4.1 floor.
The ordinary Linux quality job may remain on a newer runner; it does not produce the release binary.

The emitted Debian package is installed, queried, version-checked, removed, reinstalled, and purged
in Debian 12 and Ubuntu 24.04 containers. This proves package dependencies and ownership on both
related distributions without pretending that a headless container proves desktop behavior.

The existing Ubuntu GNOME Wayland L1-L9 record remains the one release-blocking visible lifecycle.
The CachyOS KDE Wayland record remains complementary rolling-distribution evidence. RPM, AppImage,
Snap, Flatpak, AUR, Nix, ARM64, and extra full-desktop permutations do not become 1.0 gates.

### 5. Distribution remains three doors over one product

The supported Linux delivery routes remain:

1. the verified no-sudo per-user install;
2. the Debian package; and
3. the verified portable archive.

They carry the same three siblings and delegate product integration to the same orchestrator
modules. No autostart unit, package daemon, updater, repository enrollment, sandbox escape,
format abstraction, or fourth executable is added.

## Consequences

An Applications click becomes reliable on desktops without a tray while connector startup remains
quiet. The new `open` token is a user intent, not a process role, and does not cross either connector
or bridge protocol.

Per-user installation gains two owned files. This is the minimum cost of a native application-menu
presence and uses the standard Linux data hierarchy rather than another launcher mechanism.

Browser registration and browser usability become separate facts. Existing manifests remain valid,
and the native-host wire format does not change. Install output becomes more honest on Ubuntu, where
Chromium may be a Snap, and on Flatpak-based desktops.

Building on Ubuntu 22.04 broadens ABI compatibility relative to the current Ubuntu 24.04 builder.
The build floor is not a claim that every distribution derived from that glibc can supply the
required desktop libraries; the public support statement and live evidence remain narrower.

## Acceptance evidence

1. `ghostlight open` with no authority asks the shared lifecycle seam to start the same no-argument
   sibling, then activates exactly one workbench. With an authority already running it activates
   that authority without another process or workspace.
2. Connector lifecycle tests still prove that demand-start passes no arguments.
3. A user install creates one desktop entry that invokes the exact installed executable with
   `open` and one byte-identical icon in XDG data locations. Reinstall changes nothing.
4. An upgrade rewrites an owned desktop entry from an older versioned executable. A foreign desktop
   entry or icon is preserved and reported. Uninstall removes only owned bytes and is idempotent.
5. A `/usr/bin` package launch does not create per-user desktop integration. The Debian desktop
   entry invokes `ghostlight open`.
6. Native, Snap-only, Flatpak-only, multiple-sandbox, missing, and native-plus-sandbox fixtures
   produce the closed expected browser-installation states without executing a package manager.
7. Default setup selects detected native browsers. An explicit sandbox-only selection changes no
   manifest and names the supported native-package remedy. `--all-browsers` retains deliberate
   pre-registration.
8. `doctor` prints browser package provenance and native-host registration as separate facts.
9. The release Linux binary is built on Ubuntu 22.04. Debian 12 and Ubuntu 24.04 package-smoke jobs
   install, inspect, remove, reinstall, and purge the exact candidate before bundle assembly.
10. Formatting, warnings-denied Clippy, Rust, extension, npm, MCPB, syntax, process, CLI, workbench,
    package-inspection, and release-truth gates pass with no extension change.
