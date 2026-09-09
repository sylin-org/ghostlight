# Linux platform candidates for agent-assisted developers

Research date: 2026-09-08. Status: proposal for discussion, not a support declaration or
an expansion of release gates. No machines were inspected, formatted, or configured.

## Recommendation

Put **Ubuntu 26.04.1 LTS with its default GNOME Wayland session** on the spare x86_64
machine. It adds the strongest measured developer-reach candidate and a real Debian-family
desktop to the existing CachyOS, Alpine, and Bluefin machines. Keep Ubuntu 24.04 and Debian 12
package consumers for the existing compatibility contract; a newer desktop does not replace
the older build floor or acceptance requirements.

**Omarchy with Hyprland is the strongest alternative if the immediate aim is learning from an
agent-focused desktop community.** It adds a substantially different desktop experience while
sharing the Arch package family with CachyOS. Put it next in the hardware rotation. Fedora
Workstation is the first VM candidate for developing an RPM.

These priorities are judgments combining reach, audience fit, additional coverage, and upkeep.
They are not measured market shares or an instruction to install anything.

## What the audience evidence establishes

The public reports reviewed did not provide a representative cross-tab of coding-agent use,
Linux distribution, and desktop environment. Use general developer adoption as a reach proxy,
and first-party workflow documentation as evidence of audience fit. Neither proves Ghostlight
demand. DistroWatch visits, GitHub stars, Steam shares, and AI/ML workstation marketing are not
substitutes for that cross-tab.

Stack Overflow's 2025 survey reports 51% of professional developers using AI tools daily;
84% of respondents use or plan to use them. These groups include more than autonomous-agent
users. [Survey AI results](https://survey.stackoverflow.co/2025/ai)

The operating-system question has 31,569 respondents. Its All Respondents dataset contains these
reported use percentages; the professional column means professional use, not a filtered sample
of professional developers. Categories overlap and must not be summed into market shares.
[Survey technology results](https://survey.stackoverflow.co/2025/technology#computer-operating-systems)

| OS category | Personal use | Professional use |
| --- | ---: | ---: |
| Ubuntu | 27.78% | 27.70% |
| Debian | 11.39% | 10.42% |
| Arch | 9.72% | 4.61% |
| Fedora | 5.77% | 3.70% |
| NixOS | 3.37% | 1.80% |
| Pop!_OS | 2.34% | 1.07% |
| Windows Subsystem for Linux | 15.89% | 16.79% |

Reproduction: retrieve that page's HTML and inspect `OpSys.datasets.OpSys` in the embedded
data. `percent1` is Personal use and `percent2` Professional use; multiply by 100. This
avoids confusing the page's separate write-in tables with its main charts. No AI-only
cross-tab was computed. The 2025 results were the published baseline found during this search.

Three first-party signals sharpen the shortlist:

- Omarchy's current site makes agents part of first boot and system customization, and offers
  Cursor, VS Code, Zed, and terminal/editor choices. Its manual identifies the base as Arch
  plus Hyprland. This is direct audience positioning, with no verified active-user count here.
  [Omarchy](https://omarchy.org/),
  [platform manual](https://omarchy.org/manual/omarchy-on/)
- Bluefin DX explicitly separates the host from development tools, using devcontainers,
  Homebrew, and virtual machines. Its default IDE is VS Code with devcontainer integration.
  That makes it valuable for studying where an MCP process actually runs.
  [Bluefin developer mode](https://docs.projectbluefin.io/bluefin-dx/)
- Cursor publishes DEB, RPM, and AppImage downloads; VS Code documents Debian and RPM
  installation. Claude Code documents both glibc Linux and Alpine/musl installation.
  These establish real tool delivery paths, not relative distro adoption.
  [Cursor downloads](https://cursor.com/download),
  [VS Code Linux setup](https://code.visualstudio.com/docs/setup/linux),
  [Claude Code installation](https://code.claude.com/docs/en/installation)

## Existing coverage and implementation limits

The owner reports CachyOS established and Alpine/Bluefin machines available. Only CachyOS's
desktop and live evidence are identified in the tracked records reviewed: KDE Plasma on
Wayland, with native Chromium and additional Brave tests. Alpine and Bluefin availability
does not establish their exact releases, desktop sessions, browser packages, or acceptance.
See [Linux integration](../testing/linux-integration-2026-09-08.md) and
[additional acceptance](../testing/linux-local-acceptance-2026-09-08.md).

Current source matters to the candidate ranking:

- The release workflow builds Linux x86_64 GNU binaries on Ubuntu 22.04 and emits a DEB.
  Local 1.3.5 evidence measured GLIBC 2.34 and exercised Debian 12 and Ubuntu 24.04 package
  consumers. This is not proof of every newer desktop or every system library combination.
  [Workflow](../../.github/workflows/release.yml), [status](../STATUS.md)
- The authority includes Tauri, GTK, and WebKitGTK. It has no supported headless/service-only
  mode. A successful CLI agent installation on Alpine says nothing about Ghostlight's whole
  desktop dependency chain. [Cargo dependencies](../../crates/orchestrator/Cargo.toml),
  [ADR-0127](../adr/0127-one-invoked-desktop-authority.md)
- The npm launcher selects `x86_64-unknown-linux-gnu` for Linux x64. It currently has no
  Linux ARM64 or musl choice. An APK or new architecture needs an actual build and delivery
  path, including launcher selection; renaming the existing archive is insufficient.
  [Launcher](../../packaging/npm/bin/ghostlight.js)
- Current browser discovery explicitly diagnoses sandbox-only Snap/Flatpak installations as
  unable to start the connector. Bluefin's Flatpak-first desktop is therefore an important
  compatibility investigation, not automatic RPM coverage. This describes Ghostlight's
  present contract, not a universal claim about every browser portal or future sandbox.
  [Browser package detection](../../crates/orchestrator/src/install/browser_package.rs),
  [Bluefin application model](https://docs.projectbluefin.io/)
- System-package runtime discovery has an exact `/usr/bin` rule. Nix store paths, Homebrew
  prefixes, and other read-only layouts require deliberate lifecycle design and testing.
  [ADR-0124](../adr/0124-user-writable-system-package-runtime.md)

## Candidate matrix

Priority describes additional work from the current fleet. Effort is an initial qualitative
estimate for Ghostlight packaging and integration, not installation difficulty. Desktop pairs
are proposed test configurations; they are not surveyed preferences.

| Candidate and desktop | Audience evidence | Delivery candidate | Additional value / main uncertainty | Priority and effort |
| --- | --- | --- | --- | --- |
| **Ubuntu LTS + GNOME / Wayland** | Strongest named Linux reach proxy above | Existing DEB and per-user/archive routes | Mainstream desktop, Debian dependencies, AppArmor, browser provenance, Applications/Open behavior; test tray availability explicitly | **Spare machine first; low-medium** |
| **CachyOS + KDE Plasma / Wayland** | Owner's established lane; Arch survey is only a family proxy | Existing per-user/archive route; proposed PKGBUILD | Rolling dependencies and KDE baseline; preserve evidence while adding package ownership tests | **Keep; medium for native packaging** |
| **Bluefin / Bluefin DX + GNOME / Wayland** | Explicit developer/container workflow; no adoption estimate | Host-compatible delivery still to establish | Atomic/read-only host, Flatpak browsers, host versus devcontainer MCP, SELinux; image variant matters | **Use existing machine now; high** |
| **Alpine + Xfce / X11** | musl agent tooling exists; no desktop adoption estimate | Proposed native musl APK | musl, BusyBox, OpenRC, GTK/WebKit dependencies, native Chromium; Xfce is a proposed simple desktop lane | **Use existing machine for feasibility; high** |
| **Omarchy + Hyprland / Wayland** | Most explicit agent-focused positioning among candidates reviewed | Share an Arch PKGBUILD with CachyOS where compatible | Tiling, activation/focus, notifications, bar/tray integration, scaling; new experience with less new package-family work | **Next desktop rotation; medium** |
| **Fedora Workstation + GNOME / Wayland** | Survey plus first-party developer workflow | Proposed RPM | Ordinary writable RPM system, SELinux enforcing, stock GNOME without requiring a tray extension; Bluefin is not equivalent | **First RPM VM, then visible acceptance; medium** |
| **Linux Mint + Cinnamon / X11** | Ubuntu-family reach is only a proxy; no Mint number here | Reuse DEB after dependency/lifecycle checks | Familiar panel/tray desktop and X11 coverage; avoid assuming Ubuntu success transfers | **Second wave VM; low-medium** |
| **Pop!_OS 24.04 + COSMIC / Wayland** | Smaller measured distro usage; productivity workstation focus | Reuse DEB after validation | Distinct compositor, tiling, notifications and native-window lifecycle; useful physical GPU/scaling lane | **Second wave desktop; medium** |
| **NixOS + KDE Plasma / Wayland** | Smaller but measurable developer usage | Proposed Nix derivation and declarative integration | Non-FHS paths, library patching, immutable store, runtime state and browser manifest lifetime across generations | **High-value packaging spike in VM; high** |
| **Debian 13 + Xfce / X11** | Second-largest named Linux professional-use proxy above | Reuse DEB | Current stable consumer, conservative desktop and library packaging; retain Debian 12 as the older consumer | **VM compatibility lane; low-medium** |
| **openSUSE Tumbleweed + KDE Plasma / Wayland** | Developer-oriented rolling distro; no measured share here | Reuse/adapt RPM and dependency names | Another RPM implementation and upgrade environment; substantial desktop overlap with CachyOS | **Later, after Fedora; medium** |

Current pairings and release context are supported by
[Ubuntu Desktop](https://ubuntu.com/download/desktop),
[Fedora Workstation](https://www.fedoraproject.org/workstation/),
[Mint editions](https://linuxmint.com/download.php),
[Pop!_OS download and COSMIC notes](https://system76.com/download-pop/),
[Alpine's libc/init/package model](https://www.alpinelinux.org/about/),
[Nix binary packaging](https://wiki.nixos.org/wiki/Packaging/Binaries),
[Debian releases](https://www.debian.org/releases/), and
[Tumbleweed desktops](https://get.opensuse.org/tumbleweed/).

Ubuntu currently offers 26.04.1 LTS; Pop!_OS 24.04 ships COSMIC, so treating Pop as another
GNOME test would miss its value. Choose Alpine's actual installed session before adding Xfce.
Record Bluefin's exact image, base, and DX status rather than assuming all Bluefin variants
are identical. The hardware recommendation assumes an Intel/AMD x86_64 spare; ARM64 would
change the work because Ghostlight's current launcher does not publish that Linux target.

## Package work and machine allocation

Start with one product and a few package-family implementations. A desktop difference normally
needs an acceptance row, not a separately compiled edition.

1. Preserve the verified per-user installer, portable archive, npm front door, and DEB.
   Add the Ubuntu physical desktop while retaining the old-enough builder and older consumers.
2. Develop a PKGBUILD against CachyOS and validate it on Omarchy; develop an RPM in a Fedora VM.
   These are proposed new package routes. Tauri supports RPM output, but emitting a file does
   not prove installation ownership, native messaging, or upgrades.
   [Tauri RPM packaging](https://v2.tauri.app/distribute/rpm/)
3. Run bounded Alpine and Bluefin feasibility investigations on their existing machines.
   Alpine asks whether the whole desktop product builds/runs on musl. Bluefin asks whether
   Ghostlight can participate in the normal host/browser/container workflow. Treat successful
   host-native-browser testing and successful Flatpak-browser testing as separate outcomes.
4. Add Nix when the standard package paths are stable. Preserve the same sibling/lifecycle
   authority and make store-path registration and writable runtime state explicit.

AppImage does not by itself solve glibc versus musl, browser confinement, stable native-host
paths, or host/container routing. A Ghostlight Flatpak likewise would not automatically make
Flatpak Chromium integration work. Treat those formats as possible later solutions to a
demonstrated distribution problem, not as universal coverage.

Use VMs for clean package installs and multiple distro consumers. Use physical desktop sessions
for graphics, focus, tray, scaling, suspend/resume, and independently launched real browsers.
A container with Xvfb remains useful process evidence, with narrower scope than either.

WSL deserves a separate row on the existing Windows side because of its survey reach. Test
where the MCP connector executes and whether it can reach the authority serving the Windows
browser; neither Linux CLI success nor a Linux package proves that cross-OS route. Remote SSH,
devcontainers, and cloud coding environments have the same location question. They do not
justify another bare-metal Linux installation by themselves.

## Small acceptance record for every promoted row

Record the exact OS/image version, architecture, desktop/compositor and X11/Wayland session,
browser name/version/package source, MCP client/version/execution location, Ghostlight artifact
hashes, and installation route. Separate Not tested, Passed, Blocked, and Unsupported today.

For a proposed supported route, prove:

- Clean install in both browser/extension-first and product-first order, using the browser
  launched normally from the desktop. No forced browser restart to finish installation.
- One desktop authority, Applications/Open, tray when available, close/reopen, notifications,
  and normal-user writable runtime state. Stock no-tray GNOME gets its own check.
- One terminal agent and one IDE integration through real model/MCP calls, with the actual
  browser retaining a test edit and producing a read and screenshot. Add the container-host
  route only on the relevant row; do not multiply every client by every desktop.
- Upgrade reaches the new executable set with retained connections; browser and authority
  recovery do not replay an uncertain effect. Removal preserves foreign files and user data.
  Retain the known 1.3.4/new-history downgrade failure instead of claiming transparent rollback.

These are proposed expansion criteria. Existing accepted release gates remain governed by
[ADR-0123](../adr/0123-lean-linux-install-and-visible-activation.md),
[ADR-0115](../adr/0115-packaged-native-host-lifecycle.md), and
the [pre-release integration guide](../testing/pre-release-integration.md).

## Follow-up evidence that would change the ranking

The next useful input is the exact browser/client/package combination people want to use,
especially on Bluefin, plus the spare machine's CPU/GPU and display setup. A few opted-in
community reports can identify friction but should not become population estimates. Track
incoming requests by OS, desktop, browser package, and MCP execution location without adding
product telemetry. Revisit priorities when an actual route is blocked or demand emerges.
