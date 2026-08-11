# Linux live lifecycle verification for Ghostlight 1.0

Status: planned release gate

## Purpose

Verify the packaged local desktop product on an ordinary visible Linux session. This is not a
headless, container, Xvfb, remote-debugging, or cloud-browser test.

## Host

- Current Ubuntu Desktop LTS on x86_64 or arm64.
- Normal graphical login with a visible Wayland or X11 session.
- Chrome Stable plus a second supported Chromium family when available.
- One standard local user who owns the desktop, browser profile, Ghostlight process, and MCP
  harnesses.
- Safe test identities only. Do not export or manufacture cookies or profile state.

Test the signed package and matching store extension. An unpacked source extension is useful for
development but does not satisfy this lifecycle.

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
package_signature_or_digest:
extension_version:
harness_versions:
```

Retain metadata only. Do not retain credentials, cookies, page content, screenshots of private
data, raw MCP payloads, or browser profiles.

## Lifecycle

### L1. Clean install

1. Confirm no 1.0 process, package, native-host registration, harness entry, or extension exists.
2. Verify and install the signed package as the standard user.
3. Launch Ghostlight and open it from the tray.
4. Install the matching extension visibly.
5. In MCP integrations, connect one graphical MCP client.

Pass: all three executables are version-matched; native messaging points to the packaged browser
connector; Status names healthy service/browser/authority state; unrelated client config remains
unchanged.

### L2. Visible browser journey

Run the first read journey, then screenshot, semantic form input, upload, dialog, governed denial,
and tab-close interlock journeys from `docs/1.0/ACCEPTANCE.md` on safe demo forms.

Pass: work stays in the ordinary authenticated profile and Ghostlight group; results, browser
receipts, payload-free audit, and workbench history agree.

### L3. Orchestrator restart

Keep both relay processes, the browser, and the MCP harness open. Restart only `ghostlight`, then
complete new work without restarting the shores. Interrupt one effect during a separate run and
verify its outcome is unknown and never replayed.

### L4. Browser and extension restart

Close and reopen the browser normally. Then reload the extension. In both cases verify durable
installation identity, group reuse, native-host recovery, and a new bounded call.

### L5. Login and reboot

Log out and back in, then reboot. Verify the package's autostart/tray behavior, harness connection,
browser connection, history continuity, and one new call after each transition.

### L6. Concurrent harnesses

Register a terminal harness alongside the graphical harness. Run simultaneous sessions and verify
plural workbench state, separate workspaces, one exact group per client label, and no tabs inserted
into the user's unrelated active window.

### L7. Upgrade

Upgrade a published supported version to the candidate without deleting the browser profile,
harness configuration, audit, or extension settings. Verify all sibling paths, native messaging,
tray identity, and the journeys affected by the release.

### L8. Recovery and diagnostics

Inject one realistic failure at a time: stopped orchestrator, missing native registration,
disabled extension, malformed owned harness config, expired managed authority, and unavailable
native notifications. Confirm Status or MCP integrations names the condition and no failure expands
authority or changes terminal truth.

### L9. Uninstall

Use MCP integrations to remove owned client entries. Remove the extension and package through their
normal UI. Confirm only Ghostlight-owned native registration, binaries, desktop entries, and
configured harness entries are removed. Document the retention decision for audit/history.

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
