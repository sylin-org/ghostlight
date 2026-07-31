# Linux live lifecycle verification

Status: planned
Owner action: prepare the host and provide SSH access for the test account.

## Purpose

This recipe verifies Ghostlight as a local desktop product on Linux. It is not a headless,
container, cloud-browser, or isolated-profile test. Chrome runs visibly in the same ordinary user
context as the Ghostlight service and MCP clients. A person can watch every browser action.

The first target is Ubuntu Desktop 24.04.4 LTS on x86_64. It is the established Ubuntu LTS
baseline as of July 2026. Ubuntu 26.04 LTS is a useful second target after its first point release,
not a substitute for the 24.04 baseline.

## Host recipe

- Bare metal is preferred. A full desktop VM with a visible console is an acceptable secondary
  target.
- Ubuntu Desktop 24.04.4 LTS, default GNOME and Wayland session.
- At least 8 GB RAM, 2 CPU cores, and 40 GB free disk.
- Google Chrome Stable installed from Google's `.deb`, not the Chromium Snap.
- One standard local account, suggested name `ghostlight-test`.
- The same account owns the graphical login, Chrome profile, Ghostlight service, MCP clients, and
  SSH session.
- SSH public-key authentication only. Disable root login and password authentication.
- Keep a physical display or user-visible remote desktop attached during browser actions.
- Install `tmux`, `curl`, `jq`, `git`, and ordinary diagnostic utilities for durable SSH work.
- Do not enable Chrome remote debugging, Xvfb, headless Chrome, a virtual display, or a cloud
  browser.
- Do not forward or expose a Ghostlight port. Ghostlight has no remote browser-control listener.

Use safe test identities in Chrome, but use them through the normal browser profile with ordinary
cookies, SSO, IndexedDB, extensions, and session state. Do not manufacture or import browser state
through an automation harness.

## Test harness

The primary browser harness is Google Chrome Stable running visibly under the test account. The
first MCP client is VS Code Stable, the mainstream graphical-editor baseline. Codex is the second
client; its CLI process and TOML registration provide a terminal-driven comparison over SSH.

Test the packaged product, not a repository checkout:

1. Begin with the latest published Linux archive and the Chrome Web Store extension.
2. Complete one clean lifecycle against that release.
3. Upgrade in place to the candidate archive produced by CI.
4. Repeat only the affected lifecycle stages after a fix.

Install the extension from the Chrome Web Store through visible Chrome UI. Do not load the
repository's `extension/` directory for the packaged-product pass.

## Access handoff

Before testing, the owner provides:

- SSH hostname or VPN address;
- SSH username;
- the approved public key installed in `~/.ssh/authorized_keys`;
- confirmation that the graphical session is logged in as the same user;
- confirmation that Chrome is visible and has the intended test login state;
- whether a person is available to observe browser-visible actions.

SSH access is for installation, service inspection, client execution, logs, and evidence capture.
It is not a substitute for the visible browser session.

## Evidence header

Record this before changing the machine:

```text
date_utc:
hardware_or_vm:
cpu_arch:
ubuntu_version:
kernel:
desktop_session:
display_protocol:
chrome_version:
ghostlight_version:
ghostlight_archive_sha256:
extension_version:
vscode_version:
codex_version:
test_user_uid:
```

Commands may include:

```bash
date -u --iso-8601=seconds
uname -a
lsb_release -a
printf '%s\n' "$XDG_SESSION_TYPE" "$XDG_CURRENT_DESKTOP"
google-chrome --version
id
systemctl --user status ghostlight --no-pager
```

Never record credentials, cookies, page text, screenshots containing private data, raw MCP
payloads, or browser profile files.

## Lifecycle matrix

### L1. Clean install

1. Confirm no Ghostlight process, user service, native-host manifest, or MCP registration exists.
2. Download or copy the published archive and its checksum.
3. Verify the checksum and, when available, the GitHub build-provenance attestation.
4. Extract the archive into a temporary user-owned directory.
5. Run the packaged installer as the ordinary user. Do not use `sudo` for Ghostlight.
6. Install the packaged extension visibly in Chrome.
7. Run `ghostlight install --client vscode`.
8. Restart the MCP client if its integration requires it.

Pass conditions:

- installation requires no root privilege;
- the native-host manifest names the packaged relay correctly;
- the user service is enabled and running;
- the VS Code registration is valid and preserves unrelated configuration;
- `ghostlight doctor` reports the service and extension as healthy.

### L2. Real user-context journey

1. Open a normal Chrome window under the graphical test account.
2. Confirm the intended extension is the only Ghostlight build enabled in that profile.
3. Start a VS Code MCP session as the same user.
4. Ask Ghostlight to create its managed tab, navigate to a safe authenticated test application,
   read a bounded value, and perform one reversible write while the person watches.
5. Confirm Ghostlight never touches a user-owned tab outside its managed group.
6. Confirm the expected audit records contain metadata but no page values.

Pass conditions:

- work occurs in the ordinary visible Chrome profile;
- existing user authentication is available without exporting cookies;
- actions remain inside Ghostlight-owned tabs;
- read, write, browser-visible effects, and audit all agree on the outcome.

### L3. Service restart recovery

1. Keep Chrome and the MCP client open.
2. Restart the Ghostlight user service through `systemctl --user`.
3. Observe the agent and browser relays reconnect.
4. Issue another bounded tool call without reloading the extension or restarting the client.

Pass conditions:

- the service returns on the same local endpoint;
- both relay sides reconnect within their documented window;
- the managed browser surface remains usable;
- no duplicate user service or stale process remains.

### L4. Chrome restart recovery

1. Close Chrome normally.
2. Confirm `doctor` reports the extension disconnected without corrupting service state.
3. Reopen Chrome in the same profile.
4. Confirm the extension reconnects and a new managed tab works.

### L5. Login and reboot recovery

1. Log out of the graphical session and log back in.
2. Confirm the systemd user service and Chrome integration recover.
3. Reboot the host.
4. Log in graphically as the test user.
5. Confirm `doctor`, VS Code, Chrome, and one real browser action all work.

### L6. Second client

1. Install Codex for the test user.
2. Run `ghostlight install --client codex` or follow the emitted manual step if the CLI cannot
   preserve the live configuration safely.
3. Start a Codex MCP session while VS Code is closed, then while a VS Code session is also live.
4. Confirm both sessions use the one service and receive separate managed ownership while sharing
   the same user browser context.

### L7. Upgrade

1. Preserve the installed release and configuration state.
2. Verify the candidate archive and its hash manifest.
3. Upgrade with the packaged installer as the ordinary user.
4. Confirm the service binary, relay path, native-host manifest, and MCP entries point at the new
   version.
5. Repeat L2 through L4 without reimporting browser state.

Pass conditions:

- the upgrade does not require deleting the Chrome profile or client configuration;
- the endpoint is owned by one current service;
- stale binaries do not self-heal over the candidate;
- browser and MCP sessions recover according to the documented lifecycle.

### L8. Recovery diagnostics

Inject one failure at a time:

- stop the user service;
- temporarily move the native-host manifest;
- disable the extension;
- leave a stale service state file if the normal lifecycle can produce one.

For each failure, run `ghostlight doctor`, record whether its diagnosis and next step are correct,
apply the documented repair, and repeat a bounded tool call. Do not invent corrupt state that a
real installation cannot produce.

### L9. Uninstall

1. Run the packaged uninstall path as the ordinary user.
2. Confirm the user service is stopped and removed.
3. Confirm the native-host manifest and Ghostlight MCP registrations are removed.
4. Confirm unrelated MCP client configuration remains byte-preserved where the installer promises
   lossless editing.
5. Confirm versioned binaries and transient state follow the documented retention behavior.
6. Remove the extension visibly from Chrome.

## Result record

Use one row per stage:

| Stage | Result | Duration | Evidence | Defect or note |
| --- | --- | --- | --- | --- |
| L1 clean install | NOT RUN | | | |
| L2 user-context journey | NOT RUN | | | |
| L3 service restart | NOT RUN | | | |
| L4 Chrome restart | NOT RUN | | | |
| L5 login and reboot | NOT RUN | | | |
| L6 second client | NOT RUN | | | |
| L7 upgrade | NOT RUN | | | |
| L8 diagnostics | NOT RUN | | | |
| L9 uninstall | NOT RUN | | | |

For a failure, preserve the smallest metadata-only reproduction and open one bounded issue. Fix the
specific stage and rerun it; do not reset the entire lifecycle unless the fix changes installation,
identity, or service ownership.
