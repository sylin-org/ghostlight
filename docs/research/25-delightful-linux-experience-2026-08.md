# A delightful Linux experience

Date: 2026-08-15

Status: Research input. This document records prior art, the current tree baseline, and a proposed
1.0 Linux experience. It does not change the supported-platform or packaging contract. Any startup,
distribution, or support-floor change still needs its owning ADR.

## The short answer

Linux should feel like one product with two install scopes and three doors:

1. A no-sudo per-user install is the default and the broad compatibility layer.
2. A Debian package is the polished native path for Debian and Ubuntu.
3. A verified portable archive is the offline and recovery path.

All three must install the same three sibling executables, call the same installer core, create the
same Applications entry, produce the same first-run outcome, and use the same `doctor` and
`uninstall` behavior. The package format is delivery, not a second product.

For 1.0, broad Linux support should mean a declared x86_64, glibc-based desktop compatibility
surface. It should not imply that every distribution, CPU, browser package, or sandbox is supported.
A small honest matrix is more delightful than a long download page full of weakly tested packages.

The highest-value work is not another package:

- Give every installation a normal Applications launcher that visibly opens the workbench.
- Detect Snap and Flatpak browser packages and explain why their sandbox cannot launch the native
  connector.
- Build on a declared old-enough Linux baseline instead of Ubuntu 24.04 if older distributions are
  claimed.
- Make install, update, repair, and uninstall say what is ready and what the user must do next.
- Prove one clean GNOME lifecycle and keep the current KDE development-host evidence.

## Why Linux is not one compatibility question

A distribution name is only one dimension. Ghostlight crosses five independently variable seams:

| Seam | What can differ | User-visible failure |
| --- | --- | --- |
| Binary ABI | glibc and WebKitGTK versions | The application does not start. |
| Package family | Debian, RPM, Arch, Nix, immutable systems | Dependencies or desktop files are not installed. |
| Desktop shell | GNOME, KDE, and others | The tray or launcher is absent. |
| Browser package | Native package, Snap, or Flatpak | The extension cannot start its native host. |
| Session | Wayland or X11 | Window activation, tray, focus, or rendering differs. |

The support promise must name these dimensions. "Linux" alone is not an actionable promise or a
useful test plan.

## Current Ghostlight baseline

The current tree already has the right architectural center:

- One Rust installer owns browser native-host and MCP-client registration.
- A package carries the exact orchestrator, MCP connector, and browser connector sibling set.
- Either connector demand-starts the same local authority. There is no systemd user service.
- A Debian package owns four system native-host manifests. Its first ordinary user launch repairs a
  stale per-user Ghostlight registration that would otherwise shadow the package.
- The npm and portable paths install a checksum-bound version under `~/.ghostlight/bin` and register
  per-user browser manifests.
- Install, uninstall, doctor, dry-run, repeat install, and ownership-safe removal already exist.
- The Linux KDE Wayland development host has proved the workbench, tray, native window actions,
  demand-start, browser restart, and a visible browser journey.

The remaining experience gaps are distribution and presentation gaps, not a reason for another
service architecture.

## Prior art

### Native messaging is the hard boundary

Chrome starts a native-messaging host as a separate process and requires a host manifest whose
`path` is absolute on Linux. Chrome documents fixed user and system manifest locations and an exact
extension allowlist. This is why Ghostlight needs a stable installed path and why a browser sandbox
is more consequential than the distribution underneath it.

[Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)

### Mature applications combine native packages with one fallback

Visual Studio Code publishes Debian and RPM packages, a Snap, architecture-specific packages, and
community paths for Arch and Nix. 1Password similarly publishes Debian and RPM packages plus
distribution-specific and archive alternatives. The useful pattern is not the number of badges.
It is a first-party native path for the major package families plus one vendor-controlled fallback.

Both products also demonstrate a boundary Ghostlight should state plainly: a package can change the
experience. 1Password documents that its Snap and Flatpak builds lose browser shared unlock and
other host integrations. KeePassXC documents the same isolation problem more directly: Snap and
Flatpak browsers generally cannot use its native browser integration, with narrow packaged
exceptions.

- [Visual Studio Code on Linux](https://code.visualstudio.com/docs/setup/linux)
- [1Password for Linux](https://support.1password.com/install-linux/)
- [KeePassXC browser integration](https://keepassxc.org/docs/KeePassXC_GettingStarted)

### A universal command can be the front door

Tailscale gives Linux users one copyable install command that detects the distribution, while
retaining manual package instructions. Ghostlight already has the better fit for its own trust
model: a small bootstrap or npm launcher that resolves an exact version, verifies all three
siblings, and delegates configuration to the installed executable.

The lesson is to keep the website decision small. A user should not have to understand Debian,
AppImage, native messaging, or MCP configuration before installation.

[Tailscale Linux installation](https://tailscale.com/docs/install/linux)

### Desktop integration is a standard, not a tray icon

The freedesktop Desktop Entry specification is the shared application-menu contract across Linux
desktops. The XDG Base Directory specification defines user data, configuration, state, runtime,
and executable locations. These are the portable seams for a per-user install.

GNOME's GTK documentation explicitly warns that a notification area may not exist and that a
status icon must not be the only way to reach critical functionality. GNOME stopped showing legacy
status icons by default years ago. KDE tray success therefore does not prove a GNOME first-run
experience.

- [Desktop Entry specification](https://specifications.freedesktop.org/desktop-entry/latest/)
- [XDG Base Directory specification](https://specifications.freedesktop.org/basedir/)
- [GTK StatusIcon guidance](https://gnome.pages.gitlab.gnome.org/gtk/gtk3/class.StatusIcon.html)
- [GNOME status icon design history](https://blogs.gnome.org/aday/2017/08/31/status-icons-and-gnome/)

### Snap and Flatpak change the trust boundary

Strict Snap confinement isolates an application and permits host access through declared
interfaces. Snap's own browser-native-messaging design discussion says a strictly confined browser
cannot execute an arbitrary host native-messaging binary. Flatpak similarly isolates applications
from host processes by default and exposes bounded host interactions through portals and explicit
permissions.

Ghostlight should not write into sandbox-private directories, request broad host escape permissions,
or add a second in-sandbox connector. Those approaches complicate ownership and weaken the simple
three-sibling trust story. Detection plus a direct remedy is the better 1.0 experience.

- [Snap confinement](https://snapcraft.io/docs/explanation/security/snap-confinement/)
- [Native messaging in confined browser snaps](https://forum.snapcraft.io/t/native-messaging-support-in-strictly-confined-browser-snaps/26849)
- [Flatpak basic concepts](https://docs.flatpak.org/en/latest/basic-concepts.html)
- [Flatpak command reference](https://docs.flatpak.org/en/latest/flatpak-command-reference.html)

### Linux binaries inherit their build floor

Tauri can emit Debian, RPM, AppImage, Snap, Flatpak, and AUR artifacts. That availability does not
make each format a good Ghostlight fit. Tauri warns that glibc compatibility is set by the build
host and says applications should build on the oldest base they intend to support. It names Ubuntu
22.04 and Debian 12 as suitable WebKitGTK 4.1 baselines. Building on a newer distribution can make a
binary fail on an older one before Ghostlight can show a useful error.

Tauri also describes AppImage as a dependency-bundled executable, but it does not solve
Ghostlight's stable native-host path, per-user registration, desktop entry, repair, or uninstall
needs. A verified archive plus the existing installer core is a smaller portable contract.

- [Tauri distribution formats](https://v2.tauri.app/distribute/)
- [Tauri RPM limitations](https://v2.tauri.app/distribute/rpm/#limitations)
- [Tauri AppImage distribution](https://v2.tauri.app/distribute/appimage/)
- [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/)

### Package scripts are not onboarding

Debian maintainer scripts must be idempotent and have defined failure behavior. They also run in a
package-manager context, which is the wrong place to mutate one desktop user's MCP configuration or
open a browser. Ghostlight's current package split is sound: package-owned system files are laid
down by the package; per-user reconciliation and onboarding happen on an ordinary user launch.

[Debian Policy Manual](https://www.debian.org/doc/debian-policy/)

## Recommended 1.0 support statement

Use precise public wording:

> Ghostlight 1.0 supports x86_64 glibc-based desktop Linux with a native Chrome, Edge, Brave, or
> Chromium installation. Debian and Ubuntu have a native `.deb`; other supported desktops use the
> verified per-user installer. Snap and Flatpak browser packages are not supported for native
> messaging in 1.0.

That statement should name a tested minimum distribution after the build-baseline proof. It should
not claim Alpine or other musl systems, NixOS, immutable/OSTree systems, ARM64, headless Linux, or
every Chromium derivative by implication.

## One product, two scopes, three doors

### Scope 1: per-user, no sudo

This should be the website default because it reaches Debian, Ubuntu, Fedora, RHEL-family,
openSUSE, Arch, CachyOS, and similar glibc desktops without teaching package commands.

The exact directory can remain project-owned and versioned. Integration should use XDG locations:

- a stable `ghostlight` command reachable from the user executable path;
- a Ghostlight desktop entry and icons below the user data directory;
- per-user native-host manifests in the browser-defined locations;
- versioned siblings swapped only after checksum and provenance verification;
- product state and configuration in their existing XDG-aware locations.

Before installing, the route should check the runtime libraries the candidate actually needs. If a
dependency is missing, it should name the distribution and print one exact native package-manager
command. It should not invoke sudo itself or claim that a downloaded binary is distribution-free.

The installer must not edit shell startup files merely to repair `PATH`. A desktop entry can use a
stable absolute executable path, and the install outcome can name the command path for terminal
users.

### Scope 2: native system package

The Debian package should remain the only 1.0 native Linux package. It earns that position because
there is already an accepted lifecycle, system native-host layout, candidate workflow, and owed live
matrix.

The package should own only system files: three binaries, application metadata, icons, desktop
entry, licenses, and four native-host manifests. It should not open a browser, edit a user's MCP
clients, create a systemd unit, or silently add an APT repository. First explicit user launch owns
the per-user handoff.

An RPM is the next rational native format, not a 1.0 requirement. Add it only with a Fedora or
openSUSE lifecycle host, exact browser-manifest proof, upgrade/uninstall evidence, and a new release
scope decision. Until then, the per-user route is the supported answer on those distributions.

### Door 3: verified portable archive

The archive is for offline installation, air-gapped transfer, recovery, and package maintainers.
It should contain the same three siblings and a small README that invokes the same installer core.
It is not an unregistered "run this file from Downloads" experience.

## Package-format disposition

| Format | 1.0 posture | Reason |
| --- | --- | --- |
| Per-user bootstrap or npm | Default | Broad reach, no sudo, existing exact-version verification and installer core. |
| `.deb` | First-party native | Existing accepted contract and lifecycle; best Debian/Ubuntu experience. |
| Portable archive | First-party fallback | Offline, recovery, and downstream packaging without a second installer. |
| `.rpm` | Next candidate | High coverage value, but only after one real RPM lifecycle lane exists. |
| AppImage | Do not lead with it | Does not remove registration, stable-path, desktop, repair, or uninstall work. |
| Snap | Do not ship for 1.0 | Confinement and store lifecycle complicate native messaging and the no-phone-home posture. |
| Flatpak | Do not ship for 1.0 | The host connector boundary needs broad escape or a second bridge. |
| AUR/Nix/community packages | Enable, do not claim | Publish a stable packaging contract and checksums; downstreams own their untested package. |

## The complete user journey

### 1. Choose

The Linux download page should lead with one command and two secondary links:

- Install for me -- the no-sudo user install.
- Debian or Ubuntu package -- the `.deb`.
- Offline or manual install -- the verified archive.

Distribution badges and package jargon belong below that first decision.

### 2. Install

The installer should print one progress line per meaningful phase: obtain, verify, install, connect
browsers, connect MCP clients. Mechanical file lists stay behind verbose or dry-run output.

The terminal outcome should answer four questions:

1. Which exact Ghostlight version is installed?
2. Which browser can connect, or why none can?
3. Which MCP clients were connected?
4. What single action completes first use?

### 3. Open visibly

Every route must create an Applications entry. Selecting it must reveal the workbench in one action,
whether or not an authority is already running. Tray presence is an optional convenience, never the
only recovery path.

This exposes a current contract tension. ADR-0112 makes a first no-argument launch start
backgrounded and a second launch reveal the existing authority; ADR-0118 implements the Linux
backgrounding as a hidden workbench plus a tray. That is appropriate for connector demand-start but
can make the first click on a GNOME Applications entry appear to do nothing. Resolving it needs an
ADR amendment that preserves one authority and one executable while distinguishing explicit human
activation from connector demand-start. This research does not choose the mechanism.

### 4. Connect the browser

One browser-store confirmation is an honest platform boundary. After that click, the workbench and
`doctor` should show the whole chain as one result: extension, native manifest, connector, authority,
and active browser instance.

If a detected browser is sandboxed, say so at install time and in `doctor`:

> Chromium is installed as a Snap. Its sandbox cannot start Ghostlight's native connector. Install
> Chrome, Edge, Brave, or a supported native Chromium package, then run `ghostlight doctor --fix`.

Do not report a sandboxed browser as merely "not connected," and do not send users to raw manifest
paths.

### 5. Prove first use

The handoff should end in one safe, reversible proof against a neutral page: open, read, and show the
result in the workbench. If no MCP client is present, the application can still prove browser
connectivity without inventing a second setup wizard.

### 6. Update

Keep the current atomic sibling model. A user install downloads and verifies the exact new set
before changing the active path. A Debian package upgrades through the package manager. Neither path
silently adds a vendor repository, background updater, or update ping. A user-triggered link to the
release page is enough for 1.0.

### 7. Repair and uninstall

`doctor` is the single explanation surface. It should distinguish missing, stale, shadowed,
sandboxed, incomplete, and connected states, then offer only ownership-safe fixes.

Uninstall removes exact Ghostlight-owned registrations, desktop integration, and the selected
installed version. It preserves audit and user policy by default and says where those retained files
are. A second uninstall is a no-op.

## Debian versus CachyOS in user terms

The experience after launch should be nearly identical. The installation mechanics differ:

| Concern | Debian or Ubuntu `.deb` | CachyOS or Arch-family user install |
| --- | --- | --- |
| Privilege | Package install uses root. | Default install uses no sudo. |
| Dependencies | APT declares and resolves them. | Installer checks host runtime dependencies and reports exact missing packages. |
| Binary location | `/usr/bin` sibling set. | Versioned per-user sibling set. |
| Browser manifests | Package owns system manifests; first run repairs user shadows. | Installer owns per-user manifests. |
| Application launcher | Package-owned desktop entry and icons. | Installer-owned XDG desktop entry and icons. |
| Upgrade | Install the newer package. | Verify a new versioned set, then switch the active path. |
| Uninstall | Package removes package-owned files; Ghostlight removes owned user integration. | Ghostlight removes owned integration and selected installed bytes. |

The workbench, browser extension, MCP catalog, policy behavior, audit, doctor language, and first-use
proof must not diverge by distribution.

## Smallest complete compatibility matrix

### Automated candidate gates

- Build Linux binaries on a declared oldest-supported baseline. Tauri identifies Debian 12 or
  Ubuntu 22.04 as suitable candidates; the current Ubuntu 24.04 builder does not prove either.
- Inspect the `.deb` payload, dependency metadata, desktop entry, icons, native-host manifests,
  scripts, exact sibling set, checksums, and provenance.
- Install, upgrade, uninstall, and reinstall the `.deb` in clean Debian and Ubuntu containers.
  These are package mechanics, not substitutes for a visible desktop.
- Exercise a clean per-user install in Debian/Ubuntu and one non-Debian glibc container.
- Unit-test native, Snap, and Flatpak browser detection without requiring either sandbox in CI.

### One primary visible lifecycle

Use Ubuntu LTS with GNOME and Wayland for the release-blocking L1-L9 lifecycle because it exercises
the default desktop where tray absence is most likely. Cover clean install, extension confirmation,
first proof, demand-start, explicit open, close/reopen, browser restart, login/reboot, upgrade from
public 0.8, failure recovery, and uninstall.

Ubuntu publishes LTS releases on a two-year cadence and maintains them for five years, making an LTS
the least surprising primary user environment.

[Ubuntu release cadence and support](https://ubuntu.com/project/docs/release-team/ubuntu-releases/)

### One complementary development lifecycle

Keep CachyOS KDE Wayland as complementary evidence. It exercises a rolling Arch-family system,
newer glibc, KDE tray behavior, and a second compositor path. It should catch different defects but
should not duplicate every release-blocking row when the Ubuntu lifecycle is already green.

X11 needs a focused smoke only while the project claims it or after a defect. Fedora and openSUSE
need a live lifecycle when RPM becomes first-party. ARM64 needs actual three-sibling artifacts and a
real browser host before it enters the support statement.

## Coverage without gate explosion

One gate earns its place only when it proves a distinct promise:

| Evidence | Distinct promise |
| --- | --- |
| Old-baseline build | The binary starts on the declared support floor. |
| Debian and Ubuntu package smoke | Package metadata and lifecycle work in both related distributions. |
| Ubuntu GNOME visible lifecycle | The primary install, launcher, browser, reboot, and recovery journey works. |
| CachyOS KDE development pass | The universal route survives a rolling non-Debian desktop. |
| Browser-package detection tests | Unsupported sandboxes fail with a useful remedy. |

Do not multiply the full L1-L9 lifecycle by every distribution, browser, display server, and desktop
shell. Add a lane only after a real incompatibility demonstrates that an existing lane cannot cover
it.

## What should change, and what should not

### Candidate follow-up changes

1. Amend the startup decision so an Applications launch visibly opens on first use without changing
   connector demand-start or creating a second authority.
2. Add XDG desktop-entry and icon ownership to the per-user installer and uninstaller.
3. Detect Snap and Flatpak browser packages in install and doctor, with one typed diagnosis and
   remedy.
4. Declare and enforce the Linux ABI build floor before making a minimum-distribution claim.
5. Align the website, install guide, `doctor`, and release-readiness matrix to one support statement.
6. Add RPM only after a separate scope decision and real lifecycle evidence.

### Keep these boundaries

- One installer core and one native-host lifecycle module.
- One three-sibling installed unit.
- Connector demand-start, not autostart or a resident supervisor.
- No automatic update ping, hidden repository enrollment, or store dependency.
- No policy or product journey in packaging scripts or the extension.
- No unsupported sandbox escape presented as a fix.
- No package-format abstraction until a second native package demonstrates shared behavior.

## Decision summary

For 1.0, keep `.deb`, the verified per-user install, and the portable archive. Make them feel like
one product. Spend the remaining Linux effort on visible launch, sandbox diagnosis, an honest ABI
floor, one GNOME release lifecycle, and consistent outcomes. That combination gives wider practical
coverage than adding AppImage, Snap, Flatpak, RPM, AUR, and Nix artifacts that do not share proven
install, browser, repair, upgrade, and uninstall behavior.

## Sources

- [Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)
- [Tauri distribution overview](https://v2.tauri.app/distribute/)
- [Tauri Debian packages](https://v2.tauri.app/distribute/debian/)
- [Tauri RPM packages](https://v2.tauri.app/distribute/rpm/)
- [Tauri AppImage packages](https://v2.tauri.app/distribute/appimage/)
- [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Snap confinement](https://snapcraft.io/docs/explanation/security/snap-confinement/)
- [Snap native messaging discussion](https://forum.snapcraft.io/t/native-messaging-support-in-strictly-confined-browser-snaps/26849)
- [Snap installed-package inspection](https://snapcraft.io/docs/tutorials/get-started/)
- [Flatpak basic concepts](https://docs.flatpak.org/en/latest/basic-concepts.html)
- [Flatpak command reference](https://docs.flatpak.org/en/latest/flatpak-command-reference.html)
- [Desktop Entry specification](https://specifications.freedesktop.org/desktop-entry/latest/)
- [XDG Base Directory specification](https://specifications.freedesktop.org/basedir/)
- [GTK StatusIcon guidance](https://gnome.pages.gitlab.gnome.org/gtk/gtk3/class.StatusIcon.html)
- [GNOME status icon design history](https://blogs.gnome.org/aday/2017/08/31/status-icons-and-gnome/)
- [Visual Studio Code on Linux](https://code.visualstudio.com/docs/setup/linux)
- [1Password for Linux](https://support.1password.com/install-linux/)
- [KeePassXC getting started guide](https://keepassxc.org/docs/KeePassXC_GettingStarted)
- [Tailscale Linux installation](https://tailscale.com/docs/install/linux)
- [Debian Policy Manual](https://www.debian.org/doc/debian-policy/)
- [Ubuntu release cadence and support](https://ubuntu.com/project/docs/release-team/ubuntu-releases/)
