# Linux live lifecycle verification for Ghostlight 1.0

Status: required 1.0 release gate; not run

The 0.8 line passed an earlier live Linux lifecycle, but that result is evidence, not a 1.0 pass.
The 1.0 rewrite changed the service lifecycle, packaging, executable paths, and native-host
registration. This entire record must be rerun against the candidate Debian package.

## Purpose

Verify the packaged local desktop product on an ordinary visible Linux session. This is not a
headless, container, Xvfb, remote-debugging, or cloud-browser test.

## Host

- Current Ubuntu Desktop LTS on x86_64.
- Normal GNOME graphical login with a visible Wayland session.
- Chrome Stable plus a second supported Chromium family when available.
- One standard local user who owns the desktop, browser profile, Ghostlight process, and MCP
  harnesses.
- Safe test identities only. Do not export or manufacture cookies or profile state.

Test the candidate Debian package and matching store extension. An unpacked source extension is
useful for development but does not satisfy this lifecycle. The build-only workflow produces an
immutable checksum-bound candidate; its GitHub build provenance must verify before the
public-install pass.

## Evidence header

```text
date_utc:
hardware_or_vm:
architecture:
distribution_and_version:
kernel:
desktop_and_display_protocol:
browser_versions:
ghostlight_version:
package_digest_and_provenance:
extension_version:
harness_versions:
```

Retain metadata only. Do not retain credentials, cookies, page content, screenshots of private
data, raw MCP payloads, or browser profiles.

## Lifecycle

### L1. Clean install

1. Confirm no 1.0 process, package, native-host registration, harness entry, or extension exists.
2. Verify the candidate digest and install the Debian package through the normal package manager.
3. Open Ghostlight from Applications. Verify the first click visibly opens the workbench even when
   the desktop exposes no tray, then verify tray Open when the shell provides it.
4. Run `ghostlight native-host check` as the graphical user. Every installed browser must point at
   `/usr/bin/ghostlight-browser-connector`; the first launch must repair any owned user-level drift.
5. Install the matching extension visibly.
6. In MCP integrations, connect one graphical MCP client.

Pass: all three executables are version-matched; native messaging points to the packaged browser
connector; no `ghostlight.service`, Run key, scheduled task, or other resident supervisor was
created; the package-owned desktop entry invokes `ghostlight open`; Status names native browser
package, healthy registration/service/browser/authority state; unrelated client config remains
unchanged.

### L2. Visible browser journey

Run the first read journey, then screenshot, semantic form input, upload, dialog, governed denial,
and tab-close interlock journeys from `docs/1.0/ACCEPTANCE.md` on safe demo forms.

Pass: work stays in the ordinary authenticated profile and Ghostlight group; results, browser
receipts, payload-free audit, and workbench history agree.

### L3. Orchestrator restart

Keep both connector processes, the browser, and the MCP harness open. Stop only `ghostlight`.
Verify a connector demand-starts its trusted packaged sibling, then complete new work without
restarting the shores. Interrupt one effect during a separate run and verify its outcome is unknown
and never replayed.

### L4. Browser and extension restart

Close and reopen the browser normally. Then reload the extension. In both cases verify durable
installation identity, group reuse, native-host recovery, and a new bounded call.

### L5. Login and reboot

Log out and back in, then reboot. Verify no resident service was added. Start from the browser or an
MCP harness and prove demand-start, tray recovery, history continuity, and one new call after each
transition.

### L6. Concurrent harnesses

Register a terminal harness alongside the graphical harness. Run simultaneous sessions and verify
plural workbench state, separate workspaces, one exact group per client label, and no tabs inserted
into the user's unrelated active window.

### L7. Upgrade

Install public 0.8.0 first and record its per-user manifest, client connector path, and supervisor
artifact. Upgrade to the candidate without deleting the browser profile, harness configuration,
audit, or extension settings. Launch the 1.0 package once. Verify the user manifest and every owned
client entry become explicitly updatable and then current, the recognized 0.8 supervisor is retired,
all three sibling paths point at the package, and affected journeys pass.

### L8. Recovery and diagnostics

Inject one realistic failure at a time: stopped orchestrator, missing native registration,
disabled extension, malformed owned harness config, expired managed authority, and unavailable
native notifications. Confirm Status or MCP integrations names the condition and no failure expands
authority or changes terminal truth.

### L9. Uninstall

Use MCP integrations to remove owned client entries, then run `ghostlight native-host uninstall` as
the graphical user before removing the package. Remove the extension and Debian package through
their normal UI. Confirm the package manager removes the system manifests and binaries, the command
removed only Ghostlight-owned user manifests, and unrelated configuration remains. Document the
retention decision for audit/history.

## Result

| Stage | Result | Duration | Evidence | Defect or note |
| --- | --- | --- | --- | --- |
| L1 clean install | NOT RUN | | | |
| L2 visible journey | NOT RUN | | | |
| L3 orchestrator restart | NOT RUN | | | |
| L4 browser/extension restart | NOT RUN | | | |
| L5 login/reboot | NOT RUN | | | |
| L6 concurrent harnesses | NOT RUN | | | |
| L7 upgrade | NOT RUN | | | |
| L8 recovery | NOT RUN | | | |
| L9 uninstall | NOT RUN | | | |

Open one bounded defect per failed stage and rerun the affected journey after the fix. Do not turn
a failed record into a pass by adding undocumented maintainer steps.

## CachyOS npm-candidate record - 2026-08-14

This record extends the development-host evidence below without changing the Debian L1-L9 table.
The public npm coordinate and 1.0 store adapter deliberately remain unpublished.

```text
date_utc: 2026-08-14
architecture: x86_64
distribution_and_version: CachyOS rolling
desktop_and_display_protocol: KDE Plasma, Wayland
browser_versions: Chromium 151.0.7922.137
ghostlight_version: 1.0.0 local npm candidate
package_digest_and_provenance: three checksum-bound optimized siblings; no public package or provenance
extension_version: 1.0.0 unpacked source adapter
harness_versions: locally packed npm launcher 1.0.0; Codex, Claude Code, and Visual Studio Code
```

- A clean temporary user and npm cache completed the packed install, browser registration, detected
  client registration, one-time service-first handoff, idempotent reinstall, and doctor check.
- The real user install lives at `~/.ghostlight/bin/v1.0.0`. The browser adapter reconnected through
  that exact sibling connector and demand-started that exact orchestrator.
- The installed candidate completed visible open, list, read, and screenshot against Example
  Domain. Preserve-tabs correctly refused model-driven close.
- The launcher also completed an empty-cache three-file download simulation with exact current
  bytes, visible per-file progress, and checksum verification. A public-download pass waits for the
  immutable GitHub release and npm publication.
- `doctor` now distinguishes a healthy idle demand-start installation from a broken or stale
  runtime. It does not require a ceremonial service launch after installation.

## CachyOS development-host record - 2026-08-13

This table deliberately does not change L1-L9 above. It records what the ordinary-user source and
portable lane proved at revision `61526364` before the attested Debian and matching-store-extension
gate exists.

```text
date_utc: 2026-08-13
architecture: x86_64
distribution_and_version: CachyOS rolling
kernel: 7.1.8-1-cachyos
desktop_and_display_protocol: KDE Plasma, Wayland
browser_versions: Chromium 151.0.7922.137
ghostlight_version: 1.0.0 development candidate 61526364
package_digest_and_provenance: user candidate; no native package or provenance
extension_version: 1.0.0 unpacked source adapter
harness_versions: npm launcher 1.0.0; native Codex and Claude Code registrations
```

| Related stage | Development-host result | Evidence or remaining limit |
| --- | --- | --- |
| L1 | PARTIAL PASS | Exact three-sibling user candidate installed; four browser manifests plus Codex and Claude Code current; no resident supervisor. No Debian package, provenance, package-manager install, or store adapter. |
| L2 | PARTIAL PASS | Ordinary visible profile passed open, structured read, screenshot, and presentation. Preserve-tabs correctly blocked close. The full interactive form/drag/upload/dialog matrix remains incomplete. |
| L3 | PARTIAL PASS | Browser and MCP connectors independently demand-started the trusted sibling authority. Real-process unknown-effect/no-replay passed; a separately interrupted visible effect remains incomplete. |
| L4 | PARTIAL PASS | Normal browser shutdown removed the connector; restart recovered the native connection and a bounded call. Extension disable/re-enable remains incomplete. |
| L5 | NOT RUN | Logout and reboot require an owner-visible machine transition. |
| L6 | NOT RUN | A second simultaneous live harness was not exercised. |
| L7 | DEVELOPMENT PASS | Attested public portable 0.8.0 created its real user supervisor. The corrected 1.0 migration stopped and retired it, updated all owned paths, and preserved profiles, settings, harness data, and older version directories. This is not a Debian package upgrade pass. |
| L8 | PARTIAL PASS | Stopped authority, missing service, native registration truth, and foreign/malformed preservation were exercised. Disabled extension, expired managed authority, and notification failure remain incomplete. |
| L9 | DEVELOPMENT PASS | User-level native and harness uninstall/reinstall changed only owned entries and restored identical current configuration. No extension UI or Debian package removal occurred. |
